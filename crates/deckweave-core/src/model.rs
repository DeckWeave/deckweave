use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::new_v4().simple().to_string()
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Locale {
    #[default]
    Ru,
    En,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Counter,
    Toggle,
    Plugin,
    Program,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PluginAction {
    pub package: String,
    pub action: String,
    #[serde(default)]
    pub settings: serde_json::Value,
    #[serde(default)]
    pub controller: Option<Controller>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Controller {
    Keypad,
    Encoder,
    Panel,
}
pub fn controller_for(binding: &Binding, kind: ControlKind) -> Controller {
    binding
        .plugin
        .as_ref()
        .and_then(|p| p.controller)
        .unwrap_or(if kind == ControlKind::Dial {
            Controller::Encoder
        } else {
            Controller::Keypad
        })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Binding {
    pub id: String,
    pub kind: ActionKind,
    pub label: String,
    #[serde(default)]
    pub plugin: Option<PluginAction>,
    #[serde(default)]
    pub plan: Option<Plan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Plan {
    DialBindings {
        left: Option<Box<Binding>>,
        right: Option<Box<Binding>>,
        press: Option<Box<Binding>>,
        tap: Option<Box<Binding>>,
    },
    Page {
        page_id: String,
    },
    PageCycle {
        #[serde(default)]
        direction: PageDirection,
    },
    Profile {
        profile_id: String,
    },
    Back,
    Launch {
        program: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Hotkey {
        keys: Vec<String>,
    },
    Sequence {
        steps: Vec<Step>,
    },
    Switch {
        states: Vec<Binding>,
    },
    KeyLogic {
        single: Option<Box<Binding>>,
        double: Option<Box<Binding>>,
        hold: Option<Box<Binding>>,
        double_ms: u64,
        hold_ms: u64,
    },
    DialStack {
        actions: Vec<Binding>,
    },
    ActionWheel {
        actions: Vec<Binding>,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Step {
    pub delay_ms: u64,
    pub action: Binding,
}
impl Binding {
    pub fn children(&self) -> Vec<&Binding> {
        match &self.plan {
            Some(Plan::DialBindings {
                left,
                right,
                press,
                tap,
            }) => [left, right, press, tap]
                .into_iter()
                .filter_map(|b| b.as_deref())
                .collect(),
            Some(Plan::Sequence { steps }) => steps.iter().map(|s| &s.action).collect(),
            Some(Plan::Switch { states }) => states.iter().collect(),
            Some(Plan::KeyLogic {
                single,
                double,
                hold,
                ..
            }) => [single, double, hold]
                .into_iter()
                .filter_map(|b| b.as_deref())
                .collect(),
            Some(Plan::DialStack { actions } | Plan::ActionWheel { actions }) => {
                actions.iter().collect()
            }
            _ => vec![],
        }
    }
    pub fn contains(&self, id: &str) -> bool {
        self.id == id || self.children().iter().any(|b| b.contains(id))
    }
}

impl Binding {
    pub fn new(kind: ActionKind, label: &str) -> Self {
        Self {
            id: new_id(),
            kind,
            label: label.into(),
            plugin: None,
            plan: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageDirection {
    Previous,
    Next,
    #[default]
    Dial,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Page {
    #[serde(default)]
    pub close_after_action: bool,
    pub id: String,
    pub name: String,
    pub bindings: BTreeMap<String, Binding>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub panel: Option<PanelLayout>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PanelLayout {
    pub regions: Vec<PanelRegion>,
    #[serde(default = "enabled")]
    pub swipe_pages: bool,
}
impl PanelLayout {
    pub fn dial_slots(provider: &str) -> Self {
        let dials: Vec<_> = geometry_for(provider)
            .controls
            .into_iter()
            .filter(|c| c.kind == ControlKind::Dial)
            .collect();
        Self {
            swipe_pages: true,
            regions: dials
                .iter()
                .enumerate()
                .map(|(i, d)| PanelRegion {
                    id: new_id(),
                    name: format!("D{}", d.number),
                    rect: [
                        i as f64 / dials.len() as f64,
                        0.0,
                        1.0 / dials.len() as f64,
                        1.0,
                    ],
                    dials: vec![d.id.clone()],
                })
                .collect(),
        }
    }
}
fn enabled() -> bool {
    true
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PanelRegion {
    pub id: String,
    pub name: String,
    pub rect: [f64; 4],
    #[serde(default)]
    pub dials: Vec<String>,
}
impl PanelRegion {
    pub fn control_id(&self) -> String {
        format!("panel-{}", self.id)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub active_page_id: String,
    pub pages: Vec<Page>,
    #[serde(default)]
    pub application: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    #[serde(default)]
    pub hardware_key: Option<String>,
    pub profile: Profile,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub pinned: BTreeMap<String, Binding>,
    #[serde(default)]
    pub default_profile: Option<String>,
}

impl Device {
    pub fn virtual_studio(number: usize) -> Self {
        let page_id = new_id();
        Self {
            id: new_id(),
            name: format!("Virtual Studio {number:02}"),
            provider_id: "deckweave.virtual".into(),
            hardware_key: None,
            profiles: vec![],
            pinned: BTreeMap::new(),
            default_profile: None,
            profile: Profile {
                id: new_id(),
                name: "Default".into(),
                active_page_id: page_id.clone(),
                pages: vec![Page {
                    close_after_action: false,
                    panel: None,
                    parent: None,
                    id: page_id,
                    name: "1".into(),
                    bindings: BTreeMap::from([
                        ("key-1".into(), Binding::new(ActionKind::Counter, "Counter")),
                        ("key-2".into(), Binding::new(ActionKind::Toggle, "Toggle")),
                    ]),
                }],
                application: None,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Configuration {
    pub revision: u64,
    pub locale: Locale,
    #[serde(default)]
    pub preferences: Preferences,
    pub devices: Vec<Device>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Preferences {
    pub library: LibraryPreferences,
    pub disabled_plugins: Vec<String>,
    pub last_device: Option<String>,
    pub smart_profiles: bool,
    pub preview_scale: u16,
    pub compact: bool,
}
impl<'de> Deserialize<'de> for Preferences {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Stored {
            #[serde(default)]
            library: LibraryPreferences,
            #[serde(default)]
            disabled_plugins: Option<Vec<String>>,
            #[serde(default)]
            last_device: Option<String>,
            // One-time migration of the owner's earlier opt-out. Never serialized again.
            #[serde(default)]
            allow_weather_geolocation: Option<bool>,
            #[serde(default = "enabled")]
            smart_profiles: bool,
            #[serde(default = "preview_scale")]
            preview_scale: u16,
            #[serde(default)]
            compact: bool,
        }
        let stored = Stored::deserialize(deserializer)?;
        Ok(Self {
            library: stored.library,
            disabled_plugins: stored.disabled_plugins.unwrap_or_else(|| {
                if stored.allow_weather_geolocation == Some(false) {
                    vec!["com.elgato.weather".into()]
                } else {
                    vec![]
                }
            }),
            last_device: stored.last_device,
            smart_profiles: stored.smart_profiles,
            preview_scale: stored.preview_scale,
            compact: stored.compact,
        })
    }
}
fn preview_scale() -> u16 {
    100
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            library: LibraryPreferences::default(),
            disabled_plugins: vec![],
            last_device: None,
            smart_profiles: true,
            preview_scale: 100,
            compact: false,
        }
    }
}

/// Display choices only. These never suppress execution of assigned bindings.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LibraryPreferences {
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub show_development: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            revision: 0,
            locale: Locale::Ru,
            preferences: Preferences::default(),
            devices: vec![Device::virtual_studio(1)],
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Key,
    Dial,
    Touch,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Control {
    pub pointer: bool,
    pub id: String,
    pub kind: ControlKind,
    pub rect: [f64; 4],
    pub number: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Geometry {
    pub width: f64,
    pub height: f64,
    pub surface: [f64; 4],
    pub controls: Vec<Control>,
    pub surface_pixels: [u32; 2],
}

/// Virtual provider geometry, not a claim about N4 hardware capabilities.
pub fn virtual_geometry() -> Geometry {
    let mut controls = Vec::new();
    for i in 0..10 {
        controls.push(Control {
            pointer: false,
            id: format!("key-{}", i + 1),
            kind: ControlKind::Key,
            rect: [
                32.0 + (i % 5) as f64 * 112.0,
                28.0 + (i / 5) as f64 * 112.0,
                96.0,
                96.0,
            ],
            number: i + 1,
        });
    }
    for i in 0..4 {
        controls.push(Control {
            pointer: false,
            id: format!("touch-{}", i + 1),
            kind: ControlKind::Touch,
            rect: [32.0 + i as f64 * 136.0, 264.0, 136.0, 87.04],
            number: i + 1,
        });
        controls.push(Control {
            pointer: false,
            id: format!("dial-{}", i + 1),
            kind: ControlKind::Dial,
            rect: [64.0 + i as f64 * 136.0, 370.0, 72.0, 72.0],
            number: i + 1,
        });
    }
    Geometry {
        surface_pixels: [800, 128],
        width: 608.0,
        height: 464.0,
        surface: [32.0, 264.0, 544.0, 87.04],
        controls,
    }
}

pub fn geometry_for(provider: &str) -> Geometry {
    let mut geometry = virtual_geometry();
    if provider == "deckweave.n4" {
        // Preserve the physical 800:128 strip aspect ratio in logical preview space.
        let strip_height = geometry.surface[2] * 128.0 / 800.0;
        geometry.height = 464.0;
        geometry.surface[3] = strip_height;
        for control in geometry
            .controls
            .iter_mut()
            .filter(|c| c.kind == ControlKind::Dial)
        {
            control.rect[1] = 370.0;
        }
        geometry.controls.retain(|c| c.kind != ControlKind::Touch);
        for i in 0..5 {
            geometry.controls.push(Control {
                pointer: false,
                id: format!("touch-{}", i + 1),
                kind: ControlKind::Touch,
                rect: [32.0 + i as f64 * 108.8, 264.0, 108.8, strip_height],
                number: i + 1,
            });
        }
    }
    geometry
}

pub fn page_geometry(provider: &str, page: &Page) -> Geometry {
    let mut geometry = geometry_for(provider);
    if let Some(panel) = &page.panel {
        geometry.surface[3] = geometry.surface[2] * 128.0 / 800.0;
        geometry.height = 464.0;
        for c in &mut geometry.controls {
            if c.kind == ControlKind::Dial {
                c.rect[1] = 370.0;
            }
        }
        geometry.controls.retain(|c| c.kind != ControlKind::Touch);
        let [x, y, w, h] = geometry.surface;
        geometry
            .controls
            .extend(panel.regions.iter().enumerate().map(|(i, r)| {
                Control {
                    pointer: page
                        .bindings
                        .get(&r.control_id())
                        .and_then(|b| b.plugin.as_ref())
                        .is_some_and(|p| p.controller == Some(Controller::Panel)),
                    id: r.control_id(),
                    kind: ControlKind::Touch,
                    number: i + 1,
                    rect: [
                        x + r.rect[0] * w,
                        y + r.rect[1] * h,
                        r.rect[2] * w,
                        r.rect[3] * h,
                    ],
                }
            }));
    }
    for control in &mut geometry.controls {
        if control.kind == ControlKind::Touch {
            control.pointer = page
                .bindings
                .get(&control.id)
                .and_then(|b| b.plugin.as_ref())
                .is_some_and(|p| p.controller == Some(Controller::Panel));
        }
    }
    geometry
}
pub fn device_geometry(device: &Device) -> Geometry {
    device
        .profile
        .pages
        .iter()
        .find(|p| p.id == device.profile.active_page_id)
        .map_or_else(
            || geometry_for(&device.provider_id),
            |p| page_geometry(&device.provider_id, p),
        )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Virtual,
    Offline,
    Connected,
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceRuntime {
    pub state: ConnectionState,
    pub generation: u64,
    pub error: Option<String>,
    pub input_events: u64,
    pub output_frames: u64,
    pub last_input: Option<String>,
}
impl DeviceRuntime {
    pub fn initial(provider: &str) -> Self {
        Self {
            state: if provider == "deckweave.virtual" {
                ConnectionState::Virtual
            } else {
                ConnectionState::Offline
            },
            generation: 0,
            error: None,
            input_events: 0,
            output_frames: 0,
            last_input: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    Pointer {
        phase: PointerPhase,
        x: f64,
        y: Option<f64>,
    },
    Swipe {
        right: bool,
    },
    Down,
    Up,
    Click,
    Rotate {
        ticks: i32,
    },
    Cancel,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PointerPosition {
    pub x: f64,
    pub y: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    SetPreferences {
        preferences: Preferences,
        expected_revision: u64,
    },
    DevicePreferences {
        device_id: String,
        default_profile: Option<String>,
        expected_revision: u64,
    },
    SetPanelLayout {
        device_id: String,
        page_id: String,
        layout: Option<PanelLayout>,
        expected_revision: u64,
    },
    ConfigureProgram {
        device_id: String,
        page_id: String,
        control_id: String,
        plan: Plan,
        label: String,
        expected_revision: u64,
    },
    RenameBinding {
        device_id: String,
        binding_id: String,
        label: String,
        expected_revision: u64,
    },
    SetPinned {
        device_id: String,
        control_id: String,
        pinned: bool,
        expected_revision: u64,
    },
    AddProfile {
        device_id: String,
        name: String,
        expected_revision: u64,
    },
    SelectProfile {
        device_id: String,
        profile_id: String,
        expected_revision: u64,
    },
    ProfileApplication {
        device_id: String,
        application: Option<String>,
        expected_revision: u64,
    },
    AddFolder {
        device_id: String,
        control_id: String,
        name: String,
        expected_revision: u64,
    },
    FolderBehavior {
        device_id: String,
        page_id: String,
        close_after_action: bool,
        expected_revision: u64,
    },
    CancelActions {
        device_id: String,
    },
    ConfigurePlugin {
        device_id: String,
        page_id: String,
        control_id: String,
        plugin: PluginAction,
        label: String,
        expected_revision: u64,
    },
    PluginSettings {
        device_id: String,
        binding_id: String,
        settings: serde_json::Value,
        expected_revision: u64,
    },
    AddDevice {
        expected_revision: u64,
    },
    RenameDevice {
        device_id: String,
        name: String,
        expected_revision: u64,
    },
    ConfigureControl {
        device_id: String,
        page_id: String,
        control_id: String,
        kind: Option<ActionKind>,
        label: String,
        expected_revision: u64,
    },
    AddPage {
        device_id: String,
        expected_revision: u64,
    },
    SelectPage {
        device_id: String,
        page_id: String,
        expected_revision: u64,
    },
    SetLocale {
        locale: Locale,
        expected_revision: u64,
    },
    Input {
        device_id: String,
        control_id: String,
        event: InputEvent,
    },
}

impl Command {
    pub fn device_id(&self) -> Option<&str> {
        match self {
            Self::DevicePreferences { device_id, .. } => Some(device_id),
            Self::SetPanelLayout { device_id, .. } => Some(device_id),
            Self::FolderBehavior { device_id, .. } => Some(device_id),
            Self::ConfigureProgram { device_id, .. }
            | Self::RenameBinding { device_id, .. }
            | Self::SetPinned { device_id, .. }
            | Self::AddProfile { device_id, .. }
            | Self::SelectProfile { device_id, .. }
            | Self::ProfileApplication { device_id, .. }
            | Self::AddFolder { device_id, .. }
            | Self::CancelActions { device_id, .. }
            | Self::ConfigurePlugin { device_id, .. }
            | Self::PluginSettings { device_id, .. }
            | Self::RenameDevice { device_id, .. }
            | Self::ConfigureControl { device_id, .. }
            | Self::AddPage { device_id, .. }
            | Self::SelectPage { device_id, .. }
            | Self::Input { device_id, .. } => Some(device_id),
            _ => None,
        }
    }
    pub fn expected_revision(&self) -> Option<u64> {
        match self {
            Self::SetPreferences {
                expected_revision, ..
            }
            | Self::DevicePreferences {
                expected_revision, ..
            } => Some(*expected_revision),
            Self::SetPanelLayout {
                expected_revision, ..
            } => Some(*expected_revision),
            Self::FolderBehavior {
                expected_revision, ..
            } => Some(*expected_revision),
            Self::ConfigureProgram {
                expected_revision, ..
            }
            | Self::RenameBinding {
                expected_revision, ..
            }
            | Self::SetPinned {
                expected_revision, ..
            }
            | Self::AddProfile {
                expected_revision, ..
            }
            | Self::SelectProfile {
                expected_revision, ..
            }
            | Self::ProfileApplication {
                expected_revision, ..
            }
            | Self::AddFolder {
                expected_revision, ..
            }
            | Self::ConfigurePlugin {
                expected_revision, ..
            }
            | Self::PluginSettings {
                expected_revision, ..
            }
            | Self::AddDevice { expected_revision }
            | Self::RenameDevice {
                expected_revision, ..
            }
            | Self::ConfigureControl {
                expected_revision, ..
            }
            | Self::AddPage {
                expected_revision, ..
            }
            | Self::SelectPage {
                expected_revision, ..
            }
            | Self::SetLocale {
                expected_revision, ..
            } => Some(*expected_revision),
            Self::Input { .. } | Self::CancelActions { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Feedback {
    pub device_id: String,
    pub binding_id: String,
    pub value: i64,
    pub active: bool,
    pub title: Option<String>,
    pub image: Option<String>,
    pub error: Option<String>,
    pub display: Option<String>,
    pub running: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PluginInput {
    pub device_id: String,
    pub control_id: String,
    pub binding: Binding,
    pub event: String,
    pub ticks: i32,
    pub owner_id: String,
    pub position: Option<PointerPosition>,
    pub hold: bool,
    pub canvas: [u32; 2],
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct RetainedAction {
    pub device_id: String,
    pub control_id: String,
    pub binding_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PressedControl {
    pub device_id: String,
    pub control_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub configuration: Configuration,
    pub geometries: BTreeMap<String, Geometry>,
    pub connections: BTreeMap<String, DeviceRuntime>,
    pub provider_error: Option<String>,
    pub feedback: Vec<Feedback>,
    pub pressed: Vec<PressedControl>,
    pub editing_windows: usize,
    pub smart_profiles_paused: bool,
    pub event_sequence: u64,
    pub retained_actions: Vec<RetainedAction>,
}
