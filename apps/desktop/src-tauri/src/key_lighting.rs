//! Bounded notification output. Frames use RAM output, never persistent settings.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Alert {
    pub device_id: String,
    pub control_id: String,
    pub color: [u8; 3],
    pub period_ms: u64,
    pub duration_ms: u64,
    pub enabled: bool,
}
impl Alert {
    fn validate(&self) -> Result<(), String> {
        if !self.enabled
            || self.device_id.len() > 128
            || self.control_id.len() > 128
            || self.device_id.is_empty()
            || self.control_id.is_empty()
            || !(300..=5000).contains(&self.period_ms)
            || !(300..=60000).contains(&self.duration_ms)
        {
            return Err("Choose a device, key, period 300–5000 ms and duration 300–60000 ms; enable experimental output explicitly".into());
        }
        Ok(())
    }
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub active: bool,
    pub error: Option<String>,
    pub physical: &'static str,
}
type PendingAlert = (Alert, Option<(String, String)>);
#[derive(Clone)]
struct TimerJob {
    source: String,
    context: String,
    profile: String,
    seconds: Value,
    alert: Alert,
    deadline: Instant,
}
#[derive(Default)]
struct TimerPlanner {
    jobs: BTreeMap<String, TimerJob>,
}
impl TimerPlanner {
    fn update(
        &mut self,
        mut job: TimerJob,
        before: &Value,
        settings: &Value,
        wall_ms: u64,
        now: Instant,
    ) -> Result<Option<TimerJob>, String> {
        let runtime = &settings["runtime"];
        let status = runtime["status"].as_str();
        if status != Some("running") {
            let previous = self.jobs.remove(&job.context);
            return Ok(previous.filter(|j| {
                j.profile == job.profile
                    && j.seconds == job.seconds
                    && j.alert == job.alert
                    && status == Some("done")
                    && (j.deadline <= now
                        || (before["runtime"]["status"] == "running"
                            && runtime["value"].as_f64() == Some(0.0)))
            }));
        }
        // Loading an old running timer (or its willDisappear persistence) never arms output.
        if before["runtime"]["status"] == "running" {
            if let Some(existing) = self.jobs.get(&job.context)
                && existing.profile == job.profile
                && existing.seconds == job.seconds
                && existing.alert == job.alert
            {
                return Ok(None); // Preserve the monotonic deadline across wall-clock changes.
            }
            self.jobs.remove(&job.context);
            return Ok(None);
        }
        let value = runtime["value"]
            .as_f64()
            .filter(|v| v.is_finite() && *v > 0.0 && *v <= 86_400_000.0);
        let started = runtime["startedAt"]
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= wall_ms as f64 + 1000.0);
        let (Some(value), Some(started)) = (value, started) else {
            self.jobs.remove(&job.context);
            return Ok(None);
        };
        let remaining = value - (wall_ms as f64 - started).max(0.0);
        if remaining <= 0.0 {
            self.jobs.remove(&job.context);
            return Ok(None);
        }
        if self.jobs.len() >= 128 && !self.jobs.contains_key(&job.context) {
            return Err("Too many armed countdown notifications".into());
        }
        job.deadline = now + Duration::from_millis(remaining.ceil() as u64);
        self.jobs.insert(job.context.clone(), job);
        Ok(None)
    }
    fn due(&mut self, now: Instant) -> Vec<TimerJob> {
        let keys: Vec<_> = self
            .jobs
            .iter()
            .filter(|(_, j)| j.deadline <= now)
            .map(|(k, _)| k.clone())
            .collect();
        keys.into_iter()
            .filter_map(|k| self.jobs.remove(&k))
            .collect()
    }
    fn cancel_target(&mut self, device: &str) {
        self.jobs.retain(|_, j| j.alert.device_id != device);
    }
}
#[derive(Default)]
struct Controls {
    starts: BTreeMap<String, PendingAlert>,
    stops: BTreeSet<String>,
    timers: TimerPlanner,
}
impl Controls {
    fn start(&mut self, alert: Alert, owner: Option<(String, String)>) -> Result<(), String> {
        if self.starts.len() >= 32 && !self.starts.contains_key(&alert.device_id) {
            return Err("Too many pending device notifications".into());
        }
        self.starts.insert(alert.device_id.clone(), (alert, owner));
        Ok(())
    }
    fn cancel(&mut self, device: String) -> Result<(), String> {
        if self.stops.len() >= 32 && !self.stops.contains(&device) {
            return Err("Too many pending stop requests".into());
        }
        self.starts.remove(&device);
        self.timers.cancel_target(&device);
        self.stops.insert(device);
        Ok(())
    }
}
struct Running {
    alert: Alert,
    owner: Option<(String, String)>,
    owner_profile: Option<String>,
    generation: u64,
    lease: String,
    sequence: u64,
    started: Instant,
    baseline: Option<[u8; 3]>,
}
pub struct Runtime {
    controls: Arc<(Mutex<Controls>, Condvar)>,
    stopping: Arc<AtomicBool>,
    status: std::sync::Arc<Mutex<BTreeMap<String, Status>>>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Runtime {
    pub fn start(app: tauri::AppHandle) -> Self {
        let controls = Arc::new((Mutex::new(Controls::default()), Condvar::new()));
        let pending = controls.clone();
        let stopping = Arc::new(AtomicBool::new(false));
        let exiting_flag = stopping.clone();
        let status = std::sync::Arc::new(Mutex::new(BTreeMap::new()));
        let states = status.clone();
        let worker = thread::spawn(move || {
            let mut running = BTreeMap::<String, Running>::new();
            loop {
                let mut guard = pending.0.lock().expect("lighting control mutex");
                if guard.starts.is_empty()
                    && guard.stops.is_empty()
                    && !exiting_flag.load(Ordering::Acquire)
                {
                    guard = pending
                        .1
                        .wait_timeout(guard, Duration::from_millis(100))
                        .expect("lighting control mutex")
                        .0;
                }
                let starts = std::mem::take(&mut guard.starts);
                let stops = std::mem::take(&mut guard.stops);
                guard.timers.jobs.retain(|_, job| timer_current(&app, job));
                let due = guard.timers.due(Instant::now());
                drop(guard);
                let mut starts = starts;
                for job in due {
                    if !stops.contains(&job.alert.device_id) {
                        starts.insert(
                            job.alert.device_id.clone(),
                            (job.alert, Some((job.source, job.context))),
                        );
                    }
                }
                for id in &stops {
                    if let Some(old) = running.remove(id) {
                        set_status(&states, id, false, release(&app, &old).err());
                    }
                }
                for (_, (alert, owner)) in starts {
                    if stops.contains(&alert.device_id) {
                        continue;
                    }
                    if exiting_flag.load(Ordering::Acquire) {
                        break;
                    }
                    if let Some(old) = running.remove(&alert.device_id)
                        && let Err(error) = release(&app, &old)
                    {
                        set_status(&states, &alert.device_id, false, Some(error));
                        continue;
                    }
                    match prepare(&app, alert.clone(), owner) {
                        Ok(next) => {
                            running.insert(alert.device_id.clone(), next);
                            set_status(&states, &alert.device_id, true, None);
                        }
                        Err(error) => set_status(&states, &alert.device_id, false, Some(error)),
                    }
                }
                let ids: Vec<_> = running.keys().cloned().collect();
                for id in ids {
                    let current = running.get_mut(&id).expect("listed key");
                    let expired = current.started.elapsed().as_millis()
                        >= u128::from(current.alert.duration_ms);
                    let valid = still_current(&app, current);
                    if exiting_flag.load(Ordering::Acquire)
                        || expired
                        || !valid
                        || pending.0.lock().is_ok_and(|p| p.stops.contains(&id))
                    {
                        let old = running.remove(&id).expect("listed key");
                        set_status(&states, &id, false, release(&app, &old).err());
                        continue;
                    }
                    let phase = phase_at(
                        current.started.elapsed().as_millis() as u64,
                        current.alert.period_ms,
                    );
                    // Renew the provider lease even on a long phase. Never catch up missed frames.
                    let color = if phase {
                        current.alert.color
                    } else {
                        current.baseline.unwrap_or(current.alert.color)
                    };
                    current.sequence += 1;
                    let result = with_provider(&app, |host| host.send_key_lighting(
                        &id, current.generation, &current.lease, current.sequence, 1500,
                        json!({current.alert.control_id.clone():{"r":color[0],"g":color[1],"b":color[2]}}),
                    )).and_then(|ack| baseline_from_ack(&ack, &current.alert.control_id));
                    if let Ok(baseline) = result.as_ref() {
                        current.baseline = Some(*baseline);
                    }
                    if let Err(error) = result {
                        let old = running.remove(&id).expect("listed key");
                        // An uncertain frame must not be replayed. One release is safe to request;
                        // the provider owns recovery if its restore cannot be acknowledged.
                        let restore = release(&app, &old).err();
                        let message =
                            restore.map_or_else(|| error.clone(), |r| format!("{error}; {r}"));
                        if let Some((device, binding)) = old.owner
                            && let Some(state) = app.try_state::<crate::Application>()
                            && let Ok(mut core) = crate::locked(state.inner())
                        {
                            core.action_error(&device, &binding, &message);
                        }
                        set_status(&states, &id, false, Some(message));
                    }
                }
                if exiting_flag.load(Ordering::Acquire) {
                    break;
                }
            }
        });
        Self {
            controls,
            stopping,
            status,
            worker: Mutex::new(Some(worker)),
        }
    }
    pub fn alert(&self, alert: Alert, owner: Option<(String, String)>) -> Result<(), String> {
        alert.validate()?;
        if self.stopping.load(Ordering::Acquire) {
            return Err("Lighting output is stopping".into());
        }
        self.controls
            .0
            .lock()
            .map_err(|_| "Lighting controls unavailable")?
            .start(alert, owner)?;
        self.controls.1.notify_one();
        Ok(())
    }
    pub fn cancel(&self, device: String) -> Result<(), String> {
        self.controls
            .0
            .lock()
            .map_err(|_| "Lighting controls unavailable")?
            .cancel(device)?;
        self.controls.1.notify_one();
        Ok(())
    }
    fn timer_settings(
        &self,
        job: TimerJob,
        before: &Value,
        settings: &Value,
        wall_ms: u64,
    ) -> Result<(), String> {
        if self.stopping.load(Ordering::Acquire) {
            return Err("Lighting output is stopping".into());
        }
        let mut controls = self
            .controls
            .0
            .lock()
            .map_err(|_| "Lighting controls unavailable")?;
        if let Some(due) = controls
            .timers
            .update(job, before, settings, wall_ms, Instant::now())?
        {
            controls.start(due.alert, Some((due.source, due.context)))?;
        }
        drop(controls);
        self.controls.1.notify_one();
        Ok(())
    }
    pub fn snapshot(&self) -> BTreeMap<String, Status> {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        self.controls.1.notify_one();
        if let Ok(mut worker) = self.worker.lock()
            && let Some(worker) = worker.take()
        {
            let _ = worker.join();
        }
    }
}
fn timer_current(app: &tauri::AppHandle, job: &TimerJob) -> bool {
    let Some(state) = app.try_state::<crate::Application>() else {
        return false;
    };
    let Ok(core) = crate::locked(state.inner()) else {
        return false;
    };
    if core.current_plugin_profile(&job.source, &job.context) != Some(job.profile.as_str())
        || !core.plugin_enabled("dev.deckweave.timers")
    {
        return false;
    }
    let Some((source, plugin)) = core.plugin_assignment(&job.context) else {
        return false;
    };
    source == job.source
        && plugin.package == "dev.deckweave.timers"
        && plugin.action == "dev.deckweave.timers.countdown"
        && plugin.settings["seconds"] == job.seconds
        && plugin.settings["deckweaveKeyLightingAlert"]
            == serde_json::to_value(&job.alert).unwrap_or(Value::Null)
        && (plugin.settings["runtime"]["status"] == "running"
            || (job.deadline <= Instant::now() && plugin.settings["runtime"]["status"] == "done"))
}
/// Called only after a new plugin-originated setSettings was durably accepted.
/// Snapshot load, willAppear and settings reads intentionally never call this.
pub fn timer_settings_accepted(
    app: &tauri::AppHandle,
    device: &str,
    context: &str,
    package: &str,
    action: &str,
    before: &Value,
    settings: &Value,
) -> Result<(), String> {
    if package != "dev.deckweave.timers" || action != "dev.deckweave.timers.countdown" {
        return Ok(());
    }
    let state = app
        .try_state::<crate::Application>()
        .ok_or("Application is not ready")?;
    let core = crate::locked(state.inner()).map_err(|e| e.message)?;
    let profile = core
        .current_plugin_profile(device, context)
        .map(str::to_owned);
    let valid_owner = core.plugin_enabled(package)
        && core.plugin_assignment(context).is_some_and(|(source, p)| {
            source == device
                && p.package == package
                && p.action == action
                && p.settings == *settings
        });
    drop(core);
    let value = &settings["deckweaveKeyLightingAlert"];
    let runtime = app
        .try_state::<Runtime>()
        .ok_or("Lighting runtime is not ready")?;
    if profile.is_none() || !valid_owner || value.is_null() || value["enabled"] != true {
        if let Ok(mut controls) = runtime.controls.0.lock() {
            controls.timers.jobs.remove(context);
        }
        return Ok(());
    }
    let alert: Alert = serde_json::from_value(value.clone())
        .map_err(|_| "Invalid key lighting notification settings")?;
    alert.validate()?;
    let wall_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "System clock is unavailable")?
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    runtime.timer_settings(
        TimerJob {
            source: device.into(),
            context: context.into(),
            profile: profile.expect("checked active profile"),
            seconds: settings["seconds"].clone(),
            alert,
            deadline: Instant::now(),
        },
        before,
        settings,
        wall_ms,
    )
}
fn phase_at(elapsed: u64, period: u64) -> bool {
    elapsed % period < period / 2
}
fn baseline_from_ack(ack: &Value, control: &str) -> Result<[u8; 3], String> {
    let original = &ack["baseline"][control];
    let values = ["r", "g", "b"].map(|key| original[key].as_u64().filter(|n| *n <= 255));
    if let [Some(r), Some(g), Some(b)] = values {
        Ok([r as u8, g as u8, b as u8])
    } else {
        Err("Provider did not return the original key colour; notification stopped".into())
    }
}
fn set_status(
    states: &Mutex<BTreeMap<String, Status>>,
    device: &str,
    active: bool,
    error: Option<String>,
) {
    if let Ok(mut states) = states.lock() {
        states.insert(
            device.into(),
            Status {
                active,
                error,
                physical: "unverified",
            },
        );
    }
}
fn with_provider<T>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&crate::device_providers::Host) -> Result<T, String>,
) -> Result<T, String> {
    let state = app
        .try_state::<crate::Application>()
        .ok_or("Application is not ready")?;
    match state.inner() {
        crate::Application::Ready {
            device_providers, ..
        } => f(device_providers),
        crate::Application::Failed(e) => Err(e.message.clone()),
    }
}
fn prepare(
    app: &tauri::AppHandle,
    alert: Alert,
    owner: Option<(String, String)>,
) -> Result<Running, String> {
    alert.validate()?;
    let state = app
        .try_state::<crate::Application>()
        .ok_or("Application is not ready")?;
    let snapshot = crate::locked(state.inner())
        .map_err(|e| e.message)?
        .snapshot();
    let device = snapshot
        .configuration
        .devices
        .iter()
        .find(|d| d.id == alert.device_id)
        .ok_or("Device is missing")?;
    let desc = device
        .provider_descriptor
        .as_ref()
        .ok_or("Device has no key lighting output")?;
    if desc.definition["keyLighting"]["experimental"] != true {
        return Err("Device does not declare experimental temporary key lighting".into());
    }
    if snapshot
        .lighting
        .get(&alert.device_id)
        .is_some_and(|s| s.manual_editing)
    {
        return Err("Finish manual lighting editing before starting a notification".into());
    }
    if !desc
        .geometry
        .controls
        .iter()
        .any(|c| c.id == alert.control_id && c.kind == deckweave_core::ControlKind::Key)
    {
        return Err("Choose a physical key".into());
    }
    let status = with_provider(app, |host| {
        host.session(&alert.device_id)
            .ok_or("Device is disconnected".into())
    })?;
    if status.state != "connected" || status.recovery["state"] == "pending" {
        return Err("Device is disconnected or needs recovery".into());
    }
    if status.observed["lightingMode"] != 1
        || status.observed["colorMode"] != 0
        || status.observed["brightness"].as_u64().unwrap_or(0) < 2
    {
        return Err("Temporary key lighting currently needs Static, Single colour and brightness at least 2. The current effect was not changed.".into());
    }
    let owner_profile = owner
        .as_ref()
        .map(|(source, context)| {
            let state = app
                .try_state::<crate::Application>()
                .ok_or("Application is not ready")?;
            let core = crate::locked(state.inner()).map_err(|e| e.message)?;
            core.current_plugin_profile(source, context)
                .map(str::to_owned)
                .ok_or_else(|| "Action is outside the current profile".to_owned())
        })
        .transpose()?;
    Ok(Running {
        alert,
        owner,
        owner_profile,
        generation: status.generation,
        lease: uuid::Uuid::new_v4().simple().to_string(),
        sequence: 0,
        started: Instant::now(),
        baseline: None,
    })
}
fn still_current(app: &tauri::AppHandle, running: &Running) -> bool {
    let Ok(status) = with_provider(app, |host| {
        host.session(&running.alert.device_id)
            .ok_or("Disconnected".into())
    }) else {
        return false;
    };
    if status.generation != running.generation
        || status.state != "connected"
        || status.recovery["state"] == "pending"
    {
        return false;
    }
    let Some(state) = app.try_state::<crate::Application>() else {
        return false;
    };
    let Ok(core) = crate::locked(state.inner()) else {
        return false;
    };
    if core
        .lighting_status(&running.alert.device_id)
        .is_some_and(|s| s.manual_editing)
    {
        return false;
    }
    if let Some((source, context)) = &running.owner {
        if core.current_plugin_profile(source, context) != running.owner_profile.as_deref() {
            return false;
        }
        let Some((device, plugin)) = core.plugin_assignment(context) else {
            return false;
        };
        if &device != source || !core.plugin_enabled(&plugin.package) {
            return false;
        }
        if plugin.settings["deckweaveKeyLightingAlert"]
            != serde_json::to_value(&running.alert).unwrap_or(Value::Null)
        {
            return false;
        }
    }
    true
}
fn release(app: &tauri::AppHandle, running: &Running) -> Result<(), String> {
    with_provider(app, |host| {
        host.release_key_lighting(&running.alert.device_id, running.generation, &running.lease)
    })
    .map(|_| ())
}
fn editor(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "main" || window.label().starts_with("device-") {
        Ok(())
    } else {
        Err("This window cannot control device lighting".into())
    }
}
#[tauri::command]
pub async fn key_lighting_test(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    alert: Alert,
) -> Result<(), String> {
    editor(&window)?;
    tauri::async_runtime::spawn_blocking(move || app.state::<Runtime>().alert(alert, None))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn key_lighting_stop(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    device_id: String,
) -> Result<(), String> {
    editor(&window)?;
    tauri::async_runtime::spawn_blocking(move || app.state::<Runtime>().cancel(device_id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn key_lighting_status(
    window: tauri::WebviewWindow,
    runtime: tauri::State<'_, Runtime>,
) -> Result<BTreeMap<String, Status>, String> {
    editor(&window)?;
    Ok(runtime.snapshot())
}
pub fn plugin_alert(app: &tauri::AppHandle, device: &str, context: &str) -> Result<(), String> {
    let alert = {
        let state = app
            .try_state::<crate::Application>()
            .ok_or("Application is not ready")?;
        let core = crate::locked(state.inner()).map_err(|e| e.message)?;
        let (source, plugin) = core
            .plugin_assignment(context)
            .ok_or("Action is no longer assigned")?;
        if source != device {
            return Err("Action owner changed".into());
        }
        let value = &plugin.settings["deckweaveKeyLightingAlert"];
        // An unconfigured action is intentionally silent; the timer still completes normally.
        if value.is_null() || value["enabled"] != true {
            return Ok(());
        }
        serde_json::from_value::<Alert>(value.clone())
            .map_err(|_| "Invalid key lighting notification settings")?
    };
    app.try_state::<Runtime>()
        .ok_or("Lighting runtime is not ready")?
        .alert(alert, Some((device.into(), context.into())))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn alert(device: &str) -> Alert {
        Alert {
            device_id: device.into(),
            control_id: "key-37".into(),
            color: [255, 0, 0],
            period_ms: 1000,
            duration_ms: 5000,
            enabled: true,
        }
    }
    fn job(context: &str, now: Instant) -> TimerJob {
        TimerJob {
            source: "source".into(),
            context: context.into(),
            profile: "profile".into(),
            seconds: json!(10),
            alert: alert("output"),
            deadline: now,
        }
    }
    fn runtime(status: &str, value: u64, wall: u64) -> Value {
        json!({"runtime":{"status":status,"value":value,"startedAt":wall}})
    }
    #[test]
    fn hidden_countdown_uses_one_monotonic_deadline_and_never_replays_saved_running_state() {
        let now = Instant::now();
        let running = runtime("running", 10000, 1000);
        let mut fresh = TimerPlanner::default();
        // Startup/load and a later hidden-page persist are both already-running updates.
        fresh
            .update(
                job("timer", now),
                &running,
                &runtime("running", 9000, 2000),
                2000,
                now,
            )
            .unwrap();
        assert!(fresh.jobs.is_empty());
        fresh
            .update(
                job("timer", now),
                &runtime("idle", 10000, 1000),
                &running,
                1100,
                now,
            )
            .unwrap();
        let deadline = fresh.jobs["timer"].deadline;
        assert_eq!(deadline, now + Duration::from_millis(9900));
        // willDisappear persistence does not move the deadline, including a wall-clock jump.
        fresh
            .update(
                job("timer", now),
                &running,
                &runtime("running", 9000, 100000),
                100000,
                now + Duration::from_millis(1000),
            )
            .unwrap();
        assert_eq!(fresh.jobs["timer"].deadline, deadline);
        assert!(fresh.due(deadline - Duration::from_millis(1)).is_empty());
        assert_eq!(fresh.due(deadline).len(), 1);
        assert!(fresh.due(deadline + Duration::from_secs(1)).is_empty());
        // A new process starts empty and does not recover these volatile jobs.
        assert!(TimerPlanner::default().due(deadline).is_empty());
    }
    #[test]
    fn pause_resume_and_stop_cancel_only_their_future_output() {
        let now = Instant::now();
        let idle = runtime("idle", 10000, 1000);
        let running = runtime("running", 10000, 1000);
        let paused = runtime("paused", 7000, 4000);
        let mut controls = Controls::default();
        controls
            .timers
            .update(job("timer", now), &idle, &running, 1000, now)
            .unwrap();
        controls
            .timers
            .update(
                job("timer", now),
                &running,
                &paused,
                4000,
                now + Duration::from_secs(3),
            )
            .unwrap();
        assert!(
            controls
                .timers
                .due(now + Duration::from_secs(30))
                .is_empty()
        );
        controls
            .timers
            .update(
                job("timer", now),
                &paused,
                &runtime("running", 7000, 5000),
                5000,
                now + Duration::from_secs(4),
            )
            .unwrap();
        assert_eq!(
            controls.timers.jobs["timer"].deadline,
            now + Duration::from_secs(11)
        );
        let mut sibling = job("sibling", now);
        sibling.alert.device_id = "other".into();
        controls
            .timers
            .update(sibling, &idle, &running, 1000, now)
            .unwrap();
        controls.start(alert("output"), None).unwrap();
        controls.cancel("output".into()).unwrap();
        assert!(!controls.starts.contains_key("output"));
        assert!(!controls.timers.jobs.contains_key("timer"));
        assert!(controls.timers.jobs.contains_key("sibling"));
    }
    #[test]
    fn early_plugin_done_and_late_done_each_complete_only_an_already_armed_job_once() {
        let now = Instant::now();
        let idle = runtime("idle", 10000, 1000);
        let running = runtime("running", 10000, 1000);
        let done = runtime("done", 0, 11000);
        let mut planner = TimerPlanner::default();
        assert!(
            planner
                .update(job("timer", now), &running, &done, 11000, now)
                .unwrap()
                .is_none()
        );
        planner
            .update(job("timer", now), &idle, &running, 1000, now)
            .unwrap();
        assert!(
            planner
                .update(
                    job("timer", now),
                    &running,
                    &done,
                    11000,
                    now + Duration::from_millis(9999)
                )
                .unwrap()
                .is_some()
        );
        assert!(planner.due(now + Duration::from_secs(20)).is_empty());
        assert!(
            planner
                .update(
                    job("timer", now),
                    &done,
                    &done,
                    11000,
                    now + Duration::from_secs(20)
                )
                .unwrap()
                .is_none()
        );
        planner
            .update(job("timer", now), &idle, &running, 1000, now)
            .unwrap();
        assert_eq!(planner.due(now + Duration::from_secs(10)).len(), 1);
        assert!(
            planner
                .update(
                    job("timer", now),
                    &running,
                    &done,
                    11000,
                    now + Duration::from_secs(11)
                )
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn changing_config_does_not_rearm_and_jobs_are_bounded() {
        let now = Instant::now();
        let idle = runtime("idle", 10000, 1000);
        let running = runtime("running", 10000, 1000);
        let mut planner = TimerPlanner::default();
        planner
            .update(job("timer", now), &idle, &running, 1000, now)
            .unwrap();
        let mut changed = job("timer", now);
        changed.alert.color = [0, 0, 255];
        planner
            .update(changed, &running, &running, 1000, now)
            .unwrap();
        assert!(planner.jobs.is_empty());
        for n in 0..128 {
            planner
                .update(job(&format!("timer-{n}"), now), &idle, &running, 1000, now)
                .unwrap();
        }
        assert!(
            planner
                .update(job("overflow", now), &idle, &running, 1000, now)
                .is_err()
        );
        assert_eq!(planner.jobs.len(), 128);
        for malformed in [
            runtime("running", 0, 1000),
            runtime("running", 86400001, 1000),
            runtime("running", 1000, 5000),
        ] {
            assert!(
                TimerPlanner::default()
                    .update(job("bad", now), &idle, &malformed, 1000, now)
                    .unwrap()
                    .is_none()
            );
        }
    }
    #[test]
    fn pending_changes_are_coalesced_and_stop_discards_unsent_start() {
        let mut pending = Controls::default();
        pending.start(alert("one"), None).unwrap();
        let mut latest = alert("one");
        latest.color = [0, 0, 255];
        pending.start(latest, None).unwrap();
        assert_eq!(pending.starts.len(), 1);
        assert_eq!(pending.starts["one"].0.color, [0, 0, 255]);
        pending.cancel("one".into()).unwrap();
        assert!(pending.starts.is_empty());
        assert!(pending.stops.contains("one"));
        for i in 0..32 {
            pending.start(alert(&format!("device-{i}")), None).unwrap();
        }
        assert!(pending.start(alert("overflow"), None).is_err());
        assert!(pending.start(alert("device-0"), None).is_ok());
    }
    #[test]
    fn phase_has_no_catchup_or_restart_and_input_is_bounded() {
        assert!(phase_at(0, 1000));
        assert!(!phase_at(500, 1000));
        assert!(phase_at(10000, 1000));
        let mut alert = Alert {
            device_id: "device".into(),
            control_id: "key-37".into(),
            color: [255, 0, 0],
            period_ms: 1000,
            duration_ms: 5000,
            enabled: true,
        };
        assert!(alert.validate().is_ok());
        alert.enabled = false;
        assert!(alert.validate().is_err());
        alert.enabled = true;
        alert.period_ms = 0;
        assert!(alert.validate().is_err());
        alert.period_ms = 1000;
        alert.duration_ms = 60001;
        assert!(alert.validate().is_err());
    }
    #[test]
    fn missing_or_malformed_baseline_cannot_be_replaced_with_cached_colour() {
        assert_eq!(
            baseline_from_ack(
                &json!({"baseline":{"key-37":{"r":0,"g":255,"b":12}}}),
                "key-37"
            )
            .unwrap(),
            [0, 255, 12]
        );
        for ack in [
            json!({}),
            json!({"baseline":{"key-36":{"r":0,"g":1,"b":2}}}),
            json!({"baseline":{"key-37":{"r":256,"g":1,"b":2}}}),
            json!({"baseline":{"key-37":{"r":0,"g":1}}}),
        ] {
            assert!(baseline_from_ack(&ack, "key-37").is_err());
        }
    }
}
