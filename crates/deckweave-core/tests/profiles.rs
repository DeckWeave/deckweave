use deckweave_core::*;

fn send(core: &mut Core, command: impl FnOnce(u64, &Device) -> Command) -> Snapshot {
    let d = core.snapshot().configuration.devices[0].clone();
    core.dispatch(command(core.revision(), &d), "profiles-test")
        .unwrap()
}

#[test]
fn copied_profile_rekeys_folders_panels_nested_actions_and_self_links_and_persists() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("profiles.sqlite3");
    let mut core = Core::open(&path).unwrap();
    send(&mut core, |r, d| Command::AddFolder {
        device_id: d.id.clone(),
        control_id: "key-1".into(),
        name: "Folder".into(),
        expected_revision: r,
    });
    let folder = core.snapshot().configuration.devices[0]
        .profile
        .pages
        .iter()
        .find(|p| p.parent.is_some())
        .unwrap()
        .id
        .clone();
    send(&mut core, |r, d| Command::SelectPage {
        device_id: d.id.clone(),
        page_id: folder.clone(),
        expected_revision: r,
    });
    send(&mut core, |r, d| {
        let mut action = Binding::new(ActionKind::Program, "Self");
        action.plan = Some(Plan::Sequence {
            steps: vec![Step {
                delay_ms: 25,
                action: {
                    let mut child = Binding::new(ActionKind::Program, "Profile");
                    child.plan = Some(Plan::Profile {
                        profile_id: d.profile.id.clone(),
                    });
                    child
                },
            }],
        });
        Command::ConfigureProgram {
            device_id: d.id.clone(),
            page_id: folder.clone(),
            control_id: "key-2".into(),
            plan: action.plan.unwrap(),
            label: action.label,
            expected_revision: r,
        }
    });
    let source = core.snapshot().configuration.devices[0].profile.clone();
    let snapshot = send(&mut core, |r, d| Command::DuplicateProfile {
        device_id: d.id.clone(),
        profile_id: d.profile.id.clone(),
        name: "Work copy".into(),
        expected_revision: r,
    });
    let d = &snapshot.configuration.devices[0];
    assert_ne!(source.id, d.profile.id);
    assert_eq!(source.pages.len(), d.profile.pages.len());
    assert_eq!(
        d.profile
            .pages
            .iter()
            .find(|p| p.parent.is_some())
            .unwrap()
            .parent
            .as_ref(),
        Some(&d.profile.pages[0].id)
    );
    for p in &d.profile.pages {
        assert!(!source.pages.iter().any(|old| old.id == p.id));
    }
    let nested = &d
        .profile
        .pages
        .iter()
        .find(|p| p.parent.is_some())
        .unwrap()
        .bindings["key-2"];
    let Plan::Sequence { steps } = nested.plan.as_ref().unwrap() else {
        panic!()
    };
    assert!(
        matches!(&steps[0].action.plan,Some(Plan::Profile { profile_id }) if profile_id==&d.profile.id)
    );
    assert_ne!(
        nested.id,
        source
            .pages
            .iter()
            .find(|p| p.id == folder)
            .unwrap()
            .bindings["key-2"]
            .id
    );
    assert_eq!(d.profiles[0].id, source.id);
    let copy = d.profile.id.clone();
    drop(core);
    let core = Core::open(&path).unwrap();
    assert_eq!(core.snapshot().configuration.devices[0].profile.id, copy);
}

#[test]
fn deleting_profile_protects_external_nested_references_and_last_profile() {
    let mut core = Core::memory().unwrap();
    let original = core.snapshot().configuration.devices[0].profile.id.clone();
    send(&mut core, |r, d| Command::AddProfile {
        device_id: d.id.clone(),
        name: "Second".into(),
        expected_revision: r,
    });
    let second = core.snapshot().configuration.devices[0].profile.id.clone();
    send(&mut core, |r, d| {
        let mut binding = Binding::new(ActionKind::Program, "Switch");
        binding.plan = Some(Plan::Profile {
            profile_id: original.clone(),
        });
        Command::ConfigureProgram {
            device_id: d.id.clone(),
            page_id: d.profile.active_page_id.clone(),
            control_id: "key-1".into(),
            plan: binding.plan.unwrap(),
            label: binding.label,
            expected_revision: r,
        }
    });
    send(&mut core, |r, d| Command::SelectProfile {
        device_id: d.id.clone(),
        profile_id: original.clone(),
        expected_revision: r,
    });
    let d = core.snapshot().configuration.devices[0].clone();
    let before = core.revision();
    let error = core
        .dispatch(
            Command::DeleteProfile {
                device_id: d.id.clone(),
                profile_id: d.profile.id.clone(),
                expected_revision: before,
            },
            "test",
        )
        .unwrap_err();
    assert_eq!(error.code, "profile_in_use");
    assert_eq!(core.revision(), before);
    send(&mut core, |r, d| Command::SelectProfile {
        device_id: d.id.clone(),
        profile_id: second,
        expected_revision: r,
    });
    send(&mut core, |r, d| Command::DeleteProfile {
        device_id: d.id.clone(),
        profile_id: d.profile.id.clone(),
        expected_revision: r,
    });
    let d = core.snapshot().configuration.devices[0].clone();
    assert_eq!(d.profile.id, original);
    assert!(
        core.dispatch(
            Command::DeleteProfile {
                device_id: d.id,
                profile_id: d.profile.id.clone(),
                expected_revision: core.revision()
            },
            "test"
        )
        .is_err()
    );
}

#[test]
fn profile_edits_reject_stale_targets_and_invalid_names_without_losing_data() {
    let mut core = Core::memory().unwrap();
    let original = core.snapshot().configuration.devices[0].profile.id.clone();
    send(&mut core, |r, d| Command::AddProfile {
        device_id: d.id.clone(),
        name: "Second".into(),
        expected_revision: r,
    });
    let d = core.snapshot().configuration.devices[0].clone();
    for command in [
        Command::RenameProfile {
            device_id: d.id.clone(),
            profile_id: original.clone(),
            name: "Wrong target".into(),
            expected_revision: core.revision(),
        },
        Command::DuplicateProfile {
            device_id: d.id.clone(),
            profile_id: original.clone(),
            name: "Wrong copy".into(),
            expected_revision: core.revision(),
        },
        Command::DeleteProfile {
            device_id: d.id.clone(),
            profile_id: original,
            expected_revision: core.revision(),
        },
        Command::RenameProfile {
            device_id: d.id.clone(),
            profile_id: d.profile.id.clone(),
            name: "  ".into(),
            expected_revision: core.revision(),
        },
    ] {
        assert!(core.dispatch(command, "test").is_err());
        assert_eq!(
            core.snapshot().configuration.devices[0].profile.name,
            "Second"
        );
        assert_eq!(core.snapshot().configuration.devices[0].profiles.len(), 1);
    }
    send(&mut core, |r, d| Command::RenameProfile {
        device_id: d.id.clone(),
        profile_id: d.profile.id.clone(),
        name: " Work ".into(),
        expected_revision: r,
    });
    assert_eq!(
        core.snapshot().configuration.devices[0].profile.name,
        "Work"
    );
}
