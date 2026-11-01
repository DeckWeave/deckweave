//! Lazy, per-package Elgato processes. No process, socket or asset I/O holds the core lock.
mod artwork;
pub mod catalog;
mod network;

use catalog::Package;
use deckweave_core::{Binding, Command, ControlKind, Controller, Core, Error, Snapshot};
use network::{Network, Peer, Route, Socket};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    process::{Child, Command as Process, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

enum Request {
    Inspector {
        context: String,
        reply: Sender<Result<(String, String), String>>,
    },
    Restart(String),
}
pub struct Host {
    catalog: Arc<Mutex<Vec<Package>>>,
    requests: Sender<Request>,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl Host {
    pub fn start(
        core: Arc<Mutex<Core>>,
        app: tauri::AppHandle,
        directory: PathBuf,
        epoch: Arc<AtomicU64>,
    ) -> Self {
        let mut roots = vec![
            directory.join("plugins"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../plugins"),
        ];
        if let Some(roaming) = std::env::var_os("APPDATA") {
            roots.push(PathBuf::from(roaming).join("Elgato/StreamDeck/Plugins"));
        }
        let catalog = Arc::new(Mutex::new(catalog::scan(&roots)));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let (packages, stopping) = (catalog.clone(), stop.clone());
        let thread =
            thread::spawn(move || run(core, app, directory, epoch, packages, stopping, rx));
        Self {
            catalog,
            requests: tx,
            stop,
            thread: Mutex::new(Some(thread)),
        }
    }
    pub fn packages(&self) -> Vec<Package> {
        self.catalog.lock().map(|v| v.clone()).unwrap_or_default()
    }
    pub fn inspector(&self, context: String) -> Result<(String, String), Error> {
        let (tx, rx) = mpsc::channel();
        self.requests
            .send(Request::Inspector { context, reply: tx })
            .map_err(|_| Error::new("plugin", "Plugin host stopped"))?;
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| Error::new("plugin", "Inspector request timed out"))?
            .map_err(|e| Error::new("plugin", e))
    }
    pub fn restart(&self, package: String) {
        let _ = self.requests.send(Request::Restart(package));
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut t) = self.thread.lock()
            && let Some(t) = t.take()
        {
            let _ = t.join();
        }
    }
}

#[derive(Clone)]
struct Slot {
    device: String,
    binding: Binding,
    controller: &'static str,
    number: usize,
    multi: bool,
    canvas: [u32; 2],
    owner: String,
}
impl Slot {
    fn event(&self, event: &str) -> Value {
        let mut value = json!({"event":event,"action":self.binding.plugin.as_ref().map(|p|&p.action),"context":self.binding.id,"device":self.device,"payload":{"settings":self.binding.plugin.as_ref().map(|p|&p.settings),"controller":self.controller,"coordinates":{"column":(self.number-1)%5,"row":if self.controller=="Encoder"{0}else{(self.number-1)/5}},"state":0,"isInMultiAction":self.multi,"resources":{}}});
        if self.controller == "Panel" {
            value["payload"]["canvas"] = json!(self.canvas);
        }
        value
    }
}
struct RetiredSlot {
    slot: Slot,
    until: Instant,
}
#[derive(Default)]
struct RetiredContexts {
    entries: BTreeMap<String, RetiredSlot>,
}
impl RetiredContexts {
    const LIMIT: usize = 256;
    const LIFETIME: Duration = Duration::from_secs(10);
    fn prune(&mut self, now: Instant) {
        self.entries.retain(|_, retired| retired.until > now);
    }
    fn retire(&mut self, slot: Slot, now: Instant) {
        self.prune(now);
        if self.entries.len() >= Self::LIMIT
            && !self.entries.contains_key(&slot.binding.id)
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, r)| r.until)
                .map(|(id, _)| id.clone())
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(
            slot.binding.id.clone(),
            RetiredSlot {
                slot,
                until: now + Self::LIFETIME,
            },
        );
    }
}
struct Session {
    package: Package,
    token: String,
    nonce: String,
    receiver: Receiver<Peer>,
    sender: mpsc::SyncSender<Peer>,
    socket: Option<Socket>,
    inspectors: BTreeMap<String, Socket>,
    slots: BTreeMap<String, Slot>,
    retired: RetiredContexts,
    child: Option<Child>,
    process_tree: Option<deckweave_platform::ProcessTree>,
    window: Option<String>,
    started: Instant,
    error: Option<String>,
    errors: Receiver<String>,
    visuals: BTreeMap<String, artwork::Visual>,
    renderer: Result<deckweave_n4::Renderer, String>,
}
impl Session {
    fn send(&mut self, message: Value) {
        if let Some(s) = self.socket.as_mut()
            && let Err(e) = s.send(tungstenite::Message::Text(message.to_string().into()))
            && !would_block(&e)
        {
            self.error = Some(e.to_string());
        }
    }
    fn close(mut self, app: &tauri::AppHandle, net: &Network) {
        net.remove(&self.token);
        for id in self.slots.keys() {
            net.remove(id);
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(label) = self.window.take() {
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(w) = handle.get_webview_window(&label) {
                    let _ = w.close();
                }
            });
        }
    }
}
fn slots(snapshot: &Snapshot) -> BTreeMap<String, Slot> {
    fn walk(
        binding: &Binding,
        device: &str,
        controller: &'static str,
        number: usize,
        multi: bool,
        owner: &str,
        result: &mut BTreeMap<String, Slot>,
    ) {
        if binding.plugin.is_some() {
            let controller = match binding.plugin.as_ref().and_then(|p| p.controller) {
                Some(Controller::Keypad) => "Keypad",
                Some(Controller::Encoder) => "Encoder",
                Some(Controller::Panel) => "Panel",
                None => controller,
            };
            result.insert(
                binding.id.clone(),
                Slot {
                    device: device.into(),
                    binding: binding.clone(),
                    controller,
                    number,
                    multi,
                    canvas: if controller == "Panel" {
                        [800, 128]
                    } else {
                        [200, 100]
                    },
                    owner: owner.into(),
                },
            );
        }
        let controller = if matches!(binding.plan, Some(deckweave_core::Plan::ActionWheel { .. })) {
            "Keypad"
        } else {
            controller
        };
        for child in binding.children() {
            walk(
                child,
                device,
                controller,
                number,
                multi
                    || matches!(
                        binding.plan,
                        Some(
                            deckweave_core::Plan::Sequence { .. }
                                | deckweave_core::Plan::Switch { .. }
                        )
                    ),
                owner,
                result,
            );
        }
    }
    fn find(binding: &Binding, id: &str) -> Option<Binding> {
        if binding.id == id {
            return Some(binding.clone());
        }
        binding.children().iter().find_map(|b| find(b, id))
    }
    let mut result = BTreeMap::new();
    for d in &snapshot.configuration.devices {
        if let Some(page) = d
            .profile
            .pages
            .iter()
            .find(|p| p.id == d.profile.active_page_id)
        {
            for (control, binding) in &page.bindings {
                if let Some(c) = snapshot.geometries[&d.id]
                    .controls
                    .iter()
                    .find(|c| c.id == *control)
                {
                    walk(
                        binding,
                        &d.id,
                        if c.kind == ControlKind::Dial {
                            "Encoder"
                        } else {
                            "Keypad"
                        },
                        c.number,
                        false,
                        &binding.id,
                        &mut result,
                    );
                }
            }
        }
    }
    for retained in &snapshot.retained_actions {
        if result.contains_key(&retained.binding_id) {
            continue;
        }
        if let Some(d) = snapshot
            .configuration
            .devices
            .iter()
            .find(|d| d.id == retained.device_id)
        {
            let binding = d
                .pinned
                .values()
                .chain(
                    std::iter::once(&d.profile)
                        .chain(&d.profiles)
                        .flat_map(|p| p.pages.iter())
                        .flat_map(|p| p.bindings.values()),
                )
                .find_map(|b| find(b, &retained.binding_id));
            if let Some(binding) = binding {
                let number = snapshot.geometries[&d.id]
                    .controls
                    .iter()
                    .find(|c| c.id == retained.control_id)
                    .map_or(1, |c| c.number);
                walk(
                    &binding,
                    &d.id,
                    if retained.control_id.starts_with("dial-") {
                        "Encoder"
                    } else {
                        "Keypad"
                    },
                    number,
                    false,
                    &binding.id,
                    &mut result,
                );
            }
        }
    }
    for slot in result.values_mut() {
        if let Some(d) = snapshot
            .configuration
            .devices
            .iter()
            .find(|d| d.id == slot.device)
            && let Some(page) = d
                .profile
                .pages
                .iter()
                .find(|p| p.id == d.profile.active_page_id)
            && let Some(region) = page.panel.as_ref().and_then(|p| {
                p.regions.iter().find(|r| {
                    page.bindings
                        .get(&r.control_id())
                        .is_some_and(|b| b.contains(&slot.binding.id))
                })
            })
        {
            if let Some(number) = region
                .dials
                .first()
                .and_then(|id| id.strip_prefix("dial-"))
                .and_then(|s| s.parse().ok())
            {
                slot.number = number;
            }
            if slot.controller == "Panel" {
                slot.canvas = [
                    (region.rect[2] * 800.0).round() as u32,
                    (region.rect[3] * 128.0).round() as u32,
                ];
            }
        }
    }
    result
}
fn assigned_packages(snapshot: &Snapshot) -> BTreeSet<String> {
    fn visit(binding: &Binding, out: &mut BTreeSet<String>) {
        if let Some(p) = &binding.plugin {
            out.insert(p.package.clone());
        }
        for child in binding.children() {
            visit(child, out);
        }
    }
    let mut result = BTreeSet::new();
    for device in &snapshot.configuration.devices {
        for binding in device.pinned.values().chain(
            std::iter::once(&device.profile)
                .chain(&device.profiles)
                .flat_map(|p| p.pages.iter())
                .flat_map(|p| p.bindings.values()),
        ) {
            visit(binding, &mut result);
        }
    }
    result
}
fn info(snapshot: &Snapshot, package: &Package) -> Value {
    json!({"application":{"language":if snapshot.configuration.locale==deckweave_core::Locale::Ru{"ru"}else{"en"},"platform":if cfg!(windows){"windows"}else{"mac"},"platformVersion":if cfg!(windows){"11"}else{"0"},"version":"7.6.0","font":"Arial"},"plugin":{"uuid":package.id,"version":package.version},"devicePixelRatio":1,"colors":{"buttonPressedBackgroundColor":"#303030","buttonPressedBorderColor":"#808080","buttonPressedTextColor":"#ffffff","disabledColor":"#777777","highlightColor":"#5b85ff","mouseDownColor":"#4455aa"},"devices":snapshot.configuration.devices.iter().map(|d|json!({"id":d.id,"name":d.name,"type":0,"size":{"columns":5,"rows":2}})).collect::<Vec<_>>()})
}
fn start(
    package: Package,
    net: &Network,
    snapshot: &Snapshot,
    app: &tauri::AppHandle,
    directory: &std::path::Path,
) -> Session {
    let (token, nonce) = (deckweave_core::new_id(), deckweave_core::new_id());
    let (sender, receiver) = Network::channel();
    let (error_sender, errors) = mpsc::sync_channel(8);
    let mut s = Session {
        package,
        token,
        nonce,
        receiver,
        sender,
        socket: None,
        inspectors: BTreeMap::new(),
        slots: BTreeMap::new(),
        retired: RetiredContexts::default(),
        child: None,
        process_tree: None,
        window: None,
        started: Instant::now(),
        error: None,
        errors,
        visuals: BTreeMap::new(),
        renderer: deckweave_n4::Renderer::new(),
    };
    let result = (|| -> Result<(), String> {
        if let Some(e) = &s.package.error {
            return Err(e.clone());
        }
        if let Some(error) = weather_permission_error(
            &s.package,
            snapshot.configuration.preferences.allow_weather_geolocation,
            snapshot.configuration.locale,
        ) {
            return Err(error);
        }
        let environment = catalog::prepare_runtime(&mut s.package, directory)?;
        let entry = s.package.entry.as_ref().ok_or("Missing entry point")?;
        let relative = entry
            .strip_prefix(&s.package.root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let information = info(snapshot, &s.package);
        net.add(Route {
            uuid: s.token.clone(),
            nonce: s.nonce.clone(),
            role: "registerPlugin".into(),
            root: s.package.root.clone(),
            page: relative.clone(),
            info: information.clone(),
            action_info: None,
            sender: s.sender.clone(),
            errors: Some(error_sender),
        });
        let extension = entry.extension().and_then(|s| s.to_str()).unwrap_or("");
        if matches!(extension, "html" | "htm") {
            let label = format!("plugin-{}", s.token);
            let url = net.url(&s.nonce, &relative);
            let handle = app.clone();
            let target = label.clone();
            let data = directory.join("plugin-web").join(&s.package.id);
            if cfg!(windows) {
                let edge = [
                    std::env::var_os("ProgramFiles(x86)"),
                    std::env::var_os("ProgramFiles"),
                ]
                .into_iter()
                .flatten()
                .map(PathBuf::from)
                .map(|p| p.join("Microsoft/Edge/Application/msedge.exe"))
                .find(|p| p.is_file())
                .ok_or("Microsoft Edge is unavailable")?;
                let mut command = Process::new(edge);
                command
                    .args([
                        "--headless=new",
                        "--disable-gpu",
                        "--no-first-run",
                        "--no-default-browser-check",
                        "--disable-background-timer-throttling",
                        "--disable-renderer-backgrounding",
                        "--disable-backgrounding-occluded-windows",
                    ])
                    .arg(format!("--user-data-dir={}", data.display()))
                    .arg(&url)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x08000000);
                }
                let mut child = command.spawn().map_err(|e| e.to_string())?;
                match deckweave_platform::contain_process(&child) {
                    Ok(tree) => {
                        s.process_tree = Some(tree);
                        s.child = Some(child);
                    }
                    Err(error) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(format!("Cannot supervise HTML process tree: {error}"));
                    }
                }
                return Ok(());
            }
            app.run_on_main_thread(move||{let _=WebviewWindowBuilder::new(&handle,&target,WebviewUrl::External(url.parse().expect("local URL"))).title("DeckWeave plugin runtime").data_directory(data).visible(false).skip_taskbar(true).additional_browser_args("--disable-gpu --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows").build();}).map_err(|e|e.to_string())?;
            s.window = Some(label);
        } else {
            let mut command = if matches!(extension, "js" | "mjs" | "cjs") {
                let mut c = Process::new("node");
                c.arg(entry);
                c
            } else {
                Process::new(entry)
            };
            command
                .current_dir(&s.package.root)
                .args([
                    "-port",
                    &net.port.to_string(),
                    "-pluginUUID",
                    &s.token,
                    "-registerEvent",
                    "registerPlugin",
                    "-info",
                    &information.to_string(),
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("DECKWEAVE_") {
                    command.env_remove(key);
                }
            }
            if let Some(environment) = environment {
                command
                    .env("USERPROFILE", &environment.home)
                    .env("HOME", &environment.home)
                    .env("APPDATA", &environment.roaming)
                    .env("LOCALAPPDATA", &environment.local)
                    .env("TEMP", &environment.temp)
                    .env("TMP", &environment.temp);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            s.child = Some(command.spawn().map_err(|e| e.to_string())?);
        }
        Ok(())
    })();
    if let Err(e) = result {
        s.error = Some(e);
    }
    s
}
fn run(
    core: Arc<Mutex<Core>>,
    app: tauri::AppHandle,
    directory: PathBuf,
    epoch: Arc<AtomicU64>,
    packages: Arc<Mutex<Vec<Package>>>,
    stop: Arc<AtomicBool>,
    requests: Receiver<Request>,
) {
    let mut net = None;
    let mut sessions: BTreeMap<String, Session> = BTreeMap::new();
    let mut last_revision = u64::MAX;
    let Ok(mut snapshot) = core.lock().map(|c| c.snapshot()) else {
        return;
    };
    let mut active = BTreeMap::new();
    let mut assigned = BTreeSet::new();
    let mut weather_geolocation = snapshot.configuration.preferences.allow_weather_geolocation;
    while !stop.load(Ordering::Relaxed) {
        let Ok((revision, changed, retained, inputs)) = core.lock().map(|mut c| {
            (
                c.revision(),
                if c.revision() != last_revision {
                    Some(c.snapshot())
                } else {
                    None
                },
                c.retained_actions(),
                c.take_plugin_inputs(),
            )
        }) else {
            break;
        };
        if let Some(changed) = changed {
            snapshot = changed;
        }
        let weather_permission_changed =
            weather_geolocation != snapshot.configuration.preferences.allow_weather_geolocation;
        weather_geolocation = snapshot.configuration.preferences.allow_weather_geolocation;
        if revision != last_revision {
            assigned = assigned_packages(&snapshot);
        }
        if revision != last_revision || retained != snapshot.retained_actions {
            last_revision = revision;
            snapshot.retained_actions = retained;
            active = slots(&snapshot);
        }
        let needed: BTreeSet<String> = active
            .values()
            .filter_map(|s| s.binding.plugin.as_ref().map(|p| p.package.clone()))
            .collect();
        if !needed.is_empty() && net.is_none() {
            net = Network::start().ok();
        }
        if let Some(network) = &net {
            if weather_permission_changed
                && let Some(session) = sessions.remove("com.elgato.weather")
            {
                session.close(&app, network);
            }
            let unused: Vec<_> = sessions
                .keys()
                .filter(|p| !assigned.contains(*p))
                .cloned()
                .collect();
            for id in unused {
                if let Some(mut s) = sessions.remove(&id) {
                    for slot in s.slots.values().cloned().collect::<Vec<_>>() {
                        s.send(slot.event("willDisappear"));
                    }
                    s.close(&app, network);
                }
            }
            for id in &needed {
                if !sessions.contains_key(id) {
                    let package = packages
                        .lock()
                        .ok()
                        .and_then(|p| p.iter().find(|p| p.id == *id).cloned());
                    if let Some(p) = package {
                        sessions.insert(id.clone(), start(p, network, &snapshot, &app, &directory));
                    } else {
                        for slot in active
                            .values()
                            .filter(|s| s.binding.plugin.as_ref().is_some_and(|p| p.package == *id))
                        {
                            feedback(
                                &core,
                                &app,
                                &epoch,
                                &slot.binding.id,
                                "error",
                                json!({"message":"Plugin package is not installed"}),
                            );
                        }
                    }
                }
            }
            while let Ok(request) = requests.try_recv() {
                match request {
                    Request::Restart(id) => {
                        if let Some(s) = sessions.remove(&id) {
                            s.close(&app, network);
                        }
                    }
                    Request::Inspector { context, reply } => {
                        let result =
                            prepare_inspector(&context, &mut sessions, network, &active, &snapshot);
                        let _ = reply.send(result);
                    }
                }
            }
            for (id, session) in &mut sessions {
                while let Ok(error) = session.errors.try_recv() {
                    session.error = Some(error);
                }
                while let Ok(peer) = session.receiver.try_recv() {
                    if peer.role == "registerPlugin" {
                        session.socket = Some(peer.socket);
                        session.error = None;
                        for d in &snapshot.configuration.devices {
                            session.send(json!({"event":"deviceDidConnect","device":d.id,"deviceInfo":{"name":d.name,"type":0,"size":{"columns":5,"rows":2}}}));
                        }
                        session.slots.clear();
                        session.retired.entries.clear();
                    } else {
                        let context = peer.uuid;
                        session.inspectors.insert(context.clone(), peer.socket);
                        if let Some(slot) = active.get(&context) {
                            session.send(slot.event("propertyInspectorDidAppear"));
                        }
                    }
                }
                let wanted: BTreeMap<_, _> = active
                    .iter()
                    .filter(|(_, s)| s.binding.plugin.as_ref().is_some_and(|p| p.package == *id))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if session.socket.is_none() && session.error.is_none() {
                    for ctx in wanted.keys() {
                        feedback(&core, &app, &epoch, ctx, "starting", json!({}));
                    }
                }
                if session.socket.is_some() {
                    for (ctx, old) in session.slots.clone() {
                        if !wanted.contains_key(&ctx) {
                            session.send(old.event("willDisappear"));
                            session.retired.retire(old, Instant::now());
                            network.remove(&ctx);
                            session.inspectors.remove(&ctx);
                            session.visuals.remove(&ctx);
                        }
                    }
                    for (ctx, slot) in &wanted {
                        session.retired.entries.remove(ctx);
                        if !session.slots.contains_key(ctx) {
                            artwork::initial(session, slot, &core, &app, &epoch);
                            session.send(slot.event("willAppear"));
                            feedback(&core, &app, &epoch, ctx, "ready", json!({}));
                        } else if session.slots[ctx].binding.plugin != slot.binding.plugin {
                            session.send(slot.event("didReceiveSettings"));
                            if let Some(pi) = session.inspectors.get_mut(ctx) {
                                send(pi, slot.event("didReceiveSettings"));
                            }
                        }
                    }
                }
                session.slots = wanted;
                session.retired.prune(Instant::now());
                if let Some(child) = session.child.as_mut()
                    && let Ok(Some(status)) = child.try_wait()
                {
                    session.error = Some(format!("Plugin process exited: {status}"));
                }
                if session.socket.is_none()
                    && session.started.elapsed() > Duration::from_secs(8)
                    && session.error.is_none()
                {
                    session.error = Some("Plugin did not register within 8 seconds".into());
                }
                if session.error.is_some() {
                    if let Some(mut child) = session.child.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    session.process_tree = None;
                }
                let mut incoming = vec![];
                if let Some(socket) = session.socket.as_mut() {
                    match drain(socket) {
                        Ok(messages) => incoming.extend(messages.into_iter().map(|v| (None, v))),
                        Err(e) => {
                            session.error = Some(e);
                            session.socket = None;
                        }
                    }
                }
                let mut closed = vec![];
                for (ctx, socket) in &mut session.inspectors {
                    match drain(socket) {
                        Ok(messages) => {
                            incoming.extend(messages.into_iter().map(|v| (Some(ctx.clone()), v)))
                        }
                        Err(_) => closed.push(ctx.clone()),
                    }
                }
                for ctx in closed {
                    session.inspectors.remove(&ctx);
                    if let Some(slot) = session.slots.get(&ctx).cloned() {
                        session.send(slot.event("propertyInspectorDidDisappear"));
                    }
                }
                for (pi, message) in incoming {
                    handle(
                        session,
                        pi.as_deref(),
                        message,
                        &core,
                        &app,
                        &epoch,
                        &directory,
                    );
                }
                if let Some(error) = &session.error {
                    for ctx in session.slots.keys() {
                        feedback(&core, &app, &epoch, ctx, "error", json!({"message":error}));
                    }
                }
            }
            for input in inputs {
                if let Some(plugin) = input.binding.plugin.as_ref()
                    && let Some(session) = sessions.get_mut(&plugin.package)
                    && let Some(slot) = session.slots.get(&input.binding.id).cloned()
                {
                    let mut value = slot.event(&input.event);
                    if input.event == "deckweaveCancel" {
                        session.send(slot.event("willDisappear"));
                        session.send(slot.event("willAppear"));
                        continue;
                    }
                    if session.socket.is_none() {
                        if let Ok(mut core) = core.lock() {
                            core.action_error(
                                &input.device_id,
                                &input.owner_id,
                                session
                                    .error
                                    .as_deref()
                                    .unwrap_or("Plugin is not ready; retry after it starts"),
                            );
                        }
                        notify(&app, &epoch);
                        continue;
                    }
                    if let Ok(core) = core.lock() {
                        value["payload"]["state"] =
                            json!(core.action_state(&input.device_id, &input.binding.id));
                    }
                    if input.event == "dialRotate" {
                        value["payload"]["ticks"] = json!(input.ticks);
                        value["payload"]["pressed"] = json!(false);
                    }
                    if input.event == "touchTap" {
                        value["payload"]["hold"] = json!(input.hold);
                        if let Some(position) = input.position {
                            value["payload"]["tapPos"] = json!([
                                (position.x * 200.0).floor().min(199.0),
                                (position.y.unwrap_or(0.0) * 100.0).floor().min(99.0)
                            ]);
                        }
                    }
                    if slot.controller == "Panel" {
                        value["payload"]["position"] = json!(input.position);
                        value["payload"]["canvas"] = json!(slot.canvas);
                        value["payload"]["hold"] = json!(input.hold);
                        value["payload"]["control"] = json!(input.control_id);
                        if input.event == "panelSwipe" {
                            value["payload"]["right"] = json!(input.ticks > 0);
                        }
                    }
                    if slot.controller == "Keypad" {
                        if input.event == "dialDown" {
                            value["event"] = json!("keyDown");
                        }
                        if input.event == "dialUp" {
                            value["event"] = json!("keyUp");
                        }
                    }
                    session.send(value);
                }
            }
        } else {
            while let Ok(Request::Inspector { reply, .. }) = requests.try_recv() {
                let _ = reply.send(Err("No plugin runtime is active".into()));
            }
        }
        if needed.is_empty() && sessions.is_empty() {
            net = None;
        }
        thread::sleep(Duration::from_millis(25));
    }
    if let Some(net) = net {
        for (_, s) in sessions {
            s.close(&app, &net);
        }
    }
}
fn prepare_inspector(
    context: &str,
    sessions: &mut BTreeMap<String, Session>,
    net: &Network,
    active: &BTreeMap<String, Slot>,
    snapshot: &Snapshot,
) -> Result<(String, String), String> {
    let slot = active.get(context).ok_or("Action is no longer visible")?;
    let plugin = slot.binding.plugin.as_ref().ok_or("Not a plugin action")?;
    let session = sessions
        .get_mut(&plugin.package)
        .ok_or("Plugin is not running")?;
    if let Some(error) = weather_permission_error(
        &session.package,
        snapshot.configuration.preferences.allow_weather_geolocation,
        snapshot.configuration.locale,
    ) {
        return Err(error);
    }
    let page = session
        .package
        .actions
        .iter()
        .find(|a| a.id == plugin.action)
        .and_then(|a| a.inspector.clone())
        .ok_or("Plugin has no property inspector")?;
    let nonce = deckweave_core::new_id();
    net.add(Route {
        uuid: context.into(),
        nonce: nonce.clone(),
        role: "registerPropertyInspector".into(),
        root: session.package.root.clone(),
        page: page.clone(),
        info: info(snapshot, &session.package),
        action_info: Some(slot.event("willAppear")),
        sender: session.sender.clone(),
        errors: None,
    });
    Ok((net.url(&nonce, &page), session.package.id.clone()))
}

fn weather_permission_error(
    package: &Package,
    allowed: bool,
    locale: deckweave_core::Locale,
) -> Option<String> {
    if package.id != "com.elgato.weather" || allowed {
        return None;
    }
    Some(if locale == deckweave_core::Locale::Ru {
        "Elgato Weather выключен: оригинальный плагин определяет внешний IP через api.ipify.org и передаёт его сервисам геолокации. Разрешите это в общих настройках приложения или используйте DeckWeave Weather."
    } else {
        "Elgato Weather is disabled: the original plugin detects your public IP via api.ipify.org and sends it to geolocation services. Allow this in application settings or use DeckWeave Weather."
    }.into())
}
fn handle(
    session: &mut Session,
    pi: Option<&str>,
    message: Value,
    core: &Mutex<Core>,
    app: &tauri::AppHandle,
    epoch: &AtomicU64,
    directory: &std::path::Path,
) {
    let event = message["event"].as_str().unwrap_or("");
    let context = message["context"].as_str().unwrap_or("");
    let payload = &message["payload"];
    if let Some(pi) = pi
        && pi != context
        && !matches!(
            event,
            "getGlobalSettings" | "setGlobalSettings" | "logMessage"
        )
    {
        return;
    }
    let slot = session.slots.get(context).cloned();
    match event {
        "getGlobalSettings" => {
            let path = directory
                .join("plugin-settings")
                .join(format!("{}.json", session.package.id));
            let settings = std::fs::read(path)
                .ok()
                .filter(|v| v.len() <= 65536)
                .and_then(|v| serde_json::from_slice::<Value>(&v).ok())
                .unwrap_or(json!({}));
            let value = json!({"event":"didReceiveGlobalSettings","context":if context.is_empty(){session.token.as_str()}else{context},"payload":{"settings":settings}});
            if let Some(pi) = pi {
                if let Some(socket) = session.inspectors.get_mut(pi) {
                    send(socket, value);
                }
            } else {
                session.send(value);
            }
        }
        "setGlobalSettings" => {
            if let Ok(bytes) = serde_json::to_vec(payload)
                && bytes.len() <= 65536
            {
                let folder = directory.join("plugin-settings");
                if std::fs::create_dir_all(&folder).is_ok() {
                    let path = folder.join(format!("{}.json", session.package.id));
                    let temporary = path.with_extension("tmp");
                    if std::fs::write(&temporary, bytes).is_ok() {
                        let _ = std::fs::rename(temporary, path);
                    }
                }
            }
        }
        "setSettings" => {
            let retired = slot.is_none() && pi.is_none();
            let writable = slot.or_else(|| {
                if retired {
                    session
                        .retired
                        .entries
                        .get(context)
                        .filter(|r| r.until > Instant::now())
                        .map(|r| r.slot.clone())
                } else {
                    None
                }
            });
            if let Some(slot) = writable {
                let baseline = if retired {
                    slot.binding.plugin.as_ref().map(|p| &p.settings)
                } else {
                    None
                };
                let changed = core
                    .lock()
                    .is_ok_and(|mut c| persist_settings(&mut c, &slot, payload, baseline));
                if changed {
                    // Subsequent queued writes compare against the last accepted value.
                    // If the owner/PI edits settings meanwhile, a retired writer cannot clobber it.
                    let remembered = if retired {
                        session
                            .retired
                            .entries
                            .get_mut(context)
                            .map(|r| &mut r.slot)
                    } else {
                        session.slots.get_mut(context)
                    };
                    if let Some(plugin) = remembered.and_then(|s| s.binding.plugin.as_mut()) {
                        plugin.settings = payload.clone();
                    }
                    if !retired && let Some(current) = session.slots.get(context) {
                        // Remembering the accepted baseline suppresses the next loop's
                        // settings diff, so deliver the normal settings event explicitly.
                        let value = current.event("didReceiveSettings");
                        session.send(value.clone());
                        if let Some(inspector) = session.inspectors.get_mut(context) {
                            send(inspector, value);
                        }
                    }
                    notify(app, epoch);
                }
            }
        }
        "getSettings" => {
            if let Some(slot) = slot {
                let mut value = slot.event("didReceiveSettings");
                if let Ok(core) = core.lock()
                    && let Some(settings) = core.plugin_settings(context)
                {
                    value["payload"]["settings"] = settings;
                }
                if let Some(pi) = pi {
                    if let Some(socket) = session.inspectors.get_mut(pi) {
                        send(socket, value);
                    }
                } else {
                    session.send(value);
                }
            }
        }
        "sendToPlugin" if pi.is_some() => {
            if slot.is_some() {
                session.send(message);
            }
        }
        "sendToPropertyInspector" if pi.is_none() => {
            if let Some(socket) = session.inspectors.get_mut(context) {
                send(socket, message);
            }
        }
        "setImage" | "setTitle" | "setState" | "setFeedback" | "setFeedbackLayout"
            if pi.is_none() && slot.is_some() =>
        {
            if let Err(error) = artwork::update(
                session,
                slot.as_ref().expect("checked slot"),
                event,
                payload,
                core,
                app,
                epoch,
            ) {
                feedback(core, app, epoch, context, "error", json!({"message":error}));
            }
        }
        "showAlert" if pi.is_none() && slot.is_some() => {
            feedback(
                core,
                app,
                epoch,
                context,
                "error",
                json!({"message":"Plugin reported an action error"}),
            );
            if let Some(slot) = slot
                && let Ok(mut core) = core.lock()
            {
                core.action_error(&slot.device, &slot.owner, "Plugin reported an action error");
            }
            notify(app, epoch);
        }
        "showOk" if pi.is_none() && slot.is_some() => {
            feedback(core, app, epoch, context, "ready", json!({}))
        }
        _ => {}
    }
}
fn persist_settings(
    core: &mut Core,
    slot: &Slot,
    settings: &Value,
    baseline: Option<&Value>,
) -> bool {
    let Some(expected) = slot.binding.plugin.as_ref() else {
        return false;
    };
    let Some((device, current)) = core.plugin_assignment(&slot.binding.id) else {
        return false;
    };
    if device != slot.device
        || current.package != expected.package
        || current.action != expected.action
        || current.controller != expected.controller
        || baseline.is_some_and(|baseline| baseline != &current.settings)
    {
        return false;
    }
    core.dispatch(
        Command::PluginSettings {
            device_id: slot.device.clone(),
            binding_id: slot.binding.id.clone(),
            settings: settings.clone(),
            expected_revision: core.revision(),
        },
        "plugin",
    )
    .is_ok()
}
fn feedback(
    core: &Mutex<Core>,
    app: &tauri::AppHandle,
    epoch: &AtomicU64,
    context: &str,
    event: &str,
    payload: Value,
) {
    if core
        .lock()
        .is_ok_and(|mut c| c.plugin_feedback(context, event, payload))
    {
        notify(app, epoch);
    }
}
fn notify(app: &tauri::AppHandle, epoch: &AtomicU64) {
    epoch.fetch_add(1, Ordering::Relaxed);
    let _ = app.emit("deckweave:changed", ());
}
fn send(socket: &mut Socket, value: Value) {
    let _ = socket.send(tungstenite::Message::Text(value.to_string().into()));
}
fn would_block(error: &tungstenite::Error) -> bool {
    matches!(error,tungstenite::Error::Io(e)if e.kind()==std::io::ErrorKind::WouldBlock)
}
fn drain(socket: &mut Socket) -> Result<Vec<Value>, String> {
    let mut messages = vec![];
    for _ in 0..32 {
        match socket.read() {
            Ok(tungstenite::Message::Text(text)) => {
                if let Ok(v) = serde_json::from_str(&text) {
                    messages.push(v);
                }
            }
            Ok(tungstenite::Message::Close(_)) => return Err("Plugin connection closed".into()),
            Ok(_) => {}
            Err(e) if would_block(&e) => break,
            Err(e) => return Err(e.to_string()),
        }
    }
    let _ = socket.flush();
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deckweave_core::{InputEvent, PluginAction};
    fn settings_fixture() -> (Core, Slot, String) {
        let mut core = Core::memory().unwrap();
        let device = core.snapshot().configuration.devices[0].clone();
        let page = device.profile.active_page_id.clone();
        core.dispatch(
            Command::ConfigurePlugin {
                device_id: device.id.clone(),
                page_id: page.clone(),
                control_id: "key-1".into(),
                plugin: PluginAction {
                    controller: None,
                    package: "fixture".into(),
                    action: "fixture.timer".into(),
                    settings: json!({"running":false,"duration":60}),
                },
                label: "Timer".into(),
                expected_revision: core.revision(),
            },
            "test",
        )
        .unwrap();
        let slot = slots(&core.snapshot())
            .into_values()
            .find(|slot| {
                slot.binding
                    .plugin
                    .as_ref()
                    .is_some_and(|p| p.package == "fixture")
            })
            .unwrap();
        (core, slot, page)
    }
    #[test]
    fn retired_backend_settings_survive_navigation_and_do_not_overwrite_later_inspector_edits() {
        let (mut core, slot, _) = settings_fixture();
        core.dispatch(
            Command::AddPage {
                device_id: slot.device.clone(),
                expected_revision: core.revision(),
            },
            "test",
        )
        .unwrap();
        assert!(!slots(&core.snapshot()).contains_key(&slot.binding.id));
        let mut retired = RetiredContexts::default();
        retired.retire(slot.clone(), Instant::now());
        let baseline = &retired.entries[&slot.binding.id]
            .slot
            .binding
            .plugin
            .as_ref()
            .unwrap()
            .settings;
        let started = json!({"running":true,"duration":60,"elapsed":1});
        assert!(persist_settings(&mut core, &slot, &started, Some(baseline)));
        assert_eq!(
            core.plugin_settings(&slot.binding.id),
            Some(started.clone())
        );
        // Another queued backend write uses its last accepted settings as the baseline.
        retired
            .entries
            .get_mut(&slot.binding.id)
            .unwrap()
            .slot
            .binding
            .plugin
            .as_mut()
            .unwrap()
            .settings = started.clone();
        let paused = json!({"running":false,"duration":60,"elapsed":2});
        assert!(persist_settings(&mut core, &slot, &paused, Some(&started)));
        // The user changes the duration while the old context is hidden.
        let edited = json!({"running":false,"duration":120,"elapsed":2});
        core.dispatch(
            Command::PluginSettings {
                device_id: slot.device.clone(),
                binding_id: slot.binding.id.clone(),
                settings: edited.clone(),
                expected_revision: core.revision(),
            },
            "inspector",
        )
        .unwrap();
        assert!(!persist_settings(
            &mut core,
            &slot,
            &json!({"running":false,"duration":60,"elapsed":3}),
            Some(&paused)
        ));
        assert_eq!(core.plugin_settings(&slot.binding.id), Some(edited));
    }
    #[test]
    fn retired_settings_reject_removed_rebound_and_wrong_package_or_action_contexts() {
        let (mut core, slot, page) = settings_fixture();
        let baseline = slot.binding.plugin.as_ref().unwrap().settings.clone();
        let mut impostor = slot.clone();
        impostor.binding.plugin.as_mut().unwrap().package = "other.package".into();
        assert!(!persist_settings(
            &mut core,
            &impostor,
            &json!({}),
            Some(&baseline)
        ));
        impostor = slot.clone();
        impostor.binding.plugin.as_mut().unwrap().action = "fixture.other".into();
        assert!(!persist_settings(
            &mut core,
            &impostor,
            &json!({}),
            Some(&baseline)
        ));
        core.dispatch(
            Command::ConfigurePlugin {
                device_id: slot.device.clone(),
                page_id: page,
                control_id: "key-1".into(),
                plugin: PluginAction {
                    controller: None,
                    package: "fixture".into(),
                    action: "fixture.other".into(),
                    settings: json!({"preserve":true}),
                },
                label: "Replacement".into(),
                expected_revision: core.revision(),
            },
            "test",
        )
        .unwrap();
        assert!(core.plugin_assignment(&slot.binding.id).is_none());
        assert!(!persist_settings(
            &mut core,
            &slot,
            &json!({"running":true}),
            Some(&baseline)
        ));
        let replacement = slots(&core.snapshot())
            .into_values()
            .find(|s| {
                s.binding
                    .plugin
                    .as_ref()
                    .is_some_and(|p| p.package == "fixture")
            })
            .unwrap();
        assert_eq!(
            core.plugin_settings(&replacement.binding.id),
            Some(json!({"preserve":true}))
        );
    }
    #[test]
    fn retired_contexts_expire_and_have_a_fixed_entry_limit() {
        let (_, slot, _) = settings_fixture();
        let now = Instant::now();
        let mut retired = RetiredContexts::default();
        for n in 0..300 {
            let mut candidate = slot.clone();
            candidate.binding.id = format!("retired-{n}");
            retired.retire(candidate, now);
        }
        assert_eq!(retired.entries.len(), RetiredContexts::LIMIT);
        retired.prune(now + RetiredContexts::LIFETIME);
        assert!(retired.entries.is_empty());
    }
    #[test]
    fn elgato_weather_requires_explicit_permission_before_process_or_inspector() {
        let mut package = Package {
            id: "com.elgato.weather".into(),
            name: "Elgato Weather".into(),
            version: "1.2.1.103".into(),
            actions: vec![],
            error: None,
            resource_only: false,
            development: false,
            root: PathBuf::new(),
            entry: None,
            isolated_weather: true,
        };
        let reason = weather_permission_error(&package, false, deckweave_core::Locale::En).unwrap();
        assert!(reason.contains("api.ipify.org"));
        assert!(reason.contains("application settings"));
        assert!(weather_permission_error(&package, true, deckweave_core::Locale::En).is_none());
        package.id = "other.plugin".into();
        assert!(weather_permission_error(&package, false, deckweave_core::Locale::En).is_none());
    }
    #[test]
    fn original_plugin_context_is_retained_until_release_after_navigation() {
        let mut core = Core::memory().unwrap();
        let d = core.snapshot().configuration.devices[0].clone();
        core.dispatch(
            Command::ConfigurePlugin {
                device_id: d.id.clone(),
                page_id: d.profile.active_page_id.clone(),
                control_id: "key-1".into(),
                plugin: PluginAction {
                    controller: None,
                    package: "fixture".into(),
                    action: "fixture.action".into(),
                    settings: json!({}),
                },
                label: "Test".into(),
                expected_revision: 0,
            },
            "test",
        )
        .unwrap();
        let binding = core.snapshot().configuration.devices[0].profile.pages[0].bindings["key-1"]
            .id
            .clone();
        core.dispatch(
            Command::Input {
                device_id: d.id.clone(),
                control_id: "key-1".into(),
                event: InputEvent::Down,
            },
            "test",
        )
        .unwrap();
        core.take_plugin_inputs();
        let revision = core.revision();
        core.dispatch(
            Command::AddPage {
                device_id: d.id.clone(),
                expected_revision: revision,
            },
            "test",
        )
        .unwrap();
        assert!(slots(&core.snapshot()).contains_key(&binding));
        core.dispatch(
            Command::Input {
                device_id: d.id,
                control_id: "key-1".into(),
                event: InputEvent::Up,
            },
            "test",
        )
        .unwrap();
        assert!(slots(&core.snapshot()).contains_key(&binding));
        core.take_plugin_inputs();
        assert!(!slots(&core.snapshot()).contains_key(&binding));
        assert!(assigned_packages(&core.snapshot()).contains("fixture"));
    }
}
