//! Debug-only integration driver. Uses an isolated data directory, never the owner's workspace.
use crate::{Application, locked};
use deckweave_core::{Command, InputEvent, PluginAction};
use serde_json::json;
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
#[derive(serde::Deserialize)]
pub struct Configuration {
    pub directory: PathBuf,
    pub plugin: String,
}
pub fn configuration() -> Option<&'static Configuration> {
    static CONFIG: std::sync::OnceLock<Option<Configuration>> = std::sync::OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .canonicalize()
                .ok()?;
            let bytes = std::fs::read(root.join(".local/smoke-launch.json")).ok()?;
            if bytes.len() > 4096 {
                return None;
            }
            let mut value: Configuration = serde_json::from_slice(&bytes).ok()?;
            value.directory = value.directory.canonicalize().ok()?;
            if !value.directory.starts_with(root.join(".local"))
                || value
                    .directory
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return None;
            }
            Some(value)
        })
        .as_ref()
}
pub fn start(app: &tauri::AppHandle) {
    let Some(package) = std::env::var("DECKWEAVE_SMOKE_PLUGIN")
        .ok()
        .or_else(|| configuration().map(|c| c.plugin.clone()))
    else {
        return;
    };
    if package.is_empty() {
        return;
    }
    let Some(directory) = crate::directory_override() else {
        return;
    };
    // A smoke driver is never allowed to target the ordinary application data directory.
    if !directory.is_absolute() || !directory.components().any(|c| c.as_os_str() == ".local") {
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let state = app.state::<Application>();
            let Application::Ready {
                plugins, hardware, ..
            } = state.inner()
            else {
                return Err("Runtime unavailable".into());
            };
            let action = plugins
                .packages()
                .into_iter()
                .find(|p| p.id == package)
                .and_then(|p| p.actions.first().cloned())
                .ok_or("Smoke plugin not found")?;
            let d = locked(&state)
                .map_err(|e| e.to_string())?
                .snapshot()
                .configuration
                .devices[0]
                .clone();
            let snapshot = {
                let mut core = locked(&state).map_err(|e| e.to_string())?;
                let revision = core.revision();
                core.dispatch(
                    Command::ConfigurePlugin {
                        device_id: d.id.clone(),
                        page_id: d.profile.active_page_id.clone(),
                        control_id: "key-1".into(),
                        plugin: PluginAction {
                            controller: None,
                            package: package.clone(),
                            action: action.id,
                            settings: json!({"prefix":"Smoke"}),
                        },
                        label: "Smoke".into(),
                        expected_revision: revision,
                    },
                    "smoke",
                )
                .map_err(|e| e.to_string())?
            };
            let context = snapshot.configuration.devices[0].profile.pages[0].bindings["key-1"]
                .id
                .clone();
            hardware.changed();
            let _ = app.emit("deckweave:changed", ());
            let start = Instant::now();
            let mut clicked = false;
            let close_windows = std::env::var_os("DECKWEAVE_SMOKE_CLOSE_WINDOWS").is_some();
            let mut closed = false;
            let mut clicked_background = false;
            let mut pi_checked = false;
            let mut previous_image = None;
            let mut image_changes = 0;
            while start.elapsed() < Duration::from_secs(15) {
                if !clicked && start.elapsed() > Duration::from_secs(3) {
                    let _ = locked(&state).map_err(|e| e.to_string())?.dispatch(
                        Command::Input {
                            device_id: d.id.clone(),
                            control_id: "key-1".into(),
                            event: InputEvent::Click,
                        },
                        "smoke",
                    );
                    clicked = true;
                }
                if package == "dev.deckweave.diagnostics"
                    && !pi_checked
                    && start.elapsed() > Duration::from_secs(4)
                {
                    let (url, _) = plugins
                        .inspector(context.clone())
                        .map_err(|e| e.to_string())?;
                    let url: tauri::Url = url.parse().map_err(|e| format!("Inspector URL: {e}"))?;
                    let (mut socket, _) = tungstenite::connect(format!(
                        "ws://127.0.0.1:{}",
                        url.port().ok_or("Missing port")?
                    ))
                    .map_err(|e| e.to_string())?;
                    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .map_err(|e| e.to_string())?;
                    }
                    for message in [
                        json!({"event":"registerPropertyInspector","uuid":context}),
                        json!({"event":"setSettings","context":context,"payload":{"prefix":"SavedFromPI"}}),
                        json!({"event":"getSettings","context":context}),
                    ] {
                        socket
                            .send(tungstenite::Message::Text(message.to_string().into()))
                            .map_err(|e| e.to_string())?;
                    }
                    let response: serde_json::Value = serde_json::from_str(
                        socket
                            .read()
                            .map_err(|e| e.to_string())?
                            .to_text()
                            .map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    if response["payload"]["settings"]["prefix"] != "SavedFromPI" {
                        return Err("PI settings round trip returned stale settings".into());
                    }
                    let _ = socket.close(None);
                    pi_checked = true;
                }
                if close_windows && !closed && start.elapsed() > Duration::from_secs(6) {
                    let handle = app.clone();
                    app.run_on_main_thread(move || {
                        for (label, window) in handle.webview_windows() {
                            if label == "main"
                                || label.starts_with("device-")
                                || label.starts_with("pi-")
                            {
                                let _ = window.close();
                            }
                        }
                    })
                    .map_err(|e| e.to_string())?;
                    closed = true;
                }
                if closed && !clicked_background && start.elapsed() > Duration::from_secs(8) {
                    locked(&state)
                        .map_err(|e| e.to_string())?
                        .dispatch(
                            Command::Input {
                                device_id: d.id.clone(),
                                control_id: "key-1".into(),
                                event: InputEvent::Click,
                            },
                            "smoke",
                        )
                        .map_err(|e| e.to_string())?;
                    clicked_background = true;
                }
                let snapshot = locked(&state).map_err(|e| e.to_string())?.snapshot();
                let feedback = snapshot.feedback.iter().find(|f| f.binding_id == context);
                let image = feedback.and_then(|f| f.image.clone());
                if image != previous_image {
                    image_changes += 1;
                    previous_image = image;
                }
                let report = json!({"elapsed_ms":start.elapsed().as_millis(),"package":package,"context":context,"title":feedback.and_then(|f|f.title.as_ref()),"error":feedback.and_then(|f|f.error.as_ref()),"image_bytes":feedback.and_then(|f|f.image.as_ref()).map_or(0,|s|s.len()),"image_changes":image_changes,"clicked":clicked,"clicked_background":clicked_background,"windows_closed":closed,"pi_round_trip":pi_checked,"editing_windows":snapshot.editing_windows});
                std::fs::write(
                    directory.join("plugin-smoke.json"),
                    serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                thread::sleep(Duration::from_millis(250));
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = std::fs::write(directory.join("plugin-smoke-error.txt"), error);
        }
        if std::env::var_os("DECKWEAVE_SMOKE_EXIT").is_some() {
            app.exit(0);
        }
    });
}
