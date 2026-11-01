#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deckweave_core::{Command, Core, Error, Snapshot};
mod hardware;
mod touch;
use hardware::{Hardware, LiveSnapshot};
use std::sync::{Arc, Mutex};
use tauri::{
    Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

enum Application {
    Ready {
        core: Arc<Mutex<Core>>,
        hardware: Hardware,
    },
    Failed(Error),
}

fn locked(state: &Application) -> Result<std::sync::MutexGuard<'_, Core>, Error> {
    match state {
        Application::Ready { core, .. } => core
            .lock()
            .map_err(|_| Error::new("runtime", "Runtime state is unavailable")),
        Application::Failed(error) => Err(error.clone()),
    }
}

#[tauri::command]
fn get_snapshot(
    window: WebviewWindow,
    state: State<'_, Application>,
) -> Result<LiveSnapshot, Error> {
    if window.label() != "main" && !window.label().starts_with("device-") {
        return Err(Error::new(
            "window",
            "This window cannot read the workspace",
        ));
    }
    let snapshot = locked(&state)?.snapshot();
    live(&state, snapshot)
}

fn live(state: &Application, snapshot: Snapshot) -> Result<LiveSnapshot, Error> {
    match state {
        Application::Ready { hardware, .. } => Ok(hardware.snapshot(snapshot)),
        Application::Failed(error) => Err(error.clone()),
    }
}

#[tauri::command]
fn dispatch(
    command: Command,
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, Application>,
) -> Result<LiveSnapshot, Error> {
    let label = window.label();
    if !permitted(label, &command) {
        return Err(Error::new(
            "window",
            "This window cannot change the requested device",
        ));
    }
    let snapshot = locked(&state)?.dispatch(command, label)?;
    if let Application::Ready { hardware, .. } = state.inner() {
        hardware.changed();
    }
    let _ = app.emit("deckweave:changed", snapshot.event_sequence);
    live(&state, snapshot)
}

fn permitted(label: &str, command: &Command) -> bool {
    if label == "main" {
        matches!(
            command,
            Command::AddDevice { .. } | Command::SetLocale { .. }
        )
    } else if let Some(id) = label.strip_prefix("device-") {
        matches!(command, Command::SetLocale { .. }) || command.device_id() == Some(id)
    } else {
        false
    }
}

#[tauri::command]
async fn open_device(
    device_id: String,
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, Application>,
) -> Result<(), Error> {
    if window.label() != "main" {
        return Err(Error::new("window", "Open devices from the registry"));
    }
    let name = locked(&state)?
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == device_id)
        .map(|d| d.name.clone())
        .ok_or_else(|| Error::new("not_found", "Device not found"))?;
    let label = format!("device-{device_id}");
    if let Some(existing) = app.get_webview_window(&label) {
        existing
            .show()
            .map_err(|e| Error::new("window", e.to_string()))?;
        existing
            .set_focus()
            .map_err(|e| Error::new("window", e.to_string()))?;
        return Ok(());
    }
    // Async Tauri command avoids creating a WebView from a synchronous IPC handler on Windows.
    let built = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("index.html".into()))
        .title(format!("DeckWeave — {name}"))
        .inner_size(1180.0, 860.0)
        .min_inner_size(920.0, 720.0)
        .build()
        .map_err(|e| Error::new("window", e.to_string()))?;
    locked(&state)?.editor_opened(built.label());
    let _ = app.emit("deckweave:changed", ());
    Ok(())
}

fn main() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let directory = std::env::var_os("DECKWEAVE_DATA_DIR")
                .map(std::path::PathBuf::from)
                .map(Ok)
                .unwrap_or_else(|| app.path().app_data_dir())
                .map_err(|error| Error::new("storage", error.to_string()));
            let core = directory.and_then(|directory| {
                std::fs::create_dir_all(&directory)
                    .map_err(|error| Error::new("storage", error.to_string()))?;
                Core::open(&directory.join("deckweave.sqlite3")).map(|core| (core, directory))
            });
            app.manage(match core {
                Ok((core, directory)) => {
                    let core = Arc::new(Mutex::new(core));
                    let hardware =
                        Hardware::start(Arc::clone(&core), app.handle().clone(), directory);
                    Application::Ready { core, hardware }
                }
                Err(error) => Application::Failed(error),
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::Destroyed)
                && window.label().starts_with("device-")
                && let Some(state) = window.app_handle().try_state::<Application>()
            {
                if let Ok(mut core) = locked(&state) {
                    core.editor_closed(window.label());
                }
                if let Application::Ready { hardware, .. } = state.inner() {
                    hardware.changed();
                }
                let _ = window.app_handle().emit("deckweave:changed", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            dispatch,
            open_device
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(app) => app.run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit)
                && let Some(state) = app.try_state::<Application>()
                && let Application::Ready { hardware, .. } = state.inner()
            {
                hardware.stop();
            }
        }),
        Err(error) => eprintln!("DeckWeave startup failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deckweave_core::InputEvent;

    #[test]
    fn window_scope_prevents_cross_device_execution_and_registry_execution() {
        let command = Command::Input {
            device_id: "one".into(),
            control_id: "key-1".into(),
            event: InputEvent::Down,
        };
        assert!(permitted("device-one", &command));
        assert!(!permitted("device-two", &command));
        assert!(!permitted("main", &command));
        assert!(!permitted("property-inspector", &command));
    }

    #[test]
    fn only_registry_can_add_instances() {
        let command = Command::AddDevice {
            expected_revision: 0,
        };
        assert!(permitted("main", &command));
        assert!(!permitted("device-one", &command));
    }
}
