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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Binding {
    pub id: String,
    pub kind: ActionKind,
    pub label: String,
}

impl Binding {
    pub fn new(kind: ActionKind, label: &str) -> Self {
        Self {
            id: new_id(),
            kind,
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Page {
    pub id: String,
    pub name: String,
    pub bindings: BTreeMap<String, Binding>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub active_page_id: String,
    pub pages: Vec<Page>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    #[serde(default)]
    pub hardware_key: Option<String>,
    pub profile: Profile,
}

impl Device {
    pub fn virtual_studio(number: usize) -> Self {
        let page_id = new_id();
        Self {
            id: new_id(),
            name: format!("Virtual Studio {number:02}"),
            provider_id: "deckweave.virtual".into(),
            hardware_key: None,
            profile: Profile {
                id: new_id(),
                name: "Default".into(),
                active_page_id: page_id.clone(),
                pages: vec![Page {
                    id: page_id,
                    name: "1".into(),
                    bindings: BTreeMap::from([
                        ("key-1".into(), Binding::new(ActionKind::Counter, "Counter")),
                        ("key-2".into(), Binding::new(ActionKind::Toggle, "Toggle")),
                    ]),
                }],
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Configuration {
    pub revision: u64,
    pub locale: Locale,
    pub devices: Vec<Device>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            revision: 0,
            locale: Locale::Ru,
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
}

/// Virtual provider geometry, not a claim about N4 hardware capabilities.
pub fn virtual_geometry() -> Geometry {
    let mut controls = Vec::new();
    for i in 0..10 {
        controls.push(Control {
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
            id: format!("touch-{}", i + 1),
            kind: ControlKind::Touch,
            rect: [32.0 + i as f64 * 140.0, 264.0, 136.0, 64.0],
            number: i + 1,
        });
        controls.push(Control {
            id: format!("dial-{}", i + 1),
            kind: ControlKind::Dial,
            rect: [64.0 + i as f64 * 140.0, 350.0, 72.0, 72.0],
            number: i + 1,
        });
    }
    Geometry {
        width: 608.0,
        height: 448.0,
        surface: [32.0, 264.0, 556.0, 64.0],
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
                id: format!("touch-{}", i + 1),
                kind: ControlKind::Touch,
                rect: [32.0 + i as f64 * 112.0, 264.0, 108.0, strip_height],
                number: i + 1,
            });
        }
    }
    geometry
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
    Down,
    Up,
    Click,
    Rotate { ticks: i32 },
    Cancel,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
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
            Self::RenameDevice { device_id, .. }
            | Self::ConfigureControl { device_id, .. }
            | Self::AddPage { device_id, .. }
            | Self::SelectPage { device_id, .. }
            | Self::Input { device_id, .. } => Some(device_id),
            _ => None,
        }
    }
    pub fn expected_revision(&self) -> Option<u64> {
        match self {
            Self::AddDevice { expected_revision }
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
            Self::Input { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Feedback {
    pub device_id: String,
    pub binding_id: String,
    pub value: i64,
    pub active: bool,
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
}
