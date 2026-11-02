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
    if let Some(fixtures) = std::env::var_os("DECKWEAVE_SMOKE_INSTALLER") {
        installer(app, PathBuf::from(fixtures));
        return;
    }
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

// Opt-in native-host regression: two locally generated fixture archives, isolated
// database and no hardware. Unlike file-publication unit tests, this exercises
// the actual pause/reload worker, assigned context restart and generic disabling.
fn installer(app: &tauri::AppHandle, fixtures: PathBuf) {
    let Some(directory) = crate::directory_override() else {
        return;
    };
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(".local");
    let (Ok(local), Ok(fixtures), Ok(directory)) = (
        local.canonicalize(),
        fixtures.canonicalize(),
        directory.canonicalize(),
    ) else {
        return;
    };
    if !fixtures.starts_with(&local)
        || !directory.starts_with(&local)
        || std::env::var_os("DECKWEAVE_VIRTUAL_ONLY").is_none()
    {
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        let result = (|| -> Result<_, String> {
            let state = app.state::<Application>();
            let Application::Ready {
                plugins,
                device_providers,
                ..
            } = state.inner()
            else {
                return Err("Host unavailable".into());
            };
            let manager = app.state::<crate::plugin_manager::Manager>();
            let first = manager.prepare_file(&fixtures.join("v1.zip"), &plugins.packages())?;
            let package = first.id.clone();
            manager.commit(&first.token, plugins, device_providers)?;
            let d = locked(&state)
                .map_err(|e| e.to_string())?
                .snapshot()
                .configuration
                .devices[0]
                .clone();
            let context = {
                let mut core = locked(&state).map_err(|e| e.to_string())?;
                let revision = core.revision();
                let snapshot = core
                    .dispatch(
                        Command::ConfigurePlugin {
                            device_id: d.id.clone(),
                            page_id: d.profile.active_page_id.clone(),
                            control_id: "key-1".into(),
                            plugin: PluginAction {
                                controller: None,
                                package: package.clone(),
                                action: format!("{package}.action"),
                                settings: json!({"prefix":"Kept"}),
                            },
                            label: "Installer smoke".into(),
                            expected_revision: revision,
                        },
                        "smoke",
                    )
                    .map_err(|e| e.to_string())?;
                snapshot.configuration.devices[0].profile.pages[0].bindings["key-1"]
                    .id
                    .clone()
            };
            let wait = |expected: &str, disabled: bool| -> Result<(), String> {
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(8) {
                    let snapshot = locked(&state).map_err(|e| e.to_string())?.snapshot();
                    if let Some(feedback) =
                        snapshot.feedback.iter().find(|f| f.binding_id == context)
                    {
                        if disabled
                            && feedback
                                .error
                                .as_ref()
                                .is_some_and(|e| e.contains("выключен") || e.contains("disabled"))
                        {
                            return Ok(());
                        }
                        if !disabled
                            && feedback.error.is_none()
                            && feedback.title.as_deref() == Some(expected)
                        {
                            return Ok(());
                        }
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(format!("Expected feedback {expected}, disabled={disabled}"))
            };
            wait("v1 Kept", false)?;
            let second = manager.prepare_file(&fixtures.join("v2.zip"), &plugins.packages())?;
            if second.installed_version.as_deref() != Some("1.0.0") {
                return Err("Installed version not detected".into());
            }
            manager.commit(&second.token, plugins, device_providers)?;
            wait("v2 Kept", false)?;
            for disabled in [true, false] {
                let mut core = locked(&state).map_err(|e| e.to_string())?;
                let mut preferences = core.snapshot().configuration.preferences;
                if disabled {
                    preferences.disabled_plugins.push(package.clone());
                } else {
                    preferences.disabled_plugins.retain(|id| id != &package);
                }
                let revision = core.revision();
                core.dispatch(
                    Command::SetPreferences {
                        preferences,
                        expected_revision: revision,
                    },
                    "smoke",
                )
                .map_err(|e| e.to_string())?;
                drop(core);
                wait("v2 Kept", disabled)?;
            }
            let revision = locked(&state).map_err(|e| e.to_string())?.revision();
            crate::extension_management::remove_package(&app, &package, "action", revision)?;
            if manager.lifecycle.allows(&package)
                || directory
                    .join("plugins")
                    .join(format!("{package}.sdPlugin"))
                    .exists()
            {
                return Err(
                    "Uninstall did not persist its execution gate and remove its owned package"
                        .into(),
                );
            }
            let snapshot = locked(&state).map_err(|e| e.to_string())?.snapshot();
            let retained = &snapshot.configuration.devices[0].profile.pages[0].bindings["key-1"];
            if retained.id != context
                || retained.plugin.as_ref().unwrap().settings["prefix"] != "Kept"
            {
                return Err("Uninstall altered retained assignment".into());
            }
            let restarted_gate =
                crate::plugin_lifecycle::Store::open(directory.join("plugin-lifecycle.json"));
            if restarted_gate.allows(&package) {
                return Err("Removal gate lost across restart".into());
            }
            let restore = manager.prepare_file(&fixtures.join("v2.zip"), &plugins.packages())?;
            manager.commit(&restore.token, plugins, device_providers)?;
            wait("v2 Kept", false)?;
            if !manager.lifecycle.allows(&package) {
                return Err("Explicit reinstall did not clear removal intent".into());
            }
            {
                let mut core = locked(&state).map_err(|e| e.to_string())?;
                let revision = core.revision();
                let added = core
                    .dispatch(
                        Command::AddDevice {
                            expected_revision: revision,
                        },
                        "smoke",
                    )
                    .map_err(|e| e.to_string())?;
                let second = added.configuration.devices.last().unwrap().clone();
                let removed = core
                    .dispatch(
                        Command::RemoveDevice {
                            device_id: second.id.clone(),
                            expected_revision: added.configuration.revision,
                        },
                        "smoke",
                    )
                    .map_err(|e| e.to_string())?;
                if removed.configuration.devices[0].id != d.id {
                    return Err("Removing sibling disturbed first device".into());
                }
                let restored = core
                    .dispatch(
                        Command::RestoreDevice {
                            device_id: second.id.clone(),
                            expected_revision: removed.configuration.revision,
                        },
                        "smoke",
                    )
                    .map_err(|e| e.to_string())?;
                if restored.configuration.devices.last().unwrap().profile.id != second.profile.id {
                    return Err("Restore replaced profile identity".into());
                }
            }
            let mut provider_checked = false;
            if fixtures.join("provider.zip").exists() {
                let provider =
                    manager.prepare_file(&fixtures.join("provider.zip"), &plugins.packages())?;
                if provider.kind != "device" {
                    return Err("Provider archive misclassified".into());
                }
                let provider_id = provider.id.clone();
                manager.commit(&provider.token, plugins, device_providers)?;
                let await_provider =
                    |stage: &str,
                     condition: &dyn Fn(&crate::device_providers::ProviderSnapshot) -> bool|
                     -> Result<(), String> {
                        let began = Instant::now();
                        while began.elapsed() < Duration::from_secs(8) {
                            let inventory = device_providers.snapshot();
                            if condition(&inventory) {
                                return Ok(());
                            }
                            thread::sleep(Duration::from_millis(50));
                        }
                        Err(format!(
                            "Device-provider smoke timed out: {stage}; {}",
                            serde_json::to_string(&device_providers.snapshot()).unwrap_or_default()
                        ))
                    };
                await_provider("discovery", &|s| {
                    s.discovered
                        .iter()
                        .filter(|d| d.provider_id == provider_id)
                        .count()
                        == 2
                })?;
                let a = device_providers.add(&provider_id, "mock-a")?;
                let b = device_providers.add(&provider_id, "mock-b")?;
                await_provider("two paired sessions", &|s| {
                    [&a, &b]
                        .iter()
                        .all(|id| s.sessions.get(*id).is_some_and(|s| s.state == "connected"))
                })?;
                {
                    let mut core = locked(&state).map_err(|e| e.to_string())?;
                    let revision = core.revision();
                    core.dispatch(
                        Command::SetProviderSettings {
                            device_id: b.clone(),
                            changes: std::collections::BTreeMap::from([(
                                "screenBrightness".into(),
                                json!(42),
                            )]),
                            expected_revision: revision,
                        },
                        "smoke",
                    )
                    .map_err(|e| e.to_string())?;
                }
                await_provider("settings roundtrip", &|s| {
                    s.sessions
                        .get(&b)
                        .is_some_and(|s| s.observed["screenBrightness"] == 42)
                })?;
                {
                    let mut core = locked(&state).map_err(|e| e.to_string())?;
                    let revision = core.revision();
                    core.dispatch(
                        Command::RemoveDevice {
                            device_id: a.clone(),
                            expected_revision: revision,
                        },
                        "smoke",
                    )
                    .map_err(|e| e.to_string())?;
                }
                await_provider("one instance removal", &|s| {
                    !s.sessions.contains_key(&a)
                        && s.sessions.get(&b).is_some_and(|s| s.state == "connected")
                })?;
                let revision = locked(&state).map_err(|e| e.to_string())?.revision();
                crate::extension_management::remove_package(
                    &app,
                    &provider_id,
                    "device",
                    revision,
                )?;
                if manager.lifecycle.allows(&format!("device:{provider_id}")) {
                    return Err("Provider removal gate missing".into());
                }
                let restore =
                    manager.prepare_file(&fixtures.join("provider.zip"), &plugins.packages())?;
                manager.commit(&restore.token, plugins, device_providers)?;
                await_provider("rediscovery", &|s| {
                    s.discovered
                        .iter()
                        .any(|d| d.provider_id == provider_id && d.instance_id == "mock-b")
                })?;
                if device_providers.add(&provider_id, "mock-b")? != b {
                    return Err("Provider reinstall replaced device identity".into());
                }
                await_provider("explicit rebind", &|s| {
                    s.sessions.get(&b).is_some_and(|s| s.state == "connected")
                })?;
                provider_checked = true;
            }
            Ok(
                json!({"package":package,"install":true,"live_update":true,"settings_retained":true,"disable_enable":true,"uninstall_restore":true,"restart_gate":true,"device_remove_restore":true,"provider_lifecycle":provider_checked,"hardware_access":false}),
            )
        })();
        let report = match result {
            Ok(report) => report,
            Err(error) => json!({"error":error}),
        };
        let _ = std::fs::write(
            directory.join("installer-smoke.json"),
            serde_json::to_vec_pretty(&report).unwrap_or_default(),
        );
        app.exit(0);
    });
}
