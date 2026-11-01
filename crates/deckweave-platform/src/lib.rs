//! Narrow safe OS boundary. Windows FFI is confined to the audited `windows` modules.
pub mod audio;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;
#[cfg(windows)]
pub use windows::{ProcessTree, contain_process, foreground, hotkey, session_locked};
#[cfg(not(windows))]
pub struct ProcessTree;
#[cfg(not(windows))]
pub fn contain_process(_: &std::process::Child) -> Result<ProcessTree, String> {
    Ok(ProcessTree)
}
#[cfg(not(windows))]
pub fn session_locked() -> Option<bool> {
    None
}
#[cfg(not(windows))]
pub fn foreground() -> Option<String> {
    None
}
#[cfg(not(windows))]
pub fn hotkey(_: &[String]) -> Result<(), String> {
    Err("Keyboard actions are implemented for Windows only".into())
}
