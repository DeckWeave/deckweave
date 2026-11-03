//! Shared bounded service worker. Plugin ingestion never waits for SQLite or windows.
use deckweave_platform::notification_sound::{self, Cue};
use deckweave_services::{
    Content, Inventory, Panel, PanelSpec, Placement, ServiceError, Store,
    focus::{FocusConfig, FocusJob, FocusStatus, FocusView},
    timers::{TimerRun, TimerStatus},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub fn capabilities() -> Value {
    json!({"sound":1,"panels":1,"timers":1,"focus":1,"images":0,"callbacks":0})
}
#[derive(Clone, Serialize)]
pub struct View {
    pub locale: String,
    pub panel: Panel,
    pub online: bool,
    pub value: Option<Value>,
    pub timer: Option<TimerRun>,
    pub remaining_ms: Option<u64>,
    pub error: Option<String>,
}
#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub locale: String,
    pub panels: Vec<View>,
    pub unavailable: Vec<deckweave_services::InvalidRecord>,
    pub error: Option<String>,
    pub outcomes: Vec<Value>,
    pub focus: Vec<FocusView>,
}
pub struct Request {
    pub owner: String,
    pub origin: String,
    pub generation: String,
    pub operation: String,
    pub payload: Value,
    pub request_id: String,
    pub reply: SyncSender<Value>,
}
enum Message {
    Request(Request),
    Wake,
    Stop,
}
struct Sound {
    owner: String,
    cue: Cue,
    volume: u8,
    event_id: String,
    reply: Option<(SyncSender<Value>, String)>,
}
type Connections = Arc<Mutex<BTreeMap<String, String>>>;
#[derive(Default)]
pub struct PluginDemand {
    pub panels: Vec<View>,
    pub owners: BTreeSet<String>,
    pub focus: Vec<FocusView>,
}
pub struct Runtime {
    rendered: Mutex<BTreeMap<String, Placement>>,
    render_pending: AtomicBool,
    ready: AtomicBool,
    ingress: Mutex<BTreeMap<String, (Instant, u32)>>,
    sender: SyncSender<Message>,
    pub snapshot: Arc<Mutex<Snapshot>>,
    connections: Connections,
    blocked: Arc<Mutex<BTreeSet<String>>>,
    stop: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
    audio: Mutex<Option<JoinHandle<()>>>,
}
fn wall() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
fn error(e: impl ToString) -> ServiceError {
    ServiceError::new("service", e.to_string())
}
fn response(id: &str, result: Result<Value, ServiceError>) -> Value {
    match result {
        Ok(value) => {
            json!({"event":"deckweaveServiceResult","requestId":id,"ok":true,"value":value})
        }
        Err(e) => json!({"event":"deckweaveServiceResult","requestId":id,"ok":false,"error":e}),
    }
}
impl Runtime {
    pub fn start(app: tauri::AppHandle, directory: PathBuf) -> Self {
        let (sender, receiver) = mpsc::sync_channel(32);
        let (sounds, sound_rx) = mpsc::sync_channel::<Sound>(4);
        let snapshot = Arc::new(Mutex::new(Snapshot {
            locale: "ru".into(),
            ..Snapshot::default()
        }));
        let connections = Arc::new(Mutex::new(BTreeMap::new()));
        let blocked = Arc::new(Mutex::new(BTreeSet::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let audio = {
            let stop = stop.clone();
            let blocked = blocked.clone();
            let state = snapshot.clone();
            let app = app.clone();
            thread::spawn(move || {
                while let Ok(sound) = sound_rx.recv() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let result = if blocked.lock().map_or(true, |b| b.contains(&sound.owner)) {
                        Err("Plugin is disabled".into())
                    } else {
                        notification_sound::play(sound.cue, sound.volume)
                    };
                    let outcome = json!({"event":"deckweaveSoundFinished","eventId":sound.event_id,"status":if result.is_ok(){"finished"}else{"failed"},"error":result.err()});
                    if let Some((reply, id)) = sound.reply {
                        let mut message = outcome.clone();
                        message["requestId"] = json!(id);
                        let _ = reply.try_send(message);
                    }
                    if let Ok(mut s) = state.lock() {
                        s.outcomes.push(outcome);
                        if s.outcomes.len() > 64 {
                            s.outcomes.remove(0);
                        }
                    }
                    let _ = app.emit_to("main", "deckweave:services", ());
                }
            })
        };
        let worker = {
            let state = snapshot.clone();
            let connections = connections.clone();
            let blocked = blocked.clone();
            let stop = stop.clone();
            thread::spawn(move || {
                run(
                    app,
                    directory,
                    receiver,
                    sounds,
                    state,
                    connections,
                    blocked,
                    stop,
                );
            })
        };
        Self {
            rendered: Mutex::new(BTreeMap::new()),
            render_pending: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            ingress: Mutex::new(BTreeMap::new()),
            sender,
            snapshot,
            connections,
            blocked,
            stop,
            worker: Mutex::new(Some(worker)),
            audio: Mutex::new(Some(audio)),
        }
    }
    pub fn submit(&self, request: Request) -> Result<(), ServiceError> {
        if request.payload.to_string().len() > 24576 || request.request_id.len() > 128 {
            return Err(ServiceError::new("quota", "Service request is too large"));
        }
        if request.owner != "deckweave.host" {
            let mut limits = self.ingress.lock().map_err(error)?;
            limits.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(1));
            let rate = limits
                .entry(request.owner.clone())
                .or_insert((Instant::now(), 0));
            if rate.1 >= 8 {
                return Err(ServiceError::new(
                    "busy",
                    "At most eight service requests per second per plugin",
                ));
            }
            rate.1 += 1;
        }
        self.sender
            .try_send(Message::Request(request))
            .map_err(|_| ServiceError::new("busy", "Service queue is full; retry later"))
    }
    pub fn connection(&self, owner: &str, generation: Option<&str>) {
        let mut changed = false;
        if let Ok(mut c) = self.connections.lock() {
            if let Some(g) = generation {
                if c.get(owner).map(String::as_str) != Some(g) {
                    c.insert(owner.into(), g.into());
                    changed = true;
                }
            } else {
                changed = c.remove(owner).is_some();
            }
        }
        if changed {
            let _ = self.sender.try_send(Message::Wake);
        }
    }
    pub fn blocked(&self, owners: BTreeSet<String>) {
        if let Ok(mut b) = self.blocked.lock() {
            if *b == owners {
                return;
            }
            *b = owners;
        }
        let _ = self.sender.try_send(Message::Wake);
    }
    pub fn current(&self) -> Snapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_default()
    }
    pub fn ready(&self) {
        self.ready.store(true, Ordering::Release);
        let _ = self.sender.try_send(Message::Wake);
    }
    /// The package supervisor polls often; never clone saved note bodies or outcomes there.
    pub fn plugin_demand(&self) -> PluginDemand {
        self.snapshot
            .lock()
            .map(|s| PluginDemand {
                panels: s
                    .panels
                    .iter()
                    .filter(|v| {
                        v.panel.open && matches!(v.panel.spec.content, Content::Metric { .. })
                    })
                    .cloned()
                    .collect(),
                owners: s
                    .panels
                    .iter()
                    .map(|v| v.panel.owner.clone())
                    .chain(s.focus.iter().map(|v| v.job.owner.clone()))
                    .collect(),
                focus: s.focus.clone(),
            })
            .unwrap_or_default()
    }
    pub fn locale(&self, locale: deckweave_core::Locale) {
        let locale = if locale == deckweave_core::Locale::Ru {
            "ru"
        } else {
            "en"
        };
        if let Ok(mut s) = self.snapshot.lock()
            && s.locale != locale
        {
            s.locale = locale.into();
            let _ = self.sender.try_send(Message::Wake);
        }
    }
    pub(super) fn call(&self, operation: String, payload: Value) -> Result<Value, ServiceError> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.submit(Request {
            owner: "deckweave.host".into(),
            origin: "settings".into(),
            generation: String::new(),
            operation,
            payload,
            request_id: "settings".into(),
            reply: tx,
        })?;
        let r = rx.recv_timeout(Duration::from_secs(5)).map_err(error)?;
        if r["ok"] == true {
            Ok(r["value"].clone())
        } else {
            Err(ServiceError::new(
                r["error"]["code"].as_str().unwrap_or("service"),
                r["error"]["message"]
                    .as_str()
                    .unwrap_or("Service request failed"),
            ))
        }
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.sender.try_send(Message::Stop);
        if let Ok(mut w) = self.worker.lock()
            && let Some(w) = w.take()
        {
            let _ = w.join();
        }
        if let Ok(mut a) = self.audio.lock()
            && let Some(a) = a.take()
        {
            let _ = a.join();
        }
    }
}
fn refresh(
    store: &Store,
    state: &Arc<Mutex<Snapshot>>,
    connections: &Connections,
    now: u64,
) -> Result<(), ServiceError> {
    let Inventory {
        panels,
        unavailable,
    } = store.inventory()?;
    let online = connections.lock().map_err(error)?;
    let mut snapshot = state.lock().map_err(error)?;
    let old = std::mem::take(&mut snapshot.panels);
    let locale = snapshot.locale.clone();
    snapshot.panels = panels
        .into_iter()
        .map(|panel| {
            let before = old.iter().find(|v| v.panel.id == panel.id);
            let timer = store.timer(&panel.id).ok().flatten();
            View {
                locale: locale.clone(),
                online: online.contains_key(&panel.owner),
                value: before.and_then(|v| v.value.clone()),
                remaining_ms: timer.as_ref().map(|t| t.remaining(now)),
                timer,
                panel,
                error: before.and_then(|v| v.error.clone()),
            }
        })
        .collect();
    snapshot.unavailable = unavailable;
    let focus = store.focus_inventory()?;
    snapshot.unavailable.extend(focus.unavailable);
    snapshot.focus = focus.jobs.iter().map(|job| job.view(now)).collect();
    Ok(())
}
#[expect(
    clippy::too_many_arguments,
    reason = "Dedicated worker takes ownership of bounded channels and shared lifecycle fences"
)]
fn run(
    app: tauri::AppHandle,
    directory: PathBuf,
    receiver: Receiver<Message>,
    sounds: SyncSender<Sound>,
    state: Arc<Mutex<Snapshot>>,
    connections: Connections,
    blocked: Arc<Mutex<BTreeSet<String>>>,
    stop: Arc<AtomicBool>,
) {
    let store = Store::open(&directory.join("host-services.sqlite3"));
    let store = match store {
        Ok(s) => s,
        Err(e) => {
            if let Ok(mut s) = state.lock() {
                s.error = Some(e.message);
            }
            while !stop.load(Ordering::Acquire) {
                let Ok(m) = receiver.recv() else {
                    break;
                };
                match m {
                    Message::Request(r) => {
                        let _ = r.reply.try_send(response(
                            &r.request_id,
                            Err(ServiceError::new(
                                "storage",
                                "Host services could not open their database",
                            )),
                        ));
                    }
                    Message::Stop => break,
                    _ => {}
                }
            }
            return;
        }
    };
    let base = wall();
    let started = Instant::now();
    let now = || base.saturating_add(started.elapsed().as_millis() as u64);
    if let Err(e) = store
        .finish_due(base, false)
        .and_then(|_| store.finish_focus_due(base, false))
        .and_then(|_| refresh(&store, &state, &connections, base))
        && let Ok(mut s) = state.lock()
    {
        s.error = Some(e.message);
    }
    let mut dedup = VecDeque::<(String, String, Instant)>::new();
    let mut limits = BTreeMap::<String, (Instant, u32)>::new();
    let mut cancelled = BTreeSet::new();
    render(&app, &state);
    while !stop.load(Ordering::Acquire) {
        let active = state.lock().is_ok_and(|s| {
            s.focus.iter().any(|v| v.job.status == FocusStatus::Running)
                || s.panels.iter().any(|v| {
                    v.timer
                        .as_ref()
                        .is_some_and(|t| t.status == TimerStatus::Running)
                })
        });
        let message = if active {
            receiver.recv_timeout(Duration::from_millis(1000)).ok()
        } else {
            receiver.recv().ok()
        };
        if stop.load(Ordering::Acquire) || matches!(message, Some(Message::Stop)) {
            break;
        }
        if let Some(Message::Request(request)) = message {
            let rate = limits
                .entry(request.owner.clone())
                .or_insert((Instant::now(), 0));
            if rate.0.elapsed() > Duration::from_secs(1) {
                *rate = (Instant::now(), 0);
            }
            rate.1 += 1;
            let authorized = request.owner == "deckweave.host"
                || connections
                    .lock()
                    .is_ok_and(|c| c.get(&request.owner) == Some(&request.generation));
            let denied = blocked.lock().map_or(true, |b| b.contains(&request.owner));
            let result = if !authorized || denied {
                Err(ServiceError::new(
                    "owner",
                    "Service connection is no longer active",
                ))
            } else if rate.1 > 30 {
                Err(ServiceError::new("quota", "Plugin service rate exceeded"))
            } else {
                execute(&app, &store, &state, &request, &sounds, &mut dedup, now())
            };
            let _ = request.reply.try_send(response(
                &request.request_id,
                result.and_then(|value| {
                    refresh(&store, &state, &connections, now())?;
                    Ok(value)
                }),
            ));
        }
        let blocked_now = blocked.lock().map(|b| b.clone()).unwrap_or_default();
        for owner in blocked_now.difference(&cancelled) {
            let _ = store.cancel_owner_timers(owner, now());
            let _ = store.pause_owner_focus(owner, now());
        }
        cancelled = blocked_now;
        match store.finish_focus_due(now(), true) {
            Ok(jobs) => {
                for job in jobs {
                    if !cancelled.contains(&job.owner) {
                        announce_focus(&job, &sounds, &state);
                    }
                }
            }
            Err(e) => {
                if let Ok(mut s) = state.lock() {
                    s.error = Some(e.message);
                }
            }
        }
        match store.finish_due(now(), true) {
            Ok(due) => {
                for run in due {
                    if let Ok(panel) = store.get(&run.panel)
                        && !cancelled.contains(&panel.owner)
                        && let Some(cue) = run
                            .sound
                            .and_then(|s| serde_json::from_value::<Cue>(json!(s)).ok())
                    {
                        let event_id = run.run_id;
                        if sounds
                            .try_send(Sound {
                                owner: panel.owner,
                                cue,
                                volume: run.volume,
                                event_id: event_id.clone(),
                                reply: None,
                            })
                            .is_err()
                            && let Ok(mut s) = state.lock()
                        {
                            s.outcomes.push(json!({"event":"deckweaveSoundFinished","eventId":event_id,"status":"failed","error":"Sound queue is full"}));
                            if s.outcomes.len() > 64 {
                                s.outcomes.remove(0);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if let Ok(mut s) = state.lock() {
                    s.error = Some(e.message)
                }
            }
        }
        if let Err(e) = refresh(&store, &state, &connections, now())
            && let Ok(mut s) = state.lock()
        {
            s.error = Some(e.message);
        }
        render(&app, &state);
        let _ = app.emit_to("main", "deckweave:services", ());
    }
}
/// The durable phase transition precedes this single playback attempt. Never retry
/// uncertain audio on restart; outcomes distinguish queueing failure from playback.
fn announce_focus(job: &FocusJob, sounds: &SyncSender<Sound>, state: &Arc<Mutex<Snapshot>>) {
    let Some(cue) = job
        .config
        .sound
        .as_ref()
        .and_then(|s| serde_json::from_value::<Cue>(json!(s)).ok())
    else {
        return;
    };
    let event_id = job.event_id();
    let queued = sounds
        .try_send(Sound {
            owner: job.owner.clone(),
            cue,
            volume: job.config.volume,
            event_id: event_id.clone(),
            reply: None,
        })
        .is_ok();
    if let Ok(mut s) = state.lock() {
        s.outcomes.push(json!({"event":"deckweaveSoundFinished","eventId":event_id,"workspace":job.workspace,"status":if queued{"queued"}else{"failed"},"error":if queued{Value::Null}else{json!("Sound queue is full")}}));
        if s.outcomes.len() > 64 {
            s.outcomes.remove(0);
        }
    }
}
fn execute(
    app: &tauri::AppHandle,
    store: &Store,
    state: &Arc<Mutex<Snapshot>>,
    r: &Request,
    sounds: &SyncSender<Sound>,
    dedup: &mut VecDeque<(String, String, Instant)>,
    now: u64,
) -> Result<Value, ServiceError> {
    let id = r.payload["id"].as_str().unwrap_or("");
    let host = r.owner == "deckweave.host";
    let owned = || -> Result<Panel, ServiceError> {
        let panel = store.get(id)?;
        if !host && panel.owner != r.owner {
            return Err(ServiceError::new(
                "owner",
                "Panel belongs to another plugin",
            ));
        }
        if !host && r.origin != panel.origin && r.origin != panel.service_id {
            return Err(ServiceError::new(
                "owner",
                "Panel belongs to another service instance",
            ));
        }
        Ok(panel)
    };
    match r.operation.as_str() {
        "focus_get" => {
            let workspace = r.payload["workspace"].as_str().unwrap_or("personal");
            let config = r
                .payload
                .get("config")
                .map(|v| serde_json::from_value::<FocusConfig>(v.clone()))
                .transpose()?
                .unwrap_or_default();
            Ok(json!(
                store.ensure_focus(&r.owner, workspace, config)?.view(now)
            ))
        }
        "focus_control" => {
            let workspace = r.payload["workspace"].as_str().unwrap_or("personal");
            let revision = r.payload["revision"]
                .as_i64()
                .ok_or_else(|| ServiceError::new("revision", "Expected Pomodoro revision"))?;
            let command_id = r.payload["commandId"]
                .as_str()
                .ok_or_else(|| ServiceError::new("identity", "Expected stable commandId"))?;
            let command = r.payload["command"].as_str().unwrap_or("");
            let config = r
                .payload
                .get("config")
                .map(|v| serde_json::from_value::<FocusConfig>(v.clone()))
                .transpose()?;
            let before = store.focus_job(&r.owner, workspace)?;
            let job = store.focus_control(
                &r.owner, workspace, revision, command_id, command, config, now,
            )?;
            if before
                .as_ref()
                .is_some_and(|b| b.run_id == job.run_id && b.transition < job.transition)
            {
                announce_focus(&job, sounds, state);
            }
            Ok(json!(job.view(now)))
        }
        "get" => Ok(json!(
            store
                .find_panel(
                    &r.owner,
                    &r.origin,
                    r.payload["key"].as_str().unwrap_or("main")
                )?
                .ok_or_else(|| ServiceError::new("not_found", "No saved panel for this action"))?
        )),
        "open" => {
            let spec: PanelSpec = serde_json::from_value(r.payload["spec"].clone())?;
            let key = r.payload["key"].as_str().unwrap_or("main");
            Ok(json!(store.open_panel(&r.owner, &r.origin, key, spec)?))
        }
        "configure" => {
            let p = owned()?;
            let spec = serde_json::from_value(r.payload["spec"].clone())?;
            let revision = r.payload["revision"]
                .as_i64()
                .ok_or_else(|| ServiceError::new("revision", "Expected panel revision"))?;
            Ok(json!(store.configure(&p.owner, id, revision, spec)?))
        }
        "close" | "reopen" => {
            let p = owned()?;
            let revision = r.payload["revision"]
                .as_i64()
                .ok_or_else(|| ServiceError::new("revision", "Expected panel revision"))?;
            Ok(json!(store.set_open(
                &p.owner,
                id,
                revision,
                r.operation == "reopen"
            )?))
        }
        "remove" => {
            if !host {
                return Err(ServiceError::new(
                    "owner",
                    "Remove panels from application settings",
                ));
            }
            let p = owned()?;
            store.remove(&p.owner, id, r.payload["revision"].as_i64().unwrap_or(-1))?;
            Ok(json!({"removed":true}))
        }
        "update" => {
            let p = owned()?;
            if r.payload["revision"].as_i64() != Some(p.revision) {
                return Err(ServiceError::new(
                    "conflict",
                    "Panel changed; wait for its updated service view",
                ));
            }
            if !p.open {
                return Err(ServiceError::new(
                    "closed",
                    "Panel is closed; updates do not reopen it",
                ));
            }
            if !matches!(p.spec.content, Content::Metric { .. }) {
                return Err(ServiceError::new(
                    "content",
                    "Live updates require a metric panel",
                ));
            }
            let value = &r.payload["value"];
            if value["value"].as_str().is_none_or(|s| s.len() > 256)
                || value
                    .get("detail")
                    .is_some_and(|v| v.as_str().is_none_or(|s| s.len() > 1024))
                || value
                    .as_object()
                    .is_none_or(|o| o.keys().any(|k| k != "value" && k != "detail"))
            {
                return Err(ServiceError::new("quota", "Live value is too large"));
            }
            if let Ok(mut s) = state.lock()
                && let Some(v) = s.panels.iter_mut().find(|v| v.panel.id == id)
            {
                v.value = Some(value.clone());
            }
            Ok(json!({"status":"updated"}))
        }
        "timer" => {
            let p = owned()?;
            let Content::Timer {
                sound: default_sound,
                volume: default_volume,
                ..
            } = &p.spec.content
            else {
                return Err(ServiceError::new("timer", "This panel is not a timer"));
            };
            let command = r.payload["command"].as_str().unwrap_or("");
            let sound = r
                .payload
                .get("sound")
                .map(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| default_sound.clone());
            let volume = r.payload["volume"]
                .as_u64()
                .unwrap_or(*default_volume as u64);
            if volume > 100 {
                return Err(ServiceError::new("volume", "Volume must be 0..100"));
            }
            Ok(json!(store.timer_control(
                &p.owner,
                id,
                command,
                now,
                sound,
                volume as u8
            )?))
        }
        "sound" => {
            let cue: Cue = serde_json::from_value(r.payload["sound"].clone())?;
            let volume = r.payload["volume"].as_u64().unwrap_or(35);
            if volume > 100 {
                return Err(ServiceError::new("volume", "Volume must be 0..100"));
            }
            let event = r.payload["eventId"]
                .as_str()
                .ok_or_else(|| ServiceError::new("identity", "Sound requires an eventId"))?;
            deckweave_services::identity(event)?;
            while dedup
                .front()
                .is_some_and(|e| e.2.elapsed() > Duration::from_secs(300))
                || dedup.len() > 256
            {
                dedup.pop_front();
            }
            if dedup
                .iter()
                .any(|(owner, key, _)| owner == &r.owner && key == event)
            {
                return Ok(json!({"status":"duplicate"}));
            }
            sounds
                .try_send(Sound {
                    owner: r.owner.clone(),
                    cue,
                    volume: volume as u8,
                    event_id: event.into(),
                    reply: Some((r.reply.clone(), r.request_id.clone())),
                })
                .map_err(|_| ServiceError::new("busy", "Sound queue is full"))?;
            dedup.push_back((r.owner.clone(), event.into(), Instant::now()));
            Ok(json!({"status":"accepted"}))
        }
        "locate" => {
            let p = owned()?;
            let mut placement = p.spec.placement.clone();
            placement.anchor = "top_right".into();
            placement.monitor = None;
            placement.x = 16.;
            placement.y = 16.;
            placement.click_through = false;
            placement.opacity = placement.opacity.max(30);
            let spec = PanelSpec {
                placement,
                ..p.spec
            };
            Ok(json!(store.configure(&p.owner, id, p.revision, spec)?))
        }
        "drag" => {
            let _ = owned()?;
            let id = id.to_owned();
            let label = format!("overlay-{id}");
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(w) = handle.get_webview_window(&label)
                    && w.start_dragging().is_ok()
                    && let Ok(position) = w.outer_position()
                    && let Ok(scale) = w.scale_factor()
                    && let Some(runtime) = handle.try_state::<Runtime>()
                    && let Some(view) = runtime
                        .current()
                        .panels
                        .into_iter()
                        .find(|v| v.panel.id == id)
                {
                    let mut spec = view.panel.spec;
                    spec.placement.anchor = "free".into();
                    spec.placement.x = position.x as f64 / scale;
                    spec.placement.y = position.y as f64 / scale;
                    spec.placement.monitor = w
                        .current_monitor()
                        .ok()
                        .flatten()
                        .and_then(|m| m.name().cloned());
                    let (tx, _rx) = mpsc::sync_channel(1);
                    let _ = runtime.submit(Request {
                        owner: "deckweave.host".into(),
                        origin: "settings".into(),
                        generation: String::new(),
                        operation: "configure".into(),
                        payload: json!({"id":id,"revision":view.panel.revision,"spec":spec}),
                        request_id: "placement".into(),
                        reply: tx,
                    });
                }
            });
            Ok(json!({"status":"accepted"}))
        }
        _ => Err(ServiceError::new(
            "unsupported",
            "Unknown host service operation",
        )),
    }
}
fn apply_placement(w: &WebviewWindow, p: &Placement) {
    let _ = w.set_size(tauri::LogicalSize::new(p.width, p.height));
    let monitor = w
        .available_monitors()
        .ok()
        .and_then(|ms| {
            ms.into_iter().find(|m| {
                p.monitor
                    .as_ref()
                    .is_some_and(|name| m.name() == Some(name))
            })
        })
        .or_else(|| w.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let scale = m.scale_factor();
        let area = m.work_area();
        let mx = area.position.x as f64 / scale;
        let my = area.position.y as f64 / scale;
        let width = area.size.width as f64 / scale;
        let height = area.size.height as f64 / scale;
        let x = if p.anchor.ends_with("right") {
            mx + width - p.width - p.x
        } else if p.anchor == "free" {
            p.x
        } else {
            mx + p.x
        };
        let y = if p.anchor.starts_with("bottom") {
            my + height - p.height - p.y
        } else if p.anchor == "free" {
            p.y
        } else {
            my + p.y
        };
        let _ = w.set_position(tauri::PhysicalPosition::new(
            (x.clamp(mx, mx + (width - p.width).max(0.)) * scale).round() as i32,
            (y.clamp(my, my + (height - p.height).max(0.)) * scale).round() as i32,
        ));
    }
    let _ = w.set_ignore_cursor_events(p.click_through);
}
fn render(app: &tauri::AppHandle, state: &Arc<Mutex<Snapshot>>) {
    let Some(runtime) = app.try_state::<Runtime>() else {
        return;
    };
    if !runtime.ready.load(Ordering::Acquire) || runtime.render_pending.swap(true, Ordering::AcqRel)
    {
        return;
    }
    let state = state.clone();
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let Some(runtime) = app.try_state::<Runtime>() else {
            return;
        };
        runtime.render_pending.store(false, Ordering::Release);
        let snapshot = state.lock().map(|s| s.clone()).unwrap_or_default();
        let open: BTreeSet<_> = snapshot
            .panels
            .iter()
            .filter(|v| v.panel.open)
            .map(|v| format!("overlay-{}", v.panel.id))
            .collect();
        for (label, w) in app.webview_windows() {
            if label.starts_with("overlay-") && !open.contains(&label) {
                let _ = w.destroy();
            }
        }
        if let Ok(mut cache) = runtime.rendered.lock() {
            cache.retain(|id, _| open.contains(&format!("overlay-{id}")));
        }
        for view in snapshot.panels.iter().filter(|v| v.panel.open) {
            let label = format!("overlay-{}", view.panel.id);
            let p = &view.panel.spec.placement;
            if let Some(w) = app.get_webview_window(&label) {
                let changed = runtime
                    .rendered
                    .lock()
                    .is_ok_and(|cache| cache.get(&view.panel.id) != Some(p));
                if changed {
                    apply_placement(&w, p);
                    if let Ok(mut cache) = runtime.rendered.lock() {
                        cache.insert(view.panel.id.clone(), p.clone());
                    }
                }
                let _ = w.set_title(&view.panel.spec.title);
                let _ = w.emit("deckweave:panel", view);
                continue;
            }
            let mut builder = WebviewWindowBuilder::new(
                &app,
                &label,
                WebviewUrl::App(format!("index.html?panel={}", view.panel.id).into()),
            )
            .title(&view.panel.spec.title)
            .inner_size(p.width, p.height)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .focusable(false)
            .visible(false);
            // Share the editor's WebView2 environment, including isolated smoke data.
            // A different/absent directory starts an unnecessary second browser tree.
            if let Some(directory) = crate::directory_override() {
                builder = builder.data_directory(directory.join("web-editor"));
            }
            let created = builder.build();
            match created {
                Ok(w) => {
                    apply_placement(&w, p);
                    if let Ok(mut cache) = runtime.rendered.lock() {
                        cache.insert(view.panel.id.clone(), p.clone());
                    }
                    let _ = w.show();
                }
                Err(e) => {
                    if let Ok(mut state) = runtime.snapshot.lock()
                        && let Some(v) = state
                            .panels
                            .iter_mut()
                            .find(|v| v.panel.id == view.panel.id)
                    {
                        v.error = Some(e.to_string());
                    }
                }
            }
        }
    });
}
#[tauri::command]
pub fn host_service_inventory(
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Snapshot, ServiceError> {
    if window.label() != "main" {
        return Err(ServiceError::new(
            "window",
            "Open Panels in application settings",
        ));
    }
    Ok(app
        .try_state::<Runtime>()
        .ok_or_else(|| ServiceError::new("unavailable", "Host services are unavailable"))?
        .current())
}
#[tauri::command]
pub fn host_panel_view(
    id: String,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<View, ServiceError> {
    if window.label() != format!("overlay-{id}") {
        return Err(ServiceError::new(
            "window",
            "This window cannot read that panel",
        ));
    }
    app.try_state::<Runtime>()
        .ok_or_else(|| ServiceError::new("unavailable", "Host services are unavailable"))?
        .current()
        .panels
        .into_iter()
        .find(|v| v.panel.id == id)
        .ok_or_else(|| ServiceError::new("not_found", "Panel not found"))
}
#[tauri::command]
pub async fn host_service_command(
    operation: String,
    payload: Value,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Value, ServiceError> {
    if window.label() != "main" {
        let id = payload["id"].as_str().unwrap_or("");
        if window.label() != format!("overlay-{id}")
            || !["close", "timer", "drag"].contains(&operation.as_str())
        {
            return Err(ServiceError::new(
                "window",
                "This panel cannot perform that operation",
            ));
        }
    }
    tauri::async_runtime::spawn_blocking(move || {
        app.try_state::<Runtime>()
            .ok_or_else(|| ServiceError::new("unavailable", "Host services are unavailable"))?
            .call(operation, payload)
    })
    .await
    .map_err(error)?
}
