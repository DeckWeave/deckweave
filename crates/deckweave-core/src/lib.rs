pub mod model;
pub mod store;

pub use model::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
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
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            configuration: self.config.clone(),
            geometries: self
                .config
                .devices
                .iter()
                .map(|device| (device.id.clone(), geometry_for(&device.provider_id)))
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
            feedback: self.feedback.values().cloned().collect(),
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
        }
    }

    pub fn editor_opened(&mut self, label: &str) {
        if self.editors.insert(label.into()) {
            self.sequence += 1;
        }
    }
    pub fn editor_closed(&mut self, label: &str) {
        self.editors.remove(label);
        self.held.retain(|_, held| held.source != label);
        self.sequence += 1;
    }

    pub fn dispatch(&mut self, command: Command, source: &str) -> Result<Snapshot> {
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
        match command {
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
                    id: new_id(),
                    name: (device.profile.pages.len() + 1).to_string(),
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
                require_control(&device.provider_id, &control_id)?;
                let page = device
                    .profile
                    .pages
                    .iter_mut()
                    .find(|p| p.id == page_id)
                    .ok_or_else(|| Error::new("not_found", "Page not found"))?;
                match kind {
                    None => {
                        page.bindings.remove(&control_id);
                    }
                    Some(kind) => {
                        let id = page
                            .bindings
                            .get(&control_id)
                            .filter(|b| b.kind == kind)
                            .map(|b| b.id.clone())
                            .unwrap_or_else(new_id);
                        page.bindings.insert(
                            control_id,
                            Binding {
                                id,
                                kind,
                                label: label.trim().into(),
                            },
                        );
                    }
                }
            }
            Command::Input { .. } => unreachable!(),
        }
        next.revision = expected
            .checked_add(1)
            .ok_or_else(|| Error::new("limit", "Revision overflow"))?;
        validate(&next)?;
        self.commit(next, expected)
    }

    fn commit(&mut self, next: Configuration, expected: u64) -> Result<Snapshot> {
        self.store.save(&next, expected)?;
        self.config = next;
        let ids: BTreeSet<_> = self
            .config
            .devices
            .iter()
            .flat_map(|d| {
                d.profile.pages.iter().flat_map(move |p| {
                    p.bindings
                        .values()
                        .map(move |b| (d.id.clone(), b.id.clone()))
                })
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
        if info.generation != generation || state != ConnectionState::Connected {
            self.held.retain(|(device, _), _| device != id)
        }
        info.generation = generation;
        info.state = state;
        info.error = error;
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
        let device = self
            .config
            .devices
            .iter()
            .find(|d| d.id == device_id)
            .ok_or_else(|| Error::new("not_found", "Device not found"))?;
        let control = require_control(&device.provider_id, control_id)?;
        let page = device
            .profile
            .pages
            .iter()
            .find(|p| p.id == device.profile.active_page_id)
            .ok_or_else(|| Error::new("data", "Active page is missing"))?;
        let binding = page.bindings.get(control_id).cloned();
        let key = (device_id.to_owned(), control_id.to_owned());
        match event {
            InputEvent::Click => {
                if let Some(binding) = binding {
                    self.execute(device_id, &binding, 1);
                }
            }
            InputEvent::Down => {
                self.held.entry(key).or_insert(Held {
                    source: source.into(),
                    binding,
                });
            }
            InputEvent::Up => {
                if self.held.get(&key).is_some_and(|h| h.source != source) {
                    return Err(Error::new(
                        "input_owner",
                        "Release belongs to another input source",
                    ));
                }
                if let Some(held) = self.held.remove(&key)
                    && let Some(binding) = held.binding
                {
                    // Removed/replaced bindings are cancelled; navigation alone keeps the original target.
                    if self.config.devices.iter().any(|d| {
                        d.id == device_id
                            && d.profile
                                .pages
                                .iter()
                                .any(|p| p.bindings.values().any(|b| b.id == binding.id))
                    }) {
                        self.execute(device_id, &binding, 1);
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
                    self.execute(device_id, &binding, i64::from(ticks));
                }
            }
            InputEvent::Cancel => {
                if self.held.get(&key).is_some_and(|h| h.source == source) {
                    self.held.remove(&key);
                }
            }
        }
        Ok(())
    }

    fn execute(&mut self, device_id: &str, binding: &Binding, delta: i64) {
        let entry = self
            .feedback
            .entry((device_id.into(), binding.id.clone()))
            .or_insert(Feedback {
                device_id: device_id.into(),
                binding_id: binding.id.clone(),
                value: 0,
                active: false,
            });
        match binding.kind {
            ActionKind::Counter => entry.value = entry.value.saturating_add(delta),
            ActionKind::Toggle => {
                entry.active = !entry.active;
            }
        }
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
        id(&d.profile.id)?;
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
        if d.profile.pages.is_empty()
            || d.profile.pages.len() > 64
            || !d
                .profile
                .pages
                .iter()
                .any(|p| p.id == d.profile.active_page_id)
        {
            return Err(Error::new("data", "Invalid pages or active page"));
        }
        for p in &d.profile.pages {
            id(&p.id)?;
            for (control, b) in &p.bindings {
                require_control(&d.provider_id, control)?;
                id(&b.id)?;
                if b.label.chars().count() > 80 {
                    return Err(Error::new(
                        "validation",
                        "Action title must contain at most 80 characters",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn valid_n4_identity(key: &str) -> bool {
    key.len() == 70
        && key.starts_with("n4pro-")
        && key.as_bytes()[6..].iter().all(u8::is_ascii_hexdigit)
}
