use deckweave_core::*;

fn identity(n: char) -> String {
    format!("n4pro-{}", n.to_string().repeat(64))
}
fn event(core: &mut Core, id: &str, generation: u64, control: &str, event: InputEvent) -> bool {
    core.hardware_input(
        id,
        generation,
        Some(control),
        Some(event),
        "test report".into(),
    )
    .unwrap()
}

#[test]
fn reconnect_and_restart_reuse_persisted_instance_and_assignments() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite3");
    let mut core = Core::open(&path).unwrap();
    let id = core.register_n4(&identity('a')).unwrap();
    let revision = core.snapshot().configuration.revision;
    assert_eq!(core.register_n4(&identity('a')).unwrap(), id);
    assert_eq!(core.snapshot().configuration.revision, revision);
    drop(core);
    let mut core = Core::open(&path).unwrap();
    assert_eq!(core.register_n4(&identity('a')).unwrap(), id);
    assert_eq!(
        core.snapshot().connections[&id].state,
        ConnectionState::Offline
    );
    assert!(core.register_n4("n4pro-invalid").is_err());
}

#[test]
fn stale_session_cannot_release_press_execute_or_report_output() {
    let mut core = Core::memory().unwrap();
    let id = core.register_n4(&identity('b')).unwrap();
    core.connection_changed(&id, 1, ConnectionState::Connected, None);
    assert!(event(&mut core, &id, 1, "key-1", InputEvent::Down));
    core.connection_changed(&id, 2, ConnectionState::Connected, None);
    assert!(core.snapshot().pressed.is_empty());
    assert!(!event(&mut core, &id, 1, "key-1", InputEvent::Up));
    assert!(!core.hardware_frame_sent(&id, 1));
    assert!(!core.connection_changed(&id, 1, ConnectionState::Offline, None));
    assert!(core.snapshot().feedback.is_empty());
    event(&mut core, &id, 2, "key-1", InputEvent::Up);
    assert!(core.snapshot().feedback.is_empty());
}

#[test]
fn editor_close_preserves_hardware_press_and_instances_do_not_cross_execute() {
    let mut core = Core::memory().unwrap();
    let a = core.register_n4(&identity('a')).unwrap();
    let b = core.register_n4(&identity('b')).unwrap();
    core.connection_changed(&a, 1, ConnectionState::Connected, None);
    core.connection_changed(&b, 1, ConnectionState::Connected, None);
    core.editor_opened("device-a");
    event(&mut core, &a, 1, "key-1", InputEvent::Down);
    core.editor_closed("device-a");
    assert_eq!(core.snapshot().pressed.len(), 1);
    event(&mut core, &a, 1, "key-1", InputEvent::Up);
    event(&mut core, &b, 1, "dial-1", InputEvent::Rotate { ticks: -2 });
    event(&mut core, &b, 1, "touch-5", InputEvent::Down);
    event(&mut core, &b, 1, "touch-5", InputEvent::Up);
    let s = core.snapshot();
    assert_eq!(
        s.feedback.iter().find(|f| f.device_id == a).unwrap().value,
        1
    );
    assert_eq!(
        s.feedback
            .iter()
            .find(|f| f.device_id == b && f.value == -2)
            .unwrap()
            .value,
        -2
    );
    assert!(s.feedback.iter().any(|f| f.device_id == b && f.active));
}

#[test]
fn overlapping_ui_and_hardware_release_keeps_session_and_original_owner() {
    let mut core = Core::memory().unwrap();
    let id = core.register_n4(&identity('a')).unwrap();
    core.connection_changed(&id, 1, ConnectionState::Connected, None);
    core.dispatch(
        Command::Input {
            device_id: id.clone(),
            control_id: "key-1".into(),
            event: InputEvent::Down,
        },
        "editor",
    )
    .unwrap();
    assert!(event(&mut core, &id, 1, "key-1", InputEvent::Down));
    assert!(event(&mut core, &id, 1, "key-1", InputEvent::Up));
    assert_eq!(core.snapshot().pressed.len(), 1);
    assert!(core.snapshot().feedback.is_empty());
    core.dispatch(
        Command::Input {
            device_id: id.clone(),
            control_id: "key-1".into(),
            event: InputEvent::Up,
        },
        "editor",
    )
    .unwrap();
    assert_eq!(core.snapshot().feedback[0].value, 1);
    assert_eq!(
        core.snapshot().connections[&id].state,
        ConnectionState::Connected
    );
}
