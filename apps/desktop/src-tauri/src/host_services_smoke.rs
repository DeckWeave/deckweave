//! Opt-in native integration, isolated data and virtual devices, absent in release.
use crate::host_services::Runtime;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;
pub fn directory() -> Option<PathBuf> {
    if std::env::var_os("DECKWEAVE_SMOKE_SERVICES").is_none()
        || std::env::var_os("DECKWEAVE_VIRTUAL_ONLY").is_none()
    {
        return None;
    }
    let directory = PathBuf::from(std::env::var_os("DECKWEAVE_DATA_DIR")?)
        .canonicalize()
        .ok()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../.local")
        .canonicalize()
        .ok()?;
    (directory.starts_with(&root) && directory != root).then_some(directory)
}
fn wait(mut predicate: impl FnMut() -> bool) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(90) {
        if predicate() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("Native service smoke timed out".into())
}
pub fn start(app: &tauri::AppHandle) {
    let Some(directory) = directory() else {
        return;
    };
    let app = app.clone();
    thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            let runtime = app.state::<Runtime>();
            let call = |op: &str, payload: Value| {
                runtime.call(op.into(), payload).map_err(|e| e.to_string())
            };
            let mut panels = vec![];
            for count in [0, 1, 4, 8, 0] {
                if count > 0 {
                    while panels.len() < count {
                        let index = panels.len();
                        let p = call(
                            "open",
                            json!({"key":format!("note-{index}"),"spec":{"title":format!("Note {}",index+1),"content":{"type":"note","text":"Saved native service smoke note"},"placement":{"anchor":"top_left","x":20+(index%4)*180,"y":20+(index/4)*120,"width":170,"height":100}}}),
                        )?;
                        panels.push(p);
                    }
                } else {
                    for p in &panels {
                        call("close", json!({"id":p["id"],"revision":p["revision"]}))?;
                    }
                }
                wait(|| {
                    app.webview_windows()
                        .keys()
                        .filter(|k| k.starts_with("overlay-"))
                        .count()
                        == count
                })?;
                let stage = if count == 0 && !panels.is_empty() {
                    "closed".into()
                } else {
                    format!("{count}")
                };
                std::fs::write(
                    directory.join("stage.json"),
                    json!({"stage":stage,"windows":count}).to_string(),
                )
                .map_err(|e| e.to_string())?;
                wait(|| directory.join(format!("continue-{stage}")).exists())?;
            }
            // Exercise the actual Node SDK through the authenticated package supervisor.
            let state = app.state::<crate::Application>();
            let crate::Application::Ready { hardware, .. } = state.inner() else {
                return Err("Core unavailable".into());
            };
            let context = {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device = core.snapshot().configuration.devices[0].clone();
                let revision = core.revision();
                let snapshot = core
                    .dispatch(
                        deckweave_core::Command::ConfigurePlugin {
                            device_id: device.id,
                            page_id: device.profile.pages[0].id.clone(),
                            control_id: "key-1".into(),
                            plugin: deckweave_core::PluginAction {
                                controller: None,
                                package: "dev.deckweave.monitor".into(),
                                action: "dev.deckweave.monitor.cpu".into(),
                                settings: json!({"desktopPanel":true}),
                            },
                            label: "CPU".into(),
                            expected_revision: revision,
                        },
                        "services-smoke",
                    )
                    .map_err(|e| e.to_string())?;
                snapshot.configuration.devices[0].profile.pages[0].bindings["key-1"]
                    .id
                    .clone()
            };
            hardware.changed();
            wait(|| {
                crate::locked(&state).is_ok_and(|core| {
                    core.snapshot()
                        .feedback
                        .iter()
                        .any(|f| f.binding_id == context && f.image.is_some())
                })
            })?;
            {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device_id = core.snapshot().configuration.devices[0].id.clone();
                core.dispatch(
                    deckweave_core::Command::Input {
                        device_id,
                        control_id: "key-1".into(),
                        event: deckweave_core::InputEvent::Click,
                    },
                    "services-smoke",
                )
                .map_err(|e| e.to_string())?;
            }
            wait(|| {
                runtime.current().panels.iter().any(|v| {
                    v.panel.owner == "dev.deckweave.monitor" && v.online && v.value.is_some()
                })
            })?;
            let metric = runtime
                .current()
                .panels
                .into_iter()
                .find(|v| v.panel.owner == "dev.deckweave.monitor")
                .ok_or("Missing monitor panel")?;
            {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device_id = core.snapshot().configuration.devices[0].id.clone();
                let revision = core.revision();
                core.dispatch(
                    deckweave_core::Command::AddProfile {
                        device_id,
                        name: "Separate profile".into(),
                        expected_revision: revision,
                    },
                    "services-smoke",
                )
                .map_err(|e| e.to_string())?;
            }
            hardware.changed();
            thread::sleep(Duration::from_secs(3));
            if !runtime.current().panels.iter().any(|v| {
                v.panel.id == metric.panel.id && v.panel.open && v.online && v.value.is_some()
            }) {
                return Err("Metric did not survive profile change".into());
            }
            call(
                "close",
                json!({"id":metric.panel.id,"revision":metric.panel.revision}),
            )?;
            let note_context = {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device = core.snapshot().configuration.devices[0].clone();
                let revision = core.revision();
                let snapshot = core
                    .dispatch(
                        deckweave_core::Command::ConfigurePlugin {
                            device_id: device.id,
                            page_id: device.profile.pages[0].id.clone(),
                            control_id: "key-2".into(),
                            plugin: deckweave_core::PluginAction {
                                controller: None,
                                package: "dev.deckweave.desktop".into(),
                                action: "dev.deckweave.desktop.note".into(),
                                settings: json!({"text":"Original note"}),
                            },
                            label: "Note".into(),
                            expected_revision: revision,
                        },
                        "services-smoke",
                    )
                    .map_err(|e| e.to_string())?;
                snapshot.configuration.devices[0].profile.pages[0].bindings["key-2"]
                    .id
                    .clone()
            };
            hardware.changed();
            wait(|| {
                crate::locked(&state).is_ok_and(|core| {
                    core.snapshot()
                        .feedback
                        .iter()
                        .any(|f| f.binding_id == note_context && f.image.is_some())
                })
            })?;
            {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device_id = core.snapshot().configuration.devices[0].id.clone();
                core.dispatch(
                    deckweave_core::Command::Input {
                        device_id,
                        control_id: "key-2".into(),
                        event: deckweave_core::InputEvent::Click,
                    },
                    "services-smoke",
                )
                .map_err(|e| e.to_string())?;
            }
            wait(|| {
                runtime
                    .current()
                    .panels
                    .iter()
                    .any(|v| v.panel.owner == "dev.deckweave.desktop" && v.panel.open)
            })?;
            let note = runtime
                .current()
                .panels
                .into_iter()
                .find(|v| v.panel.owner == "dev.deckweave.desktop")
                .ok_or("Missing desktop note")?;
            call(
                "close",
                json!({"id":note.panel.id,"revision":note.panel.revision}),
            )?;
            {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device_id = core.snapshot().configuration.devices[0].id.clone();
                let revision = core.revision();
                core.dispatch(
                    deckweave_core::Command::PluginSettings {
                        device_id,
                        binding_id: note_context,
                        settings: json!({"text":"Edited while closed"}),
                        expected_revision: revision,
                    },
                    "services-smoke",
                )
                .map_err(|e| e.to_string())?;
            }
            wait(|| {
                runtime.current().panels.iter().any(|v|v.panel.id==note.panel.id && !v.panel.open && matches!(&v.panel.spec.content,deckweave_services::Content::Note{text} if text=="Edited while closed"))
            })?;
            let p = call(
                "open",
                json!({"key":"timer","spec":{"title":"Native timer","content":{"type":"timer","seconds":1,"sound":null}}}),
            )?;
            call("timer", json!({"id":p["id"],"command":"start"}))?;
            call("close", json!({"id":p["id"],"revision":p["revision"]}))?;
            wait(|| {
                runtime.current().panels.iter().any(|v| {
                    v.panel.id == p["id"].as_str().unwrap()
                        && !v.panel.open
                        && v.timer.as_ref().is_some_and(|t| {
                            t.status == deckweave_services::timers::TimerStatus::Done
                        })
                })
            })?;
            call(
                "sound",
                json!({"sound":"notice","volume":0,"eventId":"smoke-silent-playback"}),
            )?;
            wait(|| {
                runtime
                    .current()
                    .outcomes
                    .iter()
                    .any(|v| v["eventId"] == "smoke-silent-playback" && v["status"] == "finished")
            })?;
            let inventory = runtime.current();
            if inventory.error.is_some() || inventory.panels.iter().any(|p| p.error.is_some()) {
                return Err("Renderer or store reported an error".into());
            }
            Ok(
                json!({"authenticated_monitor_panel":true,"monitor_survived_profile_change":true,"closed_note_settings_updated_without_reopening":true,"closed_timer_finished":true,"silent_windows_playback_finished":true,"saved_panels":inventory.panels.len(),"open_windows":app.webview_windows().keys().filter(|k|k.starts_with("overlay-")).count()}),
            )
        })();
        let failed = result.is_err();
        let report = match result {
            Ok(value) => value,
            Err(error) => json!({"error":error}),
        };
        let _ = std::fs::write(directory.join("result.json"), report.to_string());
        app.exit(if failed { 1 } else { 0 });
    });
}
