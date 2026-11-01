mod actions;
pub mod model;
mod panels;
pub mod store;
pub use actions::Effect;

pub use model::*;
use panels::relative_point;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use store::Store;

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub code: String,
    pub message: String,
}

impl Error {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::new("storage", value.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::new("data", value.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
struct Held {
    source: String,
    binding: Option<Binding>,
    started: u64,
    hold_fired: bool,
    plugin_started: bool,
    control: Control,
    bounds: [f64; 4],
    pointer: PointerData,
}
#[derive(Clone, Copy)]
struct PointerData {
    position: PointerPosition,
    hold: bool,
    canvas: [u32; 2],
}

pub struct Core {
    store: Store,
    config: Configuration,
    feedback: BTreeMap<(String, String), Feedback>,
    held: BTreeMap<(String, String), Held>,
    editors: BTreeSet<String>,
    sequence: u64,
    connections: BTreeMap<String, DeviceRuntime>,
    provider_error: Option<String>,
    plugin_inputs: VecDeque<PluginInput>,
    actions: actions::Runtime,
}

impl Core {
    pub fn open(path: &Path) -> Result<Self> {
        Self::from_store(Store::open(path)?)
    }
    pub fn memory() -> Result<Self> {
        Self::from_store(Store::memory()?)
    }

    fn from_store(store: Store) -> Result<Self> {
        let config = store.load()?;
        validate(&config)?;
        Ok(Self {
            store,
            config,
            feedback: BTreeMap::new(),
            held: BTreeMap::new(),
            editors: BTreeSet::new(),
            sequence: 0,
            connections: BTreeMap::new(),
            provider_error: None,
            plugin_inputs: VecDeque::new(),
            actions: actions::Runtime::default(),
        })
    }

    pub fn revision(&self) -> u64 {
        self.config.revision
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut configuration = self.config.clone();
        // Project pinned actions onto the active page for every consumer: UI, host and hardware.
        for device in &mut configuration.devices {
            if let Some(page) = device
                .profile
                .pages
                .iter_mut()
                .find(|p| p.id == device.profile.active_page_id)
            {
                if let Some(panel) = &page.panel {
                    for region in &panel.regions {
                        if let Some(binding) = page.bindings.get(&region.control_id()).cloned() {
                            for dial in &region.dials {
                                page.bindings.insert(dial.clone(), binding.clone());
                            }
                        }
                    }
                } else {
                    for number in 1..=4 {
                        if let Some(binding) = page.bindings.get(&format!("dial-{number}")).cloned()
                        {
                            page.bindings
                                .entry(format!("touch-{number}"))
                                .or_insert(binding);
                        }
                    }
                }
                page.bindings.extend(device.pinned.clone());
            }
        }
        Snapshot {
            configuration,
            geometries: self
                .config
                .devices
                .iter()
                .map(|device| (device.id.clone(), device_geometry(device)))
                .collect(),
            connections: self
                .config
                .devices
                .iter()
                .map(|d| {
                    (
                        d.id.clone(),
                        self.connections
                            .get(&d.id)
                            .cloned()
                            .unwrap_or_else(|| DeviceRuntime::initial(&d.provider_id)),
                    )
                })
                .collect(),
            provider_error: self.provider_error.clone(),
            feedback: self.render_feedback(),
            pressed: self
                .held
                .keys()
                .map(|(d, c)| PressedControl {
                    device_id: d.clone(),
                    control_id: c.clone(),
                })
                .collect(),
            editing_windows: self.editors.len(),
            smart_profiles_paused: !self.editors.is_empty(),
            event_sequence: self.sequence,
            retained_actions: self.retained_actions(),
        }
    }

    pub fn editor_opened(&mut self, label: &str) {
        if self.editors.insert(label.into()) {
            self.sequence += 1;
        }
    }
    pub fn retained_actions(&self) -> Vec<RetainedAction> {
        self.actions
            .retained()
            .into_iter()
            .chain(self.held.iter().filter_map(|((device, control), held)| {
                held.binding.as_ref().map(|b| RetainedAction {
                    device_id: device.clone(),
                    control_id: control.clone(),
                    binding_id: b.id.clone(),
                })
            }))
            .chain(self.plugin_inputs.iter().map(|i| RetainedAction {
                device_id: i.device_id.clone(),
                control_id: i.control_id.clone(),
                binding_id: i.binding.id.clone(),
            }))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn action_state(&self, device: &str, context: &str) -> u64 {
        self.feedback
            .get(&(device.into(), context.into()))
            .map_or(0, |f| f.value.max(0) as u64)
    }
    pub fn editor_closed(&mut self, label: &str) {
        self.editors.remove(label);
        self.cancel_holds(|_, _, held| held.source == label);
        self.actions.cancel_source(label);
        self.sequence += 1;
    }

    pub fn dispatch(&mut self, command: Command, source: &str) -> Result<Snapshot> {
        if let Command::CancelActions { device_id } = &command {
            self.plugin_inputs.retain(|i| i.device_id != *device_id);
            self.actions.cancel(device_id);
            self.cancel_holds(|device, _, _| device == device_id);
            self.sequence += 1;
            return Ok(self.snapshot());
        }
        if let Command::Input {
            device_id,
            control_id,
            event,
        } = command
        {
            self.input(&device_id, &control_id, event, source)?;
            self.sequence += 1;
            return Ok(self.snapshot());
        }
        let expected = command
            .expected_revision()
            .ok_or_else(|| Error::new("command", "Missing revision"))?;
        if expected != self.config.revision {
            return Err(Error::new(
                "conflict",
                "Configuration changed; reload and retry",
            ));
        }
        let mut next = self.config.clone();
        let panel_edit = if let Command::SetPanelLayout { device_id, .. } = &command {
            Some(device_id.clone())
        } else {
            None
        };
        match command {
            Command::SetPreferences { preferences, .. } => {
                if !(75..=150).contains(&preferences.preview_scale) {
                    return Err(Error::new(
                        "preferences",
                        "Preview scale must be between 75 and 150",
                    ));
                }
                next.preferences = preferences;
            }
            Command::DevicePreferences {
                device_id,
                default_profile,
                ..
            } => {
                device_mut(&mut next, &device_id)?.default_profile = default_profile;
            }
            Command::FolderBehavior {
                device_id,
                page_id,
                close_after_action,
                ..
            } => {
                let page = device_mut(&mut next, &device_id)?
                    .profile
                    .pages
                    .iter_mut()
                    .find(|p| p.id == page_id && p.parent.is_some())
                    .ok_or_else(|| Error::new("folder", "Folder not found"))?;
                page.close_after_action = close_after_action;
            }
            Command::SetPanelLayout {
                device_id,
                page_id,
                layout,
                ..
            } => {
                let d = device_mut(&mut next, &device_id)?;
                let page = d
                    .profile
                    .pages
                    .iter_mut()
                    .find(|p| p.id == page_id)
                    .ok_or_else(|| Error::new("not_found", "Page not found"))?;
                if let Some(panel) = &layout {
                    for region in &panel.regions {
                        for dial in &region.dials {
                            if let Some(mut old) = page.bindings.remove(dial) {
                                if let Some(plugin) = old.plugin.as_mut() {
                                    plugin.controller.get_or_insert(Controller::Encoder);
                                }
                                page.bindings.entry(region.control_id()).or_insert(old);
                            }
                        }
                    }
                }
                let ids: BTreeSet<_> = layout
                    .as_ref()
                    .map(|p| p.regions.iter().map(PanelRegion::control_id).collect())
                    .unwrap_or_default();
                page.bindings.retain(|id, _| {
                    (!id.starts_with("panel-") || ids.contains(id))
                        && (layout.is_none() || !id.starts_with("touch-"))
                });
                page.panel = layout;
            }
            Command::ConfigureProgram {
                device_id,
                page_id,
                control_id,
                plan,
                label,
                ..
            } => {
                let device = device_mut(&mut next, &device_id)?;
                let independent = matches!(plan, Plan::PageCycle { .. });
                let control_id = action_control(device, &page_id, &control_id, false, independent)?;
                let mut binding = Binding::new(ActionKind::Program, &label);
                binding.plan = Some(plan);
                if let Some(pinned) = device.pinned.get_mut(&control_id) {
                    *pinned = binding;
                } else {
                    device
                        .profile
                        .pages
                        .iter_mut()
                        .find(|p| p.id == page_id)
                        .ok_or_else(|| Error::new("not_found", "Page not found"))?
                        .bindings
                        .insert(control_id, binding);
                }
            }
            Command::RenameBinding {
                device_id,
                binding_id,
                label,
                ..
            } => {
                let device = device_mut(&mut next, &device_id)?;
                let b = device
                    .pinned
                    .values_mut()
                    .chain(
                        device
                            .profile
                            .pages
                            .iter_mut()
                            .flat_map(|p| p.bindings.values_mut()),
                    )
                    .find(|b| b.id == binding_id)
                    .ok_or_else(|| Error::new("not_found", "Action not found"))?;
                b.label = label;
            }
            Command::SetPinned {
                device_id,
                control_id,
                pinned,
                ..
            } => {
                let device = device_mut(&mut next, &device_id)?;
                let active = device.profile.active_page_id.clone();
                let control_id = if control_id.starts_with("dial-") {
                    action_control(device, &active, &control_id, false, true)?
                } else {
                    configured_control(device, &active, &control_id)?
                };
                let page = device
                    .profile
                    .pages
                    .iter_mut()
                    .find(|p| p.id == device.profile.active_page_id)
                    .ok_or_else(|| Error::new("data", "Page not found"))?;
                if pinned {
                    if let Some(binding) = page.bindings.remove(&control_id) {
                        device.pinned.insert(control_id, binding);
                    }
                } else if let Some(binding) = device.pinned.remove(&control_id) {
                    page.bindings.insert(control_id, binding);
                }
            }
            Command::AddProfile {
                device_id, name, ..
            } => {
                let device = device_mut(&mut next, &device_id)?;
                if device.profiles.len() >= 31 {
                    return Err(Error::new("limit", "At most 32 profiles"));
                }
                let template = Device::virtual_studio(1);
                let mut profile = template.profile;
                profile.name = name.trim().into();
                profile.pages[0].bindings.clear();
                profile.pages[0].panel = Some(PanelLayout::dial_slots(&device.provider_id));
                if device.default_profile.is_none() {
                    device.default_profile = Some(device.profile.id.clone());
                }
                let previous = std::mem::replace(&mut device.profile, profile);
                device.profiles.push(previous);
            }
            Command::SelectProfile {
                device_id,
                profile_id,
                ..
            } => actions::select_profile(device_mut(&mut next, &device_id)?, &profile_id)?,
            Command::ProfileApplication {
                device_id,
                application,
                ..
            } => {
                device_mut(&mut next, &device_id)?.profile.application =
                    application.filter(|s| !s.trim().is_empty());
            }
            Command::AddFolder {
                device_id,
                control_id,
                name,
                ..
            } => {
                let device = device_mut(&mut next, &device_id)?;
                let control_id =
                    configured_control(device, &device.profile.active_page_id, &control_id)?;
                if device.profile.pages.len() >= 64 {
                    return Err(Error::new("limit", "At most 64 pages"));
                }
                let parent = device.profile.active_page_id.clone();
                let id = new_id();
                let mut enter = Binding::new(ActionKind::Program, &name);
                enter.plan = Some(Plan::Page {
                    page_id: id.clone(),
                });
                let mut back = Binding::new(ActionKind::Program, "←");
                back.plan = Some(Plan::Back);
                device
                    .profile
                    .pages
                    .iter_mut()
                    .find(|p| p.id == parent)
                    .ok_or_else(|| Error::new("data", "Page not found"))?
                    .bindings
                    .insert(control_id, enter);
                device.profile.pages.push(Page {
                    close_after_action: false,
                    panel: Some(PanelLayout::dial_slots(&device.provider_id)),
                    id,
                    name,
                    parent: Some(parent),
                    bindings: BTreeMap::from([("key-1".into(), back)]),
                });
            }
            Command::ConfigurePlugin {
                device_id,
                page_id,
                control_id,
                plugin,
                label,
                ..
            } => {
                let device = device_mut(&mut next, &device_id)?;
                let control_id = action_control(
                    device,
                    &page_id,
                    &control_id,
                    plugin.controller == Some(Controller::Panel),
                    false,
                )?;
                let binding = Binding {
                    id: new_id(),
                    kind: ActionKind::Plugin,
                    label,
                    plugin: Some(plugin),
                    plan: None,
                };
                if let Some(pinned) = device.pinned.get_mut(&control_id) {
                    *pinned = binding;
                } else {
                    device
                        .profile
                        .pages
                        .iter_mut()
                        .find(|p| p.id == page_id)
                        .ok_or_else(|| Error::new("not_found", "Page not found"))?
                        .bindings
                        .insert(control_id, binding);
                }
            }
            Command::PluginSettings {
                device_id,
                binding_id,
                settings,
                ..
            } => {
                let device = device_mut(&mut next, &device_id)?;
                let binding = actions::find_mut(device, &binding_id)
                    .ok_or_else(|| Error::new("not_found", "Action not found"))?;
                binding
                    .plugin
                    .as_mut()
                    .ok_or_else(|| Error::new("action", "Not a plugin action"))?
                    .settings = settings;
            }
            Command::AddDevice { .. } => {
                if next.devices.len() >= 32 {
                    return Err(Error::new("limit", "At most 32 virtual devices"));
                }
                next.devices
                    .push(Device::virtual_studio(next.devices.len() + 1));
            }
            Command::RenameDevice {
                device_id, name, ..
            } => device_mut(&mut next, &device_id)?.name = name.trim().into(),
            Command::SetLocale { locale, .. } => next.locale = locale,
            Command::AddPage { device_id, .. } => {
                let device = device_mut(&mut next, &device_id)?;
                if device.profile.pages.len() >= 64 {
                    return Err(Error::new("limit", "At most 64 pages in this prototype"));
                }
                let page = Page {
                    close_after_action: false,
                    panel: Some(PanelLayout::dial_slots(&device.provider_id)),
                    parent: None,
                    id: new_id(),
                    name: (device
                        .profile
                        .pages
                        .iter()
                        .filter(|p| p.parent.is_none())
                        .count()
                        + 1)
                    .to_string(),
                    bindings: BTreeMap::new(),
                };
                device.profile.active_page_id.clone_from(&page.id);
                device.profile.pages.push(page);
            }
            Command::SelectPage {
                device_id, page_id, ..
            } => {
                let device = device_mut(&mut next, &device_id)?;
                if !device.profile.pages.iter().any(|p| p.id == page_id) {
                    return Err(Error::new("not_found", "Page not found"));
                }
                device.profile.active_page_id = page_id;
            }
            Command::ConfigureControl {
                device_id,
                page_id,
                control_id,
                kind,
                label,
                ..
            } => {
                let device = device_mut(&mut next, &device_id)?;
                let control_id = action_control(device, &page_id, &control_id, false, false)?;
                let page = device
                    .profile
                    .pages
                    .iter_mut()
                    .find(|p| p.id == page_id)
                    .ok_or_else(|| Error::new("not_found", "Page not found"))?;
                let bindings = if device.pinned.contains_key(&control_id) {
                    &mut device.pinned
                } else {
                    &mut page.bindings
                };
                match kind {
                    None => {
                        bindings.remove(&control_id);
                    }
                    Some(kind) => {
                        let id = bindings
                            .get(&control_id)
                            .filter(|b| b.kind == kind)
                            .map(|b| b.id.clone())
                            .unwrap_or_else(new_id);
                        bindings.insert(
                            control_id,
                            Binding {
                                id,
                                kind,
                                label: label.trim().into(),
                                plugin: None,
                                plan: None,
                            },
                        );
                    }
                }
            }
            Command::Input { .. } | Command::CancelActions { .. } => unreachable!(),
        }
        next.revision = expected
            .checked_add(1)
            .ok_or_else(|| Error::new("limit", "Revision overflow"))?;
        validate(&next)?;
        let mut snapshot = self.commit(next, expected)?;
        if let Some(device) = panel_edit {
            self.cancel_holds(|d, _, held| d == device && held.control.kind == ControlKind::Touch);
            self.actions.cancel_panel_gestures(&device);
            self.sequence += 1;
            snapshot = self.snapshot();
        }
        Ok(snapshot)
    }

    fn commit(&mut self, next: Configuration, expected: u64) -> Result<Snapshot> {
        self.store.save(&next, expected)?;
        self.config = next;
        let ids: BTreeSet<_> = self
            .config
            .devices
            .iter()
            .flat_map(|d| {
                actions::bindings(d)
                    .into_iter()
                    .map(move |b| (d.id.clone(), b.id.clone()))
            })
            .collect();
        self.feedback.retain(|key, _| ids.contains(key));
        self.sequence += 1;
        Ok(self.snapshot())
    }

    pub fn register_n4(&mut self, hardware_key: &str) -> Result<String> {
        if let Some(device) = self.config.devices.iter().find(|d| {
            d.provider_id == "deckweave.n4" && d.hardware_key.as_deref() == Some(hardware_key)
        }) {
            return Ok(device.id.clone());
        }
        if !valid_n4_identity(hardware_key) {
            return Err(Error::new("identity", "Invalid N4 identity"));
        }
        let mut next = self.config.clone();
        let expected = next.revision;
        let mut device = Device::virtual_studio(1);
        device.name = format!("Mirabox N4 Pro · {}", &hardware_key[6..14]);
        device.provider_id = "deckweave.n4".into();
        device.hardware_key = Some(hardware_key.into());
        let page = &mut device.profile.pages[0];
        page.bindings.clear();
        for key in 1..=10 {
            page.bindings.insert(
                format!("key-{key}"),
                Binding::new(
                    if key == 2 {
                        ActionKind::Toggle
                    } else {
                        ActionKind::Counter
                    },
                    &format!("K{key}"),
                ),
            );
        }
        for dial in 1..=4 {
            page.bindings.insert(
                format!("dial-{dial}"),
                Binding::new(ActionKind::Counter, &format!("D{dial}")),
            );
        }
        for zone in 1..=5 {
            page.bindings.insert(
                format!("touch-{zone}"),
                Binding::new(ActionKind::Toggle, &format!("T{zone}")),
            );
        }
        let id = device.id.clone();
        next.devices.push(device);
        next.revision += 1;
        validate(&next)?;
        self.commit(next, expected)?;
        Ok(id)
    }

    pub fn set_provider_error(&mut self, error: Option<String>) -> bool {
        if self.provider_error == error {
            return false;
        }
        self.provider_error = error;
        self.sequence += 1;
        true
    }

    pub fn connection_changed(
        &mut self,
        id: &str,
        generation: u64,
        state: ConnectionState,
        error: Option<String>,
    ) -> bool {
        let Some(device) = self.config.devices.iter().find(|d| d.id == id) else {
            return false;
        };
        let info = self
            .connections
            .entry(id.into())
            .or_insert_with(|| DeviceRuntime::initial(&device.provider_id));
        if generation < info.generation {
            return false;
        }
        if info.generation == generation && info.state == state && info.error == error {
            return false;
        }
        let cancel = info.generation != generation || state != ConnectionState::Connected;
        info.generation = generation;
        info.state = state;
        info.error = error;
        if cancel {
            self.actions.cancel(id);
            self.plugin_inputs.retain(|i| i.device_id != id);
            self.cancel_holds(|device, _, _| device == id);
        }
        self.sequence += 1;
        true
    }

    pub fn hardware_input(
        &mut self,
        id: &str,
        generation: u64,
        control: Option<&str>,
        event: Option<InputEvent>,
        description: String,
    ) -> Result<bool> {
        if !self
            .connections
            .get(id)
            .is_some_and(|s| s.state == ConnectionState::Connected && s.generation == generation)
        {
            return Ok(false);
        }
        if let (Some(control), Some(event)) = (control, event) {
            // UI testing and hardware can overlap. A foreign release must not release
            // the other source's latch or tear down an otherwise healthy USB session.
            if let Err(error) =
                self.input(id, control, event, &format!("hardware:{id}:{generation}"))
                && error.code != "input_owner"
            {
                return Err(error);
            }
        }
        if let Some(info) = self.connections.get_mut(id) {
            info.input_events += 1;
            info.last_input = Some(description);
        }
        self.sequence += 1;
        Ok(true)
    }

    pub fn hardware_frame_sent(&mut self, id: &str, generation: u64) -> bool {
        let Some(info) = self.connections.get_mut(id) else {
            return false;
        };
        if info.state != ConnectionState::Connected || info.generation != generation {
            return false;
        }
        info.output_frames += 1;
        self.sequence += 1;
        true
    }

    fn input(
        &mut self,
        device_id: &str,
        control_id: &str,
        event: InputEvent,
        source: &str,
    ) -> Result<()> {
        let point = if let InputEvent::Pointer { x, y, .. } = &event {
            if !x.is_finite()
                || *x < 0.0
                || *x > 1.0
                || y.is_some_and(|y| !y.is_finite() || !(0.0..=1.0).contains(&y))
            {
                return Err(Error::new(
                    "input",
                    "Pointer coordinates must be normalized",
                ));
            }
            Some(PointerPosition { x: *x, y: *y })
        } else {
            None
        };
        let device = self
            .config
            .devices
            .iter()
            .find(|d| d.id == device_id)
            .ok_or_else(|| Error::new("not_found", "Device not found"))?;
        let key = (device_id.to_owned(), control_id.to_owned());
        if let InputEvent::Swipe { right } = event {
            if self
                .held
                .get(&key)
                .is_some_and(|held| held.source != source)
            {
                return Err(Error::new(
                    "input_owner",
                    "Swipe belongs to another input source",
                ));
            }
            return self.swipe(device_id, control_id, right);
        }
        let ending = matches!(
            event,
            InputEvent::Up
                | InputEvent::Cancel
                | InputEvent::Pointer {
                    phase: PointerPhase::Up | PointerPhase::Cancel,
                    ..
                }
        );
        let control = if ending && let Some(held) = self.held.get(&key) {
            held.control.clone()
        } else if let Some(control) = device_geometry(device)
            .controls
            .into_iter()
            .find(|c| c.id == control_id)
        {
            control
        } else if ending {
            return Ok(());
        } else {
            return Err(Error::new("not_found", "Control not found"));
        };
        let page = device
            .profile
            .pages
            .iter()
            .find(|p| p.id == device.profile.active_page_id)
            .ok_or_else(|| Error::new("data", "Active page is missing"))?;
        let geometry = device_geometry(device);
        let [sx, sy, sw, sh] = geometry.surface;
        let bounds = [
            (control.rect[0] - sx) / sw,
            (control.rect[1] - sy) / sh,
            control.rect[2] / sw,
            control.rect[3] / sh,
        ];
        let pointer = PointerData {
            position: point.map_or(
                PointerPosition {
                    x: 0.5,
                    y: Some(0.5),
                },
                |p| relative_point(p, bounds),
            ),
            hold: false,
            canvas: [
                (bounds[2] * geometry.surface_pixels[0] as f64)
                    .round()
                    .max(1.0) as u32,
                (bounds[3] * geometry.surface_pixels[1] as f64)
                    .round()
                    .max(1.0) as u32,
            ],
        };
        if point.is_some() && control.kind != ControlKind::Touch {
            return Err(Error::new("input", "Pointer events require a panel region"));
        }
        let linked_control = page
            .panel
            .as_ref()
            .and_then(|p| p.regions.iter().find(|r| r.control_id() == control_id))
            .and_then(|r| r.dials.first())
            .cloned()
            .or_else(|| {
                let linked = binding_control(page, control_id);
                (linked != control_id && linked.starts_with("dial-")).then_some(linked)
            });
        let binding = device
            .pinned
            .get(control_id)
            .or_else(|| page.bindings.get(&binding_control(page, control_id)))
            .cloned();
        match event {
            InputEvent::Swipe { .. } => unreachable!("swipe handled before control lookup"),
            InputEvent::Click => {
                if let Some(binding) = binding {
                    if control.kind == ControlKind::Touch
                        && (binding.kind == ActionKind::Plugin
                            || matches!(
                                binding.plan,
                                Some(
                                    Plan::DialStack { .. }
                                        | Plan::ActionWheel { .. }
                                        | Plan::DialBindings { .. }
                                )
                            ))
                    {
                        if controller_for(&binding, control.kind) == Controller::Panel {
                            self.plugin_pointer(
                                device_id,
                                control_id,
                                &binding,
                                "panelDown",
                                pointer,
                            );
                        }
                        self.touch_binding(
                            device_id,
                            linked_control.as_deref().unwrap_or(control_id),
                            &binding,
                            pointer,
                        )?;
                    } else if binding.kind == ActionKind::Plugin {
                        self.plugin_input(
                            device_id,
                            control_id,
                            &binding,
                            if control.kind == ControlKind::Dial {
                                "dialDown"
                            } else {
                                "keyDown"
                            },
                            0,
                        );
                        self.plugin_input(
                            device_id,
                            control_id,
                            &binding,
                            if control.kind == ControlKind::Dial {
                                "dialUp"
                            } else {
                                "keyUp"
                            },
                            0,
                        );
                        self.close_folder_after_action(device_id, &binding.id)?;
                    } else {
                        self.run_binding(device_id, control_id, &binding, 1, false, &binding.id)?;
                        self.advance(self.actions.now);
                    }
                }
            }
            InputEvent::Down
            | InputEvent::Pointer {
                phase: PointerPhase::Down,
                ..
            } => {
                if !self.held.contains_key(&key)
                    && let Some(binding) = &binding
                {
                    self.actions.down(device_id, &binding.id);
                }
                if !self.held.contains_key(&key)
                    && let Some(binding) = &binding
                    && binding.kind == ActionKind::Plugin
                    && (control.kind != ControlKind::Touch
                        || controller_for(binding, control.kind) == Controller::Panel)
                {
                    if control.kind == ControlKind::Touch
                        && controller_for(binding, control.kind) == Controller::Panel
                    {
                        self.plugin_pointer(device_id, control_id, binding, "panelDown", pointer);
                    } else {
                        self.plugin_input(
                            device_id,
                            control_id,
                            binding,
                            if control.kind == ControlKind::Dial {
                                "dialDown"
                            } else {
                                "keyDown"
                            },
                            0,
                        );
                    }
                }
                self.held.entry(key).or_insert(Held {
                    bounds,
                    pointer,
                    control: control.clone(),
                    source: source.into(),
                    plugin_started: binding
                        .as_ref()
                        .is_some_and(|b| b.kind == ActionKind::Plugin)
                        && (control.kind != ControlKind::Touch
                            || binding.as_ref().is_some_and(|b| {
                                controller_for(b, control.kind) == Controller::Panel
                            })),
                    binding,
                    started: self.actions.now,
                    hold_fired: false,
                });
            }
            InputEvent::Up
            | InputEvent::Pointer {
                phase: PointerPhase::Up,
                ..
            } => {
                if self.held.get(&key).is_some_and(|h| h.source != source) {
                    return Err(Error::new(
                        "input_owner",
                        "Release belongs to another input source",
                    ));
                }
                if let Some(mut held) = self.held.remove(&key) {
                    if let Some(point) = point {
                        held.pointer.position = relative_point(point, held.bounds);
                    }
                    held.pointer.hold = self.actions.now.saturating_sub(held.started) >= 500;
                    if let Some(binding) = held.binding.as_ref()
                        && binding.kind == ActionKind::Plugin
                    {
                        if self.config.devices.iter().any(|d| {
                            d.id == device_id
                                && actions::bindings(d).iter().any(|b| b.id == binding.id)
                        }) {
                            if control.kind == ControlKind::Touch {
                                self.touch_binding(
                                    device_id,
                                    linked_control.as_deref().unwrap_or(control_id),
                                    binding,
                                    held.pointer,
                                )?;
                            } else {
                                self.plugin_input(
                                    device_id,
                                    control_id,
                                    binding,
                                    if control.kind == ControlKind::Dial {
                                        "dialUp"
                                    } else {
                                        "keyUp"
                                    },
                                    0,
                                );
                            }
                            self.close_folder_after_action(device_id, &binding.id)?;
                        }
                    } else {
                        if control.kind == ControlKind::Touch
                            && held.binding.as_ref().is_some_and(|b| {
                                matches!(
                                    b.plan,
                                    Some(
                                        Plan::DialStack { .. }
                                            | Plan::ActionWheel { .. }
                                            | Plan::DialBindings { .. }
                                    )
                                )
                            })
                        {
                            self.touch_binding(
                                device_id,
                                linked_control.as_deref().unwrap_or(control_id),
                                held.binding.as_ref().expect("checked binding"),
                                held.pointer,
                            )?;
                        } else {
                            self.release_binding(device_id, control_id, held)?;
                        }
                        self.advance(self.actions.now);
                    }
                }
            }
            InputEvent::Rotate { ticks } => {
                if control.kind != ControlKind::Dial {
                    return Err(Error::new("input", "Rotation requires a dial"));
                }
                if ticks == 0 || ticks.abs_diff(0) > 100 {
                    return Err(Error::new(
                        "input",
                        "Rotation must have 1..100 signed ticks",
                    ));
                }
                if let Some(binding) = binding {
                    if binding.kind == ActionKind::Plugin {
                        self.plugin_input(device_id, control_id, &binding, "dialRotate", ticks);
                        self.close_folder_after_action(device_id, &binding.id)?;
                    } else {
                        self.run_binding(
                            device_id,
                            control_id,
                            &binding,
                            i64::from(ticks),
                            true,
                            &binding.id,
                        )?;
                        self.advance(self.actions.now);
                    }
                }
            }
            InputEvent::Pointer {
                phase: PointerPhase::Move,
                ..
            } => {
                if let Some(mut held) = self.held.get(&key).cloned() {
                    if held.source != source {
                        return Err(Error::new(
                            "input_owner",
                            "Pointer belongs to another source",
                        ));
                    }
                    if let Some(point) = point {
                        held.pointer.position = relative_point(point, held.bounds);
                    }
                    if let Some(binding) = &held.binding
                        && controller_for(binding, ControlKind::Touch) == Controller::Panel
                    {
                        self.plugin_pointer(
                            device_id,
                            control_id,
                            binding,
                            "panelMove",
                            held.pointer,
                        );
                    }
                    if let Some(current) = self.held.get_mut(&key) {
                        current.pointer = held.pointer;
                    }
                }
            }
            InputEvent::Cancel
            | InputEvent::Pointer {
                phase: PointerPhase::Cancel,
                ..
            } => {
                if self.held.get(&key).is_some_and(|h| h.source == source) {
                    self.cancel_holds(|d, c, h| {
                        d == device_id && c == control_id && h.source == source
                    });
                    self.actions.cancel_gesture(device_id, control_id);
                }
            }
        }
        Ok(())
    }

    fn plugin_input(
        &mut self,
        device: &str,
        control: &str,
        binding: &Binding,
        event: &str,
        ticks: i32,
    ) {
        if self.plugin_inputs.len() >= 1024 {
            self.plugin_inputs.clear();
            self.plugin_feedback(
                &binding.id,
                "error",
                serde_json::json!("Plugin input queue overflow"),
            );
            return;
        }
        self.plugin_inputs.push_back(PluginInput {
            device_id: device.into(),
            control_id: control.into(),
            binding: binding.clone(),
            event: event.into(),
            ticks,
            owner_id: binding.id.clone(),
            position: None,
            hold: false,
            canvas: [200, 100],
        });
    }
    fn cancel_holds(&mut self, predicate: impl Fn(&str, &str, &Held) -> bool) {
        let cancelled: Vec<_> = self
            .held
            .iter()
            .filter(|((d, c), h)| predicate(d, c, h))
            .map(|((d, c), h)| (d.clone(), c.clone(), h.clone()))
            .collect();
        for (d, c, held) in cancelled {
            self.held.remove(&(d.clone(), c.clone()));
            if held.plugin_started
                && let Some(binding) = held.binding
            {
                if held.control.kind == ControlKind::Touch
                    && controller_for(&binding, held.control.kind) == Controller::Panel
                {
                    self.plugin_pointer(&d, &c, &binding, "panelCancel", held.pointer);
                } else {
                    self.plugin_input(&d, &c, &binding, "deckweaveCancel", 0);
                }
            }
        }
    }
    fn plugin_input_owned(
        &mut self,
        device: &str,
        control: &str,
        binding: &Binding,
        event: &str,
        ticks: i32,
        owner: &str,
    ) {
        self.plugin_input(device, control, binding, event, ticks);
        if let Some(input) = self.plugin_inputs.back_mut()
            && input.binding.id == binding.id
        {
            input.owner_id = owner.into();
        }
    }
    pub fn take_plugin_inputs(&mut self) -> Vec<PluginInput> {
        self.plugin_inputs.drain(..).collect()
    }
    pub fn plugin_settings(&self, context: &str) -> Option<serde_json::Value> {
        self.config
            .devices
            .iter()
            .flat_map(actions::bindings)
            .find(|b| b.id == context)
            .and_then(|b| b.plugin.as_ref())
            .map(|p| p.settings.clone())
    }
    pub fn plugin_feedback(
        &mut self,
        context: &str,
        event: &str,
        payload: serde_json::Value,
    ) -> bool {
        let found = self.config.devices.iter().find_map(|d| {
            actions::bindings(d)
                .into_iter()
                .find(|b| b.id == context && b.kind == ActionKind::Plugin)
                .map(|_| d.id.clone())
        });
        let Some(device_id) = found else {
            return false;
        };
        let entry = self
            .feedback
            .entry((device_id.clone(), context.into()))
            .or_insert(Feedback {
                device_id,
                binding_id: context.into(),
                value: 0,
                active: false,
                title: None,
                image: None,
                error: None,
                display: None,
                running: false,
            });
        let before = entry.clone();
        match event {
            "setImage" => {
                entry.image = payload["image"]
                    .as_str()
                    .filter(|s| s.len() <= 2 * 1024 * 1024)
                    .map(str::to_owned);
            }
            "setTitle" => {
                entry.title = payload["title"]
                    .as_str()
                    .map(|s| s.chars().take(160).collect())
            }
            "setState" => {
                let state = payload["state"].as_u64().unwrap_or(0).min(i64::MAX as u64);
                entry.active = state > 0;
                entry.value = state as i64;
            }
            "error" => {
                entry.running = false;
                entry.error = payload
                    .as_str()
                    .or(payload["message"].as_str())
                    .map(|s| s.chars().take(512).collect())
            }
            "ready" => {
                entry.error = None;
                entry.running = false;
                entry.display = None;
            }
            "starting" => {
                entry.running = true;
                entry.error = None;
                entry.display = Some("…".into());
            }
            _ => return false,
        }
        let changed = *entry != before;
        if changed {
            self.sequence += 1;
        }
        changed
    }
}

fn device_mut<'a>(config: &'a mut Configuration, id: &str) -> Result<&'a mut Device> {
    config
        .devices
        .iter_mut()
        .find(|d| d.id == id)
        .ok_or_else(|| Error::new("not_found", "Device not found"))
}

fn require_control(provider: &str, id: &str) -> Result<Control> {
    geometry_for(provider)
        .controls
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| Error::new("not_found", "Control not found"))
}
fn configured_control(device: &Device, page_id: &str, control: &str) -> Result<String> {
    let page = device
        .profile
        .pages
        .iter()
        .find(|p| p.id == page_id)
        .ok_or_else(|| Error::new("not_found", "Page not found"))?;
    if !page_geometry(&device.provider_id, page)
        .controls
        .iter()
        .any(|c| c.id == control)
    {
        return Err(Error::new("not_found", "Control not found"));
    }
    Ok(binding_control(page, control))
}
fn binding_control(page: &Page, control: &str) -> String {
    if page.panel.is_none()
        && !page.bindings.contains_key(control)
        && let Some(number) = control
            .strip_prefix("touch-")
            .and_then(|s| s.parse::<u8>().ok())
            .filter(|n| (1..=4).contains(n))
    {
        let dial = format!("dial-{number}");
        if page.bindings.contains_key(&dial) {
            return dial;
        }
    }
    page.panel
        .as_ref()
        .and_then(|p| {
            p.regions
                .iter()
                .find(|r| r.dials.iter().any(|d| d == control))
        })
        .map_or_else(|| control.to_owned(), PanelRegion::control_id)
}
fn action_control(
    device: &mut Device,
    page_id: &str,
    control: &str,
    panel_widget: bool,
    force_independent: bool,
) -> Result<String> {
    let resolved = configured_control(device, page_id, control)?;
    if !panel_widget && control.starts_with("dial-") && resolved != control {
        let page = device
            .profile
            .pages
            .iter_mut()
            .find(|p| p.id == page_id)
            .expect("validated page");
        let detach = page
            .panel
            .as_ref()
            .and_then(|p| {
                p.regions
                    .iter()
                    .find(|r| r.dials.iter().any(|d| d == control))
            })
            .is_some_and(|r| {
                force_independent
                    || r.dials.len() > 1
                    || page.bindings.get(&r.control_id()).is_some_and(|b| {
                        b.plugin
                            .as_ref()
                            .is_some_and(|p| p.controller == Some(Controller::Panel))
                    })
            });
        if detach {
            if force_independent
                && !page.bindings.contains_key(control)
                && let Some(old) = page.bindings.get(&resolved)
            {
                let mut copy = old.clone();
                copy.id = new_id();
                page.bindings.insert(control.into(), copy);
            }
            if let Some(panel) = &mut page.panel {
                for region in &mut panel.regions {
                    region.dials.retain(|d| d != control);
                }
            }
            return Ok(control.to_owned());
        }
    }
    Ok(resolved)
}

fn validate(config: &Configuration) -> Result<()> {
    if config.revision > 9_007_199_254_740_991 {
        return Err(Error::new(
            "limit",
            "Revision exceeds the JSON safe integer range",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut id = |value: &str| -> Result<()> {
        if value.len() != 32
            || !value.bytes().all(|b| b.is_ascii_hexdigit())
            || !ids.insert(value.to_owned())
        {
            return Err(Error::new("data", "IDs must be unique hexadecimal UUIDs"));
        }
        Ok(())
    };
    if config.devices.len() > 32 {
        return Err(Error::new("limit", "Too many devices"));
    }
    let mut hardware_keys = BTreeSet::new();
    for d in &config.devices {
        id(&d.id)?;
        if !["deckweave.virtual", "deckweave.n4"].contains(&d.provider_id.as_str()) {
            return Err(Error::new("provider", "Unsupported provider in this build"));
        }
        match (d.provider_id.as_str(), d.hardware_key.as_deref()) {
            ("deckweave.virtual", None) => {}
            ("deckweave.n4", Some(key)) if valid_n4_identity(key) && hardware_keys.insert(key) => {}
            _ => {
                return Err(Error::new(
                    "identity",
                    "Invalid or duplicate hardware identity",
                ));
            }
        }
        if d.name.trim().is_empty() || d.name.chars().count() > 80 {
            return Err(Error::new(
                "validation",
                "Device name must contain 1..80 characters",
            ));
        }
        if d.profiles.len() > 31 {
            return Err(Error::new("limit", "Too many profiles"));
        }
        for profile in std::iter::once(&d.profile).chain(&d.profiles) {
            id(&profile.id)?;
            if profile.name.trim().is_empty()
                || profile.name.chars().count() > 80
                || profile
                    .application
                    .as_ref()
                    .is_some_and(|s| s.len() > 32768)
            {
                return Err(Error::new(
                    "validation",
                    "Invalid profile name or application",
                ));
            }
            if profile.pages.is_empty()
                || profile.pages.len() > 64
                || !profile.pages.iter().any(|p| p.id == profile.active_page_id)
            {
                return Err(Error::new("data", "Invalid pages or active page"));
            }
            for page in &profile.pages {
                id(&page.id)?;
                if page.name.trim().is_empty() || page.name.chars().count() > 80 {
                    return Err(Error::new("validation", "Invalid page name"));
                }
                let mut parent = page.parent.as_ref();
                let mut visited = BTreeSet::from([page.id.as_str()]);
                while let Some(p) = parent {
                    if !visited.insert(p.as_str()) {
                        return Err(Error::new("data", "Folder cycle"));
                    }
                    parent = profile
                        .pages
                        .iter()
                        .find(|page| page.id == *p)
                        .ok_or_else(|| Error::new("data", "Folder parent missing"))?
                        .parent
                        .as_ref();
                }
                if let Some(panel) = &page.panel {
                    if panel.regions.len() > 64 {
                        return Err(Error::new("limit", "At most 64 panel regions"));
                    }
                    let mut dials = BTreeSet::new();
                    for (i, r) in panel.regions.iter().enumerate() {
                        id(&r.id)?;
                        let [x, y, w, h] = r.rect;
                        if !r.rect.iter().all(|v| v.is_finite())
                            || x < 0.0
                            || y < 0.0
                            || w < 13.0 / 800.0
                            || h < 13.0 / 128.0
                            || x + w > 1.0 + 1e-9
                            || y + h > 1.0 + 1e-9
                            || r.name.chars().count() > 80
                        {
                            return Err(Error::new("panel", "Invalid panel region bounds or name"));
                        }
                        for other in &panel.regions[..i] {
                            if x < other.rect[0] + other.rect[2] - 1e-9
                                && x + w > other.rect[0] + 1e-9
                                && y < other.rect[1] + other.rect[3] - 1e-9
                                && y + h > other.rect[1] + 1e-9
                            {
                                return Err(Error::new("panel", "Panel regions overlap"));
                            }
                        }
                        for dial in &r.dials {
                            if !dials.insert(dial)
                                || require_control(&d.provider_id, dial)?.kind != ControlKind::Dial
                            {
                                return Err(Error::new(
                                    "panel",
                                    "Each dial can be associated with one panel region",
                                ));
                            }
                        }
                    }
                }
                let geometry = page_geometry(&d.provider_id, page);
                for (control, binding) in &page.bindings {
                    if !geometry.controls.iter().any(|c| c.id == *control) {
                        return Err(Error::new(
                            "panel",
                            "Binding does not belong to this page's geometry",
                        ));
                    }
                    actions::validate_binding(binding, 0, &mut id)?;
                    let kind = geometry
                        .controls
                        .iter()
                        .find(|c| c.id == *control)
                        .expect("validated control")
                        .kind;
                    panels::validate_controller(binding, kind, true)?;
                }
            }
        }
        for (control, binding) in &d.pinned {
            let kind = require_control(&d.provider_id, control)?.kind;
            if kind == ControlKind::Touch {
                return Err(Error::new(
                    "pinned",
                    "Only physical keys and dials can be pinned across layouts",
                ));
            }
            actions::validate_binding(binding, 0, &mut id)?;
            panels::validate_controller(binding, kind, true)?;
        }
        if let Some(default) = &d.default_profile
            && !std::iter::once(&d.profile)
                .chain(&d.profiles)
                .any(|p| p.id == *default)
        {
            return Err(Error::new("data", "Default profile missing"));
        }
    }
    Ok(())
}

fn valid_n4_identity(key: &str) -> bool {
    key.len() == 70
        && key.starts_with("n4pro-")
        && key.as_bytes()[6..].iter().all(u8::is_ascii_hexdigit)
}
