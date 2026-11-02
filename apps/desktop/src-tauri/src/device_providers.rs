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
}
struct RecoveryRequest {
    device: String,
    kind: &'static str,
    payload: Value,
    reply: mpsc::Sender<Result<Value, String>>,
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
        self.recovery_call(
            device_id,
            "recover",
            json!({"previewToken":preview_token,"confirmedInstanceId":confirmed_instance}),
        )
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
                    inventory.sessions.insert(output.device_id,SessionStatus { state:"offline".into(),observed:json!({}),requested:json!(output.changes),evidence:json!({"transport":"unknown","readback":"unknown","physical":"unknown"}),recovery:Value::Null,error:Some("Device is not connected. This output was not queued; reconnect and request it again.".into()) });
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
    Read(String),
    Settings(String, BTreeMap<String, Value>, u64),
    Close(String),
    Recovery(String, mpsc::Sender<Result<Value, String>>, bool),
}
struct Pending {
    operation: Operation,
    sent: Instant,
    generation: Option<u64>,
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
}
struct ProcessState {
    sessions: BTreeMap<String, Session>,
    pending: BTreeMap<String, Pending>,
    next_request: u64,
    next_generation: u64,
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
        next_generation: 0,
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
            while let Ok(message) = transport.read.try_recv() {
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
                    Operation::Recovery(_, reply, _) => {
                        let _ = reply.send(Err(
                            "Recovery request timed out; completion is unconfirmed".into(),
                        ));
                        continue;
                    }
                    Operation::Open(id) | Operation::Read(id) | Operation::Settings(id, _, _) => id,
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
    loop {
        let request = {
            let mut controls = shared
                .controls
                .lock()
                .map_err(|_| "Recovery queue unavailable")?;
            controls
                .iter()
                .position(|request| {
                    state.sessions.values().any(|session| {
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
        state.request(
            transport,
            request.kind,
            Operation::Recovery(session_id.clone(), request.reply, restore),
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
        if pending_recovery && (!gone || session.released) {
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
            if !recovery_only && opened && !failed && previous_edit != edit && !changes.is_empty() && !state.pending.values().any(|request| matches!(&request.operation, Operation::Settings(existing, _, _) if existing == &id)) {
                let id = id.clone();
                state.request(transport, "setSettings", Operation::Settings(id.clone(), changes.clone(), edit), Some(&id), json!({"changes":changes}))?;
                if let Ok(mut queued) = shared.edits.lock()
                    && let Some(queued) = queued.get_mut(&device.id) { queued.fields.retain(|_,(revision,_)| *revision > edit); }
            }
            let refresh = shared
                .refresh
                .lock()
                .is_ok_and(|mut refresh| refresh.remove(&device.id));
            if opened && refresh && !state.pending.values().any(|request| matches!(&request.operation, Operation::Read(existing) if existing == &id)) {
                let id = id.clone();
                state.request(transport, "readState", Operation::Read(id.clone()), Some(&id), json!({}))?;
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
        state.next_generation += 1;
        let id = format!("device-{}-{}", device.id, state.next_generation);
        state.sessions.insert(
            id.clone(),
            Session {
                device: device.id.clone(),
                generation: state.next_generation,
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
            },
        );
        shared.connection(
            &device.id,
            state.next_generation,
            ConnectionState::Offline,
            None,
        );
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
        inventory.sessions.insert(
            session.device.clone(),
            SessionStatus {
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
                requested: requested.map_or_else(|| json!({}), |requested| json!(requested)),
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
                | Operation::Read(id)
                | Operation::Settings(id, _, _)
                | Operation::Close(id) => Some(id.clone()),
                Operation::Recovery(id, _, _) => Some(id.clone()),
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
                if !matches!(pending.operation, Operation::Close(_))
                    && !shared.core.lock().is_ok_and(|core| {
                        core.has_device(&session.device)
                            && core.device_lifecycle_epoch(&session.device)
                                == session.lifecycle_epoch
                    })
                {
                    if let Operation::Recovery(_, reply, _) = &pending.operation {
                        let _ = reply.send(Err(
                            "Device was rebound; request a fresh recovery preview".into(),
                        ));
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
                            | Operation::Read(_)
                            | Operation::Open(_)
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
                if let Operation::Recovery(_, reply, _) = &pending.operation {
                    let _ = reply.send(Err(message));
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
                    status(shared, session, &json!({}), None, Some(message));
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
                    state.request(
                        transport,
                        "readState",
                        Operation::Read(id.clone()),
                        Some(&id),
                        json!({}),
                    )?;
                }
                Operation::Read(id) => {
                    let session = state.sessions.get_mut(&id).unwrap();
                    update_recovery(session, &response);
                    if !shared
                        .lifecycle
                        .allows(&format!("device:{}", package.manifest.id))
                    {
                        session.failed = true;
                    }
                    status(shared, session, &response, None, None);
                }
                Operation::Settings(id, requested, edit) => {
                    let session = state.sessions.get_mut(&id).unwrap();
                    update_recovery(session, &response);
                    session.edit = edit;
                    status(shared, session, &response, Some(&requested), None);
                }
                Operation::Close(id) => {
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
                Operation::Recovery(id, reply, restore) => {
                    let device = state.sessions[&id].device.clone();
                    let completed = restore
                        && response.get("recovery").and_then(Value::as_str) == Some("completed")
                        && response
                            .pointer("/recoveryState/state")
                            .and_then(Value::as_str)
                            != Some("pending");
                    if completed {
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
fn event_session<'a>(state: &'a mut ProcessState, value: &Value) -> Option<&'a mut Session> {
    let session = state.sessions.get_mut(value.get("session")?.as_str()?)?;
    if !session.opened || session.failed || value.get("generation")?.as_u64()? != session.generation
    {
        return None;
    }
    Some(session)
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
            next_generation: 0,
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
            },
        );
        assert!(event_session(&mut state, &json!({"session":"s","generation":1})).is_none());
        assert!(event_session(&mut state, &json!({"session":"s","generation":2})).is_some());
        state.sessions.get_mut("s").unwrap().failed = true;
        assert!(event_session(&mut state, &json!({"session":"s","generation":2})).is_none());
    }
}
