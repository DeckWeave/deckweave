//! Provisional JSON-lines device-provider host. No action execution lives in a provider.
//! Reader/writer queues are bounded; native/process I/O never runs under the Core lock.
use deckweave_core::{
    ConnectionState, Control, ControlKind, Core, Geometry, InputEvent, ProviderDescriptor,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{BufRead, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tauri::Emitter;
#[cfg(test)]
#[path = "device_providers_keyboard_tests.rs"]
mod keyboard_tests;

const MAX_LINE: usize = 1024 * 1024;
const QUEUE: usize = 128;
const MAX_PENDING: usize = 64;
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub kind: String,
    pub protocol: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub runtime: String,
    pub entry: String,
    pub platforms: Vec<String>,
    pub models: Vec<String>,
}
#[derive(Clone)]
struct Package {
    manifest: Manifest,
    directory: PathBuf,
    entry: PathBuf,
}

#[derive(Clone, Serialize)]
pub struct PackageStatus {
    pub id: String,
    pub name: String,
    pub version: String,
    pub error: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct DiscoveredDevice {
    pub provider_id: String,
    pub instance_id: String,
    pub name: String,
    pub descriptor: Value,
    pub device_id: Option<String>,
    pub state: String,
    pub error: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct SessionStatus {
    pub generation: u64,
    pub settings_edit: u64,
    pub read_sequence: u64,
    pub state: String,
    pub observed: Value,
    pub requested: Value,
    pub evidence: Value,
    pub error: Option<String>,
    pub recovery: Value,
}
#[derive(Clone, Default, Serialize)]
pub struct ProviderSnapshot {
    pub packages: Vec<PackageStatus>,
    pub discovered: Vec<DiscoveredDevice>,
    pub sessions: BTreeMap<String, SessionStatus>,
}
const OBSERVER_LEASE: Duration = Duration::from_secs(3);
const DEPTH_TTL: Duration = Duration::from_millis(600);
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationSnapshot {
    pub enabled: bool,
    pub state: String,
    pub sequence: u64,
    pub samples: Vec<TravelSample>,
    pub error: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelSample {
    pub control_id: String,
    pub travel_um: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_travel_um: Option<u64>,
    pub age_ms: u64,
}
struct ReceivedTravel {
    travel: u64,
    maximum: Option<u64>,
    received: Instant,
}
#[derive(Default)]
struct Observation {
    leases: BTreeMap<String, Instant>,
    active: bool,
    needs_stop: bool,
    sequence: u64,
    samples: BTreeMap<String, ReceivedTravel>,
    error: Option<String>,
}
impl Observation {
    fn desired(&mut self) -> bool {
        self.leases
            .retain(|_, renewed| renewed.elapsed() < OBSERVER_LEASE);
        !self.leases.is_empty()
    }
    fn snapshot(&mut self) -> ObservationSnapshot {
        let desired = self.desired();
        self.samples
            .retain(|_, sample| sample.received.elapsed() < DEPTH_TTL);
        ObservationSnapshot {
            enabled: desired,
            state: if self.error.is_some() {
                "error"
            } else if desired && self.active {
                "observing"
            } else if desired {
                "starting"
            } else if self.needs_stop {
                "stopping"
            } else {
                "off"
            }
            .into(),
            sequence: self.sequence,
            samples: if desired && self.active {
                self.samples
                    .iter()
                    .map(|(control, sample)| TravelSample {
                        control_id: control.clone(),
                        travel_um: sample.travel,
                        max_travel_um: sample.maximum,
                        age_ms: sample.received.elapsed().as_millis() as u64,
                    })
                    .collect()
            } else {
                vec![]
            },
            error: self.error.clone(),
        }
    }
}
struct Shared {
    core: Arc<Mutex<Core>>,
    app: Option<tauri::AppHandle>,
    epoch: Arc<AtomicU64>,
    stop: AtomicBool,
    inventory: Mutex<ProviderSnapshot>,
    refresh: Mutex<BTreeSet<String>>,
    edits: Mutex<BTreeMap<String, DeviceEdits>>,
    paused: Mutex<BTreeSet<String>>,
    active: Mutex<BTreeSet<String>>,
    quiesced: Mutex<BTreeMap<String, Result<(), String>>>,
    reload: Mutex<BTreeSet<String>>,
    lifecycle: Arc<crate::plugin_lifecycle::Store>,
    recovery_packages: Mutex<BTreeSet<String>>,
    controls: Mutex<VecDeque<RecoveryRequest>>,
    observations: Mutex<BTreeMap<String, Observation>>,
    key_lighting: Mutex<BTreeMap<String, KeyLightingDevice>>,
    keyboard_input: Mutex<BTreeMap<String, crate::keyboard_assignments::InputConfiguration>>,
    keyboard_edits: Mutex<BTreeSet<String>>,
    // A provider process may restart while an editor awaits a terminal ACK.
    // Never reuse its session generation within this application lifetime.
    next_generation: AtomicU64,
}
struct RecoveryRequest {
    device: String,
    kind: &'static str,
    generation: Option<u64>,
    payload: Value,
    reply: mpsc::Sender<Result<Value, String>>,
}
#[derive(Default)]
struct KeyLightingDevice {
    generation: u64,
    active: Option<KeyLightingLease>,
    retired: BTreeSet<String>,
}
struct KeyLightingLease {
    id: String,
    accepted: bool,
    sequence: u64,
    slots: BTreeSet<String>,
    last_sent: Option<Instant>,
    expires: Instant,
    releasing: bool,
}
fn lighting_frame(
    definition: &Value,
    sequence: u64,
    ttl_ms: u64,
    colors: &Value,
) -> Result<BTreeSet<String>, String> {
    let cap = definition
        .get("keyLighting")
        .ok_or("Device has no experimental key lighting capability")?;
    if cap["experimental"] != true
        || cap["maxKeys"].as_u64().is_none_or(|n| n == 0 || n > 14)
        || cap["maxRateHz"].as_u64().is_none_or(|n| n == 0 || n > 10)
        || cap["maxTtlMs"]
            .as_u64()
            .is_none_or(|n| !(100..=3000).contains(&n))
    {
        return Err("Unsupported experimental key lighting capability".into());
    }
    if sequence == 0
        || sequence > 9_007_199_254_740_991
        || !(100..=cap["maxTtlMs"].as_u64().unwrap()).contains(&ttl_ms)
    {
        return Err("Invalid key lighting sequence or TTL".into());
    }
    let colors = colors
        .as_object()
        .ok_or("Key lighting colors must be an object")?;
    if colors.is_empty() || colors.len() > cap["maxKeys"].as_u64().unwrap() as usize {
        return Err("Invalid key lighting slot count".into());
    }
    for (id, rgb) in colors {
        if !definition["controls"].as_array().is_some_and(|controls| {
            controls
                .iter()
                .any(|c| c["id"] == *id && c["kind"] == "key")
        }) {
            return Err("Key lighting references an undeclared physical key".into());
        }
        if !rgb.as_object().is_some_and(|rgb| {
            rgb.len() == 3
                && ["r", "g", "b"].iter().all(|c| {
                    rgb.get(*c)
                        .and_then(Value::as_u64)
                        .is_some_and(|n| n <= 255)
                })
        }) {
            return Err("Invalid key lighting RGB color".into());
        }
    }
    Ok(colors.keys().cloned().collect())
}
#[derive(Default)]
struct DeviceEdits {
    revision: u64,
    fields: BTreeMap<String, (u64, Value)>,
}
pub struct Host {
    shared: Arc<Shared>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}
impl Host {
    pub fn start(
        core: Arc<Mutex<Core>>,
        app: tauri::AppHandle,
        epoch: Arc<AtomicU64>,
        directory: PathBuf,
        lifecycle: Arc<crate::plugin_lifecycle::Store>,
    ) -> Self {
        let shared = Arc::new(Shared {
            core,
            app: Some(app),
            epoch,
            stop: AtomicBool::new(false),
            inventory: Mutex::new(ProviderSnapshot::default()),
            refresh: Mutex::new(BTreeSet::new()),
            edits: Mutex::new(BTreeMap::new()),
            paused: Mutex::new(BTreeSet::new()),
            active: Mutex::new(BTreeSet::new()),
            quiesced: Mutex::new(BTreeMap::new()),
            reload: Mutex::new(BTreeSet::new()),
            lifecycle,
            recovery_packages: Mutex::new(BTreeSet::new()),
            controls: Mutex::new(VecDeque::new()),
            observations: Mutex::new(BTreeMap::new()),
            key_lighting: Mutex::new(BTreeMap::new()),
            keyboard_input: Mutex::new(BTreeMap::new()),
            keyboard_edits: Mutex::new(BTreeSet::new()),
            next_generation: AtomicU64::new(0),
        });
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap()
            .join("device-plugins");
        let mut roots = vec![directory.join("device-plugins")];
        // Smoke/UI fixtures must not discover real development drivers from the checkout.
        // Explicitly installed fixtures still use their isolated managed directory.
        #[cfg(debug_assertions)]
        let include_repository = std::env::var_os("DECKWEAVE_VIRTUAL_ONLY").is_none()
            && crate::smoke::configuration().is_none();
        #[cfg(not(debug_assertions))]
        let include_repository = true;
        if include_repository {
            roots.push(repo);
        }
        let context = Arc::clone(&shared);
        let workers = vec![thread::spawn(move || {
            supervise(context, roots, directory.join("device-provider-state"))
        })];
        Self {
            shared,
            workers: Mutex::new(workers),
        }
    }
    pub fn snapshot(&self) -> ProviderSnapshot {
        self.shared
            .inventory
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }
    /// Cached state for one editor. Does not clone catalog descriptors or wait for hardware.
    pub fn session(&self, device: &str) -> Option<SessionStatus> {
        self.shared
            .inventory
            .lock()
            .ok()?
            .sessions
            .get(device)
            .cloned()
    }
    /// Explicit pairing only. Discovery never creates a Core device or writes hardware.
    pub fn add(&self, provider_id: &str, instance_id: &str) -> Result<String, String> {
        self.pair(None, provider_id, instance_id)
    }
    pub fn rebind(
        &self,
        device_id: &str,
        provider_id: &str,
        instance_id: &str,
    ) -> Result<String, String> {
        self.pair(Some(device_id), provider_id, instance_id)
    }
    fn pair(
        &self,
        device_id: Option<&str>,
        provider_id: &str,
        instance_id: &str,
    ) -> Result<String, String> {
        if !self
            .shared
            .lifecycle
            .allows(&format!("device:{provider_id}"))
            && !self
                .shared
                .recovery_packages
                .lock()
                .is_ok_and(|packages| packages.contains(provider_id))
        {
            return Err("Restore device support before pairing its devices".into());
        }
        let descriptor = self
            .shared
            .inventory
            .lock()
            .map_err(|_| "Provider inventory unavailable")?
            .discovered
            .iter()
            .find(|device| {
                device.provider_id == provider_id
                    && device.instance_id == instance_id
                    && device.error.is_none()
            })
            .map(|device| device.descriptor.clone())
            .ok_or("Device is no longer discovered")?;
        let converted = descriptor_geometry(&descriptor)?;
        let name = descriptor
            .get("name")
            .and_then(Value::as_str)
            .ok_or("Missing device name")?;
        let id = {
            let mut core = self
                .shared
                .core
                .lock()
                .map_err(|_| "Workspace unavailable")?;
            if let Some(device_id) = device_id {
                core.rebind_provider_device(device_id, provider_id, instance_id, converted)
            } else {
                core.register_provider_device(provider_id, instance_id, name, converted)
            }
            .map_err(|error| error.to_string())?
        };
        if let Ok(mut inventory) = self.shared.inventory.lock() {
            for device in inventory
                .discovered
                .iter_mut()
                .filter(|device| device.provider_id == provider_id)
            {
                if device.device_id.as_deref() == Some(&id) {
                    device.device_id = None;
                    device.state = "pairing_required".into();
                }
                if device.instance_id == instance_id {
                    device.device_id = Some(id.clone());
                }
            }
        }
        self.shared.notify(true);
        Ok(id)
    }
    pub fn refresh(&self, device_id: &str) -> Result<(), String> {
        if !self
            .shared
            .core
            .lock()
            .map_err(|_| "Workspace unavailable")?
            .has_device(device_id)
        {
            return Err("Device no longer exists".into());
        }
        self.shared
            .refresh
            .lock()
            .map_err(|_| "Provider refresh unavailable")?
            .insert(device_id.into());
        Ok(())
    }
    /// Volatile editor subscription. Polling renews an existing lease only.
    pub fn set_observation(
        &self,
        device_id: &str,
        owner: &str,
        enabled: bool,
    ) -> Result<ObservationSnapshot, String> {
        if enabled {
            let core = self
                .shared
                .core
                .lock()
                .map_err(|_| "Workspace unavailable")?;
            let snapshot = core.snapshot();
            let device = snapshot
                .configuration
                .devices
                .iter()
                .find(|value| value.id == device_id)
                .ok_or("Device no longer exists")?;
            if !device
                .provider_descriptor
                .as_ref()
                .is_some_and(|descriptor| {
                    descriptor
                        .definition
                        .pointer("/observation/keyTravel")
                        .is_some()
                })
            {
                return Err("Device does not support key travel observation".into());
            }
            if !self
                .shared
                .lifecycle
                .allows(&format!("device:{}", device.provider_id))
            {
                return Err("Device support is disabled".into());
            }
            drop(core);
            if !self
                .shared
                .inventory
                .lock()
                .map_err(|_| "Provider unavailable")?
                .sessions
                .get(device_id)
                .is_some_and(|session| session.state == "connected")
            {
                return Err("Pair and connect the device before observing key travel".into());
            }
        }
        let mut observations = self
            .shared
            .observations
            .lock()
            .map_err(|_| "Observation unavailable")?;
        if !enabled && !observations.contains_key(device_id) {
            return Ok(ObservationSnapshot {
                state: "off".into(),
                ..Default::default()
            });
        }
        let observation = observations.entry(device_id.into()).or_default();
        if enabled {
            observation.leases.insert(owner.into(), Instant::now());
        } else {
            observation.leases.remove(owner);
        }
        observation.error = None;
        Ok(observation.snapshot())
    }
    pub fn observation(&self, device: &str, owner: &str) -> Result<ObservationSnapshot, String> {
        let mut observations = self
            .shared
            .observations
            .lock()
            .map_err(|_| "Observation unavailable")?;
        let Some(observation) = observations.get_mut(device) else {
            return Ok(ObservationSnapshot {
                state: "off".into(),
                ..Default::default()
            });
        };
        observation.desired();
        if let Some(renewed) = observation.leases.get_mut(owner) {
            *renewed = Instant::now();
        }
        Ok(observation.snapshot())
    }
    pub fn stop_observer(&self, owner: &str) {
        if let Ok(mut observations) = self.shared.observations.lock() {
            for observation in observations.values_mut() {
                observation.leases.remove(owner);
            }
        }
    }
    pub fn pause_package(&self, id: &str) -> Result<(), String> {
        if self
            .shared
            .recovery_packages
            .lock()
            .is_ok_and(|packages| packages.contains(id))
        {
            return Err(
                "Physical recovery is pending; provider retained for explicit recovery".into(),
            );
        }
        self.shared
            .paused
            .lock()
            .map_err(|_| "Provider pause unavailable")?
            .insert(id.into());
        let deadline = Instant::now() + Duration::from_secs(9);
        loop {
            if let Some(Err(error)) = self
                .shared
                .quiesced
                .lock()
                .map_err(|_| "Provider result unavailable")?
                .get(id)
            {
                return Err(error.clone());
            }
            if !self
                .shared
                .active
                .lock()
                .map_err(|_| "Provider state unavailable")?
                .contains(id)
            {
                return self
                    .shared
                    .quiesced
                    .lock()
                    .map_err(|_| "Provider result unavailable")?
                    .get(id)
                    .cloned()
                    .unwrap_or(Ok(()));
            }
            if Instant::now() >= deadline {
                return Err("Provider stop is still pending; package retained".into());
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
    pub fn resume_package(&self, id: &str) {
        if let Ok(mut reload) = self.shared.reload.lock() {
            reload.insert(id.into());
        }
        if let Ok(mut paused) = self.shared.paused.lock() {
            paused.remove(id);
        }
    }
    /// Terminal provider ACK only: accepted frames are queued, never claimed rendered.
    pub fn send_key_lighting(
        &self,
        device_id: &str,
        generation: u64,
        lease_id: &str,
        sequence: u64,
        ttl_ms: u64,
        colors: Value,
    ) -> Result<Value, String> {
        self.key_lighting_call(
            device_id,
            generation,
            lease_id,
            Some((sequence, ttl_ms, colors)),
        )
    }
    pub fn release_key_lighting(
        &self,
        device_id: &str,
        generation: u64,
        lease_id: &str,
    ) -> Result<Value, String> {
        self.key_lighting_call(device_id, generation, lease_id, None)
    }
    fn key_lighting_call(
        &self,
        device: &str,
        generation: u64,
        lease: &str,
        frame: Option<(u64, u64, Value)>,
    ) -> Result<Value, String> {
        if lease.is_empty() || lease.len() > 128 {
            return Err("Invalid key lighting lease ID".into());
        }
        let (definition, provider) = {
            let core = self
                .shared
                .core
                .lock()
                .map_err(|_| "Workspace unavailable")?;
            let snapshot = core.snapshot();
            let device = snapshot
                .configuration
                .devices
                .iter()
                .find(|d| d.id == device)
                .ok_or("Device no longer exists")?;
            (
                device
                    .provider_descriptor
                    .as_ref()
                    .ok_or("Device has no provider descriptor")?
                    .definition
                    .clone(),
                device.provider_id.clone(),
            )
        };
        let live = self.session(device).ok_or("Device is not connected")?;
        if live.generation != generation {
            return Err("Stale key lighting generation".into());
        }
        if frame.is_some()
            && (live.state != "connected"
                || live.recovery["state"] == "pending"
                || !self.shared.lifecycle.allows(&format!("device:{provider}")))
        {
            return Err(
                "Key lighting unavailable while disconnected or recovery is pending".into(),
            );
        }
        let slots = frame
            .as_ref()
            .map(|(seq, ttl, colors)| lighting_frame(&definition, *seq, *ttl, colors))
            .transpose()?;
        let (reply, result) = mpsc::channel();
        {
            let mut controls = self
                .shared
                .controls
                .lock()
                .map_err(|_| "Provider queue unavailable")?;
            let mut devices = self
                .shared
                .key_lighting
                .lock()
                .map_err(|_| "Key lighting unavailable")?;
            if !devices.contains_key(device) && devices.len() >= 32 {
                return Err("Too many key lighting devices".into());
            }
            if controls.len() >= 32 {
                return Err("Provider control queue is full".into());
            }
            let output = devices.entry(device.into()).or_default();
            if output.generation != generation {
                *output = KeyLightingDevice {
                    generation,
                    ..Default::default()
                };
            }
            if let Some((sequence, ttl, _)) = &frame {
                if output.retired.contains(lease) {
                    return Err("Key lighting lease was retired; use a new lease ID".into());
                }
                if let Some(active) = &output.active
                    && (active.id != lease
                        || active.releasing
                        || active.sequence >= *sequence
                        || Some(&active.slots) != slots.as_ref())
                {
                    return Err("Key lighting lease, sequence, or fixed slot set changed".into());
                }
                if output.active.is_none() {
                    if output.retired.len() >= 128 {
                        return Err("Key lighting lease limit reached for this session".into());
                    }
                    output.active = Some(KeyLightingLease {
                        id: lease.into(),
                        accepted: false,
                        sequence: *sequence,
                        slots: slots.unwrap(),
                        last_sent: None,
                        expires: Instant::now() + Duration::from_millis(*ttl),
                        releasing: false,
                    });
                }
            } else if !output.active.as_ref().is_some_and(|a| a.id == lease) {
                if output.retired.contains(lease) {
                    // This exact generation either acknowledged restoration or rejected output before it started.
                    return Ok(
                        json!({"released":true,"recovery":"not_needed","alreadyReleased":true}),
                    );
                }
                return Err("Key lighting lease is not active".into());
            }
            // A newer unsent frame explicitly supersedes its waiter; releases take priority.
            if let Some(index) = controls
                .iter()
                .position(|q| q.device == device && q.kind == "keyLighting")
            {
                let old = controls.remove(index).unwrap();
                let _ = old.reply.send(Err(
                    "Key lighting frame superseded before transmission".into()
                ));
            }
            let active = output.active.as_mut().unwrap();
            let (kind, payload) = if let Some((sequence, ttl, colors)) = frame {
                active.sequence = sequence;
                active.expires = Instant::now() + Duration::from_millis(ttl);
                (
                    "keyLighting",
                    json!({"leaseId":lease,"sequence":sequence,"ttlMs":ttl,"colors":colors}),
                )
            } else {
                if active.releasing {
                    return Err("Key lighting release already pending".into());
                }
                active.releasing = true;
                ("releaseKeyLighting", json!({"leaseId":lease}))
            };
            let request = RecoveryRequest {
                device: device.into(),
                generation: Some(generation),
                kind,
                payload,
                reply,
            };
            if kind == "releaseKeyLighting" {
                controls.push_front(request);
            } else {
                controls.push_back(request);
            }
        }
        result
            .recv_timeout(Duration::from_secs(7))
            .map_err(|_| "Key lighting request timed out; completion is unconfirmed".to_string())?
    }
    /// Explicit keyboard operations are scoped to the sampled connection. They
    /// never enter the ordinary settings replay queue.
    pub fn keyboard_call(
        &self,
        device: &str,
        generation: u64,
        kind: &'static str,
        payload: Value,
    ) -> Result<Value, String> {
        if !matches!(
            kind,
            "installKeyboardAlias"
                | "restoreKeyboardAlias"
                | "reconcileKeyboardAlias"
                | "configureKeyboardInput"
        ) {
            return Err("Unsupported keyboard operation".into());
        }
        if !self.session(device).is_some_and(|s| {
            s.generation == generation
                && (s.state == "connected"
                    || kind == "reconcileKeyboardAlias"
                        && matches!(s.state.as_str(), "error" | "recovery_pending"))
        }) {
            return Err("Keyboard connection changed; read the selected device again".into());
        }
        let (reply, result) = mpsc::channel();
        {
            let mut queue = self
                .shared
                .controls
                .lock()
                .map_err(|_| "Keyboard queue unavailable")?;
            if queue.len() >= 32 {
                return Err("Keyboard request queue is full".into());
            }
            queue.push_back(RecoveryRequest {
                device: device.into(),
                kind,
                generation: Some(generation),
                payload,
                reply,
            });
        }
        result
            .recv_timeout(Duration::from_secs(7))
            .map_err(|_| "Keyboard operation timed out; completion is unconfirmed".to_string())?
    }
    pub fn keyboard_input(
        &self,
        device: &str,
    ) -> Option<crate::keyboard_assignments::InputConfiguration> {
        self.shared.keyboard_input.lock().ok()?.get(device).cloned()
    }
    pub fn keyboard_routes_ready(
        &self,
        device: &str,
        routes: &[deckweave_core::KeyboardAliasRoute],
    ) -> bool {
        self.shared.inventory.lock().is_ok_and(|inventory| {
            inventory.sessions.get(device).is_some_and(|session| {
                session.state == "connected"
                    && routes.iter().all(|route| {
                        session.generation == route.generation
                            && session
                                .observed
                                .pointer(&format!(
                                    "/keyMappings/{}/{}/usage",
                                    route.control_id,
                                    route.layer.as_str()
                                ))
                                .and_then(Value::as_u64)
                                == Some(u64::from(route.alias) + 91)
                            && ["main", "fn"].iter().all(|layer| {
                                session
                                    .observed
                                    .pointer(&format!("/keyboardAliases/records/{layer}"))
                                    .and_then(Value::as_array)
                                    .is_some_and(|r| r.len() == 128)
                            })
                            && ["main", "fn"]
                                .iter()
                                .flat_map(|layer| {
                                    session
                                        .observed
                                        .pointer(&format!("/keyboardAliases/records/{layer}"))
                                        .and_then(Value::as_array)
                                        .into_iter()
                                        .flatten()
                                })
                                .filter(|r| {
                                    r[0] == 2 && r[2].as_u64() == Some(u64::from(route.alias) + 91)
                                })
                                .count()
                                == 1
                    })
            })
        })
    }
    pub fn set_keyboard_input(
        &self,
        device: &str,
        mut configuration: crate::keyboard_assignments::InputConfiguration,
    ) {
        if self.keyboard_editing(device) {
            configuration.ready = false;
        }
        if let Ok(mut states) = self.shared.keyboard_input.lock() {
            states.insert(device.into(), configuration);
        }
    }
    pub fn keyboard_editing(&self, device: &str) -> bool {
        self.shared
            .keyboard_edits
            .lock()
            .map_or(true, |edits| edits.contains(device))
    }
    pub fn begin_keyboard_edit(&self, device: &str) -> Result<KeyboardEdit<'_>, String> {
        if !self
            .shared
            .keyboard_edits
            .lock()
            .map_err(|_| "Keyboard edit state unavailable")?
            .insert(device.into())
        {
            return Err("Another keyboard operation is in progress".into());
        }
        self.set_keyboard_input(
            device,
            crate::keyboard_assignments::InputConfiguration::default(),
        );
        Ok(KeyboardEdit {
            host: self,
            device: device.into(),
        })
    }
    pub fn recovery_preview(
        &self,
        device_id: &str,
        snapshot_id: Option<&str>,
    ) -> Result<Value, String> {
        self.recovery_call(
            device_id,
            "recoveryPreview",
            snapshot_id.map_or_else(
                || json!({}),
                |snapshot_id| json!({"snapshotId":snapshot_id}),
            ),
        )
    }
    pub fn recover(
        &self,
        device_id: &str,
        preview_token: &str,
        confirmed_instance: &str,
    ) -> Result<Value, String> {
        self.resolve_recovery(device_id, preview_token, confirmed_instance, None)
    }
    pub fn resolve_recovery(
        &self,
        device_id: &str,
        preview_token: &str,
        confirmed_instance: &str,
        resolution: Option<&str>,
    ) -> Result<Value, String> {
        let mut payload =
            json!({"previewToken":preview_token,"confirmedInstanceId":confirmed_instance});
        if let Some(resolution) = resolution {
            if !["restore", "keepCurrent"].contains(&resolution) {
                return Err("Unsupported recovery resolution".into());
            }
            payload["resolution"] = resolution.into();
        }
        self.recovery_call(device_id, "recover", payload)
    }
    fn recovery_call(
        &self,
        device: &str,
        kind: &'static str,
        payload: Value,
    ) -> Result<Value, String> {
        if !self
            .shared
            .inventory
            .lock()
            .map_err(|_| "Provider inventory unavailable")?
            .sessions
            .contains_key(device)
        {
            return Err("Explicitly pair a connected keyboard before recovery".into());
        }
        let (reply, result) = mpsc::channel();
        {
            let mut controls = self
                .shared
                .controls
                .lock()
                .map_err(|_| "Recovery queue unavailable")?;
            if controls.len() >= 32 {
                return Err("Recovery request queue is full".into());
            }
            controls.push_back(RecoveryRequest {
                device: device.into(),
                kind,
                generation: None,
                payload,
                reply,
            });
        }
        result
            .recv_timeout(Duration::from_secs(7))
            .map_err(|_| "Recovery request timed out; no completion is confirmed".to_string())?
    }
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Ok(mut workers) = self.workers.lock() {
            for worker in workers.drain(..) {
                let _ = worker.join();
            }
        }
    }
}
impl Shared {
    fn drain_outputs(&self) {
        let outputs = self
            .core
            .lock()
            .map(|mut core| {
                core.take_provider_outputs()
                    .into_iter()
                    .filter(|output| {
                        core.has_device(&output.device_id)
                            && core.device_lifecycle_epoch(&output.device_id)
                                == output.lifecycle_epoch
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for output in outputs {
            let live = self.inventory.lock().is_ok_and(|inventory| {
                inventory
                    .sessions
                    .get(&output.device_id)
                    .is_some_and(|session| session.state == "connected")
            });
            if !live {
                if let Ok(mut inventory) = self.inventory.lock() {
                    inventory.sessions.insert(output.device_id,SessionStatus { generation:0,settings_edit:0,read_sequence:0,state:"offline".into(),observed:json!({}),requested:json!(output.changes),evidence:json!({"transport":"unknown","readback":"unknown","physical":"unknown"}),recovery:Value::Null,error:Some("Device is not connected. This output was not queued; reconnect and request it again.".into()) });
                }
                self.notify(false);
                continue;
            }
            if let Ok(mut edits) = self.edits.lock() {
                let edit = edits.entry(output.device_id).or_default();
                edit.revision += 1;
                for (field, value) in output.changes {
                    let mut merged = BTreeMap::new();
                    if let Some((_, previous)) = edit.fields.get(&field) {
                        merged.insert(field.clone(), previous.clone());
                    }
                    deckweave_core::merge_provider_patch(
                        &mut merged,
                        BTreeMap::from([(field.clone(), value)]),
                    );
                    edit.fields.insert(
                        field.clone(),
                        (edit.revision, merged.remove(&field).unwrap()),
                    );
                }
            }
        }
    }
    fn notify(&self, render: bool) {
        if render {
            self.epoch.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(app) = &self.app {
            let _ = app.emit("deckweave:changed", ());
        }
    }
    fn package(&self, manifest: &Manifest, error: Option<String>) {
        if let Ok(mut inventory) = self.inventory.lock() {
            let status = PackageStatus {
                id: manifest.id.clone(),
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                error,
            };
            if let Some(old) = inventory
                .packages
                .iter_mut()
                .find(|entry| entry.id == manifest.id)
            {
                *old = status;
            } else {
                inventory.packages.push(status);
            }
        }
        self.notify(false);
    }
    fn connection(&self, id: &str, generation: u64, state: ConnectionState, error: Option<String>) {
        let changed = self
            .core
            .lock()
            .is_ok_and(|mut core| core.connection_changed(id, generation, state, error));
        if changed {
            self.notify(true);
        }
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}
fn opaque_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}
fn reverse_dns(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    identifier(value)
        && parts.len() >= 3
        && parts.iter().all(|part| {
            part.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && part.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
                })
        })
}
fn version(value: &str) -> bool {
    let main = value.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<_> = main.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-+".contains(&byte))
}
pub fn inspect_package(directory: &Path) -> Result<Manifest, String> {
    load(directory).map(|package| package.manifest)
}
fn scan(roots: &[PathBuf]) -> Vec<Result<Package, String>> {
    let mut results = vec![];
    let mut seen = BTreeSet::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        let mut paths: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|value| value == "dwDevice"))
            .take(65)
            .collect();
        if paths.len() > 64 {
            results.push(Err("Too many device packages in one source".into()));
            paths.truncate(64);
        }
        paths.sort();
        for directory in paths.into_iter().take(64) {
            let identity = directory
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_owned();
            if reverse_dns(&identity) && !seen.insert(identity.clone()) {
                continue;
            }
            match load(&directory) {
                Ok(package) => results.push(Ok(package)),
                Err(error) => results.push(Err(error)),
            }
        }
    }
    results
}
fn load(directory: &Path) -> Result<Package, String> {
    let directory = directory
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut bytes = vec![];
    std::fs::File::open(directory.join("deckweave.device.json"))
        .map_err(|error| error.to_string())?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 64 * 1024 {
        return Err("Device manifest is too large".into());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if directory
        .extension()
        .is_some_and(|extension| extension == "dwDevice")
        && directory.file_stem().and_then(|name| name.to_str()) != Some(&manifest.id)
    {
        return Err("Device package folder identity differs from manifest".into());
    }
    let platform = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    if manifest.kind != "device"
        || manifest.protocol != 1
        || !reverse_dns(&manifest.id)
        || manifest.name.trim().is_empty()
        || manifest.name.chars().count() > 120
        || !version(&manifest.version)
        || !["node", "native"].contains(&manifest.runtime.as_str())
        || manifest.models.is_empty()
        || manifest.models.len() > 64
        || !manifest.models.iter().all(|model| reverse_dns(model))
        || !manifest.platforms.iter().any(|value| value == platform)
    {
        return Err(format!("Unsupported device manifest: {}", manifest.id));
    }
    let relative = Path::new(&manifest.entry);
    if manifest.entry.contains(['\\', ':'])
        || relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("Device entry must be a relative bundled file".into());
    }
    let entry = directory
        .join(relative)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !entry.starts_with(&directory) || !entry.is_file() {
        return Err("Device entry escapes its package".into());
    }
    Ok(Package {
        manifest,
        directory,
        entry,
    })
}

pub fn descriptor_geometry(definition: &Value) -> Result<ProviderDescriptor, String> {
    if let Some(observation) = definition.get("observation") {
        let range = observation
            .get("keyTravel")
            .ok_or("Invalid observation capability")?;
        if range["minUm"].as_u64() != Some(0)
            || !range["maxUm"]
                .as_u64()
                .is_some_and(|max| max > 0 && max <= 10_000)
            || range["staleAfterMs"].as_u64() != Some(600)
            || range["normalTyping"].as_bool().is_none()
            || !matches!(range["evidence"].as_str(), Some("partial" | "verified"))
        {
            return Err("Invalid observation range/evidence".into());
        }
    }
    let number = |object: &Value, field: &str| {
        object
            .get(field)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 100_000.0)
            .ok_or_else(|| format!("Invalid descriptor {field}"))
    };
    let geometry = definition.get("geometry").ok_or("Missing geometry")?;
    let width = number(geometry, "width")?;
    let height = number(geometry, "height")?;
    if width <= 0.0
        || height <= 0.0
        || !opaque_identity(
            definition
                .get("instanceId")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )
    {
        return Err("Invalid instance geometry/identity".into());
    }
    let source = definition
        .get("controls")
        .and_then(Value::as_array)
        .filter(|controls| !controls.is_empty() && controls.len() <= 512)
        .ok_or("Invalid controls")?;
    let mut controls = vec![];
    let mut ids = BTreeSet::new();
    let mut numbers = [0, 0, 0];
    for value in source.iter() {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| identifier(id) && ids.insert((*id).to_string()))
            .ok_or("Invalid or duplicate control ID")?;
        let kind = match value.get("kind").and_then(Value::as_str) {
            Some("key") => ControlKind::Key,
            Some("dial") => ControlKind::Dial,
            Some("touch") => ControlKind::Touch,
            _ => return Err("Unsupported control kind".into()),
        };
        let rect = [
            number(value, "x")?,
            number(value, "y")?,
            number(value, "width")?,
            number(value, "height")?,
        ];
        if rect[2] <= 0.0
            || rect[3] <= 0.0
            || rect[0] + rect[2] > width + 0.001
            || rect[1] + rect[3] > height + 0.001
        {
            return Err("Control is outside device geometry".into());
        }
        let kind_index = match kind {
            ControlKind::Key => 0,
            ControlKind::Dial => 1,
            ControlKind::Touch => 2,
        };
        numbers[kind_index] += 1;
        controls.push(Control {
            pointer: kind == ControlKind::Touch,
            id: id.into(),
            kind,
            rect,
            number: numbers[kind_index],
        });
    }
    // No synthetic LCD is attached to a keyboard key. Display transport is not yet rendered by this host.
    Ok(ProviderDescriptor {
        geometry: Geometry {
            width,
            height,
            surface: [0.0, 0.0, 0.0, 0.0],
            surface_pixels: [0, 0],
            controls,
        },
        definition: definition.clone(),
    })
}

enum PipeEvent {
    Message(Value),
    Failed(String),
    Eof,
}
struct Transport {
    child: Child,
    process_tree: Option<deckweave_platform::ProcessTree>,
    write: SyncSender<Vec<u8>>,
    read: Receiver<PipeEvent>,
    failed: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}
fn read_lines(
    mut stream: impl Read,
    mut line: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut reader = BufReader::new(&mut stream);
    loop {
        let mut bytes = vec![];
        let count = reader
            .by_ref()
            .take((MAX_LINE + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            return Ok(());
        }
        if count > MAX_LINE || bytes.last() != Some(&b'\n') {
            return Err("Device provider sent oversized or unterminated line".into());
        }
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        line(&bytes)?;
    }
}
impl Transport {
    fn start(
        package: &Package,
        state_directory: &Path,
        recovery_only: bool,
    ) -> Result<Self, String> {
        let mut command = if package.manifest.runtime == "node" {
            let mut command = Command::new("node");
            command.arg(&package.entry);
            command
        } else {
            Command::new(&package.entry)
        };
        command
            .current_dir(&package.directory)
            .env_clear()
            .env("DECKWEAVE_PROVIDER_STATE_DIR", state_directory)
            .env(
                "DECKWEAVE_PROVIDER_RECOVERY_ONLY",
                if recovery_only { "1" } else { "0" },
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in [
            "PATH",
            "SystemRoot",
            "WINDIR",
            "TEMP",
            "TMP",
            "USERPROFILE",
            "HOMEDRIVE",
            "HOMEPATH",
            "APPDATA",
            "LOCALAPPDATA",
            "PROGRAMDATA",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("Cannot start device provider: {error}"))?;
        let process_tree = match deckweave_platform::contain_process(&child) {
            Ok(tree) => tree,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Cannot contain provider process tree: {error}"));
            }
        };
        let mut input = child.stdin.take().ok_or("Missing provider stdin")?;
        let output = child.stdout.take().ok_or("Missing provider stdout")?;
        let errors = child.stderr.take().ok_or("Missing provider stderr")?;
        let (write, outgoing) = mpsc::sync_channel::<Vec<u8>>(QUEUE);
        let (incoming, read) = mpsc::sync_channel(QUEUE);
        let failed = Arc::new(AtomicBool::new(false));
        let writer_failed = Arc::clone(&failed);
        let writer = thread::spawn(move || {
            while let Ok(bytes) = outgoing.recv() {
                if input.write_all(&bytes).and_then(|_| input.flush()).is_err() {
                    writer_failed.store(true, Ordering::Relaxed);
                    break;
                }
            }
        });
        let reader_failed = Arc::clone(&failed);
        let reader = thread::spawn(move || {
            let result = read_lines(output, |bytes| {
                let value: Value = serde_json::from_slice(bytes)
                    .map_err(|_| "Malformed provider JSON".to_string())?;
                incoming
                    .try_send(PipeEvent::Message(value))
                    .map_err(|_| "Provider input queue overflow".to_string())
            });
            let event = result.map_or_else(PipeEvent::Failed, |_| PipeEvent::Eof);
            if incoming.try_send(event).is_err() {
                reader_failed.store(true, Ordering::Relaxed);
            }
        });
        // Drain stderr without retaining arbitrary provider content/keyboard data. Bounded framing still applies.
        let errors_failed = Arc::clone(&failed);
        let stderr = thread::spawn(move || {
            if read_lines(errors, |_| Ok(())).is_err() {
                errors_failed.store(true, Ordering::Relaxed);
            }
        });
        Ok(Self {
            child,
            process_tree: Some(process_tree),
            write,
            read,
            failed,
            threads: vec![writer, reader, stderr],
        })
    }
    fn send(&self, value: &Value) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        if bytes.len() > MAX_LINE {
            return Err("Provider request exceeds line bound".into());
        }
        self.write
            .try_send(bytes)
            .map_err(|_| "Provider output queue overflow".into())
    }
    fn stop(mut self) -> Result<(), String> {
        self.process_tree = None;
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_secs(2);
        while self
            .child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none()
        {
            if Instant::now() >= deadline {
                return Err("Provider process termination is unconfirmed".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
        drop(self.write);
        while self.threads.iter().any(|thread| !thread.is_finished()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        for thread in self.threads {
            if !thread.is_finished() {
                return Err("Provider pipe shutdown is unconfirmed; package retained".into());
            }
            let _ = thread.join();
        }
        Ok(())
    }
}

#[derive(Clone)]
enum Operation {
    Hello,
    Discover,
    Open(String),
    Read(String, bool),
    Settings(String, BTreeMap<String, Value>, u64),
    Close(String),
    Observe(String, bool),
    KeyLighting(
        String,
        String,
        Option<u64>,
        mpsc::Sender<Result<Value, String>>,
    ),
    Recovery(String, mpsc::Sender<Result<Value, String>>, bool, bool),
    Keyboard(String, &'static str, mpsc::Sender<Result<Value, String>>),
}
struct Pending {
    operation: Operation,
    sent: Instant,
    generation: Option<u64>,
}
pub struct KeyboardEdit<'a> {
    host: &'a Host,
    device: String,
}
impl Drop for KeyboardEdit<'_> {
    fn drop(&mut self) {
        if let Ok(mut edits) = self.host.shared.keyboard_edits.lock() {
            edits.remove(&self.device);
        }
        self.host.set_keyboard_input(
            &self.device,
            crate::keyboard_assignments::InputConfiguration::default(),
        );
    }
}
struct Session {
    device: String,
    generation: u64,
    lifecycle_epoch: u64,
    sequence: u64,
    definition: Value,
    opened: bool,
    failed: bool,
    edit: u64,
    recovery: Value,
    released: bool,
    observing: bool,
    observation_needs_stop: bool,
    last_observe: Instant,
    depth_sequence: u64,
    keyboard_sequence: u64,
    read_sequence: u64,
    recovery_preview: Option<RecoveryAuthorization>,
}
struct RecoveryAuthorization {
    token: String,
    instance: String,
    expires: Instant,
    can_keep_current: bool,
    restore_available: bool,
}
impl RecoveryAuthorization {
    fn from_preview(value: &Value) -> Option<Self> {
        Some(Self {
            token: value["previewToken"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 128)?
                .into(),
            instance: value["confirmedInstanceId"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 128)?
                .into(),
            expires: Instant::now()
                + Duration::from_millis(
                    value["expiresInMs"]
                        .as_u64()
                        .filter(|n| *n > 0 && *n <= 60_000)?,
                ),
            can_keep_current: value["canKeepCurrent"].as_bool() == Some(true),
            restore_available: value["restoreAvailable"].as_bool() != Some(false),
        })
    }
    fn matches(&self, request: &Value) -> bool {
        self.expires > Instant::now()
            && request["previewToken"].as_str() == Some(&self.token)
            && request["confirmedInstanceId"].as_str() == Some(&self.instance)
    }
}
fn authorize_resolution(
    preview: Option<&RecoveryAuthorization>,
    request: &Value,
) -> Result<(), String> {
    match request.get("resolution").and_then(Value::as_str) {
        Some("keepCurrent") => {
            if !preview.is_some_and(|preview| preview.matches(request) && preview.can_keep_current)
            {
                return Err("Keeping current settings requires a fresh preview explicitly supporting that resolution".into());
            }
        }
        None | Some("restore") => {
            if preview.is_some_and(|preview| preview.matches(request) && !preview.restore_available)
            {
                return Err("The saved record cannot restore the original settings; review its supported resolution".into());
            }
        }
        Some(_) => return Err("Unsupported recovery resolution".into()),
    }
    Ok(())
}
struct ProcessState {
    sessions: BTreeMap<String, Session>,
    pending: BTreeMap<String, Pending>,
    next_request: u64,
    hello: bool,
}
impl ProcessState {
    fn request(
        &mut self,
        transport: &Transport,
        kind: &str,
        operation: Operation,
        session: Option<&str>,
        extra: Value,
    ) -> Result<(), String> {
        if self.pending.len() >= MAX_PENDING {
            return Err("Too many provider requests".into());
        }
        self.next_request += 1;
        let id = self.next_request.to_string();
        let mut message = extra.as_object().cloned().unwrap_or_default();
        message.insert("type".into(), kind.into());
        message.insert("requestId".into(), id.clone().into());
        let generation = session
            .and_then(|id| self.sessions.get(id))
            .map(|session| session.generation);
        if let Some(session) = session {
            message.insert("session".into(), session.into());
            message.insert(
                "generation".into(),
                generation.ok_or("Unknown provider session")?.into(),
            );
        }
        transport.send(&Value::Object(message))?;
        self.pending.insert(
            id,
            Pending {
                operation,
                sent: Instant::now(),
                generation,
            },
        );
        Ok(())
    }
}

fn supervise(shared: Arc<Shared>, roots: Vec<PathBuf>, state_root: PathBuf) {
    let mut running: BTreeMap<String, (String, Arc<AtomicBool>, JoinHandle<()>)> = BTreeMap::new();
    let mut last_scan = Instant::now() - Duration::from_secs(3);
    let mut reported_errors = BTreeSet::new();
    while !shared.stop.load(Ordering::Relaxed) {
        shared.drain_outputs();
        if last_scan.elapsed() >= Duration::from_secs(2) {
            last_scan = Instant::now();
            let mut present = BTreeSet::new();
            for result in scan(&roots) {
                match result {
                    Ok(package) => {
                        let id = package.manifest.id.clone();
                        present.insert(id.clone());
                        let paused = shared.paused.lock().ok();
                        let blocked = paused.as_ref().is_none_or(|paused| paused.contains(&id))
                            || !shared.lifecycle.allows(&format!("device:{id}"));
                        let startup_recovery = blocked
                            && !running.contains_key(&id)
                            && !shared
                                .quiesced
                                .lock()
                                .is_ok_and(|result| result.contains_key(&id))
                            && shared.lifecycle.records().is_ok_and(|records| {
                                records.iter().any(|record| {
                                    record.kind == "device"
                                        && record.id == id
                                        && record.phase == "stop_pending"
                                })
                            });
                        if startup_recovery
                            && let Ok(mut recovery) = shared.recovery_packages.lock()
                        {
                            recovery.insert(id.clone());
                        }
                        let recovery_mode = shared
                            .recovery_packages
                            .lock()
                            .is_ok_and(|packages| packages.contains(&id));
                        if blocked && !(recovery_mode && !running.contains_key(&id)) {
                            if !shared
                                .recovery_packages
                                .lock()
                                .is_ok_and(|packages| packages.contains(&id))
                            {
                                if let Some((_, stop, _)) = running.get(&id) {
                                    stop.store(true, Ordering::Relaxed);
                                }
                                if running
                                    .get(&id)
                                    .is_some_and(|(_, _, worker)| worker.is_finished())
                                    && let Some((_, _, worker)) = running.remove(&id)
                                {
                                    let _ = worker.join();
                                }
                            }
                            continue;
                        }
                        let fingerprint = format!(
                            "{}:{}",
                            package.directory.display(),
                            package.manifest.version
                        );
                        let reload = shared
                            .reload
                            .lock()
                            .is_ok_and(|mut reload| reload.remove(&id));
                        if reload
                            && running
                                .get(&id)
                                .is_some_and(|(_, _, worker)| !worker.is_finished())
                        {
                            if !recovery_mode && let Some((_, stop, _)) = running.get(&id) {
                                stop.store(true, Ordering::Relaxed);
                            }
                            if let Ok(mut reload) = shared.reload.lock() {
                                reload.insert(id.clone());
                            }
                            continue;
                        }
                        if reload
                            && running
                                .get(&id)
                                .is_some_and(|(_, _, worker)| worker.is_finished())
                            && let Some((_, _, worker)) = running.remove(&id)
                        {
                            let _ = worker.join();
                        }
                        if let Some((old, _, _)) = running.get(&id) {
                            if *old == fingerprint {
                                continue;
                            }
                            let paired = shared.core.lock().is_ok_and(|core| {
                                core.snapshot()
                                    .configuration
                                    .devices
                                    .iter()
                                    .any(|device| device.provider_id == id)
                            });
                            if paired {
                                shared.package(&package.manifest, Some("Device support changed while paired. Close/remove its devices before updating.".into()));
                                continue;
                            }
                            if let Some((_, stop, worker)) = running.remove(&id) {
                                stop.store(true, Ordering::Relaxed);
                                let _ = worker.join();
                            }
                        }
                        shared.package(&package.manifest, None);
                        let context = Arc::clone(&shared);
                        let stop = Arc::new(AtomicBool::new(false));
                        let worker_stop = Arc::clone(&stop);
                        let state_directory = state_root.join(&id);
                        if let Ok(mut active) = shared.active.lock() {
                            active.insert(id.clone());
                        }
                        if let Ok(mut quiesced) = shared.quiesced.lock() {
                            quiesced.remove(&id);
                        }
                        running.insert(
                            id,
                            (
                                fingerprint,
                                stop,
                                thread::spawn(move || {
                                    run(context, package, worker_stop, state_directory)
                                }),
                            ),
                        );
                        drop(paused);
                    }
                    Err(error)
                        if reported_errors.insert(error.clone()) && reported_errors.len() <= 64 =>
                    {
                        if let Ok(mut inventory) = shared.inventory.lock() {
                            inventory.packages.push(PackageStatus {
                                id: String::new(),
                                name: "Device support".into(),
                                version: String::new(),
                                error: Some(error),
                            });
                        }
                        shared.notify(false);
                    }
                    Err(_) => {}
                }
            }
            for id in running
                .keys()
                .filter(|id| !present.contains(*id))
                .cloned()
                .collect::<Vec<_>>()
            {
                if let Some((_, stop, worker)) = running.remove(&id) {
                    stop.store(true, Ordering::Relaxed);
                    let _ = worker.join();
                }
                fail_package(&shared, &id, "Device support is no longer installed");
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    for (_, (_, stop, worker)) in running {
        stop.store(true, Ordering::Relaxed);
        let _ = worker.join();
    }
}

fn run(shared: Arc<Shared>, package: Package, stop: Arc<AtomicBool>, state_directory: PathBuf) {
    let mut opened = false;
    let result = std::fs::create_dir_all(&state_directory)
        .map_err(|error| error.to_string())
        .and_then(|_| {
            Transport::start(
                &package,
                &state_directory,
                shared
                    .recovery_packages
                    .lock()
                    .is_ok_and(|packages| packages.contains(&package.manifest.id)),
            )
        })
        .and_then(|mut transport| {
            let result = process(&shared, &package, &mut transport, &stop, &mut opened);
            let stopped = transport.stop();
            result.and(stopped)
        });
    if let Err(error) = &result {
        shared.package(&package.manifest, Some(error.clone()));
        fail_package(&shared, &package.manifest.id, error);
    }
    if let Ok(mut quiesced) = shared.quiesced.lock() {
        quiesced.insert(
            package.manifest.id.clone(),
            if opened { result } else { Ok(()) },
        );
    }
    if let Ok(mut active) = shared.active.lock() {
        active.remove(&package.manifest.id);
    }
    if let Ok(mut inventory) = shared.inventory.lock() {
        for entry in inventory
            .discovered
            .iter_mut()
            .filter(|entry| entry.provider_id == package.manifest.id)
        {
            entry.device_id = None;
            entry.state = "pairing_required".into();
        }
    }
    shared.notify(false);
}
fn fail_package(shared: &Shared, provider: &str, error: &str) {
    let snapshot = match shared.core.lock() {
        Ok(core) => core.snapshot(),
        Err(_) => return,
    };
    for device in snapshot
        .configuration
        .devices
        .iter()
        .filter(|device| device.provider_id == provider)
    {
        if let Ok(mut observations) = shared.observations.lock()
            && let Some(observation) = observations.get_mut(&device.id)
        {
            observation.leases.clear();
            observation.samples.clear();
            observation.error =
                Some("Observation stopped unexpectedly; hardware stop is unconfirmed".into());
        }
        let generation = snapshot
            .connections
            .get(&device.id)
            .map_or(0, |connection| connection.generation);
        shared.connection(
            &device.id,
            generation,
            ConnectionState::Error,
            Some(error.into()),
        );
    }
    if let Ok(mut inventory) = shared.inventory.lock() {
        for device in inventory
            .discovered
            .iter_mut()
            .filter(|device| device.provider_id == provider)
        {
            device.state = "error".into();
            device.error = Some(error.into());
        }
    }
    shared.notify(false);
}
fn process(
    shared: &Shared,
    package: &Package,
    transport: &mut Transport,
    stop: &AtomicBool,
    opened: &mut bool,
) -> Result<(), String> {
    let mut state = ProcessState {
        sessions: BTreeMap::new(),
        pending: BTreeMap::new(),
        next_request: 0,
        hello: false,
    };
    state.request(
        transport,
        "hello",
        Operation::Hello,
        None,
        json!({"protocol":1}),
    )?;
    let mut last_discover = Instant::now();
    let mut last_sync = Instant::now() - Duration::from_secs(1);
    'service: loop {
        while !shared.stop.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
            if transport.failed.load(Ordering::Relaxed) {
                return Err("Device provider pipe failed or overflowed".into());
            }
            if transport
                .child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                return Err("Device provider exited".into());
            }
            // Continuous telemetry must not starve control reconciliation and deadlines.
            for _ in 0..QUEUE {
                let Ok(message) = transport.read.try_recv() else {
                    break;
                };
                match message {
                    PipeEvent::Message(value) => {
                        handle(shared, package, transport, &mut state, value)?
                    }
                    PipeEvent::Failed(error) => return Err(error),
                    PipeEvent::Eof => return Err("Device provider closed stdout".into()),
                }
                *opened |= state.sessions.values().any(|session| session.opened);
            }
            for request_id in state
                .pending
                .iter()
                .filter(|(_, request)| request.sent.elapsed() >= TIMEOUT)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>()
            {
                let Some(request) = state.pending.remove(&request_id) else {
                    continue;
                };
                let id = match request.operation {
                    Operation::KeyLighting(id, _, _, reply) => {
                        let _ = reply.send(Err(
                            "Key lighting timed out; RAM restoration is unconfirmed".into(),
                        ));
                        id
                    }
                    Operation::Recovery(_, reply, _, _) => {
                        let _ = reply.send(Err(
                            "Recovery request timed out; completion is unconfirmed".into(),
                        ));
                        continue;
                    }
                    Operation::Keyboard(id, _, reply) => {
                        if let Some(session) = state.sessions.get_mut(&id)
                            && let Ok(mut inputs) = shared.keyboard_input.lock()
                            && let Some(input) = inputs.get_mut(&session.device)
                        {
                            input.ready = false;
                            input.error = Some(
                                "Keyboard operation timed out; explicit recheck required".into(),
                            );
                        }
                        let _ = reply.send(Err(
                            "Keyboard operation timed out; completion is unconfirmed".into(),
                        ));
                        continue;
                    }
                    Operation::Open(id)
                    | Operation::Read(id, _)
                    | Operation::Settings(id, _, _) => id,
                    Operation::Close(_) => {
                        return Err(
                            "Device provider close timed out; physical recovery is unconfirmed"
                                .into(),
                        );
                    }
                    _ => return Err("Device provider handshake/discovery timed out".into()),
                };
                if let Some(session) = state.sessions.get_mut(&id) {
                    session.failed = true;
                    shared.connection(
                        &session.device,
                        session.generation,
                        ConnectionState::Error,
                        Some("Device request timed out; closing session".into()),
                    );
                }
                if !state.pending.values().any(|pending| matches!(&pending.operation,Operation::Close(existing) if existing == &id)) { state.request(transport,"close",Operation::Close(id.clone()),Some(&id),json!({}))?; }
            }
            if state.hello
                && last_discover.elapsed() >= Duration::from_secs(5)
                && !state
                    .pending
                    .values()
                    .any(|pending| matches!(pending.operation, Operation::Discover))
            {
                state.request(transport, "discover", Operation::Discover, None, json!({}))?;
                last_discover = Instant::now();
            }
            if state.hello && last_sync.elapsed() >= Duration::from_millis(100) {
                reconcile(shared, package, transport, &mut state)?;
                last_sync = Instant::now();
            }
            thread::sleep(Duration::from_millis(10));
        }
        // No input survives shutdown. Close acknowledges recovery before package cleanup may continue.
        for session in state.sessions.values_mut() {
            session.failed = true;
            shared.connection(
                &session.device,
                session.generation,
                ConnectionState::Offline,
                None,
            );
        }
        // Stop the transient transport before close. A close ACK alone is not a frame release ACK.
        stop_key_lighting(shared, package, transport, &mut state)?;
        for session in state.sessions.keys().cloned().collect::<Vec<_>>() {
            if state.pending.values().any(|pending| matches!(&pending.operation,Operation::Close(existing) if existing == &session)) { continue; }
            state.request(
                transport,
                "close",
                Operation::Close(session.clone()),
                Some(&session),
                json!({}),
            )?;
        }
        let deadline = Instant::now() + TIMEOUT;
        while !state.sessions.is_empty() {
            if Instant::now() >= deadline {
                return Err("Provider close timed out; physical recovery is unconfirmed".into());
            }
            match transport.read.recv_timeout(Duration::from_millis(20)) {
                Ok(PipeEvent::Message(value)) => {
                    handle(shared, package, transport, &mut state, value)?
                }
                Ok(PipeEvent::Failed(error)) => return Err(error),
                Ok(PipeEvent::Eof) => {
                    return Err("Provider exited before close acknowledgement".into());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err("Provider pipe closed before close acknowledgement".into()),
            }
            if !shared.stop.load(Ordering::Relaxed)
                && shared
                    .recovery_packages
                    .lock()
                    .is_ok_and(|packages| packages.contains(&package.manifest.id))
            {
                stop.store(false, Ordering::Relaxed);
                continue 'service;
            }
            if shared.stop.load(Ordering::Relaxed)
                && shared
                    .recovery_packages
                    .lock()
                    .is_ok_and(|packages| packages.contains(&package.manifest.id))
            {
                return Err(
                    "Application stopped; durable physical recovery remains pending".into(),
                );
            }
        }
        return Ok(());
    }
}

fn stop_key_lighting(
    shared: &Shared,
    package: &Package,
    transport: &Transport,
    state: &mut ProcessState,
) -> Result<(), String> {
    if let Ok(mut controls) = shared.controls.lock() {
        controls.retain(|request| {
            if request.generation.is_some()
                && state.sessions.values().any(|s| s.device == request.device)
            {
                let _ = request.reply.send(Err(
                    "Provider stopped; key lighting request discarded".into()
                ));
                false
            } else {
                true
            }
        });
    }
    let mut released = BTreeSet::new();
    let deadline = Instant::now() + TIMEOUT;
    loop {
        for (id, session) in &state.sessions {
            if released.contains(id) || state.pending.values().any(|p| matches!(&p.operation, Operation::KeyLighting(existing, ..) if existing == id)) { continue; }
            let lease = shared.key_lighting.lock().ok().and_then(|devices| {
                devices
                    .get(&session.device)
                    .filter(|d| d.generation == session.generation)
                    .and_then(|d| d.active.as_ref())
                    .map(|a| a.id.clone())
            });
            let Some(lease) = lease else {
                released.insert(id.clone());
                continue;
            };
            // Defer mutation until the immutable session iteration ends.
            let (reply, _) = mpsc::channel();
            let id = id.clone();
            state.request(
                transport,
                "releaseKeyLighting",
                Operation::KeyLighting(id.clone(), lease.clone(), None, reply),
                Some(&id),
                json!({"leaseId":lease}),
            )?;
            released.insert(id);
            break;
        }
        if !state
            .pending
            .values()
            .any(|p| matches!(p.operation, Operation::KeyLighting(..)))
            && state.sessions.keys().all(|id| released.contains(id))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(
                "Key lighting release timed out on stop; RAM restoration is unconfirmed".into(),
            );
        }
        match transport.read.recv_timeout(Duration::from_millis(10)) {
            Ok(PipeEvent::Message(value)) => handle(shared, package, transport, state, value)?,
            Ok(PipeEvent::Failed(error)) => return Err(error),
            Ok(PipeEvent::Eof) => {
                return Err("Provider exited before key lighting release acknowledgement".into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => {
                return Err(
                    "Provider pipe closed before key lighting release acknowledgement".into(),
                );
            }
        }
    }
}
fn reconcile_key_lighting(
    shared: &Shared,
    transport: &Transport,
    state: &mut ProcessState,
    epochs: &BTreeMap<String, u64>,
    recovery_only: bool,
) -> Result<(), String> {
    let mut controls = shared
        .controls
        .lock()
        .map_err(|_| "Provider queue unavailable")?;
    let mut devices = shared
        .key_lighting
        .lock()
        .map_err(|_| "Key lighting unavailable")?;
    for (device, output) in devices.iter_mut() {
        let Some(id) = state
            .sessions
            .iter()
            .find(|(_, s)| s.device == *device && s.generation == output.generation)
            .map(|(id, _)| id.clone())
        else {
            continue;
        };
        let session = &state.sessions[&id];
        let current = epochs.get(device) == Some(&session.lifecycle_epoch);
        let unavailable = !current
            || recovery_only
            || !session.opened
            || session.failed
            || session.recovery["state"] == "pending";
        // Cancel unsent frames immediately; a fresh session must never replay them.
        controls.retain(|request| {
            if request.device == *device
                && request.generation.is_some()
                && matches!(request.kind, "keyLighting" | "releaseKeyLighting")
                && (request.generation != Some(session.generation)
                    || request.payload["leaseId"].as_str()
                        != output.active.as_ref().map(|active| active.id.as_str())
                    || unavailable && request.kind == "keyLighting")
            {
                let _ = request.reply.send(Err(
                    "Key lighting session changed or is unavailable; frame discarded".into(),
                ));
                false
            } else {
                true
            }
        });
        if state.pending.values().any(
            |p| matches!(&p.operation, Operation::KeyLighting(existing, ..) if existing == &id),
        ) {
            continue;
        }
        // Release RAM ownership on expiry, disable/removal, and before ordinary settings.
        let Some(active) = &mut output.active else {
            continue;
        };
        let ordinary_edit = shared.edits.lock().is_ok_and(|edits| {
            edits
                .get(device)
                .is_some_and(|edit| !edit.fields.is_empty())
        });
        if (unavailable || ordinary_edit || active.expires <= Instant::now()) && !active.releasing {
            active.releasing = true;
            controls.retain(|request| {
                if request.device == *device && request.kind == "keyLighting" {
                    let _ = request
                        .reply
                        .send(Err("Key lighting frame cancelled before release".into()));
                    false
                } else {
                    true
                }
            });
            let (reply, _) = mpsc::channel();
            // Internal cleanup does not consume the bounded caller queue.
            state.request(
                transport,
                "releaseKeyLighting",
                Operation::KeyLighting(id.clone(), active.id.clone(), None, reply),
                Some(&id),
                json!({"leaseId":active.id}),
            )?;
            continue;
        }
        let pending_settings = state
            .pending
            .values()
            .any(|p| matches!(&p.operation, Operation::Settings(existing, ..) | Operation::Keyboard(existing, ..) if existing == &id));
        let Some(index) = controls.iter().position(|request| {
            request.device == *device
                && request.generation == Some(session.generation)
                && (request.kind == "releaseKeyLighting"
                    || !unavailable
                        && !pending_settings
                        && active.last_sent.is_none_or(|sent| {
                            sent.elapsed()
                                >= Duration::from_millis(
                                    1000 / session
                                        .definition
                                        .pointer("/keyLighting/maxRateHz")
                                        .and_then(Value::as_u64)
                                        .unwrap_or(10)
                                        .clamp(1, 10),
                                )
                        }))
        }) else {
            continue;
        };
        let request = controls.remove(index).unwrap();
        let sequence = request.payload["sequence"].as_u64();
        if sequence.is_some() {
            active.last_sent = Some(Instant::now());
        }
        let reply = request.reply.clone();
        if let Err(error) = state.request(
            transport,
            request.kind,
            Operation::KeyLighting(id.clone(), active.id.clone(), sequence, request.reply),
            Some(&id),
            request.payload,
        ) {
            let _ = reply.send(Err(error.clone()));
            return Err(error);
        }
    }
    Ok(())
}
fn reconcile(
    shared: &Shared,
    package: &Package,
    transport: &Transport,
    state: &mut ProcessState,
) -> Result<(), String> {
    let (snapshot, epochs) = {
        let core = shared.core.lock().map_err(|_| "Workspace unavailable")?;
        let snapshot = core.snapshot();
        let epochs = snapshot
            .configuration
            .devices
            .iter()
            .map(|device| (device.id.clone(), core.device_lifecycle_epoch(&device.id)))
            .collect::<BTreeMap<_, _>>();
        (snapshot, epochs)
    };
    // Alias RPCs are explicit, generation-bound operations. Do not feed them
    // through persistent provider settings (which are a different lifecycle).
    loop {
        let request = {
            let mut controls = shared
                .controls
                .lock()
                .map_err(|_| "Keyboard queue unavailable")?;
            controls
                .iter()
                .position(|request| {
                    matches!(
                        request.kind,
                        "installKeyboardAlias"
                            | "restoreKeyboardAlias"
                            | "reconcileKeyboardAlias"
                            | "configureKeyboardInput"
                    ) && state.sessions.iter().any(|(id, session)| {
                        session.device == request.device
                            && !state.pending.values().any(|p| match &p.operation {
                                Operation::Read(s, _)
                                | Operation::Settings(s, ..)
                                | Operation::Keyboard(s, ..)
                                | Operation::Recovery(s, ..) => s == id,
                                _ => false,
                            })
                    })
                })
                .and_then(|i| controls.remove(i))
        };
        let Some(request) = request else {
            break;
        };
        let Some((id, session)) = state
            .sessions
            .iter()
            .find(|(_, s)| s.device == request.device)
        else {
            continue;
        };
        if !session.opened
            || session.failed && request.kind != "reconcileKeyboardAlias"
            || request.generation != Some(session.generation)
            || epochs.get(&session.device) != Some(&session.lifecycle_epoch)
            || session
                .definition
                .pointer("/keyboard/aliases/version")
                .and_then(Value::as_u64)
                != Some(1)
            || session.recovery["state"] == "pending" && request.kind != "reconcileKeyboardAlias"
            || !shared
                .lifecycle
                .allows(&format!("device:{}", package.manifest.id))
        {
            let _ = request.reply.send(Err(
                "Keyboard source is unavailable, unsupported or has pending recovery".into(),
            ));
            continue;
        }
        let id = id.clone();
        state.request(
            transport,
            request.kind,
            Operation::Keyboard(id.clone(), request.kind, request.reply),
            Some(&id),
            request.payload,
        )?;
    }
    loop {
        let request = {
            let mut controls = shared
                .controls
                .lock()
                .map_err(|_| "Recovery queue unavailable")?;
            controls
                .iter()
                .position(|request| {
                    request.generation.is_none()
                        && state.sessions.values().any(|session| {
                            session.device == request.device
                                && epochs.get(&session.device) == Some(&session.lifecycle_epoch)
                        })
                })
                .and_then(|index| controls.remove(index))
        };
        let Some(request) = request else {
            break;
        };
        let session_id = state
            .sessions
            .iter()
            .find(|(_, session)| {
                session.device == request.device
                    && epochs.get(&session.device) == Some(&session.lifecycle_epoch)
            })
            .map(|(id, _)| id.clone())
            .unwrap();
        let session = &state.sessions[&session_id];
        if !session.opened
            || !session
                .definition
                .pointer("/recovery/restore")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            let _ = request
                .reply
                .send(Err("Provider has no recovery session".into()));
            continue;
        }
        let restore = request.kind == "recover";
        if restore
            && let Err(error) =
                authorize_resolution(session.recovery_preview.as_ref(), &request.payload)
        {
            let _ = request.reply.send(Err(error));
            continue;
        }
        state
            .sessions
            .get_mut(&session_id)
            .unwrap()
            .recovery_preview = None;
        state.request(
            transport,
            request.kind,
            Operation::Recovery(
                session_id.clone(),
                request.reply,
                restore,
                request.payload["resolution"] == "keepCurrent",
            ),
            Some(&session_id),
            request.payload,
        )?;
    }
    let recovery_only = !shared
        .lifecycle
        .allows(&format!("device:{}", package.manifest.id))
        || shared
            .recovery_packages
            .lock()
            .is_ok_and(|packages| packages.contains(&package.manifest.id));
    reconcile_key_lighting(shared, transport, state, &epochs, recovery_only)?;
    for id in state.sessions.keys().cloned().collect::<Vec<_>>() {
        let session = &state.sessions[&id];
        let gone = !snapshot
            .configuration
            .devices
            .iter()
            .any(|device| device.id == session.device)
            || epochs.get(&session.device) != Some(&session.lifecycle_epoch)
            || !shared.inventory.lock().is_ok_and(|inventory| {
                inventory.discovered.iter().any(|entry| {
                    entry.provider_id == package.manifest.id
                        && entry.device_id.as_deref() == Some(&session.device)
                })
            });
        let pending_recovery =
            session.recovery.get("state").and_then(Value::as_str) == Some("pending");
        let released = session.released;
        let transient_active = shared.key_lighting.lock().is_ok_and(|devices| {
            devices
                .get(&session.device)
                .is_some_and(|d| d.generation == session.generation && d.active.is_some())
        });
        if (pending_recovery || session.failed || gone || recovery_only)
            && epochs.get(&session.device) == Some(&session.lifecycle_epoch)
            && let Ok(mut observations) = shared.observations.lock()
            && let Some(observation) = observations.get_mut(&session.device)
        {
            observation.leases.clear();
            observation.samples.clear();
        }
        if session.observation_needs_stop && (pending_recovery || session.failed || gone || recovery_only)
            && !state.pending.values().any(|request| matches!(&request.operation, Operation::Observe(existing, _) if existing == &id))
            && shared.observations.lock().is_ok_and(|observations| observations.get(&session.device).is_some_and(|observation| observation.error.is_none())) {
            state.request(transport, "observe", Operation::Observe(id.clone(), false), Some(&id), json!({"enabled":false}))?;
        }
        if pending_recovery && (!gone || released) {
            continue;
        }
        if gone && transient_active {
            // Rebinding/removal must finish its transient release before closing this generation.
            continue;
        }
        if gone && !state.pending.values().any(
            |request| matches!(&request.operation, Operation::Close(existing) if existing == &id),
        ) {
            // Ignore old events immediately, before the cooperative close acknowledgement.
            state.sessions.get_mut(&id).unwrap().failed = true;
            state.request(
                transport,
                "close",
                Operation::Close(id.clone()),
                Some(&id),
                json!({}),
            )?;
        }
    }
    for device in snapshot
        .configuration
        .devices
        .iter()
        .filter(|device| device.provider_id == package.manifest.id)
    {
        if let Some((id, opened, failed, previous_edit)) = state
            .sessions
            .iter()
            .find(|(_, session)| {
                session.device == device.id
                    && epochs.get(&session.device) == Some(&session.lifecycle_epoch)
            })
            .map(|(id, session)| (id.clone(), session.opened, session.failed, session.edit))
        {
            let observe = if let Ok(mut observations) = shared.observations.lock() {
                observations.get_mut(&device.id).and_then(|observation| {
                    let desired = observation.desired() && !recovery_only && opened && !failed;
                    let session = &state.sessions[&id];
                    if observation.error.is_none()
                        && (desired != session.observing
                            || !desired && session.observation_needs_stop
                            || desired && session.last_observe.elapsed() >= Duration::from_secs(1))
                    {
                        Some(desired)
                    } else {
                        None
                    }
                })
            } else {
                None
            };
            if let Some(enabled) = observe
                && !state.pending.values().any(|request| matches!(&request.operation, Operation::Observe(existing, _) if existing == &id)) {
                state.sessions.get_mut(&id).unwrap().last_observe = Instant::now();
                if enabled {
                    state.sessions.get_mut(&id).unwrap().observation_needs_stop = true;
                    if let Ok(mut observations) = shared.observations.lock()
                        && let Some(observation) = observations.get_mut(&device.id) { observation.needs_stop = true; }
                }
                state.request(transport, "observe", Operation::Observe(id.clone(), enabled), Some(&id), json!({"enabled":enabled}))?;
            }
            let (edit, changes) = shared
                .edits
                .lock()
                .ok()
                .and_then(|edits| {
                    edits.get(&device.id).map(|edit| {
                        (
                            edit.revision,
                            edit.fields
                                .iter()
                                .filter(|(_, (revision, _))| *revision > previous_edit)
                                .map(|(field, (_, value))| (field.clone(), value.clone()))
                                .collect::<BTreeMap<_, _>>(),
                        )
                    })
                })
                .unwrap_or_default();
            let transient_active = shared.key_lighting.lock().is_ok_and(|devices| {
                devices.get(&device.id).is_some_and(|d| {
                    d.generation == state.sessions[&id].generation && d.active.is_some()
                })
            });
            if !transient_active && !recovery_only && opened && !failed && previous_edit != edit && !changes.is_empty() && !state.pending.values().any(|request| matches!(&request.operation, Operation::Settings(existing, _, _) | Operation::Keyboard(existing, ..) if existing == &id)) {
                let id = id.clone();
                state.request(transport, "setSettings", Operation::Settings(id.clone(), changes.clone(), edit), Some(&id), json!({"changes":changes}))?;
                if let Ok(mut queued) = shared.edits.lock()
                    && let Some(queued) = queued.get_mut(&device.id) { queued.fields.retain(|_,(revision,_)| *revision > edit); }
            }
            // Do not start a read deadline while it is queued behind a slow hardware write.
            let refresh = opened && !state.pending.values().any(|request| matches!(&request.operation, Operation::Settings(existing, _, _) | Operation::Keyboard(existing, ..) | Operation::Read(existing, false) if existing == &id))
                && shared.refresh.lock().is_ok_and(|mut refresh| refresh.remove(&device.id));
            if opened && refresh && !state.pending.values().any(|request| matches!(&request.operation, Operation::Read(existing, _) if existing == &id)) {
                let id = id.clone();
                state.request(transport, "readState", Operation::Read(id.clone(), true), Some(&id), json!({}))?;
            }
            continue;
        }
        let Some(instance) = &device.hardware_key else {
            continue;
        };
        if state.sessions.values().any(|session| {
            session.device == device.id
                && session.lifecycle_epoch != epochs[&device.id]
                && !session.released
        }) {
            continue;
        }
        let Some(descriptor) = device.provider_descriptor.as_ref() else {
            continue;
        };
        let discovered = shared.inventory.lock().is_ok_and(|inventory| {
            inventory.discovered.iter().any(|entry| {
                entry.provider_id == package.manifest.id
                    && entry.instance_id == *instance
                    && entry.error.is_none()
            })
        });
        if !discovered {
            continue;
        }
        // Session-only identities cannot silently reattach across process lifetimes. This version requires explicit pairing each launch.
        let paired = shared.inventory.lock().is_ok_and(|inventory| {
            inventory
                .discovered
                .iter()
                .any(|entry| entry.device_id.as_deref() == Some(&device.id))
        });
        if !paired {
            continue;
        }
        let generation = shared.next_generation.fetch_add(1, Ordering::Relaxed) + 1;
        if let Ok(mut observations) = shared.observations.lock() {
            observations.remove(&device.id);
        }
        let id = format!("device-{}-{}", device.id, generation);
        state.sessions.insert(
            id.clone(),
            Session {
                device: device.id.clone(),
                generation,
                lifecycle_epoch: epochs[&device.id],
                sequence: 0,
                definition: descriptor.definition.clone(),
                opened: false,
                failed: recovery_only,
                edit: shared
                    .edits
                    .lock()
                    .ok()
                    .and_then(|edits| edits.get(&device.id).map(|edit| edit.revision))
                    .unwrap_or(0),
                recovery: Value::Null,
                released: false,
                observing: false,
                observation_needs_stop: false,
                last_observe: Instant::now(),
                depth_sequence: 0,
                keyboard_sequence: 0,
                read_sequence: 0,
                recovery_preview: None,
            },
        );
        shared.connection(&device.id, generation, ConnectionState::Offline, None);
        state.request(
            transport,
            "open",
            Operation::Open(id.clone()),
            Some(&id),
            json!({"instanceId":instance}),
        )?;
    }
    Ok(())
}

fn update_recovery(session: &mut Session, value: &Value) {
    if let Some(recovery) = value
        .get("recoveryState")
        .or_else(|| value.get("recovery"))
        .filter(|value| value.is_object())
    {
        session.recovery = recovery.clone();
        session.failed = recovery.get("state").and_then(Value::as_str) == Some("pending");
    }
}
fn status(
    shared: &Shared,
    session: &Session,
    value: &Value,
    requested: Option<&BTreeMap<String, Value>>,
    error: Option<String>,
) {
    if !shared.core.lock().is_ok_and(|core| {
        core.has_device(&session.device)
            && core.device_lifecycle_epoch(&session.device) == session.lifecycle_epoch
    }) {
        return;
    }
    if let Ok(mut inventory) = shared.inventory.lock() {
        if error.is_some()
            && session.recovery.get("state").and_then(Value::as_str) != Some("pending")
        {
            for entry in inventory
                .discovered
                .iter_mut()
                .filter(|entry| entry.device_id.as_deref() == Some(&session.device))
            {
                entry.device_id = None;
                entry.state = "pairing_required".into();
            }
        }
        let old = inventory.sessions.get(&session.device);
        let observed = value
            .get("observed")
            .cloned()
            .or_else(|| old.map(|old| old.observed.clone()))
            .unwrap_or_else(|| json!({}));
        let evidence = value.get("evidence").cloned().unwrap_or_else(
            || json!({"transport":"unknown","readback":"unknown","physical":"unknown"}),
        );
        let mut requested_state: BTreeMap<String, Value> = old
            .and_then(|old| serde_json::from_value(old.requested.clone()).ok())
            .unwrap_or_default();
        if let Some(requested) = requested {
            deckweave_core::merge_provider_patch(&mut requested_state, requested.clone());
        }
        // A successful GET cannot erase the failed write that still needs explicit recovery.
        let error = error.or_else(|| {
            (session.failed || session.recovery["state"].as_str() == Some("pending"))
                .then(|| old.and_then(|old| old.error.clone()))
                .flatten()
        });
        inventory.sessions.insert(
            session.device.clone(),
            SessionStatus {
                generation: session.generation,
                settings_edit: session.edit,
                read_sequence: session.read_sequence,
                state:
                    if session.recovery.get("state").and_then(Value::as_str) == Some("pending") {
                        "recovery_pending"
                    } else if error.is_some() || session.failed {
                        "error"
                    } else if session.opened {
                        "connected"
                    } else {
                        "opening"
                    }
                    .into(),
                observed,
                requested: json!(requested_state),
                evidence,
                error,
                recovery: session.recovery.clone(),
            },
        );
    }
    shared.notify(false);
}
fn handle(
    shared: &Shared,
    package: &Package,
    transport: &Transport,
    state: &mut ProcessState,
    value: Value,
) -> Result<(), String> {
    match value.get("type").and_then(Value::as_str) {
        Some("result") => {
            let id = value
                .get("requestId")
                .and_then(Value::as_str)
                .ok_or("Provider reply has no request ID")?;
            let Some(pending) = state.pending.get(id) else {
                return Ok(());
            };
            let session_id = match &pending.operation {
                Operation::Open(id)
                | Operation::Read(id, _)
                | Operation::Settings(id, _, _)
                | Operation::Close(id)
                | Operation::Observe(id, _) => Some(id.clone()),
                Operation::Recovery(id, _, _, _) => Some(id.clone()),
                Operation::Keyboard(id, ..) => Some(id.clone()),
                Operation::KeyLighting(id, ..) => Some(id.clone()),
                _ => None,
            };
            if let Some(id) = session_id.as_deref() {
                if value
                    .get("generation")
                    .and_then(Value::as_u64)
                    .zip(pending.generation)
                    .is_some_and(|(actual, expected)| actual < expected)
                {
                    return Ok(());
                }
                if value.get("session").and_then(Value::as_str) != Some(id)
                    || value.get("generation").and_then(Value::as_u64) != pending.generation
                {
                    return Err("Provider reply has mismatched session generation".into());
                }
                let Some(session) = state.sessions.get(id) else {
                    return Ok(());
                };
                if !matches!(
                    pending.operation,
                    Operation::Close(_) | Operation::KeyLighting(_, _, None, _)
                ) && !shared.core.lock().is_ok_and(|core| {
                    core.has_device(&session.device)
                        && core.device_lifecycle_epoch(&session.device) == session.lifecycle_epoch
                }) {
                    if let Operation::Recovery(_, reply, _, _) = &pending.operation {
                        let _ = reply.send(Err(
                            "Device was rebound; request a fresh recovery preview".into(),
                        ));
                    }
                    if let Operation::Keyboard(_, _, reply) = &pending.operation {
                        let _ = reply.send(Err(
                            "Device was rebound; keyboard operation acknowledgement discarded"
                                .into(),
                        ));
                    }
                    if let Operation::KeyLighting(_, _, _, reply) = &pending.operation {
                        let _ = reply
                            .send(Err("Device was rebound; key lighting ACK discarded".into()));
                    }
                    state.pending.remove(value["requestId"].as_str().unwrap());
                    return Ok(());
                }
                if pending.generation != Some(session.generation) {
                    return Ok(());
                }
                if session.failed
                    && !matches!(
                        pending.operation,
                        Operation::Close(_)
                            | Operation::Recovery(..)
                            | Operation::Read(..)
                            | Operation::Open(_)
                            | Operation::Observe(..)
                            | Operation::KeyLighting(..)
                            | Operation::Keyboard(..)
                    )
                {
                    state.pending.remove(value["requestId"].as_str().unwrap());
                    return Ok(());
                }
            }
            let pending = state
                .pending
                .remove(value["requestId"].as_str().unwrap())
                .unwrap();
            if value.get("ok").and_then(Value::as_bool) != Some(true) {
                let message = value
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("Device provider operation failed")
                    .chars()
                    .take(300)
                    .collect::<String>();
                if let Operation::Recovery(_, reply, _, _) = &pending.operation {
                    let _ = reply.send(Err(message));
                    return Ok(());
                }
                if let Operation::Keyboard(id, kind, reply) = &pending.operation {
                    if let Some(session) = state.sessions.get_mut(id) {
                        if let Ok(mut inputs) = shared.keyboard_input.lock()
                            && let Some(input) = inputs.get_mut(&session.device)
                        {
                            input.ready = false;
                            input.error = Some(message.clone());
                        }
                        if *kind != "configureKeyboardInput"
                            && matches!(
                                value.pointer("/error/code").and_then(Value::as_str),
                                Some(
                                    "write_uncertain"
                                        | "readback_mismatch"
                                        | "recovery_pending"
                                        | "recovery_required"
                                )
                            )
                        {
                            session.failed = true;
                            session.recovery = json!({"state":"pending"});
                            status(shared, session, &json!({}), None, Some(message.clone()));
                        }
                    }
                    let code = value
                        .pointer("/error/code")
                        .and_then(Value::as_str)
                        .unwrap_or("keyboard");
                    let _ = reply.send(Err(format!("{code}: {message}")));
                    return Ok(());
                }
                if let Operation::KeyLighting(id, _, _, reply) = &pending.operation {
                    if let Some(session) = state.sessions.get_mut(id) {
                        if value.pointer("/error/code").and_then(Value::as_str)
                            == Some("key_lighting_baseline")
                            && matches!(
                                &pending.operation,
                                Operation::KeyLighting(_, _, Some(_), _)
                            )
                            && let Ok(mut devices) = shared.key_lighting.lock()
                            && let Some(output) = devices.get_mut(&session.device)
                            && output.generation == session.generation
                            && output
                                .active
                                .as_ref()
                                .is_some_and(|active| !active.accepted)
                            && let Some(active) = output.active.take()
                        {
                            // Native contract: this error occurs before journaling/RAM output.
                            // Releasing a nonexistent hardware lease would fabricate recovery failure.
                            output.retired.insert(active.id);
                            let _ = reply.send(Err(message));
                            return Ok(());
                        }
                        if let Ok(mut lighting) = shared.key_lighting.lock()
                            && let Some(active) = lighting
                                .get_mut(&session.device)
                                .and_then(|d| d.active.as_mut())
                        {
                            active.expires = Instant::now();
                            active.releasing =
                                matches!(&pending.operation, Operation::KeyLighting(_, _, None, _));
                        }
                        if matches!(&pending.operation, Operation::KeyLighting(_, _, None, _))
                            || matches!(
                                value.pointer("/error/code").and_then(Value::as_str),
                                Some(
                                    "write_uncertain"
                                        | "readback_mismatch"
                                        | "recovery_pending"
                                        | "recovery_required"
                                )
                            )
                        {
                            session.failed = true;
                            session.recovery = json!({"state":"pending"});
                            status(shared, session, &json!({}), None, Some(message.clone()));
                        }
                    }
                    let _ = reply.send(Err(message));
                    return Ok(());
                }
                if let Operation::Observe(id, _) = &pending.operation {
                    if let Some(session) = state.sessions.get(id)
                        && let Ok(mut observations) = shared.observations.lock()
                        && let Some(observation) = observations.get_mut(&session.device)
                    {
                        observation.error = Some(message);
                        observation.samples.clear();
                    }
                    return Ok(());
                }
                if matches!(pending.operation, Operation::Close(_)) {
                    return Err(format!(
                        "Provider close failed; package retained: {message}"
                    ));
                }
                if let Some(id) = session_id {
                    let session = state.sessions.get_mut(&id).unwrap();
                    session.failed = true;
                    if matches!(
                        value.pointer("/error/code").and_then(Value::as_str),
                        Some(
                            "write_uncertain"
                                | "readback_mismatch"
                                | "recovery_pending"
                                | "recovery_required"
                        )
                    ) {
                        session.recovery = json!({"state":"pending"});
                    }
                    shared.connection(
                        &session.device,
                        session.generation,
                        ConnectionState::Error,
                        Some(message.clone()),
                    );
                    let requested = match &pending.operation {
                        Operation::Settings(_, requested, _) => Some(requested),
                        _ => None,
                    };
                    status(shared, session, &json!({}), requested, Some(message));
                    if matches!(pending.operation, Operation::Open(_)) {
                        state.sessions.remove(&id);
                    }
                    return Ok(());
                }
                return Err(message);
            }
            let response = value.get("value").cloned().unwrap_or(Value::Null);
            match pending.operation {
                Operation::Hello => {
                    if response.get("protocol").and_then(Value::as_u64) != Some(1)
                        || response.get("providerId").and_then(Value::as_str)
                            != Some(&package.manifest.id)
                    {
                        return Err("Provider handshake mismatch".into());
                    }
                    state.hello = true;
                    state.request(transport, "discover", Operation::Discover, None, json!({}))?;
                }
                Operation::Discover => {
                    let devices = response
                        .get("devices")
                        .and_then(Value::as_array)
                        .filter(|devices| devices.len() <= 32)
                        .ok_or("Invalid discovery result")?;
                    let mut found = vec![];
                    for descriptor in devices {
                        descriptor_geometry(descriptor)?;
                        let instance = descriptor["instanceId"].as_str().unwrap();
                        if found
                            .iter()
                            .any(|entry: &DiscoveredDevice| entry.instance_id == instance)
                        {
                            return Err("Duplicate provider instance identity".into());
                        }
                        let previous = shared.inventory.lock().ok().and_then(|inventory| {
                            inventory
                                .discovered
                                .iter()
                                .find(|entry| {
                                    entry.provider_id == package.manifest.id
                                        && entry.instance_id == instance
                                })
                                .cloned()
                        });
                        let saved = shared.core.lock().is_ok_and(|core| {
                            core.snapshot().configuration.devices.iter().any(|device| {
                                device.provider_id == package.manifest.id
                                    && device.hardware_key.as_deref() == Some(instance)
                            })
                        });
                        let paired = previous
                            .as_ref()
                            .and_then(|entry| entry.device_id.as_ref())
                            .is_some();
                        found.push(DiscoveredDevice {
                            provider_id: package.manifest.id.clone(),
                            instance_id: instance.into(),
                            name: descriptor
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or(&package.manifest.name)
                                .chars()
                                .take(200)
                                .collect(),
                            descriptor: descriptor.clone(),
                            device_id: previous.and_then(|entry| entry.device_id),
                            state: if saved && !paired {
                                "pairing_required"
                            } else {
                                "discovered"
                            }
                            .into(),
                            error: None,
                        });
                    }
                    if let Ok(mut inventory) = shared.inventory.lock() {
                        inventory
                            .discovered
                            .retain(|entry| entry.provider_id != package.manifest.id);
                        inventory.discovered.extend(found);
                    }
                    shared.notify(false);
                }
                Operation::Open(id) => {
                    let other_pending = state.sessions.iter().any(|(existing, session)| {
                        existing != &id
                            && session.recovery.get("state").and_then(Value::as_str)
                                == Some("pending")
                    });
                    let session = state.sessions.get_mut(&id).unwrap();
                    if response.get("descriptor") != Some(&session.definition) {
                        return Err("Provider descriptor changed while opening".into());
                    }
                    session.opened = true;
                    update_recovery(session, response.get("state").unwrap_or(&Value::Null));
                    if !shared
                        .lifecycle
                        .allows(&format!("device:{}", package.manifest.id))
                    {
                        session.failed = true;
                    }
                    if !other_pending
                        && session.recovery.get("state").and_then(Value::as_str)
                            == Some("not_needed")
                        && let Ok(mut packages) = shared.recovery_packages.lock()
                    {
                        packages.remove(&package.manifest.id);
                    }
                    shared.connection(
                        &session.device,
                        session.generation,
                        if session.failed {
                            ConnectionState::Error
                        } else {
                            ConnectionState::Connected
                        },
                        None,
                    );
                    status(
                        shared,
                        session,
                        response.get("state").unwrap_or(&Value::Null),
                        None,
                        None,
                    );
                    // Open already includes the provider's initial state. A second full
                    // read here doubles USB traffic without adding a requested observation.
                    if !response
                        .pointer("/state/observed")
                        .is_some_and(Value::is_object)
                    {
                        state.request(
                            transport,
                            "readState",
                            Operation::Read(id.clone(), false),
                            Some(&id),
                            json!({}),
                        )?;
                    }
                }
                Operation::Read(id, explicit) => {
                    let session = state.sessions.get_mut(&id).unwrap();
                    if explicit {
                        session.read_sequence += 1;
                    }
                    update_recovery(session, &response);
                    if !shared
                        .lifecycle
                        .allows(&format!("device:{}", package.manifest.id))
                    {
                        session.failed = true;
                    }
                    status(shared, session, &response, None, None);
                }
                Operation::Observe(id, enabled) => {
                    let session = state.sessions.get_mut(&id).unwrap();
                    if let Ok(mut observations) = shared.observations.lock()
                        && let Some(observation) = observations.get_mut(&session.device)
                    {
                        if response.get("observing").and_then(Value::as_bool) == Some(enabled) {
                            session.observing = enabled;
                            session.observation_needs_stop = enabled;
                            observation.active = enabled;
                            observation.needs_stop = enabled;
                            observation.error = None;
                            if !enabled {
                                observation.samples.clear();
                            }
                        } else {
                            observation.error =
                                Some("Provider did not confirm observation state".into());
                            observation.samples.clear();
                        }
                    }
                }
                Operation::Settings(id, requested, edit) => {
                    let session = state.sessions.get_mut(&id).unwrap();
                    update_recovery(session, &response);
                    session.edit = edit;
                    status(shared, session, &response, Some(&requested), None);
                }
                Operation::KeyLighting(id, lease, sequence, reply) => {
                    let session = state.sessions.get_mut(&id).unwrap();
                    let valid = match sequence {
                        Some(sequence) => {
                            response["accepted"] == true
                                && response["leaseId"] == lease
                                && response["sequence"].as_u64() == Some(sequence)
                        }
                        None => {
                            response["released"] == true && response["recovery"] == "not_needed"
                        }
                    };
                    if !valid {
                        session.failed = true;
                        session.recovery = json!({"state":"pending"});
                        status(
                            shared,
                            session,
                            &json!({}),
                            None,
                            Some(
                                "Key lighting ACK did not confirm queued frame or RAM release"
                                    .into(),
                            ),
                        );
                        let _ = reply.send(Err(
                            "Key lighting ACK is unconfirmed; explicit recovery may be required"
                                .into(),
                        ));
                    } else {
                        if sequence.is_some()
                            && let Ok(mut devices) = shared.key_lighting.lock()
                            && let Some(output) = devices.get_mut(&session.device)
                            && output.generation == session.generation
                            && let Some(active) = output.active.as_mut()
                        {
                            active.accepted = true;
                        }
                        if sequence.is_none()
                            && let Ok(mut devices) = shared.key_lighting.lock()
                            && let Some(output) = devices.get_mut(&session.device)
                            && output.generation == session.generation
                        {
                            output.retired.insert(lease);
                            output.active = None;
                        }
                        // ACKs never mutate persistent settings, observed colors, or physical evidence.
                        let _ = reply.send(Ok(response));
                    }
                }
                Operation::Close(id) => {
                    if let Some(session) = state.sessions.get(&id)
                        && let Ok(mut observations) = shared.observations.lock()
                    {
                        observations.remove(&session.device);
                    }
                    if response.get("recovery").and_then(Value::as_str) == Some("pending") {
                        let session = state.sessions.get_mut(&id).unwrap();
                        session.failed = true;
                        session.recovery = json!({"state":"pending"});
                        session.released = true;
                        if let Ok(mut packages) = shared.recovery_packages.lock() {
                            packages.insert(package.manifest.id.clone());
                        }
                        if let Ok(mut result) = shared.quiesced.lock() {
                            result.insert(package.manifest.id.clone(),Err("Physical recovery is pending; provider retained for explicit recovery".into()));
                        }
                        status(
                            shared,
                            session,
                            &json!({}),
                            None,
                            Some("Physical recovery is pending".into()),
                        );
                        return Ok(());
                    }
                    if !matches!(
                        response.get("recovery").and_then(Value::as_str),
                        Some("completed" | "not_needed")
                    ) {
                        return Err(
                            "Provider close did not confirm physical recovery; package retained"
                                .into(),
                        );
                    }
                    if let Some(session) = state.sessions.remove(&id) {
                        let (present, current_epoch) = shared
                            .core
                            .lock()
                            .map(|core| {
                                (
                                    core.has_device(&session.device),
                                    core.device_lifecycle_epoch(&session.device)
                                        == session.lifecycle_epoch,
                                )
                            })
                            .unwrap_or((true, false));
                        if (!present || current_epoch)
                            && let Ok(mut inventory) = shared.inventory.lock()
                        {
                            if let Ok(mut observations) = shared.observations.lock() {
                                observations.remove(&session.device);
                            }
                            inventory.sessions.remove(&session.device);
                            if !present {
                                for device in inventory.discovered.iter_mut().filter(|device| {
                                    device.device_id.as_deref() == Some(&session.device)
                                }) {
                                    device.device_id = None;
                                    device.state = "discovered".into();
                                }
                            }
                        }
                        shared.notify(false);
                    }
                }
                Operation::Keyboard(id, kind, reply) => {
                    let session = state.sessions.get_mut(&id).unwrap();
                    if kind == "configureKeyboardInput" {
                        let editing = shared
                            .keyboard_edits
                            .lock()
                            .map_or(true, |edits| edits.contains(&session.device));
                        if let Ok(mut inputs) = shared.keyboard_input.lock()
                            && let Some(input) = inputs.get_mut(&session.device)
                            && input.generation == session.generation
                        {
                            let valid =
                                response["configurationSequence"].as_u64() == Some(input.sequence);
                            input.ready = valid
                                && !editing
                                && response["sourceState"] == "available"
                                && response["aliases"].as_u64() == Some(input.routes.len() as u64);
                            input.error = (!input.ready && !input.routes.is_empty()).then(|| {
                                format!(
                                    "Keyboard input source: {}",
                                    response["sourceState"].as_str().unwrap_or("unconfirmed")
                                )
                            });
                        }
                    }
                    if let Some(observed) = response.get("state") {
                        update_recovery(session, observed);
                        if kind == "reconcileKeyboardAlias"
                            && session.recovery["state"] != "pending"
                        {
                            session.failed = false;
                            shared.connection(
                                &session.device,
                                session.generation,
                                ConnectionState::Connected,
                                None,
                            );
                        }
                        status(shared, session, observed, None, None);
                    }
                    let _ = reply.send(Ok(response));
                }
                Operation::Recovery(id, reply, restore, keep_current) => {
                    if keep_current
                        && (response["resolution"] != "keepCurrent"
                            || response["outcome"] != "accepted_verified_current"
                            || response["recovery"] != "completed")
                    {
                        let _ = reply.send(Err("Provider did not confirm keeping verified current settings; recovery remains unconfirmed".into()));
                        return Ok(());
                    }
                    if !restore {
                        state.sessions.get_mut(&id).unwrap().recovery_preview =
                            RecoveryAuthorization::from_preview(&response);
                    }
                    let device = state.sessions[&id].device.clone();
                    let completed = restore
                        && response.get("recovery").and_then(Value::as_str) == Some("completed")
                        && response
                            .pointer("/recoveryState/state")
                            .and_then(Value::as_str)
                            != Some("pending");
                    if completed {
                        if let Ok(mut devices) = shared.key_lighting.lock()
                            && let Some(output) = devices.get_mut(&device)
                            && let Some(active) = output.active.take()
                        {
                            output.retired.insert(active.id);
                        }
                        for (old_id, session) in state
                            .sessions
                            .iter_mut()
                            .filter(|(old_id, session)| *old_id != &id && session.device == device)
                        {
                            let _ = old_id;
                            session.recovery = json!({"state":"not_needed"});
                            session.released = true;
                        }
                    }
                    let other_pending = state.sessions.values().any(|session| {
                        session.device != device
                            && session.recovery.get("state").and_then(Value::as_str)
                                == Some("pending")
                    });
                    let session = state.sessions.get_mut(&id).unwrap();
                    if restore {
                        update_recovery(session, &response);
                        if response["kind"] == "keyLighting"
                            && response["outcome"] == "ram_restore_acknowledged"
                            && response["recovery"] == "completed"
                            && response
                                .pointer("/recoveryState/state")
                                .and_then(Value::as_str)
                                != Some("pending")
                        {
                            // RAM restoration has no fabricated full-settings readback.
                            // Its transport ACK resolves this journal without claiming physical verification.
                            session.recovery = json!({"state":"not_needed"});
                            session.failed = false;
                        }
                        if response.get("recovery").and_then(Value::as_str) == Some("completed")
                            && session.recovery.get("state").and_then(Value::as_str)
                                != Some("pending")
                        {
                            session.recovery = json!({"state":"not_needed"});
                            session.failed = false;
                            if !other_pending
                                && let Ok(mut packages) = shared.recovery_packages.lock()
                            {
                                packages.remove(&package.manifest.id);
                            }
                            if !other_pending && let Ok(mut result) = shared.quiesced.lock() {
                                result.remove(&package.manifest.id);
                            }
                            shared.connection(
                                &session.device,
                                session.generation,
                                if shared
                                    .lifecycle
                                    .allows(&format!("device:{}", package.manifest.id))
                                {
                                    ConnectionState::Connected
                                } else {
                                    ConnectionState::Offline
                                },
                                None,
                            );
                        }
                        if !shared
                            .lifecycle
                            .allows(&format!("device:{}", package.manifest.id))
                        {
                            session.failed = true;
                        }
                        status(shared, session, &response, None, None);
                    }
                    let _ = reply.send(Ok(response));
                }
            }
            Ok(())
        }
        Some("depth") => {
            depth(shared, state, &value);
            Ok(())
        }
        Some("keyboardInputState") => {
            if let Some(session) = event_session(state, &value)
                && let Ok(mut inputs) = shared.keyboard_input.lock()
                && let Some(input) = inputs.get_mut(&session.device)
                && input.generation == session.generation
                && value["configurationSequence"].as_u64() == Some(input.sequence)
            {
                input.ready = false;
                input.error = Some(
                    "Keyboard input source was lost; check the selected keyboard connection".into(),
                );
            }
            shared.notify(false);
            Ok(())
        }
        Some("keyboardAlias") => {
            if let Err(error) = keyboard_alias_input(shared, state, &value)
                && let Some(session) = event_session(state, &value)
                && let Ok(mut inputs) = shared.keyboard_input.lock()
                && let Some(input) = inputs.get_mut(&session.device)
            {
                input.ready = false;
                input.error = Some(error);
            }
            Ok(())
        }
        Some("input") => {
            if let Err(error) = input(shared, state, &value)
                && let Some(session) = event_session(state, &value)
            {
                session.failed = true;
                shared.connection(
                    &session.device,
                    session.generation,
                    ConnectionState::Error,
                    Some(error.clone()),
                );
                status(shared, session, &json!({}), None, Some(error));
            }
            Ok(())
        }
        Some("sessionError") => {
            if let Some(session) = event_session(state, &value) {
                session.failed = true;
                let error = "Device provider session failed".to_string();
                shared.connection(
                    &session.device,
                    session.generation,
                    ConnectionState::Error,
                    Some(error.clone()),
                );
                status(shared, session, &json!({}), None, Some(error));
            }
            Ok(())
        }
        _ => Err("Unsupported provider message".into()),
    }
}
// Independent from action input and configuration revisions. Missing samples mean unknown.
fn depth(shared: &Shared, state: &mut ProcessState, value: &Value) {
    let Some(session) = event_session(state, value) else {
        return;
    };
    if !session.observing
        || !shared.core.lock().is_ok_and(|core| {
            core.has_device(&session.device)
                && core.device_lifecycle_epoch(&session.device) == session.lifecycle_epoch
        })
    {
        return;
    }
    let Some(sequence) = value
        .get("sequence")
        .and_then(Value::as_u64)
        .filter(|sequence| *sequence > session.depth_sequence)
    else {
        return;
    };
    let Some(control) = value.get("control").and_then(Value::as_str) else {
        return;
    };
    if !session
        .definition
        .get("controls")
        .and_then(Value::as_array)
        .is_some_and(|controls| {
            controls.iter().any(|c| {
                c["id"].as_str() == Some(control)
                    && c.pointer("/telemetry/keyTravel").and_then(Value::as_bool) == Some(true)
            })
        })
    {
        return;
    }
    let Some(travel) = value
        .pointer("/value/travelUm")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 10_000)
    else {
        return;
    };
    let maximum = value
        .pointer("/value/maxTravelUm")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 10_000);
    if let Ok(mut observations) = shared.observations.lock()
        && let Some(observation) = observations.get_mut(&session.device)
        && observation.desired()
    {
        session.depth_sequence = sequence;
        observation.sequence = sequence;
        observation.samples.insert(
            control.into(),
            ReceivedTravel {
                travel,
                maximum,
                received: Instant::now(),
            },
        );
    }
}
fn event_session<'a>(state: &'a mut ProcessState, value: &Value) -> Option<&'a mut Session> {
    let session = state.sessions.get_mut(value.get("session")?.as_str()?)?;
    if !session.opened || session.failed || value.get("generation")?.as_u64()? != session.generation
    {
        return None;
    }
    Some(session)
}
fn keyboard_alias_input(
    shared: &Shared,
    state: &mut ProcessState,
    value: &Value,
) -> Result<(), String> {
    let Some(session) = event_session(state, value) else {
        return Ok(());
    };
    let sequence = value["sequence"]
        .as_u64()
        .ok_or("Keyboard input sequence missing")?;
    if sequence != session.keyboard_sequence + 1 {
        session.keyboard_sequence = session.keyboard_sequence.max(sequence);
        if let Ok(mut inputs) = shared.keyboard_input.lock()
            && let Some(input) = inputs.get_mut(&session.device)
        {
            input.ready = false;
            input.error =
                Some("Keyboard input sequence gap or replay; check the input connection".into());
        }
        return Err("Keyboard input sequence gap or replay; source stopped".into());
    }
    session.keyboard_sequence = sequence;
    let route = {
        let states = shared
            .keyboard_input
            .lock()
            .map_err(|_| "Keyboard input state unavailable")?;
        let Some(config) = states.get(&session.device) else {
            return Ok(());
        };
        if !config.ready
            || config.generation != session.generation
            || value["configurationSequence"].as_u64() != Some(config.sequence)
        {
            return Ok(());
        }
        config
            .routes
            .iter()
            .find(|route| {
                Some(route.control_id.as_str()) == value["control"].as_str()
                    && Some(route.layer.as_str()) == value["layer"].as_str()
                    && value["trigger"].as_u64() == Some(u64::from(route.alias))
            })
            .cloned()
    };
    let Some(route) = route else {
        return Ok(());
    };
    let changed = {
        let mut core = shared.core.lock().map_err(|_| "Workspace unavailable")?;
        if core.device_lifecycle_epoch(&session.device) != session.lifecycle_epoch {
            return Ok(());
        }
        core.activate_keyboard_alias(&route, session.generation)
            .map_err(|e| e.to_string())?
    };
    if changed {
        shared.notify(true);
    }
    Ok(())
}
fn input(shared: &Shared, state: &mut ProcessState, value: &Value) -> Result<(), String> {
    let Some(session) = event_session(state, value) else {
        return Ok(());
    };
    let sequence = value
        .get("sequence")
        .and_then(Value::as_u64)
        .ok_or("Input has no sequence")?;
    if sequence != session.sequence + 1 {
        return Err("Provider input sequence gap or replay".into());
    }
    session.sequence = sequence;
    let control = value
        .get("control")
        .and_then(Value::as_str)
        .ok_or("Missing input control")?;
    let event = value
        .get("event")
        .and_then(Value::as_str)
        .ok_or("Missing input event")?;
    let declaration = session
        .definition
        .get("controls")
        .and_then(Value::as_array)
        .and_then(|controls| {
            controls
                .iter()
                .find(|candidate| candidate.get("id").and_then(Value::as_str) == Some(control))
        })
        .ok_or("Undeclared input control")?;
    if !declaration
        .get("events")
        .and_then(Value::as_array)
        .is_some_and(|events| {
            events
                .iter()
                .any(|candidate| candidate.as_str() == Some(event))
        })
    {
        return Err("Undeclared input event".into());
    }
    let translated = match event {
        "down" => InputEvent::Down,
        "up" => InputEvent::Up,
        "click" => InputEvent::Click,
        "rotate" => {
            let ticks = value
                .get("value")
                .and_then(Value::as_i64)
                .filter(|ticks| *ticks != 0 && ticks.unsigned_abs() <= 100)
                .ok_or("Invalid dial ticks")?;
            InputEvent::Rotate {
                ticks: ticks as i32,
            }
        }
        _ => return Err("Provider input event is not supported by this host".into()),
    };
    let changed = {
        let mut core = shared.core.lock().map_err(|_| "Workspace unavailable")?;
        if core.device_lifecycle_epoch(&session.device) != session.lifecycle_epoch
            || !core.has_device(&session.device)
        {
            return Ok(());
        }
        core.hardware_input(
            &session.device,
            session.generation,
            Some(control),
            Some(translated),
            "Provider input".into(),
        )
        .map_err(|error| error.to_string())?
    };
    if changed {
        shared.notify(true);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lighting_host_fixture() -> (tempfile::TempDir, Arc<Host>, PathBuf) {
        let (directory, package) = fixture(
            r#"
import {createInterface} from 'node:readline';
import fs from 'node:fs';
const dir=process.env.DECKWEAVE_PROVIDER_STATE_DIR;
const descriptor={instanceId:'a',modelId:'dev.deckweave.fixture.model',name:'RAM fixture',identity:{confidence:'session',persistentId:null},geometry:{width:2,height:1},controls:[{id:'key-1',kind:'key',x:0,y:0,width:1,height:1,events:[]},{id:'key-2',kind:'key',x:1,y:0,width:1,height:1,events:[]}],settings:[],displays:[],recovery:{preview:true,restore:true},keyLighting:{experimental:true,maxKeys:14,maxRateHz:10,maxTtlMs:3000,baseline:{lightingMode:1,colorMode:0,minBrightness:2},physical:'unverified'}};
let pending=false;
const reply=(q,value,error)=>process.stdout.write(JSON.stringify({type:'result',requestId:q.requestId,...(q.session?{session:q.session,generation:q.generation}:{}),ok:!error,...(error?{error}:{value})})+'\n');
const state=()=>({observed:{},evidence:{transport:'accepted',readback:'unsupported',physical:'unverified'},recovery:{state:pending?'pending':'not_needed'}});
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})){
 const q=JSON.parse(line);fs.appendFileSync(dir+'/calls',JSON.stringify(q)+'\n');let value={};
 if(q.type==='hello')value={protocol:1,providerId:'dev.deckweave.fixture'};
 else if(q.type==='discover')value={devices:[descriptor]};
 else if(q.type==='open')value={descriptor,state:state()};
 else if(q.type==='readState')value=state();
 else if(q.type==='keyLighting'){
   if(q.sequence===77){setTimeout(()=>reply(q,null,{code:'key_lighting_baseline',message:'Static single colour baseline required; no RAM output started'}),250);continue;}
   if(q.sequence===99){pending=true;reply(q,null,{code:'write_uncertain',message:'RAM frame uncertainty'});continue;}
   if(q.sequence===1){setTimeout(()=>reply(q,{accepted:true,leaseId:q.leaseId,sequence:q.sequence,experimental:true}),450);continue;}
   value={accepted:true,leaseId:q.leaseId,sequence:q.sequence,experimental:true};
 }
 else if(q.type==='releaseKeyLighting')value={released:!pending,recovery:pending?'pending':'not_needed'};
 else if(q.type==='recoveryPreview')value={previewToken:'fresh-token',confirmedInstanceId:'a',expiresInMs:60000,requiresConfirmation:true,kind:'keyLighting'};
 else if(q.type==='recover'){pending=false;value={recovery:'completed',kind:'keyLighting',outcome:'ram_restore_acknowledged',evidence:{transport:'accepted',readback:'unsupported',physical:'unverified'}};}
 else if(q.type==='close')value={recovery:pending?'pending':'not_needed'};
 reply(q,value);
}
"#,
        );
        let root = directory.path().join("packages");
        let bundle = root.join("dev.deckweave.fixture.dwDevice");
        std::fs::create_dir_all(&bundle).unwrap();
        for file in ["provider.mjs", "deckweave.device.json"] {
            std::fs::copy(package.directory.join(file), bundle.join(file)).unwrap();
        }
        let state_root = directory.path().join("state");
        let host = Arc::new(headless_host(directory.path(), &root, &state_root));
        wait_until(|| host.snapshot().discovered.len() == 1);
        (
            directory,
            host,
            state_root.join("dev.deckweave.fixture/calls"),
        )
    }
    fn lighting_calls(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
    #[test]
    fn key_lighting_singleflight_coalesces_and_release_does_not_claim_rendered_state() {
        let (_directory, host, calls) = lighting_host_fixture();
        let id = host.add("dev.deckweave.fixture", "a").unwrap();
        wait_until(|| host.session(&id).is_some_and(|s| s.state == "connected"));
        let generation = host.session(&id).unwrap().generation;
        let colors = json!({"key-1":{"r":1,"g":2,"b":3}});
        let first = {
            let host = Arc::clone(&host);
            let id = id.clone();
            let colors = colors.clone();
            thread::spawn(move || {
                host.send_key_lighting(&id, generation, "lease-1", 1, 3000, colors)
            })
        };
        wait_until(|| {
            lighting_calls(&calls)
                .iter()
                .any(|q| q["type"] == "keyLighting" && q["sequence"] == 1)
        });
        let second = {
            let host = Arc::clone(&host);
            let id = id.clone();
            let colors = colors.clone();
            thread::spawn(move || {
                host.send_key_lighting(&id, generation, "lease-1", 2, 3000, colors)
            })
        };
        wait_until(|| {
            host.shared
                .controls
                .lock()
                .unwrap()
                .iter()
                .any(|q| q.payload["sequence"] == 2)
        });
        let ack = host
            .send_key_lighting(&id, generation, "lease-1", 3, 3000, colors.clone())
            .unwrap();
        assert!(first.join().unwrap().unwrap()["accepted"] == true);
        assert!(second.join().unwrap().unwrap_err().contains("superseded"));
        assert_eq!(ack["sequence"], 3);
        let frames: Vec<_> = lighting_calls(&calls)
            .into_iter()
            .filter(|q| q["type"] == "keyLighting")
            .collect();
        assert_eq!(
            frames
                .iter()
                .map(|q| q["sequence"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert!(
            host.send_key_lighting(&id, generation, "lease-1", 3, 3000, colors.clone())
                .is_err()
        );
        assert!(
            host.send_key_lighting(
                &id,
                generation,
                "lease-1",
                4,
                3000,
                json!({"key-2":{"r":1,"g":2,"b":3}})
            )
            .is_err()
        );
        assert!(
            host.send_key_lighting(&id, generation + 1, "stale", 1, 3000, colors.clone())
                .unwrap_err()
                .contains("generation")
        );
        assert_eq!(host.session(&id).unwrap().observed, json!({}));
        assert_eq!(
            host.session(&id).unwrap().evidence["physical"],
            "unverified"
        );
        assert_eq!(
            host.release_key_lighting(&id, generation, "lease-1")
                .unwrap()["recovery"],
            "not_needed"
        );
        assert!(
            host.send_key_lighting(&id, generation, "lease-1", 4, 3000, colors.clone())
                .unwrap_err()
                .contains("retired")
        );
        host.send_key_lighting(&id, generation, "lease-2", 5, 3000, colors)
            .unwrap();
        host.stop();
        let calls = lighting_calls(&calls);
        let release = calls
            .iter()
            .position(|q| q["type"] == "releaseKeyLighting" && q["leaseId"] == "lease-2")
            .unwrap();
        let close = calls.iter().position(|q| q["type"] == "close").unwrap();
        assert!(
            release < close,
            "Stop must release RAM ownership before close"
        );
    }
    #[test]
    fn key_lighting_uncertainty_blocks_frames_and_retains_explicit_recovery() {
        let (_directory, host, _calls) = lighting_host_fixture();
        let id = host.add("dev.deckweave.fixture", "a").unwrap();
        wait_until(|| host.session(&id).is_some_and(|s| s.state == "connected"));
        let generation = host.session(&id).unwrap().generation;
        let colors = json!({"key-1":{"r":0,"g":0,"b":255}});
        assert!(
            host.send_key_lighting(&id, generation, "uncertain", 99, 1000, colors.clone())
                .is_err()
        );
        wait_until(|| {
            host.session(&id)
                .is_some_and(|s| s.recovery["state"] == "pending")
        });
        assert!(
            host.send_key_lighting(&id, generation, "next", 100, 1000, colors.clone())
                .is_err()
        );
        assert!(
            host.release_key_lighting(&id, generation, "uncertain")
                .is_err()
        );
        host.recovery_preview(&id, None).unwrap();
        let result = host.recover(&id, "fresh-token", "a").unwrap();
        assert_eq!(result["outcome"], "ram_restore_acknowledged");
        assert_eq!(result["evidence"]["physical"], "unverified");
        host.send_key_lighting(&id, generation, "fresh", 101, 1000, colors)
            .unwrap();
        host.stop();
    }
    #[test]
    fn key_lighting_expiry_and_removed_instance_release_without_replay() {
        let (_directory, host, calls) = lighting_host_fixture();
        let id = host.add("dev.deckweave.fixture", "a").unwrap();
        wait_until(|| host.session(&id).is_some_and(|s| s.state == "connected"));
        let generation = host.session(&id).unwrap().generation;
        let colors = json!({"key-1":{"r":0,"g":100,"b":0}});
        host.send_key_lighting(&id, generation, "expires", 2, 200, colors.clone())
            .unwrap();
        wait_until(|| {
            lighting_calls(&calls)
                .iter()
                .any(|q| q["type"] == "releaseKeyLighting" && q["leaseId"] == "expires")
        });
        wait_until(|| {
            host.shared.key_lighting.lock().unwrap()[&id]
                .active
                .is_none()
        });
        host.send_key_lighting(&id, generation, "removed", 3, 3000, colors.clone())
            .unwrap();
        {
            let mut core = host.shared.core.lock().unwrap();
            let revision = core.revision();
            core.dispatch(
                deckweave_core::Command::RemoveDevice {
                    device_id: id.clone(),
                    expected_revision: revision,
                },
                "main",
            )
            .unwrap();
        }
        wait_until(|| host.session(&id).is_none());
        assert!(
            host.send_key_lighting(&id, generation, "stale", 4, 3000, colors)
                .is_err()
        );
        let log = lighting_calls(&calls);
        let release = log
            .iter()
            .position(|q| q["type"] == "releaseKeyLighting" && q["leaseId"] == "removed")
            .unwrap();
        let close = log.iter().position(|q| q["type"] == "close").unwrap();
        assert!(release < close);
        assert_eq!(log.iter().filter(|q| q["type"] == "keyLighting").count(), 2);
        host.stop();
    }
    #[test]
    fn key_lighting_first_baseline_rejection_never_releases_nonexistent_ram_lease() {
        let (_directory, host, calls) = lighting_host_fixture();
        let id = host.add("dev.deckweave.fixture", "a").unwrap();
        wait_until(|| host.session(&id).is_some_and(|s| s.state == "connected"));
        let generation = host.session(&id).unwrap().generation;
        let first = {
            let host = Arc::clone(&host);
            let id = id.clone();
            thread::spawn(move || {
                host.send_key_lighting(
                    &id,
                    generation,
                    "unstarted",
                    77,
                    1000,
                    json!({"key-1":{"r":1,"g":2,"b":3}}),
                )
            })
        };
        wait_until(|| {
            lighting_calls(&calls)
                .iter()
                .any(|q| q["type"] == "keyLighting" && q["sequence"] == 77)
        });
        let queued = {
            let host = Arc::clone(&host);
            let id = id.clone();
            thread::spawn(move || {
                host.send_key_lighting(
                    &id,
                    generation,
                    "unstarted",
                    78,
                    1000,
                    json!({"key-1":{"r":1,"g":2,"b":3}}),
                )
            })
        };
        wait_until(|| {
            host.shared
                .controls
                .lock()
                .unwrap()
                .iter()
                .any(|q| q.payload["sequence"] == 78)
        });
        let error = first.join().unwrap().unwrap_err();
        assert!(
            queued.join().unwrap().is_err(),
            "Retiring an unstarted lease also cancels its queued frames"
        );
        assert!(error.contains("baseline required"));
        let release = host
            .release_key_lighting(&id, generation, "unstarted")
            .unwrap();
        assert_eq!(release["alreadyReleased"], true);
        assert_eq!(host.session(&id).unwrap().state, "connected");
        assert_ne!(host.session(&id).unwrap().recovery["state"], "pending");
        host.stop();
        assert!(
            !lighting_calls(&calls)
                .iter()
                .any(|q| q["type"] == "releaseKeyLighting"),
            "No RAM output existed, so neither reconcile/runtime cleanup nor shutdown may release it"
        );
        assert_eq!(
            lighting_calls(&calls)
                .iter()
                .filter(|q| q["type"] == "keyLighting")
                .count(),
            1
        );
    }
    #[test]
    fn key_lighting_rejects_unbounded_and_nonphysical_frames() {
        let definition = json!({"controls":[{"id":"key-1","kind":"key"}],"keyLighting":{"experimental":true,"maxKeys":14,"maxRateHz":10,"maxTtlMs":3000}});
        assert!(
            lighting_frame(&definition, 1, 100, &json!({"key-1":{"r":0,"g":255,"b":1}})).is_ok()
        );
        for (sequence, ttl, colors) in [
            (0, 100, json!({"key-1":{"r":0,"g":0,"b":0}})),
            (1, 99, json!({"key-1":{"r":0,"g":0,"b":0}})),
            (1, 100, json!({"unknown":{"r":0,"g":0,"b":0}})),
            (1, 100, json!({"key-1":{"r":256,"g":0,"b":0}})),
        ] {
            assert!(lighting_frame(&definition, sequence, ttl, &colors).is_err());
        }
    }
    fn wait_until(mut ready: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(8);
        while !ready() {
            assert!(
                Instant::now() < deadline,
                "Timed out waiting for host state"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    #[ignore = "Workspace integration requires adjacent plugin-sdk checkout; never accesses hardware"]
    fn real_sdk_close_removes_only_the_removed_core_instance_after_epoch_changes() {
        let sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .unwrap()
            .join("plugin-sdk/device.mjs");
        let (directory, package) = fixture(
            r#"
import {serveDevice} from './sdk.mjs';
const descriptor=instanceId=>({instanceId,modelId:'dev.deckweave.fixture.model',name:instanceId,identity:{confidence:'session',persistentId:null},geometry:{width:2,height:1},controls:[{id:'key-1',kind:'key',x:0,y:0,width:1,height:1,events:['down','up']},{id:'dial-1',kind:'dial',x:1,y:0,width:1,height:1,events:['rotate']}],settings:[],displays:[],lighting:[]});
serveDevice({id:'dev.deckweave.fixture',discover:()=>['a','b'].map(descriptor),open(instanceId){return{descriptor:descriptor(instanceId),readState:()=>({observed:{},evidence:{transport:'accepted',readback:'verified',physical:'unsupported'}}),close:()=>({recovery:'not_needed'})};}});
"#,
        );
        let root = directory.path().join("packages");
        let bundle = root.join("dev.deckweave.fixture.dwDevice");
        std::fs::create_dir_all(&bundle).unwrap();
        for file in ["provider.mjs", "deckweave.device.json"] {
            std::fs::copy(package.directory.join(file), bundle.join(file)).unwrap();
        }
        std::fs::copy(sdk, bundle.join("sdk.mjs"))
            .expect("The explicit SDK integration test requires sibling plugin-sdk/device.mjs");
        let host = headless_host(directory.path(), &root, &directory.path().join("state"));
        wait_until(|| host.snapshot().discovered.len() == 2);
        let a = host.add("dev.deckweave.fixture", "a").unwrap();
        let b = host.add("dev.deckweave.fixture", "b").unwrap();
        wait_until(|| {
            host.snapshot()
                .sessions
                .get(&a)
                .is_some_and(|s| s.state == "connected")
                && host
                    .snapshot()
                    .sessions
                    .get(&b)
                    .is_some_and(|s| s.state == "connected")
        });
        {
            let mut core = host.shared.core.lock().unwrap();
            let revision = core.revision();
            core.dispatch(
                deckweave_core::Command::RemoveDevice {
                    device_id: a.clone(),
                    expected_revision: revision,
                },
                "main",
            )
            .unwrap();
        }
        wait_until(|| !host.snapshot().sessions.contains_key(&a));
        assert_eq!(host.snapshot().sessions[&b].state, "connected");
        assert!(
            host.snapshot()
                .discovered
                .iter()
                .find(|d| d.instance_id == "a")
                .unwrap()
                .device_id
                .is_none()
        );
        assert!(
            host.shared
                .core
                .lock()
                .unwrap()
                .snapshot()
                .configuration
                .removed_devices
                .iter()
                .any(|device| device.id == a)
        );
        host.stop();
    }
    fn headless_host(directory: &Path, root: &Path, state_root: &Path) -> Host {
        let lifecycle = Arc::new(crate::plugin_lifecycle::Store::open(
            directory.join("lifecycle.json"),
        ));
        let shared = Arc::new(Shared {
            core: Arc::new(Mutex::new(
                Core::open(&directory.join("workspace.sqlite3")).unwrap(),
            )),
            app: None,
            epoch: Arc::new(AtomicU64::new(0)),
            stop: AtomicBool::new(false),
            inventory: Mutex::new(ProviderSnapshot::default()),
            refresh: Mutex::new(BTreeSet::new()),
            edits: Mutex::new(BTreeMap::new()),
            paused: Mutex::new(BTreeSet::new()),
            active: Mutex::new(BTreeSet::new()),
            quiesced: Mutex::new(BTreeMap::new()),
            reload: Mutex::new(BTreeSet::new()),
            lifecycle: Arc::clone(&lifecycle),
            recovery_packages: Mutex::new(BTreeSet::new()),
            controls: Mutex::new(VecDeque::new()),
            observations: Mutex::new(BTreeMap::new()),
            key_lighting: Mutex::new(BTreeMap::new()),
            keyboard_input: Mutex::new(BTreeMap::new()),
            keyboard_edits: Mutex::new(BTreeSet::new()),
            next_generation: AtomicU64::new(0),
        });
        let context = Arc::clone(&shared);
        let state_root = state_root.to_path_buf();
        let root = root.to_path_buf();
        let worker = thread::spawn(move || supervise(context, vec![root], state_root));
        Host {
            shared: Arc::clone(&shared),
            workers: Mutex::new(vec![worker]),
        }
    }
    #[test]
    fn blocked_removal_retains_recovery_session_until_explicit_restore_then_stops() {
        let (directory, package) = fixture(
            r#"
import {createInterface} from 'node:readline';
import fs from 'node:fs';
const instanceId='mock-'+process.pid;const marker=process.env.DECKWEAVE_PROVIDER_STATE_DIR+'/pending.flag';const counter=process.env.DECKWEAVE_PROVIDER_STATE_DIR+'/set-count';
const descriptor={instanceId,modelId:'dev.deckweave.fixture.model',name:'Fixture',identity:{confidence:'session',persistentId:null},geometry:{width:3,height:2},controls:[{id:'key-1',kind:'key',x:0,y:0,width:1,height:1,events:['down','up']},{id:'dial-1',kind:'dial',x:1,y:0,width:1,height:1,events:['rotate']},{id:'touch-strip',kind:'touch',x:0,y:1,width:3,height:1,events:['touchStart','touchEnd']}],settings:[{id:'vibration',type:'boolean',label:{en:'Vibration',ru:'Вибрация'},persistence:'onboard',readback:true}],displays:[],recovery:{preview:true,restore:true}};
let pending=fs.existsSync(marker);
const state=()=>({observed:{vibration:true},evidence:{transport:'accepted',readback:'verified',physical:'unsupported'},recovery:{state:pending?'pending':'not_needed',pending:pending?[{snapshotId:'saved-1',summary:{groups:['lighting'],keys:[],changedBytes:1}}]:[]}});
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})){
 const q=JSON.parse(line);let value={},error;
 if(q.type==='hello')value={protocol:1,providerId:'dev.deckweave.fixture'};
 else if(q.type==='discover')value={devices:[descriptor]};
 else if(q.type==='open')value={descriptor,state:state()};
 else if(q.type==='readState')value=state();
 else if(q.type==='setSettings'){fs.writeFileSync(counter,String(Number(fs.existsSync(counter)?fs.readFileSync(counter,'utf8'):0)+1));pending=true;fs.writeFileSync(marker,'pending');error={code:'write_uncertain',message:'Simulated interrupted write'};}
 else if(q.type==='close')value={recovery:pending?'pending':'not_needed'};
 else if(q.type==='recoveryPreview')value={snapshotId:'saved-1',previewToken:'once-1',confirmedInstanceId:instanceId,requiresConfirmation:true,identityConfidence:'session',expiresInMs:60000,summary:{groups:['lighting'],keys:[],changedBytes:1}};
 else if(q.type==='recover'){if(q.previewToken!=='once-1'||q.confirmedInstanceId!==instanceId)error={code:'confirmation',message:'Confirm the currently selected instance'};else{pending=false;fs.unlinkSync(marker);value={...state(),recovery:'completed',recoveryState:{state:'not_needed',pending:[]}};}}
 process.stdout.write(JSON.stringify({type:'result',requestId:q.requestId,...(q.session?{session:q.session,generation:q.generation}:{}),ok:!error,...(error?{error}:{value})})+'\n');
}
"#,
        );
        let root = directory.path().join("packages");
        let bundle = root.join("dev.deckweave.fixture.dwDevice");
        std::fs::create_dir_all(&bundle).unwrap();
        for file in ["provider.mjs", "deckweave.device.json"] {
            std::fs::copy(package.directory.join(file), bundle.join(file)).unwrap();
        }
        let state_root = directory.path().join("state");
        let host = headless_host(directory.path(), &root, &state_root);
        let shared = Arc::clone(&host.shared);
        let lifecycle = Arc::clone(&shared.lifecycle);
        wait_until(|| !host.snapshot().discovered.is_empty());
        let first_instance = host.snapshot().discovered[0].instance_id.clone();
        let id = host.add("dev.deckweave.fixture", &first_instance).unwrap();
        wait_until(|| {
            host.snapshot()
                .sessions
                .get(&id)
                .is_some_and(|session| session.state == "connected")
        });
        {
            let mut core = shared.core.lock().unwrap();
            let revision = core.revision();
            core.dispatch(
                deckweave_core::Command::SetProviderSettings {
                    device_id: id.clone(),
                    changes: BTreeMap::from([("vibration".into(), json!(false))]),
                    expected_revision: revision,
                },
                "main",
            )
            .unwrap();
        }
        wait_until(|| {
            host.snapshot()
                .sessions
                .get(&id)
                .is_some_and(|session| session.recovery["state"] == "pending")
        });
        assert_eq!(host.snapshot().sessions[&id].requested["vibration"], false);
        assert_eq!(host.snapshot().sessions[&id].observed["vibration"], true);
        let error = host.snapshot().sessions[&id].error.clone();
        assert!(error.is_some());
        host.refresh(&id).unwrap();
        wait_until(|| !shared.refresh.lock().unwrap().contains(&id));
        // Wait for the GET acknowledgement rather than treating dequeue as completed readback.
        wait_until(|| host.snapshot().sessions[&id].evidence["readback"] == "verified");
        assert_eq!(host.snapshot().sessions[&id].requested["vibration"], false);
        assert_eq!(host.snapshot().sessions[&id].error, error);
        lifecycle
            .block(crate::plugin_lifecycle::Record {
                kind: "device".into(),
                id: "dev.deckweave.fixture".into(),
                name: "Fixture".into(),
                version: "1.0.0".into(),
                source: "managed".into(),
                phase: "stopping".into(),
                error: None,
            })
            .unwrap();
        assert!(host.pause_package("dev.deckweave.fixture").is_err());
        assert!(
            shared
                .active
                .lock()
                .unwrap()
                .contains("dev.deckweave.fixture"),
            "Recovery process must survive the source gate"
        );
        lifecycle
            .block(crate::plugin_lifecycle::Record {
                kind: "device".into(),
                id: "dev.deckweave.fixture".into(),
                name: "Fixture".into(),
                version: "1.0.0".into(),
                source: "managed".into(),
                phase: "stop_pending".into(),
                error: Some("Physical recovery is pending".into()),
            })
            .unwrap();
        host.stop();
        drop(host);
        drop(shared);
        drop(lifecycle);
        let host = headless_host(directory.path(), &root, &state_root);
        let shared = Arc::clone(&host.shared);
        let lifecycle = Arc::clone(&shared.lifecycle);
        wait_until(|| !host.snapshot().discovered.is_empty());
        assert!(
            host.snapshot().sessions.is_empty(),
            "Restart must never automatically pair a session identity"
        );
        let fresh_instance = host.snapshot().discovered[0].instance_id.clone();
        assert_ne!(fresh_instance, first_instance);
        assert_eq!(
            host.rebind(&id, "dev.deckweave.fixture", &fresh_instance)
                .unwrap(),
            id
        );
        wait_until(|| {
            host.snapshot()
                .sessions
                .get(&id)
                .is_some_and(|session| session.recovery["state"] == "pending")
        });
        assert_eq!(
            std::fs::read_to_string(state_root.join("dev.deckweave.fixture/set-count")).unwrap(),
            "1",
            "Cold recovery pairing must not replay saved desired settings"
        );
        let preview = host.recovery_preview(&id, None).unwrap();
        assert!(
            host.recover(
                &id,
                preview["previewToken"].as_str().unwrap(),
                "wrong-instance"
            )
            .is_err()
        );
        assert!(!lifecycle.allows("device:dev.deckweave.fixture"));
        let recovered = host
            .recover(
                &id,
                preview["previewToken"].as_str().unwrap(),
                &fresh_instance,
            )
            .unwrap();
        assert_eq!(recovered["recovery"], "completed");
        host.pause_package("dev.deckweave.fixture").unwrap();
        assert!(
            !shared
                .active
                .lock()
                .unwrap()
                .contains("dev.deckweave.fixture")
        );
        assert!(
            !lifecycle.allows("device:dev.deckweave.fixture"),
            "Recovery must not reconnect removed support"
        );
        host.stop();
    }
    fn fixture(source: &str) -> (tempfile::TempDir, Package) {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("provider.mjs"), source).unwrap();
        std::fs::write(directory.path().join("deckweave.device.json"),serde_json::to_vec(&json!({"kind":"device","protocol":1,"id":"dev.deckweave.fixture","name":"Fixture","version":"1.0.0","runtime":"node","entry":"provider.mjs","models":["dev.deckweave.fixture.model"],"platforms":["windows","macos","linux"]})).unwrap()).unwrap();
        let package = load(directory.path()).unwrap();
        (directory, package)
    }
    fn reply(transport: &Transport, request: Value) -> (Value, Vec<Value>) {
        let id = request["requestId"].as_str().unwrap().to_string();
        transport.send(&request).unwrap();
        let mut events = vec![];
        loop {
            match transport.read.recv_timeout(Duration::from_secs(3)).unwrap() {
                PipeEvent::Message(value) if value["requestId"].as_str() == Some(&id) => {
                    return (value, events);
                }
                PipeEvent::Message(value) => events.push(value),
                _ => panic!("Fixture transport failed"),
            }
        }
    }
    #[test]
    fn real_process_supports_two_sessions_settings_ordered_inputs_and_independent_close() {
        let (directory, package) = fixture(
            r#"
import {createInterface} from 'node:readline';
const sessions=new Map();
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})){
 const q=JSON.parse(line);let value={};
 if(q.type==='hello')value={protocol:1,providerId:'dev.deckweave.fixture'};
 else if(q.type==='discover')value={devices:['a','b'].map(instanceId=>({instanceId}))};
 else if(q.type==='open'){sessions.set(q.session,{brightness:50});value={opened:true};}
 else if(q.type==='setSettings'){
  Object.assign(sessions.get(q.session),q.changes);
  for(const [i,event]of ['down','up','rotate'].entries())process.stdout.write(JSON.stringify({type:'input',session:q.session,generation:q.generation,sequence:i+1,control:event==='rotate'?'dial-1':'key-1',event,value:event==='rotate'?2:undefined})+'\n');
  value={observed:sessions.get(q.session),evidence:{transport:'accepted',readback:'verified',physical:'unsupported'}};
 }else if(q.type==='close'){sessions.delete(q.session);value={recovery:'not_needed'};}
 else if(q.type==='readState')value={observed:sessions.get(q.session)};
 process.stdout.write(JSON.stringify({type:'result',requestId:q.requestId,ok:true,...(q.session?{session:q.session,generation:q.generation}:{}),value})+'\n');
}
"#,
        );
        let transport = Transport::start(&package, directory.path(), false).unwrap();
        assert_eq!(
            reply(
                &transport,
                json!({"type":"hello","requestId":"1","protocol":1})
            )
            .0["value"]["protocol"],
            1
        );
        assert_eq!(
            reply(&transport, json!({"type":"discover","requestId":"2"})).0["value"]["devices"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        reply(
            &transport,
            json!({"type":"open","requestId":"3","session":"a","generation":1,"instanceId":"a"}),
        );
        reply(
            &transport,
            json!({"type":"open","requestId":"4","session":"b","generation":1,"instanceId":"b"}),
        );
        let (set, events) = reply(
            &transport,
            json!({"type":"setSettings","requestId":"5","session":"a","generation":1,"changes":{"brightness":73}}),
        );
        assert_eq!(set["value"]["observed"]["brightness"], 73);
        assert_eq!(
            events
                .iter()
                .map(|event| event["sequence"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(events[2]["value"], 2);
        reply(
            &transport,
            json!({"type":"close","requestId":"6","session":"a","generation":1}),
        );
        let (sibling, _) = reply(
            &transport,
            json!({"type":"readState","requestId":"7","session":"b","generation":1}),
        );
        assert_eq!(sibling["value"]["observed"]["brightness"], 50);
        transport.stop().unwrap();
    }
    #[test]
    fn process_malformed_json_and_stdout_overflow_fail_closed() {
        let (directory, package) = fixture("process.stdout.write('not json\\n');");
        let transport = Transport::start(&package, directory.path(), false).unwrap();
        assert!(matches!(
            transport.read.recv_timeout(Duration::from_secs(3)).unwrap(),
            PipeEvent::Failed(_)
        ));
        transport.stop().unwrap();
        let (directory, package) = fixture(
            "for(let i=0;i<400;i++)console.log(JSON.stringify({type:'input',sequence:i}));",
        );
        let transport = Transport::start(&package, directory.path(), false).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !transport.failed.load(Ordering::Relaxed) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(transport.failed.load(Ordering::Relaxed));
        transport.stop().unwrap();
    }
    #[test]
    #[cfg(windows)]
    fn stopping_provider_kills_descendants_that_inherit_stdout_and_stderr() {
        let (directory, package) = fixture(
            "import {spawn} from 'node:child_process'; const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit'}); console.log(JSON.stringify({type:'spawned',pid:child.pid})); setInterval(()=>{},1000);",
        );
        let transport = Transport::start(&package, directory.path(), false).unwrap();
        assert!(
            matches!(transport.read.recv_timeout(Duration::from_secs(3)).unwrap(),PipeEvent::Message(value) if value["type"]=="spawned")
        );
        let started = Instant::now();
        transport.stop().unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "Inherited pipes must not hang provider removal"
        );
    }
    #[test]
    fn package_validation_rejects_escape_and_unknown_manifest_fields_without_execution() {
        let (directory, _) = fixture("throw new Error('must not execute inspection');");
        assert!(inspect_package(directory.path()).is_ok());
        let path = directory.path().join("deckweave.device.json");
        let original: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let mut manifest = original.clone();
        manifest["entry"] = json!("../provider.mjs");
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(inspect_package(directory.path()).is_err());
        let mut manifest = original;
        manifest["args"] = json!(["--eval", "bad"]);
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(inspect_package(directory.path()).is_err());
    }
    #[test]
    fn broken_managed_candidate_never_silently_reveals_lower_priority_provider() {
        let (directory, package) = fixture("throw new Error('inspection only');");
        let managed = directory.path().join("managed");
        let repository = directory.path().join("repository");
        let name = "dev.deckweave.fixture.dwDevice";
        for root in [&managed, &repository] {
            let target = root.join(name);
            std::fs::create_dir_all(&target).unwrap();
            std::fs::copy(
                package.directory.join("deckweave.device.json"),
                target.join("deckweave.device.json"),
            )
            .unwrap();
            std::fs::write(target.join("provider.mjs"), "// not executed").unwrap();
        }
        std::fs::remove_file(managed.join(name).join("provider.mjs")).unwrap();
        let scanned = scan(&[managed, repository]);
        assert_eq!(scanned.len(), 1);
        assert!(
            scanned[0].is_err(),
            "broken managed source remains selected; repository copy must not execute"
        );
    }
    #[test]
    fn framing_rejects_malformed_unterminated_and_oversize_without_unbounded_allocation() {
        assert!(read_lines(&b"{}"[..], |_| Ok(())).is_err());
        assert!(read_lines(vec![b'a'; MAX_LINE + 1].as_slice(), |_| Ok(())).is_err());
        let mut lines = vec![];
        read_lines(&b"{}\r\n{}\n"[..], |line| {
            lines.push(line.to_vec());
            Ok(())
        })
        .unwrap();
        assert_eq!(lines, vec![b"{}".to_vec(), b"{}".to_vec()]);
    }
    #[test]
    fn descriptor_has_no_invented_screen_and_rejects_duplicate_or_outside_controls() {
        let mut descriptor = json!({"instanceId":"a","geometry":{"width":2,"height":1},"controls":[{"id":"key-1","kind":"key","x":0,"y":0,"width":1,"height":1,"events":["down","up"]}],"settings":[]});
        let converted = descriptor_geometry(&descriptor).unwrap();
        assert_eq!(converted.geometry.surface_pixels, [0, 0]);
        descriptor["controls"][0]["width"] = json!(3);
        assert!(descriptor_geometry(&descriptor).is_err());
    }
    #[test]
    fn stale_generations_and_unopened_sessions_cannot_deliver_input() {
        let mut state = ProcessState {
            sessions: BTreeMap::new(),
            pending: BTreeMap::new(),
            next_request: 0,
            hello: false,
        };
        state.sessions.insert(
            "s".into(),
            Session {
                device: "d".into(),
                generation: 2,
                lifecycle_epoch: 0,
                sequence: 0,
                definition: json!({}),
                opened: true,
                failed: false,
                edit: 0,
                recovery: Value::Null,
                released: false,
                observing: false,
                observation_needs_stop: false,
                last_observe: Instant::now(),
                depth_sequence: 0,
                keyboard_sequence: 0,
                read_sequence: 0,
                recovery_preview: None,
            },
        );
        assert!(event_session(&mut state, &json!({"session":"s","generation":1})).is_none());
        assert!(event_session(&mut state, &json!({"session":"s","generation":2})).is_some());
        state.sessions.get_mut("s").unwrap().failed = true;
        assert!(event_session(&mut state, &json!({"session":"s","generation":2})).is_none());
    }
    #[test]
    fn depth_samples_expire_to_unknown_and_window_leases_remain_independent() {
        let mut observation = Observation {
            active: true,
            needs_stop: true,
            ..Default::default()
        };
        observation
            .leases
            .insert("main".into(), Instant::now() - OBSERVER_LEASE);
        observation.leases.insert("device-d".into(), Instant::now());
        observation.samples.insert(
            "key-1".into(),
            ReceivedTravel {
                travel: 3970,
                maximum: Some(4000),
                received: Instant::now(),
            },
        );
        observation.samples.insert(
            "key-2".into(),
            ReceivedTravel {
                travel: 0,
                maximum: None,
                received: Instant::now() - DEPTH_TTL,
            },
        );
        let snapshot = observation.snapshot();
        assert!(snapshot.enabled);
        assert_eq!(snapshot.samples.len(), 1);
        assert_eq!(snapshot.samples[0].travel_um, 3970);
        assert!(!observation.leases.contains_key("main"));
        observation.leases.clear();
        assert_eq!(observation.snapshot().state, "stopping");
        assert!(observation.snapshot().samples.is_empty());
        observation.error = Some("Stop unconfirmed".into());
        assert_eq!(observation.snapshot().state, "error");
    }
    #[test]
    fn keeping_current_requires_an_advertised_fresh_matching_preview() {
        let request =
            json!({"previewToken":"once","confirmedInstanceId":"a","resolution":"keepCurrent"});
        assert!(authorize_resolution(None, &request).is_err());
        let mut authorization = RecoveryAuthorization::from_preview(&json!({"previewToken":"once","confirmedInstanceId":"a","expiresInMs":60000,"canKeepCurrent":true,"restoreAvailable":false})).unwrap();
        assert!(authorize_resolution(Some(&authorization), &request).is_ok());
        assert!(
            authorize_resolution(
                Some(&authorization),
                &json!({"previewToken":"once","confirmedInstanceId":"a"})
            )
            .is_err()
        );
        assert!(
            authorize_resolution(
                Some(&authorization),
                &json!({"previewToken":"once","confirmedInstanceId":"b","resolution":"keepCurrent"})
            )
            .is_err()
        );
        authorization.expires = Instant::now();
        assert!(authorize_resolution(Some(&authorization), &request).is_err());
        assert!(
            authorize_resolution(
                None,
                &json!({"previewToken":"legacy","confirmedInstanceId":"a"})
            )
            .is_ok()
        );
    }
    #[test]
    fn restarting_provider_and_rebinding_same_saved_device_never_reuses_ack_generation() {
        let (directory, package) = fixture(
            r#"
import {createInterface} from 'node:readline';
const instanceId='process-'+process.pid;
const descriptor={instanceId,modelId:'dev.deckweave.fixture.model',name:'Fixture',identity:{confidence:'session',persistentId:null},geometry:{width:1,height:1},controls:[{id:'key-1',kind:'key',x:0,y:0,width:1,height:1,events:[]}],settings:[{id:'level',type:'number',min:0,max:5}],displays:[]};
let level=0;
const state=()=>({observed:{level},recovery:{state:'not_needed',pending:[]}});
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})){
 const q=JSON.parse(line);let value={};
 if(q.type==='hello')value={protocol:1,providerId:'dev.deckweave.fixture'};
 if(q.type==='discover')value={devices:[descriptor]};
 if(q.type==='open')value={descriptor,state:state()};
 if(q.type==='readState')value=state();
 if(q.type==='setSettings'){level=q.changes.level;value=state();}
 if(q.type==='close')value={recovery:'not_needed'};
 process.stdout.write(JSON.stringify({type:'result',requestId:q.requestId,...(q.session?{session:q.session,generation:q.generation}:{}),ok:true,value})+'\n');
}
"#,
        );
        let root = directory.path().join("packages");
        let bundle = root.join("dev.deckweave.fixture.dwDevice");
        std::fs::create_dir_all(&bundle).unwrap();
        for file in ["provider.mjs", "deckweave.device.json"] {
            std::fs::copy(package.directory.join(file), bundle.join(file)).unwrap();
        }
        let host = headless_host(directory.path(), &root, &directory.path().join("state"));
        wait_until(|| !host.snapshot().discovered.is_empty());
        let original_instance = host.snapshot().discovered[0].instance_id.clone();
        let id = host
            .add("dev.deckweave.fixture", &original_instance)
            .unwrap();
        wait_until(|| {
            host.session(&id)
                .is_some_and(|status| status.state == "connected")
        });
        let original = host.session(&id).unwrap();
        host.pause_package("dev.deckweave.fixture").unwrap();
        host.resume_package("dev.deckweave.fixture");
        wait_until(|| {
            host.snapshot()
                .discovered
                .iter()
                .any(|device| device.instance_id != original_instance)
        });
        let fresh_instance = host
            .snapshot()
            .discovered
            .iter()
            .find(|device| device.instance_id != original_instance)
            .unwrap()
            .instance_id
            .clone();
        assert!(
            host.snapshot()
                .discovered
                .iter()
                .all(|device| device.device_id.is_none()),
            "A restarted session identity requires explicit pairing"
        );
        assert_eq!(
            host.rebind(&id, "dev.deckweave.fixture", &fresh_instance)
                .unwrap(),
            id
        );
        wait_until(|| {
            host.session(&id)
                .is_some_and(|status| status.state == "connected")
        });
        assert!(host.session(&id).unwrap().generation > original.generation);
        {
            let mut core = host.shared.core.lock().unwrap();
            let revision = core.revision();
            core.dispatch(
                deckweave_core::Command::SetProviderSettings {
                    device_id: id.clone(),
                    changes: BTreeMap::from([("level".into(), json!(3))]),
                    expected_revision: revision,
                },
                "main",
            )
            .unwrap();
        }
        wait_until(|| host.session(&id).unwrap().observed["level"] == 3);
        let fresh_ack = host.session(&id).unwrap();
        assert!(fresh_ack.settings_edit > original.settings_edit);
        assert_ne!(
            fresh_ack.generation, original.generation,
            "A matching value/edit ACK from the new physical session cannot confirm an old editor's request"
        );
        host.stop();
    }
    #[test]
    fn delayed_settings_keep_cached_reads_responsive_and_manual_read_waits_for_terminal_ack() {
        let (directory, package) = fixture(
            r#"
import {createInterface} from 'node:readline';
import fs from 'node:fs';
const dir=process.env.DECKWEAVE_PROVIDER_STATE_DIR;
const descriptor=id=>({instanceId:id,modelId:'dev.deckweave.fixture.model',name:id,identity:{confidence:'session',persistentId:null},geometry:{width:1,height:1},controls:[{id:'key-1',kind:'key',x:0,y:0,width:1,height:1,events:[]}],settings:[{id:'level',type:'number',min:0,max:5}],displays:[]});
const sessions=new Map();
const reply=(q,value)=>process.stdout.write(JSON.stringify({type:'result',requestId:q.requestId,...(q.session?{session:q.session,generation:q.generation}:{}),ok:true,value})+'\n');
const state=s=>({observed:{level:s.level},evidence:{transport:'accepted',readback:'verified',physical:'unknown'}});
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})){
 const q=JSON.parse(line);let value={};
 if(q.type==='hello')value={protocol:1,providerId:'dev.deckweave.fixture'};
 if(q.type==='discover')value={devices:['a','b'].map(descriptor)};
 if(q.type==='open'){const s={id:q.instanceId,level:0,writing:false};sessions.set(q.session,s);value={descriptor:descriptor(q.instanceId),state:state(s)};}
 if(q.type==='readState'){const s=sessions.get(q.session);fs.appendFileSync(dir+'/reads',s.id+'\n');if(s.writing)fs.writeFileSync(dir+'/read-during-write','bad');value=state(s);}
 if(q.type==='setSettings'){const s=sessions.get(q.session);s.writing=true;fs.writeFileSync(dir+'/started','1');setTimeout(()=>{s.level=q.changes.level;s.writing=false;reply(q,state(s));},1000);continue;}
 if(q.type==='close'){sessions.delete(q.session);value={recovery:'not_needed'};}
 reply(q,value);
}
"#,
        );
        let root = directory.path().join("packages");
        let bundle = root.join("dev.deckweave.fixture.dwDevice");
        std::fs::create_dir_all(&bundle).unwrap();
        for file in ["provider.mjs", "deckweave.device.json"] {
            std::fs::copy(package.directory.join(file), bundle.join(file)).unwrap();
        }
        let state_root = directory.path().join("state");
        let host = headless_host(directory.path(), &root, &state_root);
        let provider_state = state_root.join("dev.deckweave.fixture");
        wait_until(|| host.snapshot().discovered.len() == 2);
        let a = host.add("dev.deckweave.fixture", "a").unwrap();
        let b = host.add("dev.deckweave.fixture", "b").unwrap();
        wait_until(|| {
            host.session(&a).is_some_and(|s| s.state == "connected")
                && host.session(&b).is_some_and(|s| s.state == "connected")
        });
        assert!(
            !provider_state.join("reads").exists(),
            "Open already returned verified state; do not perform a duplicate full read"
        );
        let baseline = host.session(&a).unwrap();
        {
            let mut core = host.shared.core.lock().unwrap();
            let revision = core.revision();
            core.dispatch(
                deckweave_core::Command::SetProviderSettings {
                    device_id: a.clone(),
                    changes: BTreeMap::from([("level".into(), json!(3))]),
                    expected_revision: revision,
                },
                "main",
            )
            .unwrap();
        }
        wait_until(|| provider_state.join("started").exists());
        let started = Instant::now();
        let pending = host.session(&a).unwrap();
        let _ = host.shared.core.lock().unwrap().revision();
        host.refresh(&a).unwrap();
        host.refresh(&b).unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(350),
            "Hardware work must not hold Core or cached inventory locks"
        );
        assert_eq!(pending.generation, baseline.generation);
        assert_eq!(
            pending.settings_edit, baseline.settings_edit,
            "An enqueued request is not a successful hardware ACK"
        );
        assert_eq!(pending.observed["level"], 0);
        assert_eq!(pending.read_sequence, 0);
        wait_until(|| host.session(&b).unwrap().read_sequence == 1);
        assert!(
            started.elapsed() < Duration::from_millis(800),
            "One instance's slow write must not stall a sibling's reads"
        );
        wait_until(|| host.session(&a).unwrap().read_sequence == 1);
        let completed = host.session(&a).unwrap();
        assert!(completed.settings_edit > baseline.settings_edit);
        assert_eq!(completed.observed["level"], 3);
        assert_eq!(completed.requested["level"], 3);
        assert!(
            !provider_state.join("read-during-write").exists(),
            "Manual reads must not accumulate timeout while queued behind SET"
        );
        let reads = std::fs::read_to_string(provider_state.join("reads")).unwrap();
        assert_eq!(reads.lines().filter(|id| *id == "a").count(), 1);
        assert_eq!(reads.lines().filter(|id| *id == "b").count(), 1);
        host.stop();
    }
    #[test]
    fn process_keep_current_forwards_explicit_resolution_and_refuses_an_ignored_resolution_ack() {
        let (directory, package) = fixture(
            r#"
import {createInterface} from 'node:readline';
import fs from 'node:fs';
const log=process.env.DECKWEAVE_PROVIDER_STATE_DIR+'/operations';
const descriptor={instanceId:'a',modelId:'dev.deckweave.fixture.model',name:'Fixture',identity:{confidence:'session',persistentId:null},geometry:{width:1,height:1},controls:[{id:'key-1',kind:'key',x:0,y:0,width:1,height:1,events:[]}],settings:[],displays:[],recovery:{preview:true,restore:true}};
let pending=true,calls=0;
const state=()=>({observed:{choice:1},recovery:{state:pending?'pending':'not_needed',pending:[]}});
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})){
 const q=JSON.parse(line);let value={};
 if(q.type==='hello')value={protocol:1,providerId:'dev.deckweave.fixture'};
 if(q.type==='discover')value={devices:[descriptor]};
 if(q.type==='open')value={descriptor,state:state()};
 if(q.type==='readState')value=state();
 if(q.type==='close')value={recovery:pending?'pending':'not_needed'};
 if(q.type==='setSettings')fs.appendFileSync(log,'SET\n');
 if(q.type==='recoveryPreview')value={snapshotId:'legacy',previewToken:'once',confirmedInstanceId:'a',requiresConfirmation:true,expiresInMs:60000,restoreAvailable:false,canKeepCurrent:true,reason:'legacy_lighting_projection'};
 if(q.type==='recover'){fs.appendFileSync(log,String(q.resolution)+'\n');calls++;if(calls===1)value={recovery:'completed'};else{pending=false;value={...state(),recovery:'completed',recoveryState:{state:'not_needed',pending:[]},resolution:q.resolution,outcome:'accepted_verified_current'};}}
 process.stdout.write(JSON.stringify({type:'result',requestId:q.requestId,...(q.session?{session:q.session,generation:q.generation}:{}),ok:true,value})+'\n');
}
"#,
        );
        let root = directory.path().join("packages");
        let bundle = root.join("dev.deckweave.fixture.dwDevice");
        std::fs::create_dir_all(&bundle).unwrap();
        for file in ["provider.mjs", "deckweave.device.json"] {
            std::fs::copy(package.directory.join(file), bundle.join(file)).unwrap();
        }
        let state_root = directory.path().join("state");
        let host = headless_host(directory.path(), &root, &state_root);
        wait_until(|| !host.snapshot().discovered.is_empty());
        let id = host.add("dev.deckweave.fixture", "a").unwrap();
        wait_until(|| {
            host.snapshot()
                .sessions
                .get(&id)
                .is_some_and(|session| session.recovery["state"] == "pending")
        });
        assert!(
            host.resolve_recovery(&id, "once", "a", Some("unknown"))
                .is_err()
        );
        assert!(
            host.resolve_recovery(&id, "once", "a", Some("keepCurrent"))
                .is_err()
        );
        host.recovery_preview(&id, None).unwrap();
        assert!(
            host.recover(&id, "once", "a").is_err(),
            "Legacy original data cannot be restored"
        );
        assert!(
            host.resolve_recovery(&id, "once", "b", Some("keepCurrent"))
                .is_err()
        );
        assert!(
            host.resolve_recovery(&id, "once", "a", Some("keepCurrent"))
                .unwrap_err()
                .contains("did not confirm")
        );
        assert_eq!(host.snapshot().sessions[&id].recovery["state"], "pending");
        assert!(
            host.resolve_recovery(&id, "once", "a", Some("keepCurrent"))
                .is_err(),
            "Host preview permission is one-use"
        );
        host.recovery_preview(&id, None).unwrap();
        let accepted = host
            .resolve_recovery(&id, "once", "a", Some("keepCurrent"))
            .unwrap();
        assert_eq!(accepted["resolution"], "keepCurrent");
        assert_eq!(
            host.snapshot().sessions[&id].recovery["state"],
            "not_needed"
        );
        assert_eq!(
            std::fs::read_to_string(state_root.join("dev.deckweave.fixture/operations")).unwrap(),
            "keepCurrent\nkeepCurrent\n",
            "No implicit restore or SET may be issued"
        );
        host.stop();
    }
    #[test]
    fn process_depth_observers_stop_on_last_window_lease_without_affecting_siblings_or_core() {
        let (directory, package) = fixture(
            r#"
import {createInterface} from 'node:readline';
const descriptor=id=>({instanceId:id,modelId:'dev.deckweave.fixture.model',name:id,identity:{confidence:'session',persistentId:null},geometry:{width:1,height:1},controls:[{id:'key-1',kind:'key',x:0,y:0,width:1,height:1,events:[],telemetry:{keyTravel:true}}],settings:[],displays:[],observation:{keyTravel:{minUm:0,maxUm:4000,staleAfterMs:600,normalTyping:true,evidence:'partial'}}});
const sessions=new Map();
const send=v=>process.stdout.write(JSON.stringify(v)+'\n');
const timer=setInterval(()=>{for(const [id,s]of sessions)if(s.on){send({type:'depth',session:id,generation:s.gen-1,sequence:999,control:'key-1',value:{travelUm:9999}});send({type:'depth',session:id,generation:s.gen,sequence:++s.seq,control:'key-1',value:{travelUm:1230,maxTravelUm:3970}});}},34);timer.unref();
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})){
const q=JSON.parse(line);let value={};
if(q.type==='hello')value={protocol:1,providerId:'dev.deckweave.fixture'};
if(q.type==='discover')value={devices:['a','b'].map(descriptor)};
if(q.type==='open'){sessions.set(q.session,{gen:q.generation,seq:0,on:false});value={descriptor:descriptor(q.instanceId),state:{observed:{}}};}
if(q.type==='readState')value={observed:{}};
if(q.type==='observe'){sessions.get(q.session).on=q.enabled;value={observing:q.enabled};}
if(q.type==='close'){sessions.delete(q.session);value={recovery:'not_needed'};}
send({type:'result',requestId:q.requestId,...(q.session?{session:q.session,generation:q.generation}:{}),ok:true,value});
}
"#,
        );
        let root = directory.path().join("packages");
        let bundle = root.join("dev.deckweave.fixture.dwDevice");
        std::fs::create_dir_all(&bundle).unwrap();
        for file in ["provider.mjs", "deckweave.device.json"] {
            std::fs::copy(package.directory.join(file), bundle.join(file)).unwrap();
        }
        let host = headless_host(directory.path(), &root, &directory.path().join("state"));
        wait_until(|| host.snapshot().discovered.len() == 2);
        let a = host.add("dev.deckweave.fixture", "a").unwrap();
        let b = host.add("dev.deckweave.fixture", "b").unwrap();
        wait_until(|| {
            host.snapshot()
                .sessions
                .values()
                .filter(|s| s.state == "connected")
                .count()
                == 2
        });
        assert_eq!(host.observation(&a, "main").unwrap().state, "off");
        let revision = host.shared.core.lock().unwrap().revision();
        host.set_observation(&a, "main", true).unwrap();
        host.set_observation(&a, "device-a", true).unwrap();
        host.set_observation(&b, "device-b", true).unwrap();
        wait_until(|| !host.observation(&a, "main").unwrap().samples.is_empty());
        let sample = host.observation(&a, "main").unwrap();
        assert_eq!(
            sample.samples[0].travel_um, 1230,
            "stale generation must not inject telemetry"
        );
        assert_eq!(
            host.shared.core.lock().unwrap().revision(),
            revision,
            "telemetry must not write Core configuration"
        );
        host.stop_observer("main");
        assert!(host.observation(&a, "device-a").unwrap().enabled);
        {
            let mut observations = host.shared.observations.lock().unwrap();
            observations
                .get_mut(&a)
                .unwrap()
                .leases
                .insert("device-a".into(), Instant::now() - OBSERVER_LEASE);
        }
        assert!(
            !host.observation(&a, "device-a").unwrap().enabled,
            "expired polling cannot revive a lease"
        );
        wait_until(|| host.observation(&a, "device-a").unwrap().state == "off");
        assert_eq!(host.observation(&b, "device-b").unwrap().state, "observing");
        host.stop_observer("device-b");
        wait_until(|| host.observation(&b, "device-b").unwrap().state == "off");
        host.stop();
    }
}
