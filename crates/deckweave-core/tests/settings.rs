use deckweave_core::*;
#[test]
fn preferences_and_device_defaults_save_atomically_and_survive_restart() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.sqlite3");
    let mut core = Core::open(&path).unwrap();
    let id = core.snapshot().configuration.devices[0].id.clone();
    let profile = core.snapshot().configuration.devices[0].profile.id.clone();
    let revision = core.revision();
    core.dispatch(
        Command::SetPreferences {
            preferences: Preferences {
                disabled_plugins: vec!["example.disabled".into()],
                last_device: Some(id.clone()),
                smart_profiles: false,
                preview_scale: 125,
                compact: true,
                library: LibraryPreferences {
                    hidden: vec!["plugin:example.test".into()],
                    order: vec!["plugin:example.test".into()],
                    show_development: true,
                },
            },
            expected_revision: revision,
        },
        "settings",
    )
    .unwrap();
    assert!(
        core.dispatch(
            Command::SetPreferences {
                preferences: Preferences::default(),
                expected_revision: revision
            },
            "stale"
        )
        .is_err()
    );
    let revision = core.revision();
    assert!(
        core.dispatch(
            Command::SetPreferences {
                preferences: Preferences {
                    preview_scale: 0,
                    ..Default::default()
                },
                expected_revision: revision
            },
            "invalid"
        )
        .is_err()
    );
    assert!(
        core.dispatch(
            Command::DevicePreferences {
                device_id: id.clone(),
                default_profile: Some("missing".into()),
                expected_revision: revision
            },
            "invalid"
        )
        .is_err()
    );
    assert_eq!(core.revision(), revision);
    core.dispatch(
        Command::DevicePreferences {
            device_id: id,
            default_profile: Some(profile.clone()),
            expected_revision: revision,
        },
        "settings",
    )
    .unwrap();
    drop(core);
    let core = Core::open(&path).unwrap();
    let snapshot = core.snapshot();
    assert!(snapshot.configuration.preferences.compact);
    assert_eq!(
        snapshot.configuration.preferences.library.hidden,
        ["plugin:example.test"]
    );
    assert!(snapshot.configuration.preferences.library.show_development);
    assert_eq!(snapshot.configuration.preferences.preview_scale, 125);
    assert!(!snapshot.configuration.preferences.smart_profiles);
    assert_eq!(
        snapshot.configuration.preferences.disabled_plugins,
        ["example.disabled"]
    );
    assert_eq!(
        snapshot.configuration.preferences.last_device.as_deref(),
        Some(snapshot.configuration.devices[0].id.as_str())
    );
    assert_eq!(
        snapshot.configuration.devices[0].default_profile.as_ref(),
        Some(&profile)
    );
}

#[test]
fn legacy_plugin_opt_out_migrates_once_without_a_special_serialized_preference() {
    let blocked: Preferences =
        serde_json::from_value(serde_json::json!({"allow_weather_geolocation":false})).unwrap();
    assert_eq!(blocked.disabled_plugins, ["com.elgato.weather"]);
    let stored = serde_json::to_value(&blocked).unwrap();
    assert!(stored.get("allow_weather_geolocation").is_none());
    let reloaded: Preferences = serde_json::from_value(stored).unwrap();
    assert_eq!(reloaded.disabled_plugins, blocked.disabled_plugins);
    for value in [
        serde_json::json!({}),
        serde_json::json!({"allow_weather_geolocation":true}),
        serde_json::json!({"allow_weather_geolocation":false,"disabled_plugins":[]}),
    ] {
        assert!(
            serde_json::from_value::<Preferences>(value)
                .unwrap()
                .disabled_plugins
                .is_empty()
        );
    }
    let generic:Preferences=serde_json::from_value(serde_json::json!({"allow_weather_geolocation":false,"disabled_plugins":["dev.example.test"]})).unwrap();
    assert_eq!(generic.disabled_plugins, ["dev.example.test"]);
}
#[test]
fn global_smart_profile_switch_and_settings_window_pause_do_not_override_manual_profile() {
    let mut core = Core::memory().unwrap();
    let d = core.snapshot().configuration.devices[0].clone();
    core.dispatch(
        Command::ProfileApplication {
            device_id: d.id.clone(),
            application: Some("obs64.exe".into()),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    core.dispatch(
        Command::AddProfile {
            device_id: d.id.clone(),
            name: "Manual".into(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    let manual = core.snapshot().configuration.devices[0].profile.id.clone();
    core.dispatch(
        Command::SetPreferences {
            preferences: Preferences {
                smart_profiles: false,
                ..Default::default()
            },
            expected_revision: core.revision(),
        },
        "settings",
    )
    .unwrap();
    assert!(!core.foreground_application(Some("obs64.exe")).unwrap());
    assert_eq!(core.snapshot().configuration.devices[0].profile.id, manual);
    core.dispatch(
        Command::SetPreferences {
            preferences: Preferences::default(),
            expected_revision: core.revision(),
        },
        "settings",
    )
    .unwrap();
    core.editor_opened("main");
    assert!(!core.foreground_application(Some("obs64.exe")).unwrap());
    core.editor_closed("main");
    assert!(core.foreground_application(Some("obs64.exe")).unwrap());
    assert_eq!(
        core.snapshot().configuration.devices[0].profile.id,
        d.profile.id
    );
}
