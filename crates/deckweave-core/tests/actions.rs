use deckweave_core::*;
fn send(core: &mut Core, mut command: Command) -> Snapshot {
    let revision = core.revision();
    match &mut command {
        Command::ConfigureProgram {
            expected_revision, ..
        }
        | Command::AddProfile {
            expected_revision, ..
        }
        | Command::SelectProfile {
            expected_revision, ..
        }
        | Command::SelectPage {
            expected_revision, ..
        }
        | Command::AddPage {
            expected_revision, ..
        }
        | Command::ProfileApplication {
            expected_revision, ..
        }
        | Command::AddFolder {
            expected_revision, ..
        }
        | Command::SetPinned {
            expected_revision, ..
        } => *expected_revision = revision,
        _ => {}
    }
    core.dispatch(command, "test").unwrap()
}
fn program(core: &mut Core, control: &str, plan: Plan) -> Binding {
    let d = core.snapshot().configuration.devices[0].clone();
    let s = send(
        core,
        Command::ConfigureProgram {
            device_id: d.id,
            page_id: d.profile.active_page_id,
            control_id: control.into(),
            plan,
            label: "Program".into(),
            expected_revision: 0,
        },
    );
    s.configuration.devices[0]
        .profile
        .pages
        .iter()
        .find(|p| p.id == s.configuration.devices[0].profile.active_page_id)
        .unwrap()
        .bindings[control]
        .clone()
}
fn input(core: &mut Core, control: &str, event: InputEvent) {
    let d = core.snapshot().configuration.devices[0].id.clone();
    send(
        core,
        Command::Input {
            device_id: d,
            control_id: control.into(),
            event,
        },
    );
}
fn value(core: &Core, binding: &Binding) -> i64 {
    core.snapshot()
        .feedback
        .iter()
        .find(|f| f.binding_id == binding.id)
        .map_or(0, |f| f.value)
}
fn count(label: &str) -> Binding {
    Binding::new(ActionKind::Counter, label)
}
fn sequence(action: Binding, delay_ms: u64) -> Binding {
    let mut b = Binding::new(ActionKind::Program, &action.label);
    b.plan = Some(Plan::Sequence {
        steps: vec![Step { delay_ms, action }],
    });
    b
}

#[test]
fn switch_alternates_sequences_and_changes_the_same_button() {
    let mut c = Core::memory().unwrap();
    let a = count("A");
    let b = count("B");
    let parent = program(
        &mut c,
        "key-1",
        Plan::Switch {
            states: vec![sequence(a.clone(), 0), sequence(b.clone(), 0)],
        },
    );
    for expected in ["B", "A", "B"] {
        input(&mut c, "key-1", InputEvent::Click);
        assert_eq!(
            c.snapshot()
                .feedback
                .iter()
                .find(|f| f.binding_id == parent.id)
                .unwrap()
                .title
                .as_deref(),
            Some(expected)
        );
    }
    assert_eq!(value(&c, &a), 2);
    assert_eq!(value(&c, &b), 1);
}
#[test]
fn scheduler_delays_cancels_and_does_not_replay_after_restart() {
    let mut c = Core::memory().unwrap();
    let a = count("A");
    let b = count("B");
    program(
        &mut c,
        "key-1",
        Plan::Sequence {
            steps: vec![
                Step {
                    delay_ms: 0,
                    action: a.clone(),
                },
                Step {
                    delay_ms: 1000,
                    action: b.clone(),
                },
            ],
        },
    );
    input(&mut c, "key-1", InputEvent::Click);
    assert_eq!(value(&c, &a), 1);
    assert_eq!(value(&c, &b), 0);
    c.advance(999);
    assert_eq!(value(&c, &b), 0);
    c.advance(1000);
    assert_eq!(value(&c, &b), 1);
    input(&mut c, "key-1", InputEvent::Click);
    let d = c.snapshot().configuration.devices[0].id.clone();
    send(&mut c, Command::CancelActions { device_id: d });
    c.advance(3000);
    assert_eq!(value(&c, &a), 2);
    assert_eq!(value(&c, &b), 1);
}
#[test]
fn key_logic_resolves_single_double_hold_and_cancellation_exclusively() {
    let mut c = Core::memory().unwrap();
    let single = count("Single");
    let double = count("Double");
    let hold = count("Hold");
    program(
        &mut c,
        "key-1",
        Plan::KeyLogic {
            single: Some(Box::new(single.clone())),
            double: Some(Box::new(double.clone())),
            hold: Some(Box::new(hold.clone())),
            double_ms: 300,
            hold_ms: 600,
        },
    );
    input(&mut c, "key-1", InputEvent::Down);
    c.advance(50);
    input(&mut c, "key-1", InputEvent::Up);
    c.advance(349);
    assert_eq!(value(&c, &single), 0);
    c.advance(350);
    assert_eq!(value(&c, &single), 1);
    assert!(c.snapshot().feedback.iter().any(|f|f.title.as_deref()==Some("Single")&&f.display.as_deref()==Some("Single")));
    input(&mut c, "key-1", InputEvent::Down);
    c.advance(400);
    input(&mut c, "key-1", InputEvent::Up);
    c.advance(500);
    input(&mut c, "key-1", InputEvent::Down);
    c.advance(550);
    input(&mut c, "key-1", InputEvent::Up);
    c.advance(900);
    assert_eq!(value(&c, &double), 1);
    assert!(c.snapshot().feedback.iter().any(|f|f.title.as_deref()==Some("Double")&&f.display.as_deref()==Some("Double")));
    assert_eq!(value(&c, &single), 1);
    input(&mut c, "key-1", InputEvent::Down);
    c.advance(1500);
    assert_eq!(value(&c, &hold), 1);
    input(&mut c, "key-1", InputEvent::Up);
    c.advance(2000);
    assert_eq!(value(&c, &single), 1);
    input(&mut c, "key-1", InputEvent::Down);
    input(&mut c, "key-1", InputEvent::Cancel);
    c.advance(4000);
    assert_eq!(value(&c, &hold), 1);
}

#[test]
fn failed_os_step_cancels_even_zero_delay_later_steps() {
    let mut core = Core::memory().unwrap();
    let later = count("Must not execute");
    let mut launch = Binding::new(ActionKind::Program, "Launch");
    launch.plan = Some(Plan::Launch {
        program: "missing.exe".into(),
        args: vec![],
    });
    let root = program(
        &mut core,
        "key-1",
        Plan::Sequence {
            steps: vec![
                Step {
                    delay_ms: 0,
                    action: launch,
                },
                Step {
                    delay_ms: 0,
                    action: later.clone(),
                },
            ],
        },
    );
    input(&mut core, "key-1", InputEvent::Click);
    assert_eq!(value(&core, &later), 0);
    let effects = core.take_effects();
    assert_eq!(effects.len(), 1);
    let d = core.snapshot().configuration.devices[0].id.clone();
    core.action_error(&d, &root.id, "Launch failed");
    core.advance(1000);
    assert_eq!(value(&core, &later), 0);
}
#[test]
fn folder_navigation_pinned_and_profiles_share_persisted_runtime_state() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.sqlite3");
    let mut c = Core::open(&path).unwrap();
    let d = c.snapshot().configuration.devices[0].clone();
    let root = d.profile.active_page_id.clone();
    send(
        &mut c,
        Command::SetPinned {
            device_id: d.id.clone(),
            control_id: "key-2".into(),
            pinned: true,
            expected_revision: 0,
        },
    );
    send(
        &mut c,
        Command::AddFolder {
            device_id: d.id.clone(),
            control_id: "key-3".into(),
            name: "Folder".into(),
            expected_revision: 0,
        },
    );
    input(&mut c, "key-3", InputEvent::Click);
    assert_ne!(
        c.snapshot().configuration.devices[0].profile.active_page_id,
        root
    );
    input(&mut c, "key-2", InputEvent::Click);
    assert!(c.snapshot().feedback.iter().any(|f| f.active));
    input(&mut c, "key-1", InputEvent::Click);
    assert_eq!(
        c.snapshot().configuration.devices[0].profile.active_page_id,
        root
    );
    send(
        &mut c,
        Command::AddProfile {
            device_id: d.id.clone(),
            name: "Work".into(),
            expected_revision: 0,
        },
    );
    drop(c);
    let mut c = Core::open(&path).unwrap();
    assert_eq!(c.snapshot().configuration.devices[0].profile.name, "Work");
    input(&mut c, "key-2", InputEvent::Click);
    assert!(c.snapshot().feedback.iter().any(|f| f.active));
}
#[test]
fn smart_profile_switching_pauses_globally_and_resumes_to_foreground() {
    let mut c = Core::memory().unwrap();
    let d = c.snapshot().configuration.devices[0].clone();
    send(
        &mut c,
        Command::AddProfile {
            device_id: d.id.clone(),
            name: "Editor".into(),
            expected_revision: 0,
        },
    );
    send(
        &mut c,
        Command::ProfileApplication {
            device_id: d.id.clone(),
            application: Some("editor.exe".into()),
            expected_revision: 0,
        },
    );
    let target = c.snapshot().configuration.devices[0].profile.id.clone();
    assert!(c.foreground_application(Some("unmapped.exe")).unwrap());
    assert_eq!(
        c.snapshot().configuration.devices[0].profile.id,
        d.profile.id
    );
    c.editor_opened("device-other");
    assert!(
        !c.foreground_application(Some("C:\\Apps\\editor.exe"))
            .unwrap()
    );
    assert_eq!(
        c.snapshot().configuration.devices[0].profile.id,
        d.profile.id
    );
    c.editor_closed("device-other");
    assert!(
        c.foreground_application(Some("C:\\Apps\\editor.exe"))
            .unwrap()
    );
    assert_eq!(c.snapshot().configuration.devices[0].profile.id, target);
    assert!(!c.foreground_application(None).unwrap());
    assert_eq!(c.snapshot().configuration.devices[0].profile.id, target);
}

#[test]
fn manual_profile_is_not_reset_when_smart_profiles_are_unconfigured() {
    let mut core = Core::memory().unwrap();
    let d = core.snapshot().configuration.devices[0].id.clone();
    send(
        &mut core,
        Command::AddProfile {
            device_id: d,
            name: "Manual".into(),
            expected_revision: 0,
        },
    );
    assert!(!core.foreground_application(Some("notepad.exe")).unwrap());
    assert_eq!(
        core.snapshot().configuration.devices[0].profile.name,
        "Manual"
    );
}
#[test]
fn dial_stack_push_selects_rotation_operates_and_wheel_rotation_never_executes() {
    let mut c = Core::memory().unwrap();
    let a = count("A");
    let b = count("B");
    program(
        &mut c,
        "dial-1",
        Plan::DialStack {
            actions: vec![a.clone(), b.clone()],
        },
    );
    input(&mut c, "dial-1", InputEvent::Click);
    assert_eq!(value(&c, &a), 0);
    assert_eq!(value(&c, &b), 0);
    input(&mut c, "dial-1", InputEvent::Rotate { ticks: 3 });
    assert_eq!(value(&c, &b), 3);
    let x = count("X");
    let y = count("Y");
    program(
        &mut c,
        "dial-2",
        Plan::ActionWheel {
            actions: vec![x.clone(), y.clone()],
        },
    );
    input(&mut c, "dial-2", InputEvent::Rotate { ticks: 1 });
    assert_eq!(value(&c, &y), 0);
    input(&mut c, "dial-2", InputEvent::Click);
    assert_eq!(value(&c, &y), 1);
}
