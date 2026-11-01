use deckweave_core::*;
use serde_json::json;

#[test]
fn touch_cancellation_never_delivers_a_key_down_and_key_cancellation_resets_context() {
    let mut core = Core::memory().unwrap();
    let d = core.snapshot().configuration.devices[0].clone();
    for control in ["touch-1", "key-1"] {
        let revision = core.revision();
        core.dispatch(
            Command::ConfigurePlugin {
                device_id: d.id.clone(),
                page_id: d.profile.active_page_id.clone(),
                control_id: control.into(),
                plugin: PluginAction {
                    controller: None,
                    package: "fixture".into(),
                    action: "fixture.action".into(),
                    settings: json!({}),
                },
                label: "Test".into(),
                expected_revision: revision,
            },
            "test",
        )
        .unwrap();
    }
    let input = |core: &mut Core, control: &str, event: InputEvent| {
        core.dispatch(
            Command::Input {
                device_id: d.id.clone(),
                control_id: control.into(),
                event,
            },
            "test",
        )
        .unwrap();
    };
    input(&mut core, "touch-1", InputEvent::Down);
    assert!(core.take_plugin_inputs().is_empty());
    input(&mut core, "touch-1", InputEvent::Cancel);
    assert!(core.take_plugin_inputs().is_empty());
    input(&mut core, "touch-1", InputEvent::Down);
    input(&mut core, "touch-1", InputEvent::Up);
    assert_eq!(
        core.take_plugin_inputs()
            .iter()
            .map(|e| e.event.as_str())
            .collect::<Vec<_>>(),
        ["keyDown", "keyUp"]
    );
    input(&mut core, "key-1", InputEvent::Down);
    assert_eq!(core.take_plugin_inputs()[0].event, "keyDown");
    core.editor_closed("test");
    let cancelled = core.take_plugin_inputs();
    assert_eq!(cancelled.len(), 1);
    assert_eq!(cancelled[0].event, "deckweaveCancel");
    assert!(core.snapshot().pressed.is_empty());
}

#[test]
fn plugin_context_settings_and_input_survive_navigation_without_crossing_bindings() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("plugin.sqlite3");
    let mut core = Core::open(&path).unwrap();
    let d = core.snapshot().configuration.devices[0].clone();
    let page = d.profile.active_page_id.clone();
    core.dispatch(
        Command::ConfigurePlugin {
            device_id: d.id.clone(),
            page_id: page.clone(),
            control_id: "key-1".into(),
            plugin: PluginAction {
                controller: None,
                package: "fixture".into(),
                action: "fixture.counter".into(),
                settings: json!({"prefix":"A"}),
            },
            label: "Test".into(),
            expected_revision: 0,
        },
        "test",
    )
    .unwrap();
    let binding =
        core.snapshot().configuration.devices[0].profile.pages[0].bindings["key-1"].clone();
    for event in [
        InputEvent::Down,
        InputEvent::Down,
        InputEvent::Up,
        InputEvent::Up,
    ] {
        core.dispatch(
            Command::Input {
                device_id: d.id.clone(),
                control_id: "key-1".into(),
                event,
            },
            "test",
        )
        .unwrap();
    }
    let events = core.take_plugin_inputs();
    assert_eq!(
        events.iter().map(|e| e.event.as_str()).collect::<Vec<_>>(),
        vec!["keyDown", "keyUp"]
    );
    assert!(events.iter().all(|e| e.binding.id == binding.id));
    assert!(core.plugin_feedback(&binding.id, "setTitle", json!({"title":"Live"})));
    let sequence = core.snapshot().event_sequence;
    assert!(!core.plugin_feedback(&binding.id, "setTitle", json!({"title":"Live"})));
    assert_eq!(sequence, core.snapshot().event_sequence);
    assert!(!core.plugin_feedback("unknown", "setTitle", json!({"title":"Wrong"})));
    core.dispatch(
        Command::PluginSettings {
            device_id: d.id.clone(),
            binding_id: binding.id.clone(),
            settings: json!({"prefix":"Saved"}),
            expected_revision: 1,
        },
        "plugin",
    )
    .unwrap();
    drop(core);
    let core = Core::open(&path).unwrap();
    let saved = &core.snapshot().configuration.devices[0].profile.pages[0].bindings["key-1"];
    assert_eq!(saved.id, binding.id);
    assert_eq!(
        saved.plugin.as_ref().unwrap().settings,
        json!({"prefix":"Saved"})
    );
    assert!(core.snapshot().feedback.is_empty());
}
