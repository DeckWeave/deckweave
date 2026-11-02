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
fn presented_descriptor() -> ProviderDescriptor {
    let mut d = descriptor();
    d.definition = serde_json::json!({
        "settings":[
            {"id":"effect","type":"choice","choices":[{"value":"rainbow"},{"value":"solid"}]},
            {"id":"enabled","type":"boolean"},
            {"id":"redAmount","type":"number","min":0,"max":255,"step":1},
            {"id":"greenAmount","type":"number","min":0,"max":255,"step":1},
            {"id":"blueAmount","type":"number","min":0,"max":255},
            {"id":"pace","type":"number","min":0,"max":5,"presentation":"slider","visibleWhen":{"settingId":"enabled","values":[true]}}
        ],
        "lighting":[{"id":"keys","controls":["key-42"],"settings":["effect","redAmount","greenAmount","blueAmount","pace"],
            "colorChannels":[{"id":"primary","label":{"en":"Color","ru":"Цвет"},"r":"redAmount","g":"greenAmount","b":"blueAmount","visibleWhen":{"settingId":"effect","values":["solid"]}}]}]
    });
    d
}
#[test]
fn provider_presentation_uses_declared_generic_channels_and_preserves_optional_metadata() {
    let mut core = Core::memory().unwrap();
    let d = presented_descriptor();
    let id = core
        .register_provider_device("example.keyboard", "opaque", "Keyboard", d.clone())
        .unwrap();
    let saved = core
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|device| device.id == id)
        .unwrap();
    assert_eq!(saved.provider_descriptor.unwrap().definition, d.definition);
    assert!(
        saved.provider_settings.is_empty(),
        "Presentation discovery must not request hardware changes"
    );
}
#[test]
fn invalid_presentation_cannot_introduce_settings_outside_the_declared_group_or_choice_domain() {
    let cases = [
        (
            "/lighting/0/colorChannels/0/r",
            serde_json::json!("unknown"),
        ),
        (
            "/lighting/0/colorChannels/0/g",
            serde_json::json!("redAmount"),
        ),
        ("/lighting/0/colorChannels/0/b", serde_json::json!("pace")),
        (
            "/lighting/0/settings",
            serde_json::json!(["effect", "redAmount", "greenAmount"]),
        ),
        (
            "/lighting/0/colorChannels/0/visibleWhen/settingId",
            serde_json::json!("pace"),
        ),
        (
            "/lighting/0/colorChannels/0/visibleWhen/values",
            serde_json::json!(["unsupported"]),
        ),
        (
            "/lighting/0/colorChannels/0/visibleWhen/values",
            serde_json::json!(["solid", "solid"]),
        ),
        (
            "/lighting/0/colorChannels/0/visibleWhen/values",
            serde_json::json!([]),
        ),
        (
            "/lighting/0/colorChannels/0/visibleWhen/values",
            serde_json::json!([{"script":"anything"}]),
        ),
        (
            "/lighting/0/colorChannels/0/label",
            serde_json::json!({"html":"<script>"}),
        ),
        ("/settings/2/max", serde_json::json!(256)),
        ("/settings/2/min", serde_json::json!(1)),
        ("/settings/2/step", serde_json::json!(0.5)),
        ("/settings/1/presentation", serde_json::json!("slider")),
        ("/settings/5/presentation", serde_json::json!("script")),
        ("/settings/5/visibleWhen/values", serde_json::json!([1])),
        (
            "/settings/5/visibleWhen/settingId",
            serde_json::json!("missing"),
        ),
    ];
    for (path, value) in cases {
        let mut d = presented_descriptor();
        // JSON pointers cannot insert absent fields; object assignment handles the optional cases.
        if path.ends_with("/presentation") && path.starts_with("/settings/1/") {
            d.definition["settings"][1]["presentation"] = value;
        } else {
            *d.definition.pointer_mut(path).unwrap() = value;
        }
        let mut core = Core::memory().unwrap();
        let revision = core.revision();
        assert_eq!(
            core.register_provider_device("example.keyboard", "opaque", "Keyboard", d)
                .unwrap_err()
                .code,
            "descriptor",
            "{path}"
        );
        assert_eq!(core.revision(), revision);
    }
}
#[test]
fn visibility_cycles_and_duplicate_color_presentations_are_rejected() {
    let mut cycle = presented_descriptor();
    cycle.definition["settings"][0]["visibleWhen"] =
        serde_json::json!({"settingId":"enabled","values":[true]});
    cycle.definition["settings"][1]["visibleWhen"] =
        serde_json::json!({"settingId":"effect","values":["solid"]});
    let mut duplicate = presented_descriptor();
    let color = duplicate.definition["lighting"][0]["colorChannels"][0].clone();
    duplicate.definition["lighting"][0]["colorChannels"]
        .as_array_mut()
        .unwrap()
        .push(color);
    for d in [cycle, duplicate] {
        let mut core = Core::memory().unwrap();
        assert_eq!(
            core.register_provider_device("example.keyboard", "opaque", "Keyboard", d)
                .unwrap_err()
                .code,
            "descriptor"
        );
    }
}
#[test]
fn static_color_visibility_uses_the_declared_choice_domain_even_without_color_channels() {
    let mut d = presented_descriptor();
    d.definition["lighting"][0]
        .as_object_mut()
        .unwrap()
        .remove("colorChannels");
    d.definition["lighting"][0]["staticColors"] = serde_json::json!({
        "settingId":"individualColors", "address":"controlId",
        "visibleWhen":{"settingId":"effect","values":["solid"]}
    });
    Core::memory()
        .unwrap()
        .register_provider_device("example.keyboard", "a", "Keyboard", d.clone())
        .unwrap();
    for condition in [
        serde_json::json!({"settingId":"effect","values":["unsupported"]}),
        serde_json::json!({"settingId":"missing","values":[true]}),
        serde_json::json!({"settingId":"pace","values":[1]}),
        serde_json::json!({"settingId":"enabled","values":[1]}),
        serde_json::json!({"settingId":"effect","values":[]}),
        serde_json::json!({"settingId":"effect","values":["solid"],"evaluate":"anything"}),
    ] {
        let mut invalid = d.clone();
        invalid.definition["lighting"][0]["staticColors"]["visibleWhen"] = condition;
        let mut core = Core::memory().unwrap();
        let revision = core.revision();
        assert_eq!(
            core.register_provider_device("example.keyboard", "a", "Keyboard", invalid)
                .unwrap_err()
                .code,
            "descriptor"
        );
        assert_eq!(core.revision(), revision);
    }
    d.definition["lighting"][0]["staticColors"]["visibleWhen"] =
        serde_json::json!({"settingId":"enabled","values":[true,false]});
    Core::memory()
        .unwrap()
        .register_provider_device("example.keyboard", "a", "Keyboard", d)
        .unwrap();
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
#[test]
fn passive_light_strip_decoration_preserves_geometry_without_creating_a_control() {
    let mut c = Core::memory().unwrap();
    let mut d = descriptor();
    d.geometry.width = 1044.0;
    d.geometry.height = 356.0;
    d.definition["decorations"] = serde_json::json!([{"id":"led-panel","kind":"light-strip","label":{"en":"LED panel","ru":"LED-панель"},"x":928,"y":185,"width":116,"height":43}]);
    let id = c
        .register_provider_device("example.keyboard", "decorated", "Keyboard", d)
        .unwrap();
    let s = c.snapshot();
    assert_eq!(s.geometries[&id].controls.len(), 2);
    assert!(
        s.geometries[&id]
            .controls
            .iter()
            .all(|control| control.id != "led-panel")
    );
    let device = s.configuration.devices.iter().find(|d| d.id == id).unwrap();
    assert_eq!(
        device.provider_descriptor.as_ref().unwrap().definition["decorations"][0]["kind"],
        "light-strip"
    );
    assert!(
        c.dispatch(
            Command::ConfigureControl {
                device_id: id.clone(),
                page_id: device.profile.active_page_id.clone(),
                control_id: "led-panel".into(),
                kind: Some(ActionKind::Counter),
                label: "Not assignable".into(),
                expected_revision: c.revision()
            },
            "editor"
        )
        .is_err()
    );
    assert!(
        c.dispatch(
            Command::Input {
                device_id: id,
                control_id: "led-panel".into(),
                event: InputEvent::Click
            },
            "editor"
        )
        .is_err()
    );
}
#[test]
fn malformed_or_active_decorations_are_rejected_before_registration() {
    let base = serde_json::json!({"id":"led-panel","kind":"light-strip","x":928,"y":185,"width":116,"height":43});
    let mut invalid = vec![serde_json::json!([base.clone(), base.clone()])];
    for patch in [
        serde_json::json!({"width":117}),
        serde_json::json!({"width":0}),
        serde_json::json!({"y":-1}),
        serde_json::json!({"x":"928"}),
        serde_json::json!({"kind":"touch"}),
        serde_json::json!({"id":"key-42"}),
        serde_json::json!({"id":""}),
        serde_json::json!({"events":[]}),
        serde_json::json!({"actions":[]}),
        serde_json::json!({"target":"screen"}),
        serde_json::json!({"label":{"en":4}}),
        serde_json::json!({"label":{"de":"Unexpected locale"}}),
        serde_json::json!({"label":{"en":"x".repeat(129)}}),
    ] {
        let mut item = base.clone();
        item.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        invalid.push(serde_json::json!([item]));
    }
    invalid.push(serde_json::json!(
        (0..33)
            .map(|n| {
                let mut item = base.clone();
                item["id"] = serde_json::json!(format!("passive-{n}"));
                item
            })
            .collect::<Vec<_>>()
    ));
    for decorations in invalid {
        let mut c = Core::memory().unwrap();
        let mut d = descriptor();
        d.geometry.width = 1044.0;
        d.geometry.height = 356.0;
        d.definition["decorations"] = decorations;
        let revision = c.revision();
        assert_eq!(
            c.register_provider_device("example.keyboard", "invalid", "Invalid decoration", d)
                .unwrap_err()
                .code,
            "descriptor"
        );
        assert_eq!(c.revision(), revision);
    }
}
#[test]
fn optional_key_travel_capability_preserves_old_descriptors_and_valid_sdk_ranges() {
    for capability in [
        None,
        Some(
            serde_json::json!({"minUm":0,"maxUm":4000,"staleAfterMs":600,"normalTyping":true,"evidence":"partial"}),
        ),
        Some(
            serde_json::json!({"minUm":0.0,"maxUm":10000.0,"staleAfterMs":600.0,"normalTyping":false,"evidence":"verified"}),
        ),
    ] {
        let mut c = Core::memory().unwrap();
        let mut d = descriptor();
        if let Some(capability) = capability {
            d.definition["observation"] = serde_json::json!({"keyTravel":capability});
        }
        let original = d.definition.get("observation").cloned();
        let id = c
            .register_provider_device("example.keyboard", "travel", "Keyboard", d)
            .unwrap();
        let s = c.snapshot();
        let device = s.configuration.devices.iter().find(|d| d.id == id).unwrap();
        assert_eq!(
            device
                .provider_descriptor
                .as_ref()
                .unwrap()
                .definition
                .get("observation"),
            original.as_ref()
        );
        assert_eq!(s.geometries[&id].controls.len(), 2);
    }
}
#[test]
fn malformed_key_travel_metadata_fails_registration_without_changing_saved_state() {
    let valid = serde_json::json!({"minUm":0,"maxUm":4000,"staleAfterMs":600,"normalTyping":true,"evidence":"partial"});
    let mut invalid = vec![
        serde_json::Value::Null,
        serde_json::json!([]),
        serde_json::json!({}),
        serde_json::json!({"keyTravel":null}),
        serde_json::json!({"keyTravel":valid.clone(),"unexpected":{}}),
    ];
    for patch in [
        serde_json::json!({"minUm":-1}),
        serde_json::json!({"minUm":1}),
        serde_json::json!({"minUm":"0"}),
        serde_json::json!({"maxUm":0}),
        serde_json::json!({"maxUm":10001}),
        serde_json::json!({"maxUm":3999.5}),
        serde_json::json!({"maxUm":null}),
        serde_json::json!({"staleAfterMs":601}),
        serde_json::json!({"staleAfterMs":"600"}),
        serde_json::json!({"normalTyping":1}),
        serde_json::json!({"evidence":"accepted"}),
        serde_json::json!({"extra":true}),
    ] {
        let mut range = valid.clone();
        range
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        invalid.push(serde_json::json!({"keyTravel":range}));
    }
    for observation in invalid {
        let mut c = Core::memory().unwrap();
        let mut d = descriptor();
        d.definition["observation"] = observation;
        let revision = c.revision();
        assert_eq!(
            c.register_provider_device("example.keyboard", "invalid-travel", "Invalid travel", d)
                .unwrap_err()
                .code,
            "descriptor"
        );
        assert_eq!(c.revision(), revision);
    }
}
