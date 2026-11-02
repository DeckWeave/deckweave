use deckweave_core::*;
fn key(c: char) -> String {
    format!("n4pro-{}", c.to_string().repeat(64))
}
fn remove(c: &mut Core, id: &str) {
    c.dispatch(
        Command::RemoveDevice {
            device_id: id.into(),
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
}
#[test]
fn remove_restore_survive_restart_and_keep_identity_and_sibling() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite3");
    let mut c = Core::open(&path).unwrap();
    let a = c.register_n4(&key('a')).unwrap();
    let b = c.register_n4(&key('b')).unwrap();
    let original = c
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == a)
        .unwrap()
        .clone();
    c.connection_changed(&a, 1, ConnectionState::Connected, None);
    c.connection_changed(&b, 1, ConnectionState::Connected, None);
    for id in [&a, &b] {
        c.hardware_input(id, 1, Some("key-1"), Some(InputEvent::Down), "test".into())
            .unwrap();
    }
    remove(&mut c, &a);
    let s = c.snapshot();
    assert_eq!(s.pressed.len(), 1);
    assert_eq!(s.pressed[0].device_id, b);
    assert!(!c.has_device(&a));
    assert_eq!(
        s.configuration.removed_devices[0].profile.id,
        original.profile.id
    );
    assert_eq!(c.register_n4(&key('a')).unwrap_err().code, "device_removed");
    assert!(
        !c.hardware_input(&a, 1, Some("key-1"), Some(InputEvent::Up), "stale".into())
            .unwrap()
    );
    assert!(c.take_plugin_inputs().is_empty());
    assert!(s.feedback.iter().all(|f| f.device_id != a));
    drop(c);
    let mut c = Core::open(&path).unwrap();
    assert!(c.is_hardware_removed("deckweave.n4", &key('a')));
    c.dispatch(
        Command::RestoreDevice {
            device_id: a.clone(),
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    assert_eq!(c.register_n4(&key('a')).unwrap(), a);
    let s = c.snapshot();
    let restored = s.configuration.devices.iter().find(|d| d.id == a).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(original).unwrap()
    );
    assert_eq!(s.connections[&a].state, ConnectionState::Offline);
}
#[test]
fn virtual_restore_and_forget_are_distinct_and_cannot_forget_active_device() {
    let mut c = Core::memory().unwrap();
    let d = c.snapshot().configuration.devices[0].clone();
    assert!(
        c.dispatch(
            Command::ForgetDevice {
                device_id: d.id.clone(),
                expected_revision: c.revision()
            },
            "main"
        )
        .is_err()
    );
    remove(&mut c, &d.id);
    assert!(c.snapshot().configuration.devices.is_empty());
    c.dispatch(
        Command::RestoreDevice {
            device_id: d.id.clone(),
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    assert_eq!(
        c.snapshot().connections[&d.id].state,
        ConnectionState::Virtual
    );
    remove(&mut c, &d.id);
    c.dispatch(
        Command::ForgetDevice {
            device_id: d.id.clone(),
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    assert!(c.snapshot().configuration.removed_devices.is_empty());
    assert!(
        c.dispatch(
            Command::RestoreDevice {
                device_id: d.id,
                expected_revision: c.revision()
            },
            "main"
        )
        .is_err()
    );
}
#[test]
fn forgetting_hardware_does_not_undo_removal_but_explicit_readd_can() {
    let mut c = Core::memory().unwrap();
    let a = c.register_n4(&key('a')).unwrap();
    remove(&mut c, &a);
    c.dispatch(
        Command::ForgetDevice {
            device_id: a.clone(),
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    assert_eq!(c.register_n4(&key('a')).unwrap_err().code, "device_removed");
    c.dispatch(
        Command::AllowHardware {
            provider_id: "deckweave.n4".into(),
            hardware_key: key('a'),
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    assert_ne!(c.register_n4(&key('a')).unwrap(), a);
}
#[test]
fn removing_device_cancels_queued_sequence_and_pending_gestures_without_release() {
    let mut c = Core::memory().unwrap();
    let d = c.snapshot().configuration.devices[0].clone();
    c.dispatch(
        Command::ConfigureProgram {
            device_id: d.id.clone(),
            page_id: d.profile.active_page_id.clone(),
            control_id: "key-1".into(),
            plan: Plan::Sequence {
                steps: vec![Step {
                    delay_ms: 1000,
                    action: Binding::new(ActionKind::Counter, "later"),
                }],
            },
            label: "Sequence".into(),
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    c.dispatch(
        Command::Input {
            device_id: d.id.clone(),
            control_id: "key-1".into(),
            event: InputEvent::Click,
        },
        "main",
    )
    .unwrap();
    assert!(!c.retained_actions().is_empty());
    remove(&mut c, &d.id);
    c.advance(2000);
    assert!(c.retained_actions().is_empty());
    assert!(c.take_effects().is_empty());
    assert!(c.snapshot().feedback.is_empty());
}
#[test]
fn settings_are_unspecified_until_edit_and_validated_before_commit() {
    let mut c = Core::memory().unwrap();
    let id = c.register_n4(&key('a')).unwrap();
    let d = c
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .clone();
    assert_eq!(d.hardware_settings, DeviceHardwareSettings::default());
    let revision = c.revision();
    assert!(
        c.dispatch(
            Command::SetDeviceHardwareSettings {
                device_id: id.clone(),
                settings: DeviceHardwareSettings {
                    screen_brightness: Some(101),
                    ..Default::default()
                },
                expected_revision: revision
            },
            "main"
        )
        .is_err()
    );
    assert_eq!(c.revision(), revision);
    let settings = DeviceHardwareSettings {
        screen_brightness: Some(15),
        dial_brightness: Some(80),
        vibration: Some(false),
    };
    c.dispatch(
        Command::SetDeviceHardwareSettings {
            device_id: id.clone(),
            settings: settings.clone(),
            expected_revision: revision,
        },
        "main",
    )
    .unwrap();
    remove(&mut c, &id);
    assert_eq!(
        c.snapshot().configuration.removed_devices[0].hardware_settings,
        settings
    );
}
#[test]
fn migration_backs_up_v1_and_unknown_provider_remains_inert() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite3");
    let mut config = Configuration {
        format_version: 2,
        ..Configuration::default()
    };
    config.devices[0].provider_id = "example.unavailable".into();
    config.devices[0].profile.pages[0].bindings.insert(
        "unknown-physical-key".into(),
        Binding::new(ActionKind::Counter, "preserve"),
    );
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE configuration(id INTEGER PRIMARY KEY,revision INTEGER,json TEXT); PRAGMA user_version=1;").unwrap();
    conn.execute(
        "INSERT INTO configuration VALUES(1,0,?1)",
        [serde_json::to_string(&config).unwrap()],
    )
    .unwrap();
    drop(conn);
    let mut c = Core::open(&path).unwrap();
    let s = c.snapshot();
    assert!(s.geometries[&config.devices[0].id].controls.is_empty());
    assert!(
        c.dispatch(
            Command::Input {
                device_id: config.devices[0].id.clone(),
                control_id: "key-1".into(),
                event: InputEvent::Click
            },
            "main"
        )
        .is_err()
    );
    drop(c);
    let files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("pre-v3"))
        .collect();
    assert_eq!(files.len(), 1);
    let backup = rusqlite::Connection::open(files[0].path()).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    let conn = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
}
#[test]
fn package_removal_cancels_nested_scenarios_but_keeps_unrelated_controls() {
    let mut c = Core::memory().unwrap();
    let d = c.snapshot().configuration.devices[0].clone();
    let mut plugin = Binding::new(ActionKind::Plugin, "package action");
    plugin.plugin = Some(PluginAction {
        package: "example.removed".into(),
        action: "example.action".into(),
        settings: serde_json::json!({}),
        controller: None,
    });
    let mut launch = Binding::new(ActionKind::Program, "launch");
    launch.plan = Some(Plan::Launch {
        program: "example.exe".into(),
        args: vec![],
    });
    for (control, mut steps) in [
        (
            "key-1",
            vec![
                Step {
                    delay_ms: 1000,
                    action: plugin.clone(),
                },
                Step {
                    delay_ms: 1000,
                    action: launch.clone(),
                },
            ],
        ),
        (
            "key-2",
            vec![Step {
                delay_ms: 1000,
                action: launch,
            }],
        ),
    ] {
        for step in &mut steps {
            step.action.id = new_id();
        }
        c.dispatch(
            Command::ConfigureProgram {
                device_id: d.id.clone(),
                page_id: d.profile.active_page_id.clone(),
                control_id: control.into(),
                plan: Plan::Sequence { steps },
                label: control.into(),
                expected_revision: c.revision(),
            },
            "main",
        )
        .unwrap();
        c.dispatch(
            Command::Input {
                device_id: d.id.clone(),
                control_id: control.into(),
                event: InputEvent::Click,
            },
            "main",
        )
        .unwrap();
    }
    assert_eq!(c.retained_actions().len(), 2);
    c.cancel_package("example.removed");
    assert_eq!(c.retained_actions().len(), 1);
    c.advance(5000);
    assert!(c.take_plugin_inputs().is_empty());
    let effects = c.take_effects();
    assert_eq!(effects.len(), 1);
    assert_eq!(
        effects[0].binding_id,
        c.snapshot().configuration.devices[0].profile.pages[0].bindings["key-2"].id
    );
    assert!(
        c.snapshot().configuration.devices[0].profile.pages[0].bindings["key-1"]
            .children()
            .iter()
            .any(|b| b.plugin.is_some())
    );
}
#[test]
fn migration_rejects_already_open_old_writer_and_preserves_unknown_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.sqlite3");
    let mut legacy = serde_json::to_value(Configuration::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("format_version");
    legacy["reserved_future_data"] = serde_json::json!({"preserve":[1,2,3]});
    let old = rusqlite::Connection::open(&path).unwrap();
    old.execute_batch("CREATE TABLE configuration(id INTEGER PRIMARY KEY,revision INTEGER,json TEXT);PRAGMA user_version=1;").unwrap();
    old.execute(
        "INSERT INTO configuration VALUES(1,0,?1)",
        [serde_json::to_string(&legacy).unwrap()],
    )
    .unwrap();
    let c = Core::open(&path).unwrap();
    assert_eq!(c.snapshot().configuration.format_version, 3);
    assert!(
        old.execute(
            "UPDATE configuration SET revision=1,json=?1 WHERE id=1",
            [serde_json::to_string(&legacy).unwrap()]
        )
        .is_err()
    );
    let (revision, json): (i64, String) = old
        .query_row(
            "SELECT revision,json FROM configuration WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(revision, 0);
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["reserved_future_data"], legacy["reserved_future_data"]);
    assert_eq!(json["format_version"], 3);
    assert_eq!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("pre-v3"))
            .count(),
        1
    );
}
#[test]
fn pre_guard_v2_database_is_upgraded_and_future_format_is_not_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("early-v2.sqlite3");
    let mut legacy = serde_json::to_value(Configuration::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("format_version");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE configuration(id INTEGER PRIMARY KEY,revision INTEGER,json TEXT);PRAGMA user_version=2;").unwrap();
    conn.execute(
        "INSERT INTO configuration VALUES(1,0,?1)",
        [serde_json::to_string(&legacy).unwrap()],
    )
    .unwrap();
    drop(conn);
    assert_eq!(
        Core::open(&path)
            .unwrap()
            .snapshot()
            .configuration
            .format_version,
        3
    );
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert!(
        conn.execute(
            "UPDATE configuration SET json=?1",
            [serde_json::to_string(&legacy).unwrap()]
        )
        .is_err()
    );
    conn.execute_batch("DROP TRIGGER configuration_v3_guard")
        .unwrap();
    legacy["format_version"] = serde_json::json!(4);
    let original = serde_json::to_string(&legacy).unwrap();
    conn.execute("UPDATE configuration SET json=?1", [&original])
        .unwrap();
    drop(conn);
    assert_eq!(Core::open(&path).err().unwrap().code, "schema");
    let conn = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        conn.query_row("SELECT json FROM configuration", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        original
    );
}
