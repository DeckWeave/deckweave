use deckweave_core::*;
use serde_json::json;
use std::collections::BTreeMap;

fn keyboard(core: &mut Core) -> String {
    let controls: Vec<_> = (0..16)
        .map(|n| Control {
            id: format!("key-{n}"),
            kind: ControlKind::Key,
            rect: [(n % 4) as f64, (n / 4) as f64, 1.0, 1.0],
            number: n + 1,
            pointer: false,
        })
        .collect();
    let definition = json!({"settings":[],"controls":controls.iter().map(|c| json!({"id":c.id,"kind":"key","label":c.id})).collect::<Vec<_>>(),"keyboard":{"onboardMappings":{"settingId":"mapping","layers":["main","fn"],"bindingFormat":"keyboard-usage"}}});
    let id = core
        .register_provider_device(
            "example.keyboard",
            "session-a",
            "Keyboard",
            ProviderDescriptor {
                geometry: Geometry {
                    width: 4.0,
                    height: 4.0,
                    surface: [0.0; 4],
                    surface_pixels: [0; 2],
                    controls,
                },
                definition,
            },
        )
        .unwrap();
    core.connection_changed(&id, 7, ConnectionState::Connected, None);
    id
}
fn observed() -> KeyboardObservation {
    KeyboardObservation {
        main: (0..128).map(|n| [2, 0, 4 + (n % 80) as u8, 0]).collect(),
        fn_layer: (0..128).map(|n| [2, 0, 4 + (n % 80) as u8, 0]).collect(),
        control_indices: (0..16).map(|n| (format!("key-{n}"), n)).collect(),
        layer_revisions: BTreeMap::from([
            (KeyboardLayer::Main, "a".repeat(64)),
            (KeyboardLayer::Fn, "b".repeat(64)),
        ]),
    }
}
fn assign(
    core: &mut Core,
    device: &str,
    control: usize,
    layer: KeyboardLayer,
    observation: KeyboardObservation,
) -> KeyboardAssignmentOperation {
    let d = core
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == device)
        .unwrap();
    core.prepare_keyboard_assignment(KeyboardAssignmentRequest {
        device_id: device.into(),
        profile_id: d.profile.id,
        page_id: Some(d.profile.active_page_id),
        control_id: format!("key-{control}"),
        layer,
        binding: Binding::new(ActionKind::Counter, "Host action"),
        observation,
        generation: 7,
        identity: "confirmed-a".into(),
        expected_revision: core.revision(),
    })
    .unwrap()
}
fn readback(
    operation: &KeyboardAssignmentOperation,
    record: RawKeyboardRecord,
) -> KeyboardAssignmentReadback {
    KeyboardAssignmentReadback {
        operation_id: operation.operation_id.clone(),
        generation: operation.generation,
        identity: operation.identity.clone(),
        record,
    }
}
fn verify(core: &mut Core, operation: &KeyboardAssignmentOperation) {
    assert_eq!(
        core.complete_keyboard_assignment(readback(operation, operation.installed))
            .unwrap(),
        KeyboardAliasPhase::Verified
    );
}
fn legacy(core: &mut Core, device: &str, control: usize, trigger: u8) {
    core.dispatch(
        Command::SetFnBinding {
            device_id: device.into(),
            control_id: format!("key-{control}"),
            trigger,
            configure_hardware: false,
            binding: Some(Binding::new(ActionKind::Counter, "Legacy")),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
}

#[test]
fn allocation_accounts_for_inactive_profiles_requested_records_and_hidden_slots() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    legacy(&mut core, &device, 0, 13);
    core.dispatch(
        Command::AddProfile {
            device_id: device.clone(),
            name: "Other".into(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    core.dispatch(
        Command::SetProviderSettings {
            device_id: device.clone(),
            changes: serde_json::from_value(json!({"mapping":{"key-3":{"main":{"usage":105}}}}))
                .unwrap(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    core.take_provider_outputs();
    let mut observation = observed();
    observation.fn_layer[127] = [2, 0, 106, 0];
    let operation = assign(&mut core, &device, 1, KeyboardLayer::Main, observation);
    assert_eq!(operation.alias, 16);
    assert!(core.keyboard_alias_routes().is_empty());
    assert!(
        core.take_provider_outputs().is_empty(),
        "Prepare only saves intent; it never emits SET"
    );
}

#[test]
fn aliases_are_per_keyboard_and_main_and_fn_share_capacity() {
    let mut core = Core::memory().unwrap();
    let one = keyboard(&mut core);
    let descriptor = core
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == one)
        .unwrap()
        .provider_descriptor
        .clone()
        .unwrap();
    let two = core
        .register_provider_device("example.keyboard", "session-b", "Second", descriptor)
        .unwrap();
    core.connection_changed(&two, 7, ConnectionState::Connected, None);
    let first = assign(&mut core, &one, 0, KeyboardLayer::Fn, observed());
    let next = assign(&mut core, &one, 1, KeyboardLayer::Main, observed());
    let independent = assign(&mut core, &two, 0, KeyboardLayer::Fn, observed());
    assert_eq!((first.alias, next.alias, independent.alias), (13, 14, 13));
    assert!(
        core.fn_routes().is_empty(),
        "Owned pending sources never enter the global backend"
    );
}

#[test]
fn capacity_exhaustion_and_unknown_records_are_honest_and_do_not_save_partial_bindings() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let d = core
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == device)
        .unwrap()
        .clone();
    let mut observation = observed();
    for (n, alias) in (13..=24).enumerate() {
        observation.main[116 + n] = [2, 0, alias + 91, 0];
    }
    let revision = core.revision();
    let request = KeyboardAssignmentRequest {
        device_id: device.clone(),
        profile_id: d.profile.id,
        page_id: None,
        control_id: "key-0".into(),
        layer: KeyboardLayer::Fn,
        binding: Binding::new(ActionKind::Counter, "No capacity"),
        observation,
        generation: 7,
        identity: "confirmed-a".into(),
        expected_revision: revision,
    };
    assert_eq!(
        core.prepare_keyboard_assignment(request.clone())
            .unwrap_err()
            .code,
        "keyboard_capacity"
    );
    let mut unknown = request;
    unknown.observation = observed();
    unknown.observation.fn_layer[0] = [8, 1, 2, 3];
    assert_eq!(
        core.prepare_keyboard_assignment(unknown).unwrap_err().code,
        "keyboard_read_only"
    );
    assert_eq!(core.revision(), revision);
    let d = core
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == device)
        .unwrap();
    assert!(d.keyboard_aliases.is_empty());
    assert!(d.profile.fn_bindings.is_empty());
}

#[test]
fn completion_uses_captured_profile_and_routes_reject_stale_generation_binding_and_profile() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    core.dispatch(
        Command::AddProfile {
            device_id: device.clone(),
            name: "Other".into(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    verify(&mut core, &operation);
    assert!(core.keyboard_alias_routes().is_empty());
    core.dispatch(
        Command::SelectProfile {
            device_id: device.clone(),
            profile_id: operation.profile_id.clone().unwrap(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    let route = core.keyboard_alias_routes().pop().unwrap();
    assert!(core.activate_keyboard_alias(&route, 7).unwrap());
    assert!(!core.activate_keyboard_alias(&route, 8).unwrap());
    assert!(core.snapshot().pressed.is_empty());
    core.dispatch(
        Command::SetFnBinding {
            device_id: device.clone(),
            control_id: "key-0".into(),
            trigger: 13,
            configure_hardware: false,
            binding: None,
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    assert!(!core.activate_keyboard_alias(&route, 7).unwrap());
    assert_eq!(
        core.snapshot()
            .configuration
            .devices
            .iter()
            .find(|d| d.id == device)
            .unwrap()
            .keyboard_aliases
            .len(),
        1
    );
    let next = assign(&mut core, &device, 1, KeyboardLayer::Fn, observed());
    assert_eq!(
        next.alias, 14,
        "Removing an action retains physical ownership"
    );
    assert!(
        core.take_provider_outputs().is_empty(),
        "Profile switches and binding removal do not rewrite hardware"
    );
}

#[test]
fn exact_restore_preserves_host_actions_and_only_releases_after_readback() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    verify(&mut core, &operation);
    let mut observation = observed();
    observation.fn_layer[0] = operation.installed;
    let restore = core
        .prepare_keyboard_restore(KeyboardRestoreRequest {
            device_id: device.clone(),
            control_id: "key-0".into(),
            layer: KeyboardLayer::Fn,
            observation,
            generation: 7,
            identity: "confirmed-a".into(),
            expected_revision: core.revision(),
        })
        .unwrap();
    assert_ne!(restore.operation_id, operation.operation_id);
    assert_eq!(restore.installation_id, operation.installation_id);
    assert!(core.keyboard_alias_routes().is_empty());
    core.fail_keyboard_assignment(&restore.operation_id, "Readback timed out")
        .unwrap();
    assert_eq!(
        core.reconcile_keyboard_alias(readback(&restore, operation.original))
            .unwrap(),
        KeyboardAliasPhase::Released
    );
    let saved = core
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == device)
        .unwrap();
    assert!(
        saved.profile.fn_bindings.contains_key("key-0"),
        "Restoration keeps the user's host action dormant"
    );
    assert!(core.fn_routes().is_empty());
    let reuse = assign(&mut core, &device, 1, KeyboardLayer::Main, observed());
    assert_eq!(reuse.alias, 13);
}

#[test]
fn external_edit_is_never_restored_over_and_keeps_reservation() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    verify(&mut core, &operation);
    let external = [13, 0, 0, 17];
    assert_eq!(
        core.reconcile_keyboard_alias(readback(&operation, external))
            .unwrap(),
        KeyboardAliasPhase::Conflict
    );
    let mut observation = observed();
    observation.fn_layer[0] = external;
    assert_eq!(
        core.prepare_keyboard_restore(KeyboardRestoreRequest {
            device_id: device.clone(),
            control_id: "key-0".into(),
            layer: KeyboardLayer::Fn,
            observation,
            generation: 7,
            identity: "confirmed-a".into(),
            expected_revision: core.revision()
        })
        .unwrap_err()
        .code,
        "keyboard_restore_conflict"
    );
    assert!(core.keyboard_alias_routes().is_empty());
    assert_eq!(
        assign(&mut core, &device, 1, KeyboardLayer::Main, observed()).alias,
        14
    );
    assert!(core.take_provider_outputs().is_empty());
}

#[test]
fn crashed_pending_operation_never_replays_or_routes_until_explicit_reconciliation() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("state.sqlite3");
    let mut core = Core::open(&path).unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    drop(core);
    let mut core = Core::open(&path).unwrap();
    assert!(core.take_provider_outputs().is_empty());
    assert!(core.keyboard_alias_routes().is_empty());
    core.connection_changed(&device, 8, ConnectionState::Connected, None);
    let mut recovered = readback(&operation, operation.installed);
    recovered.generation = 8;
    assert_eq!(
        core.complete_keyboard_assignment(recovered.clone())
            .unwrap_err()
            .code,
        "keyboard_identity"
    );
    assert_eq!(
        core.reconcile_keyboard_alias(recovered).unwrap(),
        KeyboardAliasPhase::Verified
    );
    assert_eq!(core.keyboard_alias_routes().len(), 1);
    drop(core);
    let mut core = Core::open(&path).unwrap();
    core.connection_changed(&device, 8, ConnectionState::Connected, None);
    assert!(
        core.keyboard_alias_routes().is_empty(),
        "Persisted Verified is not an input-session lease"
    );
}

#[test]
fn changed_session_identity_requires_explicit_confirmed_matching_proof() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    verify(&mut core, &operation);
    core.connection_changed(&device, 8, ConnectionState::Connected, None);
    let mut next = readback(&operation, operation.installed);
    next.identity = "confirmed-b".into();
    next.generation = 8;
    assert!(core.complete_keyboard_assignment(next.clone()).is_err());
    assert!(core.reconcile_keyboard_alias(next.clone()).is_err());
    let mut wrong = next.clone();
    wrong.record = [2, 0, 99, 0];
    assert!(core.reconcile_keyboard_alias_confirmed(wrong).is_err());
    assert_eq!(
        core.reconcile_keyboard_alias_confirmed(next).unwrap(),
        KeyboardAliasPhase::Verified
    );
    assert_eq!(core.keyboard_alias_routes()[0].identity, "confirmed-b");
    assert!(
        !core
            .activate_keyboard_alias(
                &KeyboardAliasRoute {
                    identity: "confirmed-a".into(),
                    ..core.keyboard_alias_routes()[0].clone()
                },
                8
            )
            .unwrap()
    );
}

#[test]
fn failed_before_install_can_be_explicitly_cancelled_after_complete_original_readback() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    core.fail_keyboard_assignment(&operation.operation_id, "Write was not sent")
        .unwrap();
    let cancel = core
        .prepare_keyboard_restore(KeyboardRestoreRequest {
            device_id: device.clone(),
            control_id: "key-0".into(),
            layer: KeyboardLayer::Fn,
            observation: observed(),
            generation: 7,
            identity: "confirmed-a".into(),
            expected_revision: core.revision(),
        })
        .unwrap();
    assert!(!cancel.requires_write);
    assert_eq!(
        core.snapshot()
            .configuration
            .devices
            .iter()
            .find(|d| d.id == device)
            .unwrap()
            .keyboard_aliases[0]
            .phase,
        KeyboardAliasPhase::Released
    );
    assert_eq!(
        assign(&mut core, &device, 1, KeyboardLayer::Main, observed()).alias,
        13
    );
}

#[test]
fn migration_preserves_fn_launch_unknown_json_and_backup_and_rejects_old_writer() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("v2.sqlite3");
    let mut seed = Core::memory().unwrap();
    let device = keyboard(&mut seed);
    let mut config = seed.snapshot().configuration;
    config.format_version = 2;
    let d = config.devices.iter_mut().find(|d| d.id == device).unwrap();
    let mut launch = Binding::new(ActionKind::Program, "Открыть программу");
    launch.plan = Some(Plan::Launch {
        program: "notepad.exe".into(),
        args: vec![],
    });
    let binding_id = launch.id.clone();
    d.profile.fn_bindings.insert(
        "key-0".into(),
        FnBinding {
            trigger: 13,
            binding: launch,
        },
    );
    let revision = config.revision as i64;
    let mut raw = serde_json::to_value(config).unwrap();
    raw["reserved"] = json!({"keep":true});
    let index = raw["devices"]
        .as_array()
        .unwrap()
        .iter()
        .position(|d| d["id"] == device)
        .unwrap();
    raw["devices"][index]["profile"]["future_options"] = json!({"keep":[1,2,3]});
    let old = rusqlite::Connection::open(&path).unwrap();
    old.execute_batch("CREATE TABLE configuration(id INTEGER PRIMARY KEY,revision INTEGER,json TEXT);PRAGMA user_version=2;CREATE TRIGGER configuration_v2_guard BEFORE UPDATE ON configuration WHEN json_extract(NEW.json,'$.format_version') IS NOT 2 BEGIN SELECT RAISE(ABORT,'format2'); END;").unwrap();
    old.execute(
        "INSERT INTO configuration VALUES(1,?1,?2)",
        rusqlite::params![revision, serde_json::to_string(&raw).unwrap()],
    )
    .unwrap();
    let mut core = Core::open(&path).unwrap();
    let saved = core
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == device)
        .unwrap();
    assert_eq!(saved.profile.fn_bindings["key-0"].binding.id, binding_id);
    assert!(
        matches!(&saved.profile.fn_bindings["key-0"].binding.plan,Some(Plan::Launch {program,..}) if program=="notepad.exe")
    );
    assert_eq!(
        saved.keyboard_aliases[0].phase,
        KeyboardAliasPhase::LegacyUnverified
    );
    assert!(saved.keyboard_aliases[0].original.is_none());
    assert!(core.fn_routes().is_empty());
    assert!(core.keyboard_alias_routes().is_empty());
    assert!(core.take_provider_outputs().is_empty());
    assert!(
        old.execute(
            "UPDATE configuration SET json=?1",
            [serde_json::to_string(&raw).unwrap()]
        )
        .is_err()
    );
    core.dispatch(
        Command::SetPreferences {
            preferences: Preferences::default(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    let stored: String = old
        .query_row("SELECT json FROM configuration", [], |r| r.get(0))
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["reserved"], raw["reserved"]);
    assert_eq!(
        stored["devices"][index]["profile"]["future_options"],
        raw["devices"][index]["profile"]["future_options"]
    );
    let backups: Vec<_> = std::fs::read_dir(folder.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("pre-v3"))
        .collect();
    assert_eq!(backups.len(), 1);
    let backup = rusqlite::Connection::open(backups[0].path()).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &backup
                .query_row("SELECT json FROM configuration", [], |r| r
                    .get::<_, String>(0))
                .unwrap()
        )
        .unwrap(),
        raw
    );
    assert_eq!(
        old.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
}

#[test]
fn complete_observation_rejects_duplicate_aliases_in_hidden_and_modified_records() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    let mut observation = observed();
    observation.fn_layer[0] = operation.installed;
    assert!(
        core.validate_keyboard_alias_observation(&device, &observation)
            .is_ok()
    );
    observation.main[127] = [2, 1, operation.installed[2], 0];
    assert_eq!(
        core.validate_keyboard_alias_observation(&device, &observation)
            .unwrap_err()
            .code,
        "keyboard_alias_conflict"
    );
    observation.fn_layer[0] = operation.original;
    assert!(
        core.validate_keyboard_alias_observation(&device, &observation)
            .is_err(),
        "An owned alias moved to an unowned slot is still a conflict"
    );
}

#[test]
fn removal_and_rebind_never_retain_an_input_session_and_unresolved_device_cannot_be_forgotten() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    verify(&mut core, &operation);
    core.dispatch(
        Command::RemoveDevice {
            device_id: device.clone(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    assert_eq!(
        core.dispatch(
            Command::ForgetDevice {
                device_id: device.clone(),
                expected_revision: core.revision()
            },
            "test"
        )
        .unwrap_err()
        .code,
        "keyboard_ownership"
    );
    core.dispatch(
        Command::RestoreDevice {
            device_id: device.clone(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    core.connection_changed(&device, 7, ConnectionState::Connected, None);
    assert!(
        core.keyboard_alias_routes().is_empty(),
        "Restoring an instance cannot reuse a pre-removal proof"
    );
    assert!(core.take_provider_outputs().is_empty());
}

#[test]
fn original_readback_can_reattach_uncertain_intent_without_enabling_a_route_or_writing() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    core.fail_keyboard_assignment(&operation.operation_id, "Write outcome unknown")
        .unwrap();
    core.connection_changed(&device, 8, ConnectionState::Connected, None);
    let mut readback = readback(&operation, operation.original);
    readback.generation = 8;
    readback.identity = "explicitly-confirmed-new-instance".into();
    assert_eq!(
        core.reconcile_keyboard_alias_confirmed(readback).unwrap(),
        KeyboardAliasPhase::Uncertain
    );
    assert!(core.keyboard_alias_routes().is_empty());
    assert!(core.take_provider_outputs().is_empty());
}

#[test]
fn malformed_raw_records_are_rejected_and_owned_generic_mapping_changes_cannot_bypass_restoration()
{
    let invalid =
        json!({"main":[[2,0,300,0]],"fn_layer":[],"control_indices":{},"layer_revisions":{}});
    assert!(serde_json::from_value::<KeyboardObservation>(invalid).is_err());
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    verify(&mut core, &operation);
    let revision = core.revision();
    assert_eq!(
        core.dispatch(
            Command::SetProviderSettings {
                device_id: device,
                changes: serde_json::from_value(json!({"mapping":{"key-0":{"fn":{"usage":4}}}}))
                    .unwrap(),
                expected_revision: revision
            },
            "test"
        )
        .unwrap_err()
        .code,
        "keyboard_ownership"
    );
    assert_eq!(core.revision(), revision);
    assert!(core.take_provider_outputs().is_empty());
}

#[test]
fn captured_alias_route_rejects_same_id_nested_plan_edits_but_not_unrelated_preferences() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    verify(&mut core, &operation);
    let saved = core
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == device)
        .unwrap();
    let mut binding = saved.profile.fn_bindings["key-0"].binding.clone();
    binding.kind = ActionKind::Program;
    let mut launch = Binding::new(ActionKind::Program, "Launch");
    launch.plan = Some(Plan::Launch {
        program: "before.exe".into(),
        args: vec![],
    });
    binding.plan = Some(Plan::Sequence {
        steps: vec![Step {
            delay_ms: 0,
            action: launch,
        }],
    });
    core.dispatch(
        Command::SetFnBinding {
            device_id: device.clone(),
            control_id: "key-0".into(),
            trigger: operation.alias,
            configure_hardware: false,
            binding: Some(binding.clone()),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    let captured = core.keyboard_alias_routes().pop().unwrap();
    core.dispatch(
        Command::SetPreferences {
            preferences: Preferences {
                compact: true,
                ..Preferences::default()
            },
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    assert_eq!(
        core.keyboard_alias_routes()[0],
        captured,
        "A preference change must not churn input configuration"
    );
    if let Some(Plan::Sequence { steps }) = &mut binding.plan {
        steps[0].action.plan = Some(Plan::Launch {
            program: "after.exe".into(),
            args: vec![],
        });
    }
    core.dispatch(
        Command::SetFnBinding {
            device_id: device,
            control_id: "key-0".into(),
            trigger: operation.alias,
            configure_hardware: false,
            binding: Some(binding),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    let fresh = core.keyboard_alias_routes().pop().unwrap();
    assert_eq!(captured.binding_id, fresh.binding_id);
    assert_ne!(captured.execution_fingerprint, fresh.execution_fingerprint);
    assert!(!core.activate_keyboard_alias(&captured, 7).unwrap());
    assert!(core.take_effects().is_empty());
    assert!(core.activate_keyboard_alias(&fresh, 7).unwrap());
    core.advance(0);
    let effects = core.take_effects();
    assert!(matches!(effects[0].plan, Plan::Launch { ref program, .. } if program == "after.exe"));
}

#[test]
fn captured_alias_route_rejects_same_id_plugin_settings_edits_without_exposing_settings() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    verify(&mut core, &operation);
    let saved = core
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == device)
        .unwrap();
    let mut binding = saved.profile.fn_bindings["key-0"].binding.clone();
    binding.kind = ActionKind::Plugin;
    binding.plugin = Some(PluginAction {
        package: "example.test".into(),
        action: "example.test.run".into(),
        settings: json!({"token":"old-secret"}),
        controller: Some(Controller::Keypad),
    });
    let binding_id = binding.id.clone();
    core.dispatch(
        Command::SetFnBinding {
            device_id: device.clone(),
            control_id: "key-0".into(),
            trigger: operation.alias,
            configure_hardware: false,
            binding: Some(binding),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    let captured = core.keyboard_alias_routes().pop().unwrap();
    core.dispatch(
        Command::PluginSettings {
            device_id: device,
            binding_id,
            settings: json!({"token":"new-secret"}),
            expected_revision: core.revision(),
        },
        "plugin",
    )
    .unwrap();
    let fresh = core.keyboard_alias_routes().pop().unwrap();
    assert_eq!(captured.binding_id, fresh.binding_id);
    assert_ne!(captured.execution_fingerprint, fresh.execution_fingerprint);
    let serialized = serde_json::to_string(&fresh).unwrap();
    assert!(!serialized.contains("new-secret") && !serialized.contains("token"));
    assert!(!core.activate_keyboard_alias(&captured, 7).unwrap());
    assert!(core.take_plugin_inputs().is_empty());
    assert!(core.activate_keyboard_alias(&fresh, 7).unwrap());
    assert_eq!(core.take_plugin_inputs().len(), 2);
}

#[test]
fn generic_mapping_edits_cannot_duplicate_reserved_alias_or_install_a_pending_owner() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    let original_host_id = core
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == device)
        .unwrap()
        .profile
        .fn_bindings["key-0"]
        .binding
        .id
        .clone();
    let changes = [
        json!({"mapping":{"key-0":{"fn":{"usage":104}}}}),
        json!({"mapping":{"key-1":{"main":{"usage":104}}}}),
        json!({"mapping":{"key-0":{"main":{"usage":104}}}}),
    ];
    for changes in changes {
        let revision = core.revision();
        assert_eq!(
            core.dispatch(
                Command::SetProviderSettings {
                    device_id: device.clone(),
                    changes: serde_json::from_value(changes).unwrap(),
                    expected_revision: revision
                },
                "editor"
            )
            .unwrap_err()
            .code,
            "keyboard_ownership"
        );
        assert_eq!(core.revision(), revision);
    }
    verify(&mut core, &operation);
    let revision = core.revision();
    assert_eq!(
        core.dispatch(
            Command::SetProviderSettings {
                device_id: device.clone(),
                changes: serde_json::from_value(json!({"mapping":{"key-1":{"fn":{"usage":104}}}}))
                    .unwrap(),
                expected_revision: revision
            },
            "editor"
        )
        .unwrap_err()
        .code,
        "keyboard_ownership"
    );
    let saved = core
        .snapshot()
        .configuration
        .devices
        .into_iter()
        .find(|d| d.id == device)
        .unwrap();
    assert_eq!(
        saved.profile.fn_bindings["key-0"].binding.id,
        original_host_id
    );
    assert_eq!(
        saved.keyboard_aliases[0].phase,
        KeyboardAliasPhase::Verified
    );
    assert!(core.take_provider_outputs().is_empty());
}

#[test]
fn verified_owned_noops_never_reach_generic_hardware_output_and_other_sources_still_edit() {
    let mut core = Core::memory().unwrap();
    let device = keyboard(&mut core);
    let operation = assign(&mut core, &device, 0, KeyboardLayer::Fn, observed());
    verify(&mut core, &operation);
    let revision = core.revision();
    core.dispatch(
        Command::SetProviderSettings {
            device_id: device.clone(),
            changes: serde_json::from_value(json!({"mapping":{"key-0":{"fn":{"usage":104}}}}))
                .unwrap(),
            expected_revision: revision,
        },
        "editor",
    )
    .unwrap();
    assert_eq!(
        core.revision(),
        revision,
        "An entirely owned no-op must not publish a new revision"
    );
    assert!(core.take_provider_outputs().is_empty());
    core.dispatch(Command::SetProviderSettings {device_id:device,changes:serde_json::from_value(json!({"mapping":{"key-0":{"fn":{"usage":104},"main":{"usage":5}},"key-1":{"fn":{"usage":6}}}})).unwrap(),expected_revision:revision},"editor").unwrap();
    let outputs = core.take_provider_outputs();
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        outputs[0].changes,
        serde_json::from_value(
            json!({"mapping":{"key-0":{"main":{"usage":5}},"key-1":{"fn":{"usage":6}}}})
        )
        .unwrap()
    );
    assert_eq!(core.revision(), revision + 1);
}
