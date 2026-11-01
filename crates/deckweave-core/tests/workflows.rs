use deckweave_core::*;

fn run(core: &mut Core, command: Command) -> Snapshot {
    core.dispatch(command, "test").unwrap()
}
fn configure(
    core: &mut Core,
    device: &str,
    page: &str,
    control: &str,
    kind: ActionKind,
    label: &str,
) -> Snapshot {
    run(
        core,
        Command::ConfigureControl {
            device_id: device.into(),
            page_id: page.into(),
            control_id: control.into(),
            kind: Some(kind),
            label: label.into(),
            expected_revision: core.snapshot().configuration.revision,
        },
    )
}
fn input(core: &mut Core, device: &str, control: &str, event: InputEvent) -> Snapshot {
    run(
        core,
        Command::Input {
            device_id: device.into(),
            control_id: control.into(),
            event,
        },
    )
}

#[test]
fn configuration_and_navigation_survive_restart_but_pressed_does_not() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.sqlite3");
    let mut core = Core::open(&path).unwrap();
    let device = core.snapshot().configuration.devices[0].clone();
    let s = run(
        &mut core,
        Command::AddPage {
            device_id: device.id.clone(),
            expected_revision: 0,
        },
    );
    let page = s.configuration.devices[0].profile.active_page_id.clone();
    configure(
        &mut core,
        &device.id,
        &page,
        "touch-1",
        ActionKind::Toggle,
        "Microphone",
    );
    input(&mut core, &device.id, "touch-1", InputEvent::Down);
    drop(core);
    let reopened = Core::open(&path).unwrap().snapshot();
    assert_eq!(
        reopened.configuration.devices[0].profile.active_page_id,
        page
    );
    assert_eq!(
        reopened.configuration.devices[0].profile.pages[1].bindings["touch-1"].label,
        "Microphone"
    );
    assert!(reopened.pressed.is_empty());
    assert!(reopened.feedback.is_empty());
}

#[test]
fn down_is_visible_and_repeated_down_or_orphan_up_does_not_duplicate_action() {
    let mut core = Core::memory().unwrap();
    let id = core.snapshot().configuration.devices[0].id.clone();
    assert!(core.snapshot().feedback.is_empty()); // reading/selecting is not executing
    assert_eq!(
        input(&mut core, &id, "key-1", InputEvent::Down)
            .pressed
            .len(),
        1
    );
    input(&mut core, &id, "key-1", InputEvent::Down);
    let s = input(&mut core, &id, "key-1", InputEvent::Up);
    assert!(s.pressed.is_empty());
    assert_eq!(s.feedback[0].value, 1);
    assert_eq!(
        input(&mut core, &id, "key-1", InputEvent::Up).feedback[0].value,
        1
    );
}

#[test]
fn release_after_navigation_targets_original_binding() {
    let mut core = Core::memory().unwrap();
    let original = core.snapshot().configuration.devices[0].clone();
    input(&mut core, &original.id, "key-1", InputEvent::Down);
    let s = run(
        &mut core,
        Command::AddPage {
            device_id: original.id.clone(),
            expected_revision: 0,
        },
    );
    let new_page = s.configuration.devices[0].profile.active_page_id.clone();
    configure(
        &mut core,
        &original.id,
        &new_page,
        "key-1",
        ActionKind::Toggle,
        "Different action",
    );
    let s = input(&mut core, &original.id, "key-1", InputEvent::Up);
    assert_eq!(s.feedback.len(), 1);
    assert_eq!(
        s.feedback[0].binding_id,
        original.profile.pages[0].bindings["key-1"].id
    );
    assert_eq!(s.feedback[0].value, 1);
    assert!(!s.feedback[0].active);
}

#[test]
fn removal_during_press_cancels_old_action() {
    let mut core = Core::memory().unwrap();
    let d = core.snapshot().configuration.devices[0].clone();
    input(&mut core, &d.id, "key-1", InputEvent::Down);
    run(
        &mut core,
        Command::ConfigureControl {
            device_id: d.id.clone(),
            page_id: d.profile.active_page_id,
            control_id: "key-1".into(),
            kind: None,
            label: "".into(),
            expected_revision: 0,
        },
    );
    assert!(
        input(&mut core, &d.id, "key-1", InputEvent::Up)
            .feedback
            .is_empty()
    );
}

#[test]
fn two_instances_have_independent_feedback_and_navigation() {
    let mut core = Core::memory().unwrap();
    let s = run(
        &mut core,
        Command::AddDevice {
            expected_revision: 0,
        },
    );
    let a = s.configuration.devices[0].id.clone();
    let b = s.configuration.devices[1].id.clone();
    input(&mut core, &a, "key-1", InputEvent::Down);
    input(&mut core, &a, "key-1", InputEvent::Up);
    run(
        &mut core,
        Command::AddPage {
            device_id: a.clone(),
            expected_revision: 1,
        },
    );
    let s = core.snapshot();
    assert_eq!(s.configuration.devices[0].profile.pages.len(), 2);
    assert_eq!(s.configuration.devices[1].profile.pages.len(), 1);
    assert!(
        s.feedback
            .iter()
            .all(|f| f.device_id == a && f.device_id != b)
    );
}

#[test]
fn stale_or_invalid_edit_cannot_overwrite_saved_configuration() {
    let mut core = Core::memory().unwrap();
    let id = core.snapshot().configuration.devices[0].id.clone();
    run(
        &mut core,
        Command::RenameDevice {
            device_id: id.clone(),
            name: "Studio".into(),
            expected_revision: 0,
        },
    );
    let error = core
        .dispatch(
            Command::RenameDevice {
                device_id: id.clone(),
                name: "Stale".into(),
                expected_revision: 0,
            },
            "test",
        )
        .unwrap_err();
    assert_eq!(error.code, "conflict");
    assert!(
        core.dispatch(
            Command::RenameDevice {
                device_id: id,
                name: " ".into(),
                expected_revision: 1
            },
            "test"
        )
        .is_err()
    );
    assert_eq!(core.snapshot().configuration.revision, 1);
    assert_eq!(core.snapshot().configuration.devices[0].name, "Studio");
}

#[test]
fn failed_sql_commit_does_not_publish_candidate_state() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.sqlite3");
    let mut core = Core::open(&path).unwrap();
    let initial = core.snapshot().configuration.devices[0].clone();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER deny_change BEFORE UPDATE ON configuration BEGIN SELECT RAISE(ABORT, 'injected disk failure'); END;").unwrap();
    assert!(
        core.dispatch(
            Command::RenameDevice {
                device_id: initial.id,
                name: "Unsaved".into(),
                expected_revision: 0
            },
            "test"
        )
        .is_err()
    );
    assert_eq!(core.snapshot().configuration.devices[0].name, initial.name);
    assert_eq!(
        Core::open(&path).unwrap().snapshot().configuration.revision,
        0
    );
}

#[test]
fn closing_editor_cancels_its_press_without_running_action_and_pause_is_global() {
    let mut core = Core::memory().unwrap();
    let id = core.snapshot().configuration.devices[0].id.clone();
    core.editor_opened("a");
    core.editor_opened("b");
    core.editor_opened("a");
    core.dispatch(
        Command::Input {
            device_id: id,
            control_id: "key-1".into(),
            event: InputEvent::Down,
        },
        "a",
    )
    .unwrap();
    core.editor_closed("a");
    assert!(core.snapshot().smart_profiles_paused);
    assert_eq!(core.snapshot().editing_windows, 1);
    assert!(core.snapshot().pressed.is_empty());
    assert!(core.snapshot().feedback.is_empty());
    core.editor_closed("b");
    assert!(!core.snapshot().smart_profiles_paused);
}

#[test]
fn future_schema_and_corrupt_payload_are_not_reset() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.sqlite3");
    drop(Core::open(&path).unwrap());
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch("PRAGMA user_version=999;")
        .unwrap();
    assert!(Core::open(&path).is_err());
    assert_eq!(
        connection
            .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        999
    );
    connection
        .execute_batch("PRAGMA user_version=1; UPDATE configuration SET json='not json';")
        .unwrap();
    assert!(Core::open(&path).is_err());
    assert_eq!(
        connection
            .query_row::<String, _, _>("SELECT json FROM configuration", [], |r| r.get(0))
            .unwrap(),
        "not json"
    );
}

#[test]
fn dial_rotation_uses_signed_ticks_and_rejects_invalid_target_without_side_effects() {
    let mut core = Core::memory().unwrap();
    let d = core.snapshot().configuration.devices[0].clone();
    configure(
        &mut core,
        &d.id,
        &d.profile.active_page_id,
        "dial-1",
        ActionKind::Counter,
        "Dial",
    );
    input(&mut core, &d.id, "dial-1", InputEvent::Rotate { ticks: -3 });
    assert_eq!(core.snapshot().feedback[0].value, -3);
    assert!(
        core.dispatch(
            Command::Input {
                device_id: d.id,
                control_id: "key-1".into(),
                event: InputEvent::Rotate { ticks: 1 }
            },
            "test"
        )
        .is_err()
    );
    assert_eq!(core.snapshot().feedback.len(), 1);
}
