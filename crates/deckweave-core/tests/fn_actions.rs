use deckweave_core::*;
use serde_json::json;
fn keyboard(core: &mut Core) -> String {
    core.register_provider_device("example.keyboard","a","Keyboard",ProviderDescriptor {
        geometry:Geometry {width:2.0,height:1.0,surface:[0.0;4],surface_pixels:[0;2],controls:vec![
            Control{id:"key-t".into(),kind:ControlKind::Key,rect:[0.0,0.0,1.0,1.0],number:1,pointer:false},
            Control{id:"key-fn".into(),kind:ControlKind::Key,rect:[1.0,0.0,1.0,1.0],number:2,pointer:false},
        ]},definition:json!({"settings":[],"controls":[{"id":"key-t","kind":"key","label":"T"},{"id":"key-fn","kind":"key","label":"Fn"}],"keyboard":{"onboardMappings":{"settingId":"mapping","layers":["main","fn"],"bindingFormat":"keyboard-usage"}}})
    }).unwrap()
}
#[test]
fn legacy_automatic_write_path_is_rejected_without_saving_or_queuing_hardware() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let revision = core.revision();
    let command = serde_json::from_value(json!({"type":"set_fn_binding","device_id":device,"control_id":"key-t","configure_hardware":true,"binding":Binding::new(ActionKind::Counter,"Host"),"expected_revision":revision})).unwrap();
    assert_eq!(
        core.dispatch(command, "editor").unwrap_err().code,
        "keyboard_ownership"
    );
    assert_eq!(core.revision(), revision);
    assert!(core.take_provider_outputs().is_empty());
    assert!(core.fn_routes().is_empty());
}
#[test]
fn queued_global_alias_cannot_activate_after_a_new_pending_fn_mapping_edit() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let binding = Binding::new(ActionKind::Counter, "Fn T");
    let id = binding.id.clone();
    configure(&mut core, &device, "key-t", Some(binding)).unwrap();
    core.connection_changed(&device, 7, ConnectionState::Connected, None);
    let captured_route = core
        .fn_routes()
        .into_iter()
        .find(|r| r.device_id == device)
        .unwrap();
    assert!(core.activate_fn(&captured_route, 7).unwrap());
    core.dispatch(
        Command::SetProviderSettings {
            device_id: device.clone(),
            changes: serde_json::from_value(json!({"mapping":{"key-t":{"fn":{"usage":105}}}}))
                .unwrap(),
            expected_revision: core.revision(),
        },
        "editor",
    )
    .unwrap();
    // The provider has not acknowledged the mapping yet; the old F13 OS event
    // must still be rejected against the latest persisted request under this lock.
    assert_eq!(core.take_provider_outputs().len(), 1);
    assert!(!core.activate_fn(&captured_route, 7).unwrap());
    let snapshot = core.snapshot();
    assert_eq!(
        snapshot
            .feedback
            .iter()
            .find(|f| f.binding_id == id)
            .unwrap()
            .value,
        1
    );
    assert!(snapshot.pressed.is_empty());
    assert!(core.take_plugin_inputs().is_empty());
    assert!(core.take_provider_outputs().is_empty());
}
fn configure(
    core: &mut Core,
    device: &str,
    control: &str,
    binding: Option<Binding>,
) -> Result<Snapshot> {
    core.dispatch(
        Command::SetFnBinding {
            device_id: device.into(),
            control_id: control.into(),
            trigger: 13,
            configure_hardware: false,
            binding,
            expected_revision: core.revision(),
        },
        "editor",
    )
}
#[test]
fn fn_one_shot_uses_current_profile_generation_and_never_creates_holds_or_hardware_writes() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let binding = Binding::new(ActionKind::Counter, "Fn T");
    let binding_id = binding.id.clone();
    configure(&mut core, &device, "key-t", Some(binding)).unwrap();
    assert!(core.take_provider_outputs().is_empty());
    let route = core
        .fn_routes()
        .into_iter()
        .find(|r| r.device_id == device)
        .unwrap();
    assert_eq!(
        core.current_plugin_profile(&device, &binding_id),
        Some(route.profile_id.as_str())
    );
    assert!(!core.activate_fn(&route, 1).unwrap());
    core.connection_changed(&device, 7, ConnectionState::Connected, None);
    assert!(core.activate_fn(&route, 7).unwrap());
    assert!(core.snapshot().pressed.is_empty());
    assert_eq!(
        core.snapshot()
            .feedback
            .iter()
            .find(|f| f.binding_id == binding_id)
            .unwrap()
            .value,
        1
    );
    core.connection_changed(&device, 8, ConnectionState::Connected, None);
    assert!(!core.activate_fn(&route, 7).unwrap());
    core.dispatch(
        Command::AddProfile {
            device_id: device.clone(),
            name: "Other".into(),
            expected_revision: core.revision(),
        },
        "editor",
    )
    .unwrap();
    assert!(!core.activate_fn(&route, 8).unwrap());
    assert!(core.current_plugin_profile(&device, &binding_id).is_none());
    core.dispatch(
        Command::SelectProfile {
            device_id: device.clone(),
            profile_id: route.profile_id.clone(),
            expected_revision: core.revision(),
        },
        "editor",
    )
    .unwrap();
    assert!(core.activate_fn(&route, 8).unwrap());
    configure(&mut core, &device, "key-t", None).unwrap();
    assert!(!core.activate_fn(&route, 8).unwrap());
    assert!(core.take_provider_outputs().is_empty());
}
#[test]
fn fn_plugin_test_is_an_explicit_logical_tap_and_existing_inspector_settings_find_the_binding() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let mut binding = Binding::new(ActionKind::Plugin, "Timer");
    let id = binding.id.clone();
    binding.plugin = Some(PluginAction {
        package: "example.timer".into(),
        action: "example.timer.start".into(),
        settings: json!({}),
        controller: Some(Controller::Keypad),
    });
    configure(&mut core, &device, "key-t", Some(binding)).unwrap();
    core.dispatch(
        Command::TestFnBinding {
            device_id: device.clone(),
            control_id: "key-t".into(),
        },
        "editor",
    )
    .unwrap();
    let events = core.take_plugin_inputs();
    assert_eq!(
        events.iter().map(|e| e.event.as_str()).collect::<Vec<_>>(),
        vec!["keyDown", "keyUp"]
    );
    assert!(events.iter().all(|e| e.binding.id == id));
    assert!(core.snapshot().pressed.is_empty());
    core.dispatch(
        Command::PluginSettings {
            device_id: device.clone(),
            binding_id: id.clone(),
            settings: json!({"seconds":5}),
            expected_revision: core.revision(),
        },
        "plugin",
    )
    .unwrap();
    assert_eq!(
        core.snapshot()
            .configuration
            .devices
            .iter()
            .find(|d| d.id == device)
            .unwrap()
            .profile
            .fn_bindings["key-t"]
            .binding
            .plugin
            .as_ref()
            .unwrap()
            .settings["seconds"],
        5
    );
    assert!(core.take_provider_outputs().is_empty());
}
#[test]
fn physical_fn_and_hold_grammar_are_rejected_even_when_nested() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    assert!(
        configure(
            &mut core,
            &device,
            "key-fn",
            Some(Binding::new(ActionKind::Counter, "Invalid"))
        )
        .is_err()
    );
    let mut hold = Binding::new(ActionKind::Program, "Hold");
    hold.plan = Some(Plan::KeyLogic {
        single: Some(Box::new(Binding::new(ActionKind::Counter, "Single"))),
        double: None,
        hold: None,
        double_ms: 250,
        hold_ms: 500,
    });
    let mut multi = Binding::new(ActionKind::Program, "Multi");
    multi.plan = Some(Plan::Sequence {
        steps: vec![Step {
            delay_ms: 0,
            action: hold,
        }],
    });
    let revision = core.revision();
    assert!(configure(&mut core, &device, "key-t", Some(multi)).is_err());
    assert_eq!(core.revision(), revision);
}
