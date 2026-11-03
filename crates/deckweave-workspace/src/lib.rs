//! Fixed operations for the personal workspace. No paths, arguments, text or keycodes from IPC.
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: u32,
    pub operation: String,
}
#[derive(Debug, Serialize)]
pub struct Response {
    pub id: Option<u32>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Code,
    Codex,
    Browser,
    Telegram,
    Telemost,
    Zotero,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operation {
    pub target: Target,
    pub keys: &'static [u16],
}
// Win32 virtual keys. The protocol cannot supply or compose them.
const CTRL: u16 = 0x11;
const SHIFT: u16 = 0x10;
const ALT: u16 = 0x12;
pub fn operation(name: &str) -> Result<Operation, String> {
    use Target::*;
    let (target, keys): (_, &[u16]) = match name {
        "launch.code" => (Code, &[]),
        "launch.codex" => (Codex, &[]),
        "launch.browser" => (Browser, &[]),
        "launch.telegram" => (Telegram, &[]),
        "launch.telemost" => (Telemost, &[]),
        "launch.zotero" => (Zotero, &[]),
        "browser.back" => (Browser, &[ALT, 0x25]),
        "browser.forward" => (Browser, &[ALT, 0x27]),
        "browser.reload" => (Browser, &[CTRL, b'R' as u16]),
        "browser.newTab" => (Browser, &[CTRL, b'T' as u16]),
        "browser.nextTab" => (Browser, &[CTRL, 0x09]),
        "browser.previousTab" => (Browser, &[CTRL, SHIFT, 0x09]),
        "browser.address" => (Browser, &[CTRL, b'L' as u16]),
        "browser.find" => (Browser, &[CTRL, b'F' as u16]),
        "browser.zoomIn" => (Browser, &[CTRL, 0xbb]),
        "browser.zoomOut" => (Browser, &[CTRL, 0xbd]),
        "browser.zoomReset" => (Browser, &[CTRL, b'0' as u16]),
        "codex.commandMenu" => (Codex, &[CTRL, SHIFT, b'P' as u16]),
        "codex.sidebar" => (Codex, &[CTRL, b'B' as u16]),
        "codex.bottomPanel" => (Codex, &[CTRL, b'J' as u16]),
        "codex.terminal" => (Codex, &[CTRL, 0xc0]),
        "codex.find" => (Codex, &[CTRL, b'F' as u16]),
        "codex.nextChat" => (Codex, &[CTRL, 0x09]),
        "codex.previousChat" => (Codex, &[CTRL, SHIFT, 0x09]),
        "codex.newChat" => (Codex, &[CTRL, b'N' as u16]),
        _ => return Err("Unsupported workspace operation; choose a listed command".into()),
    };
    Ok(Operation { target, keys })
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;
pub fn execute(name: &str) -> Result<(), String> {
    let op = operation(name)?;
    #[cfg(windows)]
    return windows::execute(op);
    #[cfg(not(windows))]
    {
        let _ = op;
        Err("Workspace application controls currently require Windows".into())
    }
}

pub fn handle(line: &[u8]) -> Response {
    match serde_json::from_slice::<Request>(line) {
        Ok(request) => match execute(&request.operation) {
            Ok(()) => Response {
                id: Some(request.id),
                ok: true,
                error: None,
            },
            Err(error) => Response {
                id: Some(request.id),
                ok: false,
                error: Some(error),
            },
        },
        Err(_) => Response {
            id: None,
            ok: false,
            error: Some("Invalid workspace request".into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_allowlist_has_no_submission_or_approval_shortcuts() {
        for name in ["browser.reload", "codex.newChat", "launch.code"] {
            let op = operation(name).unwrap();
            assert!(!op.keys.contains(&0x0d));
        }
        for name in [
            "hotkey",
            "launch",
            "codex.approve",
            "codex.send",
            "browser.submit",
            "report.bold",
        ] {
            assert!(operation(name).is_err());
        }
    }
    #[test]
    fn rejects_arbitrary_fields_and_invalid_request_identity() {
        assert!(
            serde_json::from_str::<Request>(r#"{"id":1,"operation":"launch.code","path":"evil"}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Request>(r#"{"id":-1,"operation":"launch.code"}"#).is_err());
        assert!(!handle(br#"{"id":1,"operation":"codex.approve"}"#).ok);
    }
}
