//! Native workers own their HID handles. Core locks never cover USB, rendering or file I/O.
use base64::{Engine, engine::general_purpose::STANDARD};
use deckweave_core::{ActionKind, ConnectionState, ControlKind, Core, InputEvent, Snapshot};
use deckweave_n4::{Input, KeyVisual, RegionVisual, Renderer};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
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
    trace: Mutex<VecDeque<Trace>>,
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
            trace: Mutex::new(VecDeque::new()),
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
        LiveSnapshot {
            snapshot,
            key_images,
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
    let mut known = BTreeSet::new();
    let mut workers = vec![];
    let mut last_scan = Instant::now() - Duration::from_secs(3);
    let mut last_diagnostic = None;
    while !shared.stop.load(Ordering::Relaxed) {
        if last_scan.elapsed() >= Duration::from_secs(2) {
            last_scan = Instant::now();
            match deckweave_n4::discover() {
                Ok(candidates) => {
                    let mut registration_error = None;
                    for candidate in candidates {
                        if known.contains(&candidate.identity) {
                            continue;
                        }
                        let registered =
                            shared
                                .core
                                .lock()
                                .map_err(|e| e.to_string())
                                .and_then(|mut c| {
                                    c.register_n4(&candidate.identity)
                                        .map_err(|e| e.to_string())
                                });
                        match registered {
                            Ok(id) => {
                                known.insert(candidate.identity.clone());
                                let context = Arc::clone(&shared);
                                workers.push(thread::spawn(move || {
                                    device_worker(context, id, candidate.identity)
                                }));
                                shared.notify(true);
                            }
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
        if !known.is_empty()
            && let Ok(mut snapshot) = shared.core.lock().map(|c| c.snapshot())
            && last_diagnostic != Some(snapshot.event_sequence)
        {
            last_diagnostic = Some(snapshot.event_sequence);
            for feedback in &mut snapshot.feedback {
                feedback.image = None;
            }
            let trace = shared.trace.lock().ok();
            let mut diagnostic = serde_json::json!({
                "snapshot": snapshot,
                "recent_inputs": trace.as_deref(),
                "note": "Output frames count successful HID transfers, not display acknowledgements."
            });
            redact_settings(&mut diagnostic);
            drop(trace);
            if let Ok(json) = serde_json::to_vec_pretty(&diagnostic) {
                let temporary = directory.join("n4-diagnostics.tmp");
                if std::fs::write(&temporary, json).is_ok() {
                    let _ = std::fs::rename(temporary, directory.join("n4-diagnostics.json"));
                }
            }
        }
        shared.wait(Duration::from_secs(1));
    }
    for worker in workers {
        let _ = worker.join();
    }
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
fn device_worker(shared: Arc<Shared>, id: String, identity: String) {
    let renderer = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            shared.connection(&id, 1, ConnectionState::Error, Some(e));
            return;
        }
    };
    let mut generation = 0;
    while !shared.stop.load(Ordering::Relaxed) {
        match deckweave_n4::Connection::open(&identity) {
            Ok(connection) => {
                generation += 1;
                shared.connection(&id, generation, ConnectionState::Connected, None);
                let result = session(&shared, &id, generation, &connection, &renderer);
                shared.connection(&id, generation, ConnectionState::Offline, result.err());
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
}

#[derive(Clone, PartialEq, Eq)]
struct Desired {
    key: u8,
    page_id: String,
    binding_id: Option<String>,
    visual: KeyVisual,
    regions: Vec<RegionVisual>,
    image: Option<String>,
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
                image: feedback
                    .filter(|f| f.error.is_none())
                    .and_then(|f| f.image.clone()),
                regions: vec![],
                key,
                page_id: page.id.clone(),
                binding_id: binding.map(|b| b.id.clone()),
                visual: KeyVisual {
                    title: feedback
                        .and_then(|f| f.title.clone())
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
                image: page
                    .bindings
                    .get(&c.id)
                    .and_then(|b| {
                        snapshot.feedback.iter().find(|f| {
                            f.device_id == id && f.binding_id == b.id && f.error.is_none()
                        })
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
    let mut changed = target
        .iter()
        .filter(|frame| sent.get(&frame.key) != Some(frame));
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
    while !shared.stop.load(Ordering::Relaxed) {
        let count = connection.read(&mut buffer)?;
        if count > 0
            && let Some(decoded) = deckweave_n4::decode(&buffer[..count])
        {
            let (control, event) = {
                let snapshot = shared.core.lock().map_err(|e| e.to_string())?.snapshot();
                if let Some((control, event)) =
                    touch.route(&decoded, &snapshot.geometries[id], Instant::now())
                {
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
            let snapshot = shared.core.lock().map_err(|e| e.to_string())?.snapshot();
            target = desired(&snapshot, id);
            epoch = current;
        }
        // Complete one indivisible BAT/chunks/STP transaction between input reads.
        if let Some(frame) = next_frame(&target, &sent, last_key) {
            let jpeg = if let Some(image) = &frame.image {
                renderer
                    .image_data_jpeg(image)
                    .ok()
                    .map(Ok)
                    .unwrap_or_else(|| renderer.jpeg(&frame.visual))?
            } else if frame.key == 11 {
                renderer.strip(&frame.regions)?
            } else {
                renderer.jpeg(&frame.visual)?
            };
            if frame.key == 11 {
                connection.present_strip(&jpeg)?;
            } else {
                connection.present(frame.key, &jpeg)?;
            }
            let image = Frame {
                page_id: frame.page_id.clone(),
                binding_id: frame.binding_id.clone(),
                data_url: format!("data:image/jpeg;base64,{}", STANDARD.encode(&jpeg)),
            };
            if let Ok(mut images) = shared.images.lock() {
                images.entry(id.into()).or_default().insert(
                    if frame.key == 11 {
                        "touch-strip".into()
                    } else {
                        format!("key-{}", frame.key)
                    },
                    image,
                );
            }
            sent.insert(frame.key, frame.clone());
            last_key = frame.key;
            if shared
                .core
                .lock()
                .map_err(|e| e.to_string())?
                .hardware_frame_sent(id, generation)
            {
                shared.notify(false);
            }
        }
        if heartbeat.elapsed() >= Duration::from_millis(deckweave_n4::packets::HEARTBEAT_MS) {
            connection.heartbeat()?;
            heartbeat = Instant::now();
        }
    }
    Ok(())
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
}
