//! Native workers own their HID handles. Core locks never cover USB, rendering or file I/O.
use base64::{Engine, engine::general_purpose::STANDARD};
use deckweave_core::{
    ActionKind, ConnectionState, ControlKind, Core, DeviceHardwareSettings, InputEvent, Snapshot,
};
use deckweave_n4::{Input, KeyVisual, RegionVisual, Renderer};
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;

#[derive(Clone, Serialize)]
pub struct Frame {
    page_id: String,
    binding_id: Option<String>,
    data_url: String,
}
type Images = BTreeMap<String, BTreeMap<String, Frame>>;

#[derive(Serialize)]
pub struct LiveSnapshot {
    #[serde(flatten)]
    snapshot: Snapshot,
    key_images: Images,
    hardware_settings: BTreeMap<String, HardwareSettingsStatus>,
}

#[derive(Clone, Default)]
struct HardwareSettingsRuntime {
    sent: DeviceHardwareSettings,
    connected: bool,
    failed: Option<(&'static str, String)>,
}

#[derive(Serialize)]
struct SettingStatus<T: Serialize> {
    requested: Option<T>,
    sent: Option<T>,
    state: &'static str,
    error: Option<String>,
}
#[derive(Serialize)]
struct HardwareSettingsStatus {
    screen_brightness: SettingStatus<u8>,
    dial_brightness: SettingStatus<u8>,
    vibration: SettingStatus<bool>,
}
impl HardwareSettingsRuntime {
    fn field<T: Copy + Eq + Serialize>(
        &self,
        name: &str,
        requested: Option<T>,
        sent: Option<T>,
    ) -> SettingStatus<T> {
        let error = self
            .failed
            .as_ref()
            .filter(|(field, _)| *field == name)
            .map(|(_, error)| error.clone());
        let state = if requested.is_none() {
            "unchanged"
        } else if error.is_some() {
            "error"
        } else if !self.connected {
            "offline"
        } else if requested == sent {
            "sent_no_readback"
        } else {
            "pending"
        };
        SettingStatus {
            requested,
            sent,
            state,
            error,
        }
    }
    fn status(&self, requested: &DeviceHardwareSettings) -> HardwareSettingsStatus {
        HardwareSettingsStatus {
            screen_brightness: self.field(
                "screen_brightness",
                requested.screen_brightness,
                self.sent.screen_brightness,
            ),
            dial_brightness: self.field(
                "dial_brightness",
                requested.dial_brightness,
                self.sent.dial_brightness,
            ),
            vibration: self.field("vibration", requested.vibration, self.sent.vibration),
        }
    }
    /// Only explicit configured values write. Several revisions before this poll collapse to the latest.
    /// Called by the HID owner between complete image transfers, never under the Core lock.
    fn send_changed(
        &mut self,
        requested: &DeviceHardwareSettings,
        mut write: impl FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<bool, String> {
        let mut changed = false;
        macro_rules! send {
            ($field:ident) => {
                if let Some(value) = requested.$field
                    && self.sent.$field != Some(value)
                {
                    if let Err(error) = write(&deckweave_n4::packets::$field(value)) {
                        self.failed = Some((stringify!($field), error.clone()));
                        return Err(error);
                    }
                    self.sent.$field = Some(value);
                    self.failed = None;
                    changed = true;
                }
            };
        }
        send!(screen_brightness);
        send!(dial_brightness);
        send!(vibration);
        Ok(changed)
    }
}

#[derive(Serialize)]
struct Trace {
    unix_ms: u128,
    device_id: String,
    generation: u64,
    raw: String,
    decoded: Input,
}

struct Shared {
    core: Arc<Mutex<Core>>,
    app: tauri::AppHandle,
    stop: AtomicBool,
    epoch: Arc<AtomicU64>,
    images: Mutex<Images>,
    hardware_settings: Mutex<BTreeMap<String, HardwareSettingsRuntime>>,
    trace: Mutex<VecDeque<Trace>>,
    output_performance: Mutex<BTreeMap<String, OutputPerformance>>,
    output_performance_revision: AtomicU64,
}

pub struct Hardware {
    shared: Arc<Shared>,
    supervisor: Mutex<Option<JoinHandle<()>>>,
}

impl Hardware {
    pub fn start(core: Arc<Mutex<Core>>, app: tauri::AppHandle, directory: PathBuf) -> Self {
        let shared = Arc::new(Shared {
            core,
            app,
            stop: AtomicBool::new(false),
            epoch: Arc::new(AtomicU64::new(1)),
            images: Mutex::new(BTreeMap::new()),
            hardware_settings: Mutex::new(BTreeMap::new()),
            trace: Mutex::new(VecDeque::new()),
            output_performance: Mutex::new(BTreeMap::new()),
            output_performance_revision: AtomicU64::new(0),
        });
        let worker = Arc::clone(&shared);
        let supervisor = thread::spawn(move || {
            #[cfg(debug_assertions)]
            let virtual_only = std::env::var_os("DECKWEAVE_VIRTUAL_ONLY").is_some()
                || crate::smoke::configuration().is_some();
            #[cfg(not(debug_assertions))]
            let virtual_only = false;
            if !virtual_only {
                supervise(worker, directory);
            }
        });
        Self {
            shared,
            supervisor: Mutex::new(Some(supervisor)),
        }
    }
    pub fn changed(&self) {
        self.shared.epoch.fetch_add(1, Ordering::Relaxed);
    }
    pub fn invalidation(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.shared.epoch)
    }
    pub fn snapshot(&self, snapshot: Snapshot) -> LiveSnapshot {
        let key_images = self
            .shared
            .images
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default();
        let hardware_settings = self
            .shared
            .hardware_settings
            .lock()
            .map(|runtime| {
                snapshot
                    .configuration
                    .devices
                    .iter()
                    .filter(|device| device.provider_id == "deckweave.n4")
                    .map(|device| {
                        (
                            device.id.clone(),
                            runtime
                                .get(&device.id)
                                .cloned()
                                .unwrap_or_default()
                                .status(&device.hardware_settings),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        LiveSnapshot {
            snapshot,
            key_images,
            hardware_settings,
        }
    }
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Ok(mut handle) = self.supervisor.lock()
            && let Some(handle) = handle.take()
        {
            let _ = handle.join();
        }
    }
}

impl Shared {
    fn notify(&self, render: bool) {
        if render {
            self.epoch.fetch_add(1, Ordering::Relaxed);
        }
        let _ = self.app.emit("deckweave:changed", ());
    }
    fn connection(&self, id: &str, generation: u64, state: ConnectionState, error: Option<String>) {
        let changed = self
            .core
            .lock()
            .is_ok_and(|mut c| c.connection_changed(id, generation, state, error));
        if changed {
            self.notify(true);
        }
    }
    fn wait(&self, duration: Duration) {
        let until = Instant::now() + duration;
        while !self.stop.load(Ordering::Relaxed) && Instant::now() < until {
            thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(windows)]
fn supervise(shared: Arc<Shared>, directory: PathBuf) {
    let mut workers: BTreeMap<String, JoinHandle<()>> = BTreeMap::new();
    let mut last_scan = Instant::now() - Duration::from_secs(3);
    let mut last_diagnostic = None;
    let mut last_diagnostic_write = Instant::now();
    while !shared.stop.load(Ordering::Relaxed) {
        if last_scan.elapsed() >= Duration::from_secs(2) {
            last_scan = Instant::now();
            let finished: Vec<_> = workers
                .iter()
                .filter(|(_, worker)| worker.is_finished())
                .map(|(identity, _)| identity.clone())
                .collect();
            for identity in finished {
                if let Some(worker) = workers.remove(&identity) {
                    let _ = worker.join();
                }
            }
            match deckweave_n4::discover() {
                Ok(candidates) => {
                    let mut registration_error = None;
                    for candidate in candidates {
                        if workers.contains_key(&candidate.identity) {
                            continue;
                        }
                        let registered =
                            shared
                                .core
                                .lock()
                                .map_err(|e| e.to_string())
                                .and_then(|mut c| {
                                    if c.is_hardware_removed("deckweave.n4", &candidate.identity) {
                                        return Ok(None);
                                    }
                                    c.register_n4(&candidate.identity)
                                        .map(|id| {
                                            let epoch = c.device_lifecycle_epoch(&id);
                                            Some((id, epoch))
                                        })
                                        .map_err(|e| e.to_string())
                                });
                        match registered {
                            Ok(Some((id, lifecycle_epoch))) => {
                                let identity = candidate.identity.clone();
                                let context = Arc::clone(&shared);
                                workers.insert(
                                    identity,
                                    thread::spawn(move || {
                                        device_worker(
                                            context,
                                            id,
                                            candidate.identity,
                                            lifecycle_epoch,
                                        )
                                    }),
                                );
                                shared.notify(true);
                            }
                            Ok(None) => {}
                            Err(e) => registration_error = Some(e),
                        }
                    }
                    if shared
                        .core
                        .lock()
                        .is_ok_and(|mut c| c.set_provider_error(registration_error))
                    {
                        shared.notify(false);
                    }
                }
                Err(e) => {
                    if shared
                        .core
                        .lock()
                        .is_ok_and(|mut c| c.set_provider_error(Some(e)))
                    {
                        shared.notify(false);
                    }
                }
            }
        }
        // Bounded diagnostic data, no image blobs, no serial numbers or USB paths.
        let performance_revision = shared.output_performance_revision.load(Ordering::Relaxed);
        if !workers.is_empty()
            && let Ok(mut snapshot) = shared.core.lock().map(|c| c.snapshot())
            && diagnostic_due(
                last_diagnostic,
                snapshot.event_sequence,
                performance_revision,
                last_diagnostic_write.elapsed(),
            )
        {
            let observed = (snapshot.event_sequence, performance_revision);
            for feedback in &mut snapshot.feedback {
                feedback.image = None;
            }
            let trace = shared.trace.lock().ok();
            let mut diagnostic = serde_json::json!({
                "snapshot": snapshot,
                "recent_inputs": trace.as_deref(),
                "output_performance": shared.output_performance.lock().ok().as_deref(),
                "note": "Output frames and completion timings describe successful HID transfers, not display acknowledgements. Timings begin when the hardware worker observes a page change."
            });
            redact_settings(&mut diagnostic);
            drop(trace);
            if let Ok(json) = serde_json::to_vec_pretty(&diagnostic) {
                let temporary = directory.join("n4-diagnostics.tmp");
                if std::fs::write(&temporary, json).is_ok()
                    && std::fs::rename(temporary, directory.join("n4-diagnostics.json")).is_ok()
                {
                    // A concurrent metrics publication after this observation remains dirty.
                    // Failed writes are retried rather than consuming the diagnostic revision.
                    last_diagnostic = Some(observed);
                    last_diagnostic_write = Instant::now();
                }
            }
        }
        shared.wait(Duration::from_secs(1));
    }
    for (_, worker) in workers {
        let _ = worker.join();
    }
}

#[cfg(any(windows, test))]
fn diagnostic_due(
    last: Option<(u64, u64)>,
    event_sequence: u64,
    performance_revision: u64,
    elapsed: Duration,
) -> bool {
    let Some((last_event, last_performance)) = last else {
        return true;
    };
    last_event != event_sequence
        || (last_performance != performance_revision && elapsed >= Duration::from_secs(5))
}

#[cfg(any(windows, test))]
fn redact_settings(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                if key == "settings" {
                    *value = serde_json::json!({"redacted":true});
                } else {
                    redact_settings(value);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                redact_settings(value);
            }
        }
        _ => {}
    }
}

#[cfg(not(windows))]
fn supervise(_: Arc<Shared>, _: PathBuf) {}

#[cfg(windows)]
fn device_worker(shared: Arc<Shared>, id: String, identity: String, lifecycle_epoch: u64) {
    let renderer = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            shared.connection(&id, 1, ConnectionState::Error, Some(e));
            return;
        }
    };
    let mut generation = 0;
    while !shared.stop.load(Ordering::Relaxed) {
        if !device_present(&shared, &id, lifecycle_epoch) {
            break;
        }
        match deckweave_n4::Connection::open(&identity) {
            Ok(connection) => {
                generation += 1;
                shared.connection(&id, generation, ConnectionState::Connected, None);
                let result = session(
                    &shared,
                    &id,
                    generation,
                    lifecycle_epoch,
                    &connection,
                    &renderer,
                );
                if let Ok(mut settings) = shared.hardware_settings.lock()
                    && let Some(settings) = settings.get_mut(&id)
                {
                    settings.connected = false;
                }
                shared.connection(&id, generation, ConnectionState::Offline, result.err());
                shared.notify(false);
            }
            Err(deckweave_n4::OpenError::Offline) => {
                shared.connection(&id, generation, ConnectionState::Offline, None)
            }
            Err(deckweave_n4::OpenError::Failed(e)) => {
                shared.connection(&id, generation, ConnectionState::Error, Some(e))
            }
        }
        shared.wait(Duration::from_secs(1));
    }
    if let Ok(mut images) = shared.images.lock() {
        images.remove(&id);
    }
    if let Ok(mut settings) = shared.hardware_settings.lock() {
        settings.remove(&id);
    }
    shared.notify(false);
}

#[cfg(windows)]
fn device_present(shared: &Shared, id: &str, lifecycle_epoch: u64) -> bool {
    shared
        .core
        .lock()
        .is_ok_and(|core| core.has_device(id) && core.device_lifecycle_epoch(id) == lifecycle_epoch)
}

#[derive(Clone, PartialEq, Eq)]
struct Desired {
    key: u8,
    page_id: String,
    binding_id: Option<String>,
    visual: KeyVisual,
    regions: Vec<RegionVisual>,
    image: Option<String>,
    alert: bool,
}

impl Desired {
    // Page/binding identity describes the preview, not the physical pixels. In particular,
    // empty keys and pinned actions need not be transferred again on every page change.
    fn same_pixels(&self, other: &Self) -> bool {
        if self.alert != other.alert {
            return false;
        }
        if self.key == 11 || other.key == 11 {
            return self.key == other.key && self.regions == other.regions;
        }
        match (&self.image, &other.image) {
            (Some(a), Some(b)) => a == b && self.visual == other.visual,
            (None, None) => self.visual == other.visual,
            _ => false,
        }
    }
    fn source_bytes(&self) -> usize {
        self.image.as_ref().map_or(0, String::len)
            + self
                .regions
                .iter()
                .map(|r| r.image.as_ref().map_or(0, String::len))
                .sum::<usize>()
            + self.visual.title.len()
            + self.visual.value.len()
            + self
                .regions
                .iter()
                .map(|r| r.visual.title.len() + r.visual.value.len() + r.detail.len())
                .sum::<usize>()
    }
}

struct PreparedFrame {
    source: Desired,
    jpeg: Vec<u8>,
    data_url: String,
}
impl PreparedFrame {
    fn bytes(&self) -> usize {
        self.source.source_bytes() + self.jpeg.len() + self.data_url.len()
    }
}

#[derive(Default)]
struct FrameCache {
    frames: VecDeque<Arc<PreparedFrame>>,
    bytes: usize,
}
impl FrameCache {
    const LIMIT: usize = 8 * 1024 * 1024;
    const ENTRIES: usize = 64;
    fn get(&mut self, desired: &Desired) -> Option<Arc<PreparedFrame>> {
        let index = self
            .frames
            .iter()
            .position(|p| p.source.same_pixels(desired))?;
        let frame = self.frames.remove(index)?;
        self.frames.push_back(Arc::clone(&frame));
        Some(frame)
    }
    fn insert(&mut self, frame: PreparedFrame) -> Arc<PreparedFrame> {
        let frame = Arc::new(frame);
        let bytes = frame.bytes();
        if bytes <= Self::LIMIT {
            while self.bytes + bytes > Self::LIMIT || self.frames.len() >= Self::ENTRIES {
                if let Some(old) = self.frames.pop_front() {
                    self.bytes -= old.bytes();
                }
            }
            self.bytes += bytes;
            self.frames.push_back(Arc::clone(&frame));
        }
        frame
    }
    fn prepare(
        &mut self,
        frame: &Desired,
        renderer: &Renderer,
    ) -> Result<(Arc<PreparedFrame>, bool), String> {
        if let Some(prepared) = self.get(frame) {
            return Ok((prepared, true));
        }
        let jpeg = if let Some(image) = &frame.image {
            renderer
                .image_data_jpeg_with_alert(image, frame.alert)
                .or_else(|_| renderer.jpeg_with_alert(&frame.visual, frame.alert))?
        } else if frame.key == 11 {
            renderer.strip(&frame.regions)?
        } else {
            renderer.jpeg_with_alert(&frame.visual, frame.alert)?
        };
        let data_url = format!("data:image/jpeg;base64,{}", STANDARD.encode(&jpeg));
        Ok((
            self.insert(PreparedFrame {
                source: frame.clone(),
                jpeg,
                data_url,
            }),
            false,
        ))
    }
}

#[derive(Clone, Default, Serialize)]
struct Timing {
    calls: u64,
    total_us: u64,
    max_us: u64,
}
impl Timing {
    fn record(&mut self, started: Instant) {
        let elapsed = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        self.calls += 1;
        self.total_us = self.total_us.saturating_add(elapsed);
        self.max_us = self.max_us.max(elapsed);
    }
}

#[derive(Clone, Default, Serialize)]
struct OutputPerformance {
    pending_input_reads: Timing,
    idle_input_reads: Timing,
    snapshots: Timing,
    render_and_encode: Timing,
    hid_transfers: Timing,
    cache_hits: u64,
    cache_entries: usize,
    cache_bytes: usize,
    preview_notifications: u64,
    page_batches: VecDeque<PageBatch>,
}
#[derive(Clone, Serialize)]
struct PageBatch {
    page_id: String,
    elapsed_us: u64,
    frames: u64,
    render_us: u64,
    hid_us: u64,
    input_wait_us: u64,
    superseded: bool,
}
struct PendingPage {
    page_id: String,
    started: Instant,
    frames: u64,
    render_us: u64,
    hid_us: u64,
    input_wait_us: u64,
}
impl PendingPage {
    fn complete(self, performance: &mut OutputPerformance, superseded: bool) {
        if performance.page_batches.len() == 32 {
            performance.page_batches.pop_front();
        }
        performance.page_batches.push_back(PageBatch {
            page_id: self.page_id,
            elapsed_us: self.started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
            frames: self.frames,
            render_us: self.render_us,
            hid_us: self.hid_us,
            input_wait_us: self.input_wait_us,
            superseded,
        });
    }
}

fn input_timeout_ms(pending_output: bool) -> u8 {
    if pending_output { 0 } else { 25 }
}

fn navigation_image(
    binding: &deckweave_core::Binding,
    profile: &deckweave_core::Profile,
) -> Option<String> {
    use deckweave_core::Plan;
    let icon = match binding.plan.as_ref()? {
        Plan::Page { page_id } if profile.pages.first().is_some_and(|p| &p.id == page_id) => {
            r#"<path d="m3 11 9-8 9 8M5 9v12h5v-7h4v7h5V9"/>"#
        }
        Plan::Page { page_id }
            if profile
                .pages
                .iter()
                .any(|p| &p.id == page_id && p.parent.is_some()) =>
        {
            r#"<path d="M3 6h7l2 3h9v12H3zM3 6V3h7l2 3"/>"#
        }
        Plan::Page { .. } => {
            r#"<rect x="3" y="3" width="18" height="18" rx="1"/><path d="M3 9h18M9 9v12m4-7 3 3-3 3"/>"#
        }
        Plan::Profile { .. } => r#"<path d="m3 7 9-4 9 4-9 4zM3 12l9 4 9-4M3 17l9 4 9-4"/>"#,
        Plan::Back => r#"<path d="m11 4-8 8 8 8M3 12h18"/>"#,
        Plan::PageCycle { .. } => r#"<path d="m8 3-5 5 5 5M3 8h14m-1 3 5 5-5 5M7 16h14"/>"#,
        _ => return None,
    };
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    };
    let mut lines = vec![String::new()];
    for word in binding.label.split_whitespace() {
        if !lines.last()?.is_empty()
            && lines.last()?.chars().count() + word.chars().count() + 1 > 12
            && lines.len() < 2
        {
            lines.push(String::new());
        }
        let line = lines.last_mut()?;
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    let captions: String=lines.iter().enumerate().map(|(i,line)|{
        let label=if line.chars().count()>13 {format!("{}…",line.chars().take(12).collect::<String>())}else{line.clone()};
        format!(r##"<text x="56" y="{}" fill="#f0f0f0" text-anchor="middle" font-family="IBM Plex Sans" font-size="15" font-weight="600">{}</text>"##,if lines.len()==1{91}else{83+i*18},escape(&label))
    }).collect();
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="112" height="112"><rect width="112" height="112" fill="#20201f"/><svg x="37" y="17" width="38" height="38" viewBox="0 0 24 24" fill="none" stroke="#e8b466" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">{icon}</svg>{captions}</svg>"##
    );
    Some(format!(
        "data:image/svg+xml;base64,{}",
        STANDARD.encode(svg)
    ))
}

fn desired(snapshot: &Snapshot, id: &str) -> Vec<Desired> {
    let Some(device) = snapshot.configuration.devices.iter().find(|d| d.id == id) else {
        return vec![];
    };
    let Some(page) = device
        .profile
        .pages
        .iter()
        .find(|p| p.id == device.profile.active_page_id)
    else {
        return vec![];
    };
    let mut frames: Vec<_> = (1..=10)
        .map(|key| {
            let control = format!("key-{key}");
            let binding = page.bindings.get(&control);
            let feedback = binding.and_then(|b| {
                snapshot
                    .feedback
                    .iter()
                    .find(|f| f.device_id == id && f.binding_id == b.id)
            });
            Desired {
                alert: feedback.is_some_and(|f| f.error.is_some()),
                image: feedback
                    .and_then(|f| f.image.clone())
                    .or_else(|| binding.and_then(|b| navigation_image(b, &device.profile))),
                regions: vec![],
                key,
                page_id: page.id.clone(),
                binding_id: binding.map(|b| b.id.clone()),
                visual: KeyVisual {
                    title: feedback
                        .and_then(|f| f.title.clone())
                        .filter(|s| !s.is_empty() || !feedback.is_some_and(|f| f.error.is_some()))
                        .or_else(|| binding.map(|b| b.label.clone()))
                        .unwrap_or_default(),
                    value: binding
                        .map(|b| match b.kind {
                            ActionKind::Counter => {
                                feedback.map(|f| f.value).unwrap_or(0).to_string()
                            }
                            ActionKind::Toggle => if feedback.is_some_and(|f| f.active) {
                                "ON"
                            } else {
                                "OFF"
                            }
                            .into(),
                            ActionKind::Plugin | ActionKind::Program => {
                                feedback.and_then(|f| f.display.clone()).unwrap_or_default()
                            }
                        })
                        .unwrap_or_default(),
                    active: feedback.is_some_and(|f| f.active),
                    pressed: snapshot
                        .pressed
                        .iter()
                        .any(|p| p.device_id == id && p.control_id == control),
                },
            }
        })
        .collect();
    let geometry = &snapshot.geometries[id];
    let regions = geometry
        .controls
        .iter()
        .filter(|c| c.kind == ControlKind::Touch)
        .map(|c| {
            let (visual, _) = control_visual(snapshot, id, page, &c.id);
            let dial = format!("dial-{}", c.number);
            let (dial_visual, has_dial) = control_visual(snapshot, id, page, &dial);
            RegionVisual {
                alert: page.bindings.get(&c.id).is_some_and(|b| {
                    snapshot
                        .feedback
                        .iter()
                        .any(|f| f.device_id == id && f.binding_id == b.id && f.error.is_some())
                }),
                image: page
                    .bindings
                    .get(&c.id)
                    .and_then(|b| {
                        snapshot
                            .feedback
                            .iter()
                            .find(|f| f.device_id == id && f.binding_id == b.id)
                    })
                    .and_then(|f| f.image.clone()),
                rect: crate::touch::rect(geometry, c),
                visual,
                detail: if page.panel.is_none() && has_dial {
                    format!("{}: {}", dial_visual.title, dial_visual.value)
                } else {
                    String::new()
                },
            }
        })
        .collect();
    frames.push(Desired {
        alert: false,
        image: None,
        key: 11,
        page_id: page.id.clone(),
        binding_id: None,
        visual: KeyVisual {
            title: String::new(),
            value: String::new(),
            active: false,
            pressed: false,
        },
        regions,
    });
    frames
}

fn control_visual(
    snapshot: &Snapshot,
    id: &str,
    page: &deckweave_core::Page,
    control: &str,
) -> (KeyVisual, bool) {
    let binding = page.bindings.get(control);
    let feedback = binding.and_then(|b| {
        snapshot
            .feedback
            .iter()
            .find(|f| f.device_id == id && f.binding_id == b.id)
    });
    (
        KeyVisual {
            title: feedback
                .and_then(|f| f.title.clone())
                .or_else(|| binding.map(|b| b.label.clone()))
                .unwrap_or_default(),
            value: binding
                .map(|b| match b.kind {
                    ActionKind::Counter => feedback.map(|f| f.value).unwrap_or(0).to_string(),
                    ActionKind::Toggle => if feedback.is_some_and(|f| f.active) {
                        "ON"
                    } else {
                        "OFF"
                    }
                    .into(),
                    ActionKind::Plugin | ActionKind::Program => {
                        feedback.and_then(|f| f.display.clone()).unwrap_or_default()
                    }
                })
                .unwrap_or_default(),
            active: feedback.is_some_and(|f| f.active),
            pressed: snapshot
                .pressed
                .iter()
                .any(|p| p.device_id == id && p.control_id == control),
        },
        binding.is_some(),
    )
}

fn next_frame<'a>(
    target: &'a [Desired],
    sent: &BTreeMap<u8, Desired>,
    after: u8,
) -> Option<&'a Desired> {
    let mut changed = target.iter().filter(|frame| {
        !sent
            .get(&frame.key)
            .is_some_and(|old| old.same_pixels(frame))
    });
    changed
        .clone()
        .find(|frame| frame.key > after)
        .or_else(|| changed.next())
}

#[cfg(windows)]
fn session(
    shared: &Shared,
    id: &str,
    generation: u64,
    lifecycle_epoch: u64,
    connection: &deckweave_n4::Connection,
    renderer: &Renderer,
) -> Result<(), String> {
    let mut buffer = [0u8; 1024];
    let mut heartbeat = Instant::now();
    let mut epoch = 0;
    let mut target = vec![];
    let mut sent: BTreeMap<u8, Desired> = BTreeMap::new();
    let mut last_key = 0;
    let mut touch = crate::touch::TouchRouter::default();
    let mut cache = FrameCache::default();
    let mut performance = OutputPerformance::default();
    let mut page_batch: Option<PendingPage> = None;
    let mut last_page = String::new();
    let mut preview_dirty = false;
    let mut last_preview = Instant::now() - Duration::from_millis(33);
    let mut last_metrics = Instant::now();
    let mut settings = HardwareSettingsRuntime {
        connected: true,
        ..Default::default()
    };
    if let Ok(mut states) = shared.hardware_settings.lock() {
        states.insert(id.into(), settings.clone());
    }
    let snapshot = shared.core.lock().map_err(|e| e.to_string())?.snapshot();
    let Some(mut geometry) = snapshot.geometries.get(id).cloned() else {
        return Ok(());
    };
    drop(snapshot);
    while !shared.stop.load(Ordering::Relaxed) {
        if !device_present(shared, id, lifecycle_epoch) {
            return Ok(());
        }
        let pending = shared.epoch.load(Ordering::Relaxed) != epoch
            || next_frame(&target, &sent, last_key).is_some();
        let read_started = Instant::now();
        let count = connection.read_timeout(&mut buffer, input_timeout_ms(pending))?;
        if !device_present(shared, id, lifecycle_epoch) {
            return Ok(());
        }
        if pending {
            performance.pending_input_reads.record(read_started);
        } else {
            performance.idle_input_reads.record(read_started);
        }
        if let Some(batch) = &mut page_batch {
            batch.input_wait_us += read_started.elapsed().as_micros() as u64;
        }
        if count > 0
            && let Some(decoded) = deckweave_n4::decode(&buffer[..count])
        {
            // A layout can change while an input read is in progress. Resolve a newly
            // arrived touch against the current layout, never a previously cached page.
            if matches!(
                decoded,
                Input::Touch { .. } | Input::Pointer { .. } | Input::Swipe { .. }
            ) && shared.epoch.load(Ordering::Relaxed) != epoch
            {
                let started = Instant::now();
                let snapshot = shared.core.lock().map_err(|e| e.to_string())?.snapshot();
                let Some(current_geometry) = snapshot.geometries.get(id) else {
                    return Ok(());
                };
                geometry = current_geometry.clone();
                performance.snapshots.record(started);
            }
            let (control, event) = {
                if let Some((control, event)) = touch.route(&decoded, &geometry, Instant::now()) {
                    (Some(control), Some(event))
                } else {
                    translate(&decoded)
                }
            };
            let description = serde_json::to_string(&decoded).map_err(|e| e.to_string())?;
            let changed = shared
                .core
                .lock()
                .map_err(|e| e.to_string())?
                .hardware_input(id, generation, control.as_deref(), event, description)
                .map_err(|e| e.to_string())?;
            if let Ok(mut trace) = shared.trace.lock() {
                if trace.len() == 256 {
                    trace.pop_front();
                }
                trace.push_back(Trace {
                    unix_ms: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis(),
                    device_id: id.into(),
                    generation,
                    raw: buffer[..count.min(32)]
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                    decoded,
                });
            }
            if changed {
                shared.notify(control.is_some());
            }
        }
        if let Some((control, event)) = touch.expire(Instant::now()) {
            shared
                .core
                .lock()
                .map_err(|e| e.to_string())?
                .hardware_input(
                    id,
                    generation,
                    Some(&control),
                    Some(event),
                    "Touch contact timeout: cancelled".into(),
                )
                .map_err(|e| e.to_string())?;
            shared.notify(true);
        }
        let current = shared.epoch.load(Ordering::Relaxed);
        if current != epoch {
            let started = Instant::now();
            let snapshot = shared.core.lock().map_err(|e| e.to_string())?.snapshot();
            performance.snapshots.record(started);
            let Some(current_geometry) = snapshot.geometries.get(id) else {
                return Ok(());
            };
            geometry = current_geometry.clone();
            let Some(device) = snapshot
                .configuration
                .devices
                .iter()
                .find(|device| device.id == id)
            else {
                return Ok(());
            };
            let settings_result =
                settings.send_changed(&device.hardware_settings, |report| connection.write(report));
            if let Ok(mut states) = shared.hardware_settings.lock() {
                states.insert(id.into(), settings.clone());
            }
            match settings_result {
                Ok(true) => shared.notify(false),
                Ok(false) => {}
                Err(error) => {
                    shared.notify(false);
                    return Err(error);
                }
            }
            target = desired(&snapshot, id);
            epoch = current;
            if let Some(frame) = target.first()
                && frame.page_id != last_page
            {
                if let Some(batch) = page_batch.take() {
                    batch.complete(&mut performance, true);
                }
                last_page = frame.page_id.clone();
                page_batch = Some(PendingPage {
                    page_id: frame.page_id.clone(),
                    started,
                    frames: 0,
                    render_us: 0,
                    hid_us: 0,
                    input_wait_us: 0,
                });
            }
            // Metadata follows the selected page even when identical pixels need no USB write.
            if let Ok(mut images) = shared.images.lock()
                && let Some(device_images) = images.get_mut(id)
            {
                for frame in &target {
                    if sent
                        .get(&frame.key)
                        .is_some_and(|old| old.same_pixels(frame))
                        && let Some(image) = device_images.get_mut(&frame_control(frame.key))
                        && (image.page_id != frame.page_id || image.binding_id != frame.binding_id)
                    {
                        image.page_id.clone_from(&frame.page_id);
                        image.binding_id.clone_from(&frame.binding_id);
                        sent.insert(frame.key, frame.clone());
                        preview_dirty = true;
                    }
                }
            }
        }
        // Complete one indivisible BAT/chunks/STP transaction between input reads.
        if let Some(frame) = next_frame(&target, &sent, last_key) {
            let render_started = Instant::now();
            let (prepared, hit) = cache.prepare(frame, renderer)?;
            performance.render_and_encode.record(render_started);
            performance.cache_hits += u64::from(hit);
            let render_us = render_started.elapsed().as_micros() as u64;
            let write_started = Instant::now();
            if frame.key == 11 {
                connection.present_strip(&prepared.jpeg)?;
            } else {
                connection.present(frame.key, &prepared.jpeg)?;
            }
            performance.hid_transfers.record(write_started);
            if let Some(batch) = &mut page_batch {
                batch.frames += 1;
                batch.render_us += render_us;
                batch.hid_us += write_started.elapsed().as_micros() as u64;
            }
            let image = Frame {
                page_id: frame.page_id.clone(),
                binding_id: frame.binding_id.clone(),
                data_url: prepared.data_url.clone(),
            };
            if let Ok(mut images) = shared.images.lock() {
                images
                    .entry(id.into())
                    .or_default()
                    .insert(frame_control(frame.key), image);
            }
            sent.insert(frame.key, frame.clone());
            last_key = frame.key;
            if shared
                .core
                .lock()
                .map_err(|e| e.to_string())?
                .hardware_frame_sent(id, generation)
            {
                preview_dirty = true;
            }
        }
        let settled = next_frame(&target, &sent, last_key).is_none();
        let completed_page = settled && page_batch.is_some();
        if settled && let Some(batch) = page_batch.take() {
            batch.complete(&mut performance, false);
        }
        if preview_dirty && (settled || last_preview.elapsed() >= Duration::from_millis(33)) {
            shared.notify(false);
            performance.preview_notifications += 1;
            last_preview = Instant::now();
            preview_dirty = false;
        }
        if completed_page || last_metrics.elapsed() >= Duration::from_secs(1) {
            performance.cache_entries = cache.frames.len();
            performance.cache_bytes = cache.bytes;
            if let Ok(mut metrics) = shared.output_performance.lock() {
                metrics.insert(id.into(), performance.clone());
                shared
                    .output_performance_revision
                    .fetch_add(1, Ordering::Relaxed);
            }
            last_metrics = Instant::now();
        }
        if heartbeat.elapsed() >= Duration::from_millis(deckweave_n4::packets::HEARTBEAT_MS) {
            connection.heartbeat()?;
            heartbeat = Instant::now();
        }
    }
    Ok(())
}

fn frame_control(key: u8) -> String {
    if key == 11 {
        "touch-strip".into()
    } else {
        format!("key-{key}")
    }
}

fn translate(input: &Input) -> (Option<String>, Option<InputEvent>) {
    let edge = |pressed: bool| {
        if pressed {
            InputEvent::Down
        } else {
            InputEvent::Up
        }
    };
    match input {
        Input::Key { key, pressed } => (Some(format!("key-{key}")), Some(edge(*pressed))),
        Input::Rotate { dial, ticks } => (
            Some(format!("dial-{dial}")),
            Some(InputEvent::Rotate { ticks: *ticks }),
        ),
        Input::DialClick { dial } => (Some(format!("dial-{dial}")), Some(InputEvent::Click)),
        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn plugin_errors_keep_key_and_panel_artwork_and_success_restores_unmarked_pixels() {
        use super::*;
        let mut core = Core::memory().unwrap();
        let device = core.snapshot().configuration.devices[0].clone();
        for (control, controller) in [
            ("key-1", deckweave_core::Controller::Keypad),
            ("touch-1", deckweave_core::Controller::Panel),
        ] {
            core.dispatch(
                deckweave_core::Command::ConfigurePlugin {
                    device_id: device.id.clone(),
                    page_id: device.profile.active_page_id.clone(),
                    control_id: control.into(),
                    plugin: deckweave_core::PluginAction {
                        package: "dev.test.error".into(),
                        action: "dev.test.error.action".into(),
                        settings: serde_json::json!({}),
                        controller: Some(controller),
                    },
                    label: "Application".into(),
                    expected_revision: core.revision(),
                },
                "test",
            )
            .unwrap();
        }
        let page = core.snapshot().configuration.devices[0].profile.pages[0].clone();
        let image=format!("data:image/svg+xml;base64,{}",STANDARD.encode(r##"<svg xmlns="http://www.w3.org/2000/svg" width="112" height="112"><rect width="112" height="112" fill="#237345"/><text x="12" y="65" font-size="20" fill="white">Visible</text></svg>"##));
        for binding in page
            .bindings
            .values()
            .filter(|b| b.kind == ActionKind::Plugin)
        {
            core.plugin_feedback(&binding.id, "setImage", serde_json::json!({"image":image}));
            core.plugin_feedback(&binding.id, "setTitle", serde_json::json!({"title":""}));
        }
        let renderer = Renderer::new().unwrap();
        let mut cache = FrameCache::default();
        let initial = desired(&core.snapshot(), &device.id);
        let key = initial.iter().find(|f| f.key == 1).unwrap();
        let strip = initial.iter().find(|f| f.key == 11).unwrap();
        let original_key = cache.prepare(key, &renderer).unwrap().0;
        let original_strip = cache.prepare(strip, &renderer).unwrap().0;
        for binding in page
            .bindings
            .values()
            .filter(|b| b.kind == ActionKind::Plugin)
        {
            core.action_error(&device.id, &binding.id, "Target window unavailable");
        }
        let failed = desired(&core.snapshot(), &device.id);
        let key = failed.iter().find(|f| f.key == 1).unwrap();
        let strip = failed.iter().find(|f| f.key == 11).unwrap();
        assert_eq!(key.image.as_ref(), Some(&image));
        assert!(key.alert);
        assert_eq!(strip.regions[0].image.as_ref(), Some(&image));
        assert!(strip.regions[0].alert);
        assert_eq!(key.visual.title, "Application");
        let failed_key = cache.prepare(key, &renderer).unwrap().0;
        let failed_strip = cache.prepare(strip, &renderer).unwrap().0;
        assert_ne!(failed_key.jpeg, original_key.jpeg);
        assert_ne!(failed_strip.jpeg, original_strip.jpeg);
        for binding in page
            .bindings
            .values()
            .filter(|b| b.kind == ActionKind::Plugin)
        {
            core.plugin_feedback(&binding.id, "ready", serde_json::json!({}));
        }
        let restored = desired(&core.snapshot(), &device.id);
        assert_eq!(
            cache
                .prepare(restored.iter().find(|f| f.key == 1).unwrap(), &renderer)
                .unwrap()
                .0
                .jpeg,
            original_key.jpeg
        );
        assert_eq!(
            cache
                .prepare(restored.iter().find(|f| f.key == 11).unwrap(), &renderer)
                .unwrap()
                .0
                .jpeg,
            original_strip.jpeg
        );
    }

    #[test]
    fn native_navigation_has_icons_readable_labels_and_preserves_feedback_priority() {
        use super::*;
        let mut core = Core::memory().unwrap();
        let device = core.snapshot().configuration.devices[0].clone();
        core.dispatch(
            deckweave_core::Command::ConfigureProgram {
                device_id: device.id.clone(),
                page_id: device.profile.active_page_id.clone(),
                control_id: "key-1".into(),
                label: "Codex <команды>".into(),
                plan: deckweave_core::Plan::Page {
                    page_id: device.profile.active_page_id.clone(),
                },
                expected_revision: 0,
            },
            "test",
        )
        .unwrap();
        let frame = desired(&core.snapshot(), &device.id).remove(0);
        let uri = frame.image.as_ref().unwrap();
        let svg =
            String::from_utf8(STANDARD.decode(uri.split_once(',').unwrap().1).unwrap()).unwrap();
        assert!(svg.contains("font-size=\"15\""));
        assert!(svg.contains("&lt;команды&gt;"));
        assert!(svg.contains("<path"));
        let renderer = Renderer::new().unwrap();
        let mut cache = FrameCache::default();
        let (prepared, _) = cache.prepare(&frame, &renderer).unwrap();
        assert!(!prepared.jpeg.is_empty());
        core.action_error(
            &device.id,
            frame.binding_id.as_deref().unwrap(),
            "Unavailable target",
        );
        let failed = desired(&core.snapshot(), &device.id).remove(0);
        assert_eq!(failed.image, frame.image);
        assert!(failed.alert);
        let (alert_frame, hit) = cache.prepare(&failed, &renderer).unwrap();
        assert!(!hit);
        assert_ne!(alert_frame.jpeg, prepared.jpeg);
    }

    #[test]
    fn unconfigured_hardware_never_writes_and_changed_fields_do_not_repeat() {
        use super::*;
        let mut runtime = HardwareSettingsRuntime {
            connected: true,
            ..Default::default()
        };
        assert!(
            !runtime
                .send_changed(&DeviceHardwareSettings::default(), |_| panic!(
                    "opening settings must not write"
                ))
                .unwrap()
        );
        assert_eq!(
            runtime
                .status(&DeviceHardwareSettings::default())
                .screen_brightness
                .state,
            "unchanged"
        );
        let wanted = DeviceHardwareSettings {
            screen_brightness: Some(80),
            dial_brightness: Some(25),
            vibration: Some(false),
        };
        let mut reports = vec![];
        assert!(
            runtime
                .send_changed(&wanted, |report| {
                    reports.push(report.to_vec());
                    Ok(())
                })
                .unwrap()
        );
        assert_eq!(reports.len(), 3);
        assert_eq!(&reports[0][6..9], b"LIG");
        assert_eq!(&reports[1][6..11], b"LBLIG");
        assert_eq!(&reports[2][6..11], b"QUCMD");
        assert!(
            !runtime
                .send_changed(&wanted, |_| panic!("unchanged values must not repeat"))
                .unwrap()
        );
        let status = runtime.status(&wanted);
        assert_eq!(status.screen_brightness.state, "sent_no_readback");
        assert_eq!(status.vibration.sent, Some(false));
        let latest = DeviceHardwareSettings {
            screen_brightness: Some(95),
            ..wanted.clone()
        };
        let mut writes = 0;
        runtime
            .send_changed(&latest, |report| {
                writes += 1;
                assert_eq!(&report[6..9], b"LIG");
                Ok(())
            })
            .unwrap();
        assert_eq!(writes, 1);
    }
    #[test]
    fn partial_hardware_failure_keeps_requested_values_and_does_not_claim_readback() {
        use super::*;
        let wanted = DeviceHardwareSettings {
            screen_brightness: Some(60),
            dial_brightness: Some(20),
            vibration: Some(true),
        };
        let mut runtime = HardwareSettingsRuntime {
            connected: true,
            ..Default::default()
        };
        let mut writes = 0;
        assert!(
            runtime
                .send_changed(&wanted, |_| {
                    writes += 1;
                    if writes == 2 {
                        Err("unplugged".into())
                    } else {
                        Ok(())
                    }
                })
                .is_err()
        );
        assert_eq!(writes, 2, "abort before writing unrelated remaining fields");
        let status = runtime.status(&wanted);
        assert_eq!(status.screen_brightness.state, "sent_no_readback");
        assert_eq!(status.dial_brightness.state, "error");
        assert_eq!(status.dial_brightness.sent, None);
        assert_eq!(status.dial_brightness.requested, Some(20));
        assert_eq!(status.vibration.state, "pending");
        runtime.connected = false;
        assert_eq!(runtime.status(&wanted).screen_brightness.state, "offline");
        let reconnected = HardwareSettingsRuntime {
            connected: true,
            ..Default::default()
        };
        assert_eq!(
            reconnected.status(&wanted).screen_brightness.state,
            "pending",
            "old successful transfer is not evidence for a new session"
        );
    }
    #[test]
    fn diagnostics_publish_late_metrics_without_another_core_event_or_hot_idle_writes() {
        use std::time::Duration;
        let before_metrics_publication = Some((7, 2));
        // A page completes just after the supervisor persisted its last frame event.
        // A later metrics revision must eventually be written despite unchanged Core sequence.
        assert!(!super::diagnostic_due(
            before_metrics_publication,
            7,
            3,
            Duration::from_secs(1)
        ));
        assert!(super::diagnostic_due(
            before_metrics_publication,
            7,
            3,
            Duration::from_secs(5)
        ));
        // Continued idle polling does not write unchanged metrics or bypass throttling.
        assert!(!super::diagnostic_due(
            Some((7, 3)),
            7,
            3,
            Duration::from_secs(60)
        ));
        assert!(!super::diagnostic_due(
            Some((7, 3)),
            7,
            4,
            Duration::from_secs(4)
        ));
        // Actual input/configuration events retain the existing next-supervisor-tick behavior.
        assert!(super::diagnostic_due(Some((7, 3)), 8, 3, Duration::ZERO));
    }
    #[test]
    fn diagnostics_redact_nested_plugin_settings() {
        let mut value = serde_json::json!({"configuration":{"devices":[{"settings":{"password":"private"},"children":[{"settings":{"token":"private"}}]}]},"events":3});
        super::redact_settings(&mut value);
        assert!(!value.to_string().contains("private"));
        assert_eq!(value["events"], 3);
        assert_eq!(
            value["configuration"]["devices"][0]["settings"],
            serde_json::json!({"redacted":true})
        );
    }
    use super::*;
    #[test]
    fn captured_touch_edges_complete_one_action_using_the_display_layout() {
        let capture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../crates/deckweave-n4/tests/fixtures/owner-1021.json"
        ))
        .unwrap();
        let mut core = Core::memory().unwrap();
        let id = core
            .register_n4(&format!("n4pro-{}", "a".repeat(64)))
            .unwrap();
        core.connection_changed(&id, 1, ConnectionState::Connected, None);
        let mut router = crate::touch::TouchRouter::default();
        for case in capture["touch"].as_array().unwrap() {
            let bytes: Vec<u8> = case["raw"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .map(|b| u8::from_str_radix(b, 16).unwrap())
                .collect();
            let input = deckweave_n4::decode(&bytes).unwrap();
            let snapshot = core.snapshot();
            let (control, event) = router
                .route(&input, &snapshot.geometries[&id], Instant::now())
                .unwrap();
            core.hardware_input(
                &id,
                1,
                Some(&control),
                Some(event),
                "captured contact".into(),
            )
            .unwrap();
        }
        let snapshot = core.snapshot();
        let frames = desired(&snapshot, &id);
        let strip = frames.iter().find(|f| f.key == 11).unwrap();
        assert_eq!(snapshot.feedback.len(), 1);
        assert!(snapshot.feedback[0].active);
        assert!(snapshot.pressed.is_empty());
        assert!(strip.regions[0].visual.active);
        assert!(!strip.regions[1].visual.active);
        assert_eq!(strip.regions[0].detail, "D1: 0");
    }
    #[test]
    fn captured_physical_keys_execute_key_actions_and_leave_touch_unchanged() {
        let capture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../crates/deckweave-n4/tests/fixtures/owner-1021.json"
        ))
        .unwrap();
        let mut core = Core::memory().unwrap();
        let id = core
            .register_n4(&format!("n4pro-{}", "a".repeat(64)))
            .unwrap();
        core.connection_changed(&id, 1, ConnectionState::Connected, None);
        for case in capture["keys"].as_array().unwrap() {
            let bytes: Vec<u8> = case["raw"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .map(|b| u8::from_str_radix(b, 16).unwrap())
                .collect();
            let input = deckweave_n4::decode(&bytes).unwrap();
            let (control, event) = translate(&input);
            core.hardware_input(
                &id,
                1,
                control.as_deref(),
                event,
                "captured physical key".into(),
            )
            .unwrap();
        }
        let snapshot = core.snapshot();
        let device = snapshot
            .configuration
            .devices
            .iter()
            .find(|d| d.id == id)
            .unwrap();
        let page = &device.profile.pages[0];
        assert_eq!(snapshot.feedback.len(), 10);
        for key in 1..=10 {
            let binding = &page.bindings[&format!("key-{key}")];
            let feedback = snapshot
                .feedback
                .iter()
                .find(|f| f.binding_id == binding.id)
                .unwrap();
            if key == 2 {
                assert!(feedback.active);
            } else {
                assert_eq!(feedback.value, 1);
            }
        }
        assert!(snapshot.pressed.is_empty());
    }
    #[test]
    fn pointer_and_swipe_are_observations_until_calibrated() {
        assert!(translate(&Input::Pointer { x: 400, y: 55 }).1.is_none());
        assert!(translate(&Input::Swipe { right: true }).1.is_none());
        assert!(matches!(
            translate(&Input::DialClick { dial: 2 }).1,
            Some(InputEvent::Click)
        ));
    }
    #[test]
    fn output_target_follows_active_page_and_binding_identity() {
        let mut core = Core::memory().unwrap();
        let id = core
            .register_n4(&format!("n4pro-{}", "a".repeat(64)))
            .unwrap();
        let before = desired(&core.snapshot(), &id);
        core.dispatch(
            deckweave_core::Command::AddPage {
                device_id: id.clone(),
                expected_revision: core.snapshot().configuration.revision,
            },
            "test",
        )
        .unwrap();
        let after = desired(&core.snapshot(), &id);
        assert_eq!(after.len(), 11);
        assert!(
            after
                .iter()
                .all(|f| f.binding_id.is_none() && f.visual.title.is_empty())
        );
        assert_ne!(before[0].page_id, after[0].page_id);
    }
    #[test]
    fn busy_first_key_cannot_starve_other_changed_keys() {
        let mut core = Core::memory().unwrap();
        let id = core
            .register_n4(&format!("n4pro-{}", "a".repeat(64)))
            .unwrap();
        let frames = desired(&core.snapshot(), &id);
        let mut sent = BTreeMap::new();
        assert_eq!(next_frame(&frames, &sent, 0).unwrap().key, 1);
        let mut outdated = frames[0].clone();
        outdated.visual.value = "old".into();
        sent.insert(1, outdated);
        assert_eq!(next_frame(&frames, &sent, 1).unwrap().key, 2);
        assert_eq!(next_frame(&frames, &sent, 11).unwrap().key, 1);
    }
    #[test]
    fn pending_output_polls_input_between_each_frame_without_timeout_waits() {
        let mut core = Core::memory().unwrap();
        let id = core
            .register_n4(&format!("n4pro-{}", "a".repeat(64)))
            .unwrap();
        let frames = desired(&core.snapshot(), &id);
        let mut sent = BTreeMap::new();
        let mut after = 0;
        let mut input_polls = 0;
        while let Some(frame) = next_frame(&frames, &sent, after) {
            // Even an empty input queue must not add 25 ms to every output frame.
            assert_eq!(input_timeout_ms(true), 0);
            input_polls += 1;
            sent.insert(frame.key, frame.clone());
            after = frame.key;
        }
        assert_eq!(input_polls, 11);
        assert_eq!(
            input_timeout_ms(false),
            25,
            "idle workers must not busy-spin"
        );
    }
    #[test]
    fn identical_pixels_do_not_retransfer_when_page_and_binding_identity_change() {
        let mut core = Core::memory().unwrap();
        let id = core
            .register_n4(&format!("n4pro-{}", "a".repeat(64)))
            .unwrap();
        let before = desired(&core.snapshot(), &id);
        let sent = before.iter().map(|f| (f.key, f.clone())).collect();
        let mut after = before.clone();
        for frame in &mut after {
            frame.page_id = "another-page".into();
            frame.binding_id = Some("another-binding".into());
        }
        assert!(next_frame(&after, &sent, 0).is_none());
        after[4].visual.value = "changed".into();
        assert_eq!(next_frame(&after, &sent, 0).unwrap().key, 5);
        after[10].regions[0].visual.value = "changed".into();
        assert_eq!(next_frame(&after, &sent, 5).unwrap().key, 11);
    }
    #[test]
    fn prepared_cache_reuses_cross_page_pixels_and_keeps_memory_and_entry_bounds() {
        let mut core = Core::memory().unwrap();
        let id = core
            .register_n4(&format!("n4pro-{}", "a".repeat(64)))
            .unwrap();
        let renderer = Renderer::new().unwrap();
        let mut frames = desired(&core.snapshot(), &id);
        let mut cache = FrameCache::default();
        let (first, hit) = cache.prepare(&frames[0], &renderer).unwrap();
        assert!(!hit);
        frames[0].page_id = "another-page".into();
        frames[0].binding_id = None;
        let (second, hit) = cache.prepare(&frames[0], &renderer).unwrap();
        assert!(hit);
        assert!(Arc::ptr_eq(&first, &second));
        for n in 0..80 {
            frames[0].visual.value = n.to_string();
            cache.prepare(&frames[0], &renderer).unwrap();
        }
        assert_eq!(cache.frames.len(), FrameCache::ENTRIES);
        assert!(cache.bytes <= FrameCache::LIMIT);
        let oversized = PreparedFrame {
            source: frames[0].clone(),
            jpeg: vec![0; FrameCache::LIMIT + 1],
            data_url: String::new(),
        };
        cache.insert(oversized);
        assert_eq!(cache.frames.len(), FrameCache::ENTRIES);
        assert!(cache.bytes <= FrameCache::LIMIT);
    }
    #[test]
    #[ignore = "manual CPU-only performance sample; does not open hardware"]
    fn output_render_benchmark() {
        let mut core = Core::memory().unwrap();
        let id = core
            .register_n4(&format!("n4pro-{}", "a".repeat(64)))
            .unwrap();
        let frames = desired(&core.snapshot(), &id);
        let renderer = Renderer::new().unwrap();
        let iterations = 20;
        let cold_started = Instant::now();
        for _ in 0..iterations {
            for frame in &frames {
                let mut cold_cache = FrameCache::default();
                cold_cache.prepare(frame, &renderer).unwrap();
            }
        }
        let cold_us = cold_started.elapsed().as_micros();
        let mut warm_cache = FrameCache::default();
        for frame in &frames {
            warm_cache.prepare(frame, &renderer).unwrap();
        }
        let warm_started = Instant::now();
        for _ in 0..iterations {
            for frame in &frames {
                assert!(warm_cache.prepare(frame, &renderer).unwrap().1);
            }
        }
        println!(
            "{{\"scope\":\"cpu-only-no-usb\",\"pages\":{iterations},\"frames_per_page\":{},\"cold_render_us\":{cold_us},\"warm_cache_us\":{},\"cache_bytes\":{},\"previous_idle_read_floor_us_per_page\":275000,\"pending_idle_read_floor_us_per_page\":0}}",
            frames.len(),
            warm_started.elapsed().as_micros(),
            warm_cache.bytes
        );
    }
}
