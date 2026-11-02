use deckweave_core::*;
use std::collections::BTreeMap;
fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        geometry: Geometry {
            width: 3.0,
            height: 1.0,
            surface: [0.0, 0.0, 3.0, 1.0],
            surface_pixels: [1, 1],
            controls: vec![
                Control {
                    id: "key-42".into(),
                    kind: ControlKind::Key,
                    rect: [0.0, 0.0, 2.0, 1.0],
                    number: 42,
                    pointer: false,
                },
                Control {
                    id: "dial-9".into(),
                    kind: ControlKind::Dial,
                    rect: [2.0, 0.0, 1.0, 1.0],
                    number: 9,
                    pointer: false,
                },
            ],
        },
        definition: serde_json::json!({"settings":[{"id":"brightness","type":"number","min":0,"max":5,"step":1},{"id":"mode","type":"choice","choices":[{"value":0,"label":{"en":"Off"}},{"value":1,"label":{"en":"On"}}]},{"id":"vibration","type":"boolean"}],"lighting":[{"id":"keys","controls":["key-42"],"staticColors":{"settingId":"staticColors","address":"controlId"}}]}),
    }
}
#[test]
fn provider_uses_actual_geometry_profiles_and_settings_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite3");
    let mut core = Core::open(&path).unwrap();
    let id = core
        .register_provider_device("example.keyboard", "opaque-a", "Keyboard A", descriptor())
        .unwrap();
    let other = core
        .register_provider_device("example.keyboard", "opaque-b", "Keyboard B", descriptor())
        .unwrap();
    assert_ne!(id, other);
    assert_eq!(
        core.register_provider_device(
            "example.keyboard",
            "opaque-a",
            "Ignored rename",
            descriptor()
        )
        .unwrap(),
        id
    );
    let device = core
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .clone();
    core.dispatch(
        Command::ConfigureControl {
            device_id: id.clone(),
            page_id: device.profile.active_page_id,
            control_id: "key-42".into(),
            kind: Some(ActionKind::Counter),
            label: "actual key".into(),
            expected_revision: core.revision(),
        },
        "editor",
    )
    .unwrap();
    core.dispatch(
        Command::Input {
            device_id: id.clone(),
            control_id: "key-42".into(),
            event: InputEvent::Click,
        },
        "editor",
    )
    .unwrap();
    assert_eq!(
        core.snapshot()
            .feedback
            .iter()
            .find(|f| f.device_id == id)
            .unwrap()
            .value,
        1
    );
    assert!(
        core.dispatch(
            Command::Input {
                device_id: id.clone(),
                control_id: "key-1".into(),
                event: InputEvent::Click
            },
            "editor"
        )
        .is_err()
    );
    core.dispatch(
        Command::SetProviderSettings {
            device_id: id.clone(),
            changes: BTreeMap::from([
                ("brightness".into(), serde_json::json!(3)),
                ("mode".into(), serde_json::json!(1)),
                (
                    "staticColors".into(),
                    serde_json::json!({"key-42":{"r":0,"g":55,"b":255}}),
                ),
            ]),
            expected_revision: core.revision(),
        },
        "editor",
    )
    .unwrap();
    drop(core);
    let core = Core::open(&path).unwrap();
    let s = core.snapshot();
    assert_eq!(s.geometries[&id].controls[0].id, "key-42");
    assert_eq!(s.geometries[&id].controls[0].rect, [0.0, 0.0, 2.0, 1.0]);
    assert_eq!(
        s.configuration
            .devices
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .provider_settings["brightness"],
        3
    );
    assert!(
        s.configuration
            .devices
            .iter()
            .find(|d| d.id == other)
            .unwrap()
            .provider_settings
            .is_empty()
    );
}
#[test]
fn provider_rejects_undeclared_settings_bad_ranges_addresses_and_layout_changes() {
    let mut core = Core::memory().unwrap();
    let id = core
        .register_provider_device("example.keyboard", "a", "A", descriptor())
        .unwrap();
    for changes in [
        serde_json::json!({"undeclared":1}),
        serde_json::json!({"brightness":6}),
        serde_json::json!({"brightness":2.5}),
        serde_json::json!({"vibration":1}),
        serde_json::json!({"mode":"1"}),
        serde_json::json!({"staticColors":{"key-1":{"r":1,"g":1,"b":1}}}),
        serde_json::json!({"staticColors":{"key-42":{"r":256,"g":1,"b":1}}}),
    ] {
        let revision = core.revision();
        assert!(
            core.dispatch(
                Command::SetProviderSettings {
                    device_id: id.clone(),
                    changes: serde_json::from_value(changes).unwrap(),
                    expected_revision: revision
                },
                "editor"
            )
            .is_err()
        );
        assert_eq!(core.revision(), revision);
    }
    let mut changed = descriptor();
    changed.geometry.controls[0].rect[2] = 1.0;
    assert_eq!(
        core.register_provider_device("example.keyboard", "a", "A", changed)
            .unwrap_err()
            .code,
        "descriptor"
    );
    let mut invalid = descriptor();
    invalid.geometry.controls[0].rect[2] = 4.0;
    assert!(
        core.register_provider_device("example.keyboard", "b", "B", invalid)
            .is_err()
    );
    assert!(
        core.register_provider_device("deckweave.n4", "b", "B", descriptor())
            .is_err()
    );
    core.dispatch(
        Command::RemoveDevice {
            device_id: id.clone(),
            expected_revision: core.revision(),
        },
        "main",
    )
    .unwrap();
    assert_eq!(core.device_lifecycle_epoch(&id), 1);
    assert_eq!(
        core.register_provider_device("example.keyboard", "a", "A", descriptor())
            .unwrap_err()
            .code,
        "device_removed"
    );
    core.dispatch(
        Command::RestoreDevice {
            device_id: id.clone(),
            expected_revision: core.revision(),
        },
        "main",
    )
    .unwrap();
    assert_eq!(
        core.register_provider_device("example.keyboard", "a", "A", descriptor())
            .unwrap(),
        id
    );
    assert_eq!(core.device_lifecycle_epoch(&id), 1);
}
#[test]
fn removing_assignment_cancels_a_held_button_without_executing_release() {
    let mut core = Core::memory().unwrap();
    let d = core.snapshot().configuration.devices[0].clone();
    core.dispatch(
        Command::Input {
            device_id: d.id.clone(),
            control_id: "key-1".into(),
            event: InputEvent::Down,
        },
        "editor",
    )
    .unwrap();
    core.dispatch(
        Command::ConfigureControl {
            device_id: d.id.clone(),
            page_id: d.profile.active_page_id,
            control_id: "key-1".into(),
            kind: None,
            label: String::new(),
            expected_revision: core.revision(),
        },
        "editor",
    )
    .unwrap();
    assert!(core.snapshot().pressed.is_empty());
    core.dispatch(
        Command::Input {
            device_id: d.id,
            control_id: "key-1".into(),
            event: InputEvent::Up,
        },
        "editor",
    )
    .unwrap();
    assert!(core.snapshot().feedback.is_empty());
}
#[test]
fn declared_keyboard_edits_validate_and_coalesce_without_losing_selected_keys() {
    let mut c = Core::memory().unwrap();
    let mut descriptor = descriptor();
    descriptor.definition["controls"] = serde_json::json!([{"id":"key-42","kind":"key"}]);
    descriptor.definition["keyboard"] = serde_json::json!({"onboardMappings":{"settingId":"keyMappings","address":"controlId","layers":["main","fn"],"bindingFormat":"keyboard-usage"},"actuation":{"settingId":"keyActuation","address":"controlId","minUm":100,"maxUm":3200,"stepUm":10,"rapidMinUm":10,"fields":["triggerUm","pressUm","releaseUm","rapidTrigger","wholeTravel","rampage"]}});
    let id = c
        .register_provider_device("example.keyboard", "a", "A", descriptor)
        .unwrap();
    for changes in [
        serde_json::json!({"keyMappings":{"key-42":{"main":{"usage":4}}}}),
        serde_json::json!({"keyMappings":{"key-42":{"fn":{"usage":5}}}}),
        serde_json::json!({"keyActuation":{"key-42":{"triggerUm":1200}}}),
        serde_json::json!({"keyActuation":{"key-42":{"rapidTrigger":true,"pressUm":100,"releaseUm":100}}}),
    ] {
        c.dispatch(
            Command::SetProviderSettings {
                device_id: id.clone(),
                changes: serde_json::from_value(changes).unwrap(),
                expected_revision: c.revision(),
            },
            "editor",
        )
        .unwrap();
    }
    let outputs = c.take_provider_outputs();
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        outputs[0].changes["keyMappings"]["key-42"]["main"]["usage"],
        4
    );
    assert_eq!(
        outputs[0].changes["keyMappings"]["key-42"]["fn"]["usage"],
        5
    );
    assert_eq!(
        outputs[0].changes["keyActuation"]["key-42"]["triggerUm"],
        1200
    );
    assert_eq!(outputs[0].changes["keyActuation"]["key-42"]["pressUm"], 100);
    for invalid in [
        serde_json::json!({"keyMappings":{"key-42":{"main":{"usage":0}}}}),
        serde_json::json!({"keyMappings":{"key-42":{"other":{"usage":4}}}}),
        serde_json::json!({"keyMappings":{"key-1":{"main":{"usage":4}}}}),
        serde_json::json!({"keyActuation":{"key-42":{"triggerUm":1215}}}),
        serde_json::json!({"keyActuation":{"key-42":{"axisType":1}}}),
        serde_json::json!({"keyActuation":{"key-42":{"pressUm":0}}}),
    ] {
        assert!(
            c.dispatch(
                Command::SetProviderSettings {
                    device_id: id.clone(),
                    changes: serde_json::from_value(invalid).unwrap(),
                    expected_revision: c.revision()
                },
                "editor"
            )
            .is_err()
        );
    }
    assert!(c.take_provider_outputs().is_empty());
}
#[test]
fn host_converted_devices_without_lcd_surface_pair_and_do_not_invent_pixels() {
    for (width, height, key_rect, touch) in [
        (3.0, 2.0, [0.0, 0.0, 1.0, 1.0], true),
        (1044.0, 356.0, [15.0, 15.0, 40.0, 40.0], false),
    ] {
        let mut c = Core::memory().unwrap();
        let mut g = descriptor().geometry;
        g.width = width;
        g.height = height;
        g.surface = [0.0; 4];
        g.surface_pixels = [0; 2];
        g.controls = vec![Control {
            id: "key-1".into(),
            kind: ControlKind::Key,
            rect: key_rect,
            number: 1,
            pointer: false,
        }];
        if touch {
            g.controls.push(Control {
                id: "touch-strip".into(),
                kind: ControlKind::Touch,
                rect: [0.0, 1.0, 3.0, 1.0],
                number: 1,
                pointer: true,
            });
        }
        let id = c
            .register_provider_device(
                "example.no-output",
                "opaque",
                "Input only",
                ProviderDescriptor {
                    geometry: g.clone(),
                    definition: serde_json::json!({"settings":[],"displays":[],"lighting":[]}),
                },
            )
            .unwrap();
        let s = c.snapshot();
        assert_eq!(s.geometries[&id].surface_pixels, [0, 0]);
        let page = s
            .configuration
            .devices
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .profile
            .active_page_id
            .clone();
        c.dispatch(
            Command::ConfigureControl {
                device_id: id.clone(),
                page_id: page.clone(),
                control_id: "key-1".into(),
                kind: Some(ActionKind::Counter),
                label: "Software test".into(),
                expected_revision: c.revision(),
            },
            "editor",
        )
        .unwrap();
        c.dispatch(
            Command::Input {
                device_id: id.clone(),
                control_id: "key-1".into(),
                event: InputEvent::Click,
            },
            "editor",
        )
        .unwrap();
        assert_eq!(
            c.snapshot()
                .feedback
                .iter()
                .find(|f| f.device_id == id)
                .unwrap()
                .value,
            1
        );
        if touch {
            c.dispatch(
                Command::Input {
                    device_id: id.clone(),
                    control_id: "touch-strip".into(),
                    event: InputEvent::Pointer {
                        phase: PointerPhase::Down,
                        x: 0.5,
                        y: Some(0.5),
                    },
                },
                "editor",
            )
            .unwrap();
            c.dispatch(
                Command::Input {
                    device_id: id.clone(),
                    control_id: "touch-strip".into(),
                    event: InputEvent::Pointer {
                        phase: PointerPhase::Up,
                        x: 0.5,
                        y: Some(0.5),
                    },
                },
                "editor",
            )
            .unwrap();
        }
        g.surface_pixels = [1, 0];
        assert!(
            c.register_provider_device(
                "example.no-output",
                "bad",
                "Bad",
                ProviderDescriptor {
                    geometry: g,
                    definition: serde_json::json!({"settings":[]})
                }
            )
            .is_err()
        );
    }
}
