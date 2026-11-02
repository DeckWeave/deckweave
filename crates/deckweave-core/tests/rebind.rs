use deckweave_core::*;
use std::collections::BTreeMap;
fn descriptor(instance: &str) -> ProviderDescriptor {
    ProviderDescriptor {
        geometry: Geometry {
            width: 2.0,
            height: 1.0,
            surface: [0.0; 4],
            surface_pixels: [0; 2],
            controls: vec![
                Control {
                    id: "key-a".into(),
                    kind: ControlKind::Key,
                    rect: [0.0, 0.0, 1.0, 1.0],
                    number: 1,
                    pointer: false,
                },
                Control {
                    id: "key-b".into(),
                    kind: ControlKind::Key,
                    rect: [1.0, 0.0, 1.0, 1.0],
                    number: 2,
                    pointer: false,
                },
            ],
        },
        definition: serde_json::json!({"instanceId":instance,"modelId":"example.keyboard.white","settings":[{"id":"brightness","type":"number","min":0,"max":5,"step":1}],"controls":[{"id":"key-a","kind":"key"},{"id":"key-b","kind":"key"}],"lighting":[]}),
    }
}
fn press(c: &mut Core, id: &str, control: &str, event: InputEvent) {
    c.dispatch(
        Command::Input {
            device_id: id.into(),
            control_id: control.into(),
            event,
        },
        "editor",
    )
    .unwrap();
}
#[test]
fn explicit_rebind_preserves_configuration_and_cancels_old_runtime_without_touching_sibling() {
    let mut c = Core::memory().unwrap();
    let id = c
        .register_provider_device(
            "example.keyboard",
            "session-old",
            "Custom keyboard",
            descriptor("session-old"),
        )
        .unwrap();
    let sibling = c
        .register_provider_device(
            "example.keyboard",
            "sibling",
            "Sibling",
            descriptor("sibling"),
        )
        .unwrap();
    let page = c
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .profile
        .active_page_id
        .clone();
    c.dispatch(
        Command::ConfigureProgram {
            device_id: id.clone(),
            page_id: page.clone(),
            control_id: "key-a".into(),
            label: "Delayed".into(),
            plan: Plan::Sequence {
                steps: vec![Step {
                    delay_ms: 1000,
                    action: Binding::new(ActionKind::Counter, "later"),
                }],
            },
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    c.dispatch(
        Command::SetProviderSettings {
            device_id: id.clone(),
            changes: BTreeMap::from([("brightness".into(), serde_json::json!(3))]),
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    press(&mut c, &id, "key-a", InputEvent::Click);
    press(&mut c, &id, "key-a", InputEvent::Down);
    press(&mut c, &sibling, "key-a", InputEvent::Down);
    assert!(!c.retained_actions().is_empty());
    let before = c
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .clone();
    assert_eq!(
        c.rebind_provider_device(
            &id,
            "example.keyboard",
            "session-new",
            descriptor("session-new")
        )
        .unwrap(),
        id
    );
    let s = c.snapshot();
    let after = s.configuration.devices.iter().find(|d| d.id == id).unwrap();
    assert_eq!(after.name, "Custom keyboard");
    assert_eq!(after.hardware_key.as_deref(), Some("session-new"));
    assert_eq!(after.profile.id, before.profile.id);
    assert_eq!(
        after.profile.pages[0].bindings["key-a"].id,
        before.profile.pages[0].bindings["key-a"].id
    );
    assert_eq!(after.provider_settings, before.provider_settings);
    assert_eq!(
        after.provider_descriptor.as_ref().unwrap().definition["instanceId"],
        "session-new"
    );
    assert_eq!(c.device_lifecycle_epoch(&id), 1);
    assert_eq!(s.pressed.len(), 1);
    assert_eq!(s.pressed[0].device_id, sibling);
    assert!(c.retained_actions().iter().all(|a| a.device_id != id));
    assert!(c.take_provider_outputs().is_empty());
    assert!(
        !c.hardware_input(&id, 1, Some("key-a"), Some(InputEvent::Up), "stale".into())
            .unwrap()
    );
    c.connection_changed(&id, 2, ConnectionState::Connected, None);
    assert!(
        !c.hardware_input(&id, 1, Some("key-a"), Some(InputEvent::Up), "stale".into())
            .unwrap()
    );
    c.advance(2000);
    assert!(c.take_effects().is_empty());
}
#[test]
fn explicit_rebind_after_restart_and_saved_removal_reuses_ids_instead_of_creating_device() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite3");
    let mut c = Core::open(&path).unwrap();
    let id = c
        .register_provider_device(
            "example.keyboard",
            "process-one",
            "My keyboard",
            descriptor("process-one"),
        )
        .unwrap();
    let original = c
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .clone();
    drop(c);
    let mut c = Core::open(&path).unwrap();
    let count = c.snapshot().configuration.devices.len();
    assert_eq!(
        c.rebind_provider_device(
            &id,
            "example.keyboard",
            "process-two",
            descriptor("process-two")
        )
        .unwrap(),
        id
    );
    assert_eq!(c.snapshot().configuration.devices.len(), count);
    c.dispatch(
        Command::RemoveDevice {
            device_id: id.clone(),
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    assert!(c.is_hardware_removed("example.keyboard", "process-two"));
    assert_eq!(
        c.rebind_provider_device(
            &id,
            "example.keyboard",
            "process-three",
            descriptor("process-three")
        )
        .unwrap(),
        id
    );
    let s = c.snapshot();
    assert_eq!(s.configuration.devices.len(), count);
    assert!(s.configuration.removed_devices.is_empty());
    assert!(!c.is_hardware_removed("example.keyboard", "process-two"));
    let restored = s.configuration.devices.iter().find(|d| d.id == id).unwrap();
    assert_eq!(restored.name, original.name);
    assert_eq!(restored.profile.id, original.profile.id);
    assert_eq!(
        restored.profile.active_page_id,
        original.profile.active_page_id
    );
    assert!(c.take_provider_outputs().is_empty());
    drop(c);
    let mut c = Core::open(&path).unwrap();
    assert_eq!(
        c.snapshot()
            .configuration
            .devices
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .hardware_key
            .as_deref(),
        Some("process-three")
    );
    assert!(c.take_provider_outputs().is_empty());
}
#[test]
fn rebind_rejects_wrong_provider_model_connected_target_and_claimed_identity_atomically() {
    let mut c = Core::memory().unwrap();
    let a = c
        .register_provider_device("example.keyboard", "a", "A", descriptor("a"))
        .unwrap();
    let b = c
        .register_provider_device("example.keyboard", "b", "B", descriptor("b"))
        .unwrap();
    let revision = c.revision();
    assert_eq!(
        c.rebind_provider_device(&a, "different.provider", "new", descriptor("new"))
            .unwrap_err()
            .code,
        "provider"
    );
    let mut wrong = descriptor("new");
    wrong.definition["modelId"] = serde_json::json!("different.model");
    assert_eq!(
        c.rebind_provider_device(&a, "example.keyboard", "new", wrong)
            .unwrap_err()
            .code,
        "model"
    );
    assert_eq!(
        c.rebind_provider_device(&a, "example.keyboard", "b", descriptor("b"))
            .unwrap_err()
            .code,
        "identity"
    );
    assert_eq!(c.revision(), revision);
    assert_eq!(c.device_lifecycle_epoch(&a), 0);
    c.connection_changed(&a, 1, ConnectionState::Connected, None);
    assert_eq!(
        c.rebind_provider_device(&a, "example.keyboard", "new", descriptor("new"))
            .unwrap_err()
            .code,
        "connected"
    );
    assert_eq!(c.revision(), revision);
    c.connection_changed(&a, 1, ConnectionState::Offline, None);
    c.dispatch(
        Command::RemoveDevice {
            device_id: b,
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    let revision = c.revision();
    assert_eq!(
        c.rebind_provider_device(&a, "example.keyboard", "b", descriptor("b"))
            .unwrap_err()
            .code,
        "identity"
    );
    assert_eq!(c.revision(), revision);
}
