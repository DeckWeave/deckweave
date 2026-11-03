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
            // Run the real Workspace action and vendored SDK with a local OS-launch
            // fixture. Fail once, then succeed: no real application is manipulated.
            let workspace_context = {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device = core.snapshot().configuration.devices[0].clone();
                let mut context = String::new();
                for control in ["key-4", "key-5"] {
                    let revision = core.revision();
                    let snapshot=core.dispatch(deckweave_core::Command::ConfigurePlugin {
                        device_id:device.id.clone(),page_id:device.profile.active_page_id.clone(),control_id:control.into(),
                        plugin:deckweave_core::PluginAction{package:"dev.deckweave.workspace".into(),action:"dev.deckweave.workspace.command".into(),controller:None,settings:json!({"operation":"launch.code","label":"Application","language":"ru"})},
                        label:"Application".into(),expected_revision:revision
                    },"workspace-artwork-smoke").map_err(|e|e.to_string())?;
                    if control == "key-4" {
                        context = snapshot.configuration.devices[0].profile.pages[0].bindings
                            [control]
                            .id
                            .clone();
                    }
                }
                context
            };
            hardware.changed();
            let workspace_feedback = || {
                crate::locked(&state)
                    .ok()?
                    .snapshot()
                    .feedback
                    .into_iter()
                    .find(|f| f.binding_id == workspace_context)
            };
            wait(|| workspace_feedback().is_some_and(|f| f.image.is_some()))?;
            let workspace_before = workspace_feedback().ok_or("Workspace appearance missing")?;
            let workspace_image = workspace_before
                .image
                .clone()
                .ok_or("Workspace image missing")?;
            let workspace_assignment = crate::locked(&state)
                .map_err(|e| e.to_string())?
                .plugin_assignment(&workspace_context)
                .ok_or("Workspace assignment missing")?;
            let click_workspace = || -> Result<(), String> {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device_id = core.snapshot().configuration.devices[0].id.clone();
                core.dispatch(
                    deckweave_core::Command::Input {
                        device_id,
                        control_id: "key-4".into(),
                        event: deckweave_core::InputEvent::Click,
                    },
                    "workspace-artwork-smoke",
                )
                .map_err(|e| e.to_string())?;
                Ok(())
            };
            click_workspace()?;
            wait(|| {
                workspace_feedback().is_some_and(|f| {
                    f.error.as_deref() == Some("Deliberate target window unavailable")
                        && f.image.is_some()
                        && f.image.as_deref() != Some(&workspace_image)
                })
            })?;
            let failed_workspace =
                workspace_feedback().ok_or("Workspace failed feedback missing")?;
            if failed_workspace.image.as_ref().is_none_or(String::is_empty) {
                return Err("Workspace action disappeared after failure".into());
            }
            // A recovery press must clear the error and restore the initial artwork.
            click_workspace()?;
            wait(|| {
                workspace_feedback().is_some_and(|f| {
                    f.error.is_none() && f.image.as_deref() == Some(&workspace_image)
                })
            })?;
            let workspace_after = crate::locked(&state)
                .map_err(|e| e.to_string())?
                .plugin_assignment(&workspace_context)
                .ok_or("Workspace assignment lost after press")?;
            if workspace_after != workspace_assignment {
                return Err("Workspace input rewrote its assignment".into());
            }

            // The real Focus plugin creates and controls this job through its SDK.
            // No host-owned stand-in, user database, physical device, or new overlay.
            let focus_owner = "dev.deckweave.focus";
            let focus_workspace = "services-smoke-focus";
            let before_focus = runtime.current();
            let overlay_count = || {
                app.webview_windows()
                    .keys()
                    .filter(|key| key.starts_with("overlay-"))
                    .count()
            };
            wait(|| overlay_count() == 0)?;
            let focus_panels_before = before_focus.panels.len();
            let focus_jobs_before = before_focus.focus.len();
            let (focus_context, focus_profile) = {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device = core.snapshot().configuration.devices[0].clone();
                let revision = core.revision();
                let snapshot = core
                    .dispatch(
                        deckweave_core::Command::ConfigurePlugin {
                            device_id: device.id,
                            page_id: device.profile.pages[0].id.clone(),
                            control_id: "key-3".into(),
                            plugin: deckweave_core::PluginAction {
                                controller: None,
                                package: focus_owner.into(),
                                action: "dev.deckweave.focus.pomodoro".into(),
                                settings: json!({
                                    "workspace":focus_workspace,
                                    "workMinutes":1.0/60.0,
                                    "shortMinutes":1.0/60.0,
                                    "longMinutes":1.0/60.0,
                                    "rounds":2,"sound":"notice","volume":0,
                                    "operation":"start","holdReset":false
                                }),
                            },
                            label: "Focus smoke".into(),
                            expected_revision: revision,
                        },
                        "services-smoke",
                    )
                    .map_err(|e| e.to_string())?;
                let profile = &snapshot.configuration.devices[0].profile;
                (
                    profile.pages[0].bindings["key-3"].id.clone(),
                    profile.id.clone(),
                )
            };
            hardware.changed();
            wait(|| {
                let sdk_image = crate::locked(&state).is_ok_and(|core| {
                    core.snapshot().feedback.iter().any(|feedback| {
                        feedback.binding_id == focus_context && feedback.image.is_some()
                    })
                });
                sdk_image
                    && runtime.current().focus.iter().any(|view| {
                        view.job.owner == focus_owner
                            && view.job.workspace == focus_workspace
                            && view.job.status == deckweave_services::focus::FocusStatus::Idle
                    })
            })
            .map_err(|e| format!("Focus SDK appearance/load: {e}"))?;
            let find_focus = || {
                runtime.current().focus.into_iter().find(|view| {
                    view.job.owner == focus_owner && view.job.workspace == focus_workspace
                })
            };
            let idle = find_focus().ok_or("Focus plugin did not create its workspace job")?;
            if idle.job.config.work_seconds != 1
                || idle.job.config.short_seconds != 1
                || idle.job.config.long_seconds != 1
                || idle.job.config.rounds != 2
                || idle.job.config.sound.as_deref() != Some("notice")
                || idle.job.config.volume != 0
            {
                return Err(
                    "Focus plugin did not apply the isolated one-second configuration".into(),
                );
            }
            {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device_id = core.snapshot().configuration.devices[0].id.clone();
                core.dispatch(
                    deckweave_core::Command::Input {
                        device_id,
                        control_id: "key-3".into(),
                        event: deckweave_core::InputEvent::Click,
                    },
                    "services-smoke",
                )
                .map_err(|e| e.to_string())?;
            }
            wait(|| {
                find_focus().is_some_and(|view| {
                    view.job.status == deckweave_services::focus::FocusStatus::Running
                        && view.job.phase == deckweave_services::focus::FocusPhase::Work
                })
            })
            .map_err(|e| format!("Focus SDK Core Input start: {e}"))?;
            let running = find_focus().ok_or("Missing running Focus job")?;
            let hidden_profile = {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device_id = core.snapshot().configuration.devices[0].id.clone();
                let revision = core.revision();
                let snapshot = core
                    .dispatch(
                        deckweave_core::Command::AddProfile {
                            device_id,
                            name: "Focus hidden smoke".into(),
                            expected_revision: revision,
                        },
                        "services-smoke",
                    )
                    .map_err(|e| e.to_string())?;
                snapshot.configuration.devices[0].profile.id.clone()
            };
            hardware.changed();
            let owner_retained_while_hidden = {
                let demand = runtime.plugin_demand();
                let hidden = crate::locked(&state).is_ok_and(|core| {
                    core.snapshot().configuration.devices[0].profile.id == hidden_profile
                });
                hidden
                    && demand.owners.contains(focus_owner)
                    && demand.focus.iter().any(|view| {
                        view.job.owner == focus_owner
                            && view.job.workspace == focus_workspace
                            && view.job.status == deckweave_services::focus::FocusStatus::Running
                            && view.job.run_id == running.job.run_id
                    })
            };
            if !owner_retained_while_hidden {
                return Err(
                    "Focus owner/job was not retained while running on the hidden profile".into(),
                );
            }
            wait(|| {
                find_focus().is_some_and(|view| {
                    view.job.run_id == running.job.run_id
                        && view.job.status == deckweave_services::focus::FocusStatus::Ready
                        && view.job.phase == deckweave_services::focus::FocusPhase::ShortBreak
                        && view.job.transition == 1
                        && view.job.round == 1
                        && view.job.due_ms.is_none()
                        && view.live_remaining_ms == 1000
                })
            })
            .map_err(|e| format!("Focus host completion while hidden: {e}"))?;
            let completed = find_focus().ok_or("Missing completed Focus job")?;
            let focus_event = completed.job.event_id();
            wait(|| {
                runtime.current().outcomes.iter().any(|outcome| {
                    outcome["eventId"] == focus_event && outcome["status"] == "finished"
                })
            })
            .map_err(|e| format!("Focus silent native sound completion: {e}"))?;
            // More than a complete break duration passes while hidden. There must
            // be no automatic next phase and no second transition/sound attempt.
            thread::sleep(Duration::from_millis(1250));
            let hidden_ready = find_focus().ok_or("Focus job disappeared while hidden")?;
            if hidden_ready.job.run_id != completed.job.run_id
                || hidden_ready.job.revision != completed.job.revision
                || hidden_ready.job.transition != 1
                || hidden_ready.job.status != deckweave_services::focus::FocusStatus::Ready
                || hidden_ready.job.phase != deckweave_services::focus::FocusPhase::ShortBreak
                || hidden_ready.job.due_ms.is_some()
                || hidden_ready.live_remaining_ms != 1000
            {
                return Err("Hidden Focus job automatically advanced or changed identity".into());
            }
            if overlay_count() != 0 || runtime.current().panels.len() != focus_panels_before {
                return Err("Hidden Focus job created an unexpected desktop panel/overlay".into());
            }
            let image_before_return = crate::locked(&state)
                .map_err(|e| e.to_string())?
                .snapshot()
                .feedback
                .into_iter()
                .find(|feedback| feedback.binding_id == focus_context)
                .and_then(|feedback| feedback.image);
            {
                let mut core = crate::locked(&state).map_err(|e| e.to_string())?;
                let device_id = core.snapshot().configuration.devices[0].id.clone();
                let revision = core.revision();
                core.dispatch(
                    deckweave_core::Command::SelectProfile {
                        device_id,
                        profile_id: focus_profile.clone(),
                        expected_revision: revision,
                    },
                    "services-smoke",
                )
                .map_err(|e| e.to_string())?;
            }
            hardware.changed();
            wait(|| {
                crate::locked(&state).is_ok_and(|core| {
                    let snapshot = core.snapshot();
                    snapshot.configuration.devices[0].profile.id == focus_profile
                        && snapshot.feedback.iter().any(|feedback| {
                            feedback.binding_id == focus_context
                                && feedback.image.is_some()
                                && feedback.image != image_before_return
                        })
                })
            })
            .map_err(|e| format!("Focus SDK completed-phase image after profile return: {e}"))?;
            thread::sleep(Duration::from_millis(1250));
            let returned = find_focus().ok_or("Focus job disappeared after profile return")?;
            let focus_snapshot = runtime.current();
            let queued = focus_snapshot
                .outcomes
                .iter()
                .filter(|outcome| {
                    outcome["eventId"] == focus_event && outcome["status"] == "queued"
                })
                .count();
            let finished = focus_snapshot
                .outcomes
                .iter()
                .filter(|outcome| {
                    outcome["eventId"] == focus_event && outcome["status"] == "finished"
                })
                .count();
            let focus_jobs = focus_snapshot
                .focus
                .iter()
                .filter(|view| {
                    view.job.owner == focus_owner && view.job.workspace == focus_workspace
                })
                .count();
            if returned.job.run_id != completed.job.run_id
                || returned.job.revision != completed.job.revision
                || returned.job.transition != 1
                || returned.job.round != 1
                || returned.job.phase != deckweave_services::focus::FocusPhase::ShortBreak
                || returned.job.status != deckweave_services::focus::FocusStatus::Ready
                || returned.job.due_ms.is_some()
                || returned.live_remaining_ms != 1000
                || focus_jobs != 1
                || focus_snapshot.focus.len() != focus_jobs_before + 1
                || queued != 1
                || finished != 1
            {
                return Err(
                    "Focus return created a fresh timer, auto-started a phase, or duplicated sound"
                        .into(),
                );
            }
            if focus_snapshot.panels.len() != focus_panels_before
                || overlay_count() != 0
                || focus_snapshot
                    .panels
                    .iter()
                    .any(|view| view.panel.owner == focus_owner)
            {
                return Err(
                    "Focus hardware widget created an unexpected desktop panel/overlay".into(),
                );
            }
            let inventory = runtime.current();
            if inventory.error.is_some() || inventory.panels.iter().any(|p| p.error.is_some()) {
                return Err("Renderer or store reported an error".into());
            }
            Ok(
                json!({"authenticated_monitor_panel":true,"monitor_survived_profile_change":true,"closed_note_settings_updated_without_reopening":true,"closed_timer_finished":true,"silent_windows_playback_finished":true,
                    "workspace_error_artwork_visible":true,"workspace_success_restored_artwork":true,"workspace_assignments_preserved":true,
                    "authenticated_focus_sdk_appearance":true,"focus_started_via_core_input":true,
                    "focus_owner_retained_while_hidden":owner_retained_while_hidden,
                    "focus_completed_one_phase_while_hidden":true,"focus_silent_windows_playback_finished":true,
                    "focus_profile_return_preserved_job":true,"focus_manual_next_phase_preserved":true,
                    "focus_no_desktop_overlay":true,
                    "focus_evidence":{"owner":focus_owner,"workspace":focus_workspace,
                        "run_id":returned.job.run_id,"revision":returned.job.revision,
                        "phase":returned.job.phase,"status":returned.job.status,"round":returned.job.round,
                        "transition":returned.job.transition,"remaining_ms":returned.live_remaining_ms,
                        "config":returned.job.config,"matching_jobs":focus_jobs,
                        "sound_event_id":focus_event,"sound_queued_count":queued,"sound_finished_count":finished,
                        "open_overlays":overlay_count(),"added_panels":focus_snapshot.panels.len()-focus_panels_before},
                    "saved_panels":inventory.panels.len(),"open_windows":overlay_count()}),
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
