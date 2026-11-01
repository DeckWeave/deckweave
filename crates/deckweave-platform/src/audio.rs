//! Safe master-output audio boundary used by the optional Audio plugin helper.
use serde::{Deserialize, Serialize};

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;
#[cfg(windows)]
pub use windows::AudioSession;

#[derive(Debug, Clone, Serialize)]
pub struct AudioState {
    pub volume: f32,
    pub muted: bool,
    pub endpoint: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioCommand {
    Status,
    SetVolume { value: f32 },
    AdjustVolume { delta: f32 },
    ToggleMute,
}

impl AudioCommand {
    pub fn validate(self) -> Result<Self, String> {
        let valid = match self {
            Self::SetVolume { value } => value.is_finite() && (0.0..=100.0).contains(&value),
            Self::AdjustVolume { delta } => delta.is_finite() && (-100.0..=100.0).contains(&delta),
            _ => true,
        };
        valid
            .then_some(self)
            .ok_or_else(|| "Audio value must be finite and within range".into())
    }
}

#[cfg(not(windows))]
pub struct AudioSession;
#[cfg(not(windows))]
impl AudioSession {
    pub fn new() -> Result<Self, String> {
        Err("Master audio is currently supported on Windows only".into())
    }
    pub fn execute(&self, _: AudioCommand) -> Result<AudioState, String> {
        Err("Master audio is currently supported on Windows only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_volume_before_os_boundary() {
        assert!(AudioCommand::SetVolume { value: -1.0 }.validate().is_err());
        assert!(
            AudioCommand::SetVolume { value: f32::NAN }
                .validate()
                .is_err()
        );
        assert!(
            AudioCommand::AdjustVolume { delta: 101.0 }
                .validate()
                .is_err()
        );
        assert!(AudioCommand::SetVolume { value: 100.0 }.validate().is_ok());
    }
}
