use deckweave_core::*;
use std::collections::BTreeMap;
fn setup() -> (Core, String, LightingPreset, LightingPreset) {
    let mut c = Core::memory().unwrap();
    let g = Geometry {
        width: 1.0,
        height: 1.0,
        surface: [0.0, 0.0, 1.0, 1.0],
        surface_pixels: [1, 1],
        controls: vec![Control {
            id: "key-1".into(),
            kind: ControlKind::Key,
            rect: [0.0, 0.0, 1.0, 1.0],
            number: 1,
            pointer: false,
        }],
    };
    let id=c.register_provider_device("example.light","a","A",ProviderDescriptor{geometry:g,definition:serde_json::json!({"settings":[{"id":"brightness","group":"lighting","type":"number","min":0,"max":100,"step":1,"persistence":"volatile"}],"lighting":[{"id":"keys","controls":["key-1"],"settings":["brightness"]}]})}).unwrap();
    let base = LightingPreset {
        id: new_id(),
        name: "Base".into(),
        settings: BTreeMap::from([("brightness".into(), serde_json::json!(20))]),
    };
    let other = LightingPreset {
        id: new_id(),
        name: "Other".into(),
        settings: BTreeMap::from([("brightness".into(), serde_json::json!(80))]),
    };
    for preset in [&base, &other] {
        c.dispatch(
            Command::SaveLightingPreset {
                device_id: id.clone(),
                preset: preset.clone(),
                expected_revision: c.revision(),
            },
            "editor",
        )
        .unwrap();
    }
    assert!(c.take_provider_outputs().is_empty());
    (c, id, base, other)
}
fn select(c: &mut Core, id: &str, preset: &LightingPreset) {
    c.dispatch(
        Command::SelectLightingPreset {
            device_id: id.into(),
            preset_id: preset.id.clone(),
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
}
#[test]
fn manual_lock_rejects_automation_without_replay_and_expiry_restores_latest_base() {
    let (mut c, id, base, other) = setup();
    select(&mut c, &id, &base);
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 20);
    assert_eq!(
        c.request_lighting_preset(&id, &other.id, Some(100), "action")
            .unwrap_err()
            .code,
        "lighting_locked"
    );
    assert!(c.take_provider_outputs().is_empty());
    c.release_lighting_edit(&id, "other window");
    assert!(c.snapshot().lighting[&id].manual_editing);
    c.release_lighting_edit(&id, "editor");
    assert!(c.take_provider_outputs().is_empty());
    let token = c
        .request_lighting_preset(&id, &other.id, Some(100), "action")
        .unwrap();
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 80);
    assert!(c.advance(100));
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 20);
    assert!(!c.stop_lighting_request(&id, token).unwrap());
    assert!(!c.snapshot().lighting[&id].temporary);
}
#[test]
fn superseded_token_cannot_restore_over_newer_request_or_manual_edit() {
    let (mut c, id, base, other) = setup();
    select(&mut c, &id, &base);
    c.take_provider_outputs();
    c.release_lighting_edit(&id, "editor");
    let old = c
        .request_lighting_preset(&id, &other.id, Some(100), "action")
        .unwrap();
    let new = c
        .request_lighting_preset(&id, &base.id, Some(500), "action")
        .unwrap();
    assert!(!c.stop_lighting_request(&id, old).unwrap());
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 20);
    c.dispatch(
        Command::SetProviderSettings {
            device_id: id.clone(),
            changes: BTreeMap::from([("brightness".into(), serde_json::json!(55))]),
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    assert!(!c.stop_lighting_request(&id, new).unwrap());
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 55);
    c.advance(1000);
    assert!(c.take_provider_outputs().is_empty());
}
#[test]
fn explicit_profile_association_requests_once_and_removed_preset_stays_visible() {
    let (mut c, id, base, other) = setup();
    select(&mut c, &id, &base);
    c.take_provider_outputs();
    c.release_lighting_edit(&id, "editor");
    let first = c
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .profile
        .id
        .clone();
    c.dispatch(
        Command::AddProfile {
            device_id: id.clone(),
            name: "Other".into(),
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    assert!(c.take_provider_outputs().is_empty());
    c.dispatch(
        Command::ProfileLightingPreset {
            device_id: id.clone(),
            preset_id: Some(other.id.clone()),
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    assert!(c.take_provider_outputs().is_empty());
    let second = c
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .profile
        .id
        .clone();
    c.dispatch(
        Command::SelectProfile {
            device_id: id.clone(),
            profile_id: first,
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    assert!(c.take_provider_outputs().is_empty());
    c.dispatch(
        Command::SelectProfile {
            device_id: id.clone(),
            profile_id: second,
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 80);
    c.dispatch(
        Command::DeleteLightingPreset {
            device_id: id.clone(),
            preset_id: other.id.clone(),
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    assert_eq!(
        c.snapshot()
            .configuration
            .devices
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .profile
            .lighting_preset
            .as_deref(),
        Some(other.id.as_str())
    );
}
#[test]
fn native_timed_preset_action_stops_on_cancel_and_removal_discards_outputs() {
    let (mut c, id, base, other) = setup();
    select(&mut c, &id, &base);
    c.take_provider_outputs();
    c.release_lighting_edit(&id, "editor");
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
            page_id: page,
            control_id: "key-1".into(),
            label: "Temporary".into(),
            plan: Plan::LightingPreset {
                preset_id: other.id,
                temporary_ms: Some(500),
            },
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
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 80);
    c.dispatch(
        Command::CancelActions {
            device_id: id.clone(),
        },
        "editor",
    )
    .unwrap();
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 20);
    select(&mut c, &id, &base);
    c.dispatch(
        Command::RemoveDevice {
            device_id: id.clone(),
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    assert!(c.take_provider_outputs().is_empty());
    c.dispatch(
        Command::RestoreDevice {
            device_id: id,
            expected_revision: c.revision(),
        },
        "main",
    )
    .unwrap();
    assert!(c.take_provider_outputs().is_empty());
}
#[test]
fn navigating_away_closing_editor_and_unassigning_release_only_their_lighting_ownership() {
    let (mut c, id, base, other) = setup();
    select(&mut c, &id, &base);
    c.take_provider_outputs();
    c.dispatch(
        Command::ReleaseLightingEdit {
            device_id: id.clone(),
        },
        "editor",
    )
    .unwrap();
    assert!(!c.snapshot().lighting[&id].manual_editing);
    assert!(c.take_provider_outputs().is_empty());
    select(&mut c, &id, &base);
    c.take_provider_outputs();
    c.editor_closed("different-window");
    assert!(c.snapshot().lighting[&id].manual_editing);
    c.editor_closed("editor");
    assert!(!c.snapshot().lighting[&id].manual_editing);
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
            control_id: "key-1".into(),
            label: "Temporary".into(),
            plan: Plan::LightingPreset {
                preset_id: other.id,
                temporary_ms: Some(1000),
            },
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
    c.take_provider_outputs();
    c.dispatch(
        Command::ConfigureControl {
            device_id: id.clone(),
            page_id: page,
            control_id: "key-1".into(),
            label: String::new(),
            kind: None,
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    assert!(!c.snapshot().lighting[&id].temporary);
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 20);
    c.advance(2000);
    assert!(c.take_provider_outputs().is_empty());
}
#[test]
fn presets_and_profile_links_survive_without_provider_and_do_not_replay_on_restart() {
    let (mut c, id, base, other) = setup();
    select(&mut c, &id, &base);
    c.take_provider_outputs();
    c.dispatch(
        Command::ProfileLightingPreset {
            device_id: id.clone(),
            preset_id: Some(other.id.clone()),
            expected_revision: c.revision(),
        },
        "editor",
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite3");
    let mut store = deckweave_core::store::Store::open(&path).unwrap();
    store.save(&c.snapshot().configuration, 0).unwrap();
    drop(store);
    let mut reopened = Core::open(&path).unwrap();
    assert!(reopened.take_provider_outputs().is_empty());
    let s = reopened.snapshot();
    let d = s.configuration.devices.iter().find(|d| d.id == id).unwrap();
    assert_eq!(d.lighting_presets.len(), 2);
    assert_eq!(
        d.profile.lighting_preset.as_deref(),
        Some(other.id.as_str())
    );
    assert_eq!(d.provider_settings["brightness"], 20);
    assert_eq!(s.connections[&id].state, ConnectionState::Offline);
    assert!(s.lighting.is_empty());
}
#[test]
fn temporary_onboard_lighting_requires_recovery_and_is_not_enqueued() {
    let (mut original, id, base, other) = setup();
    select(&mut original, &id, &base);
    original.take_provider_outputs();
    let mut configuration = original.snapshot().configuration;
    configuration
        .devices
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap()
        .provider_descriptor
        .as_mut()
        .unwrap()
        .definition["settings"][0]["persistence"] = serde_json::json!("onboard");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("onboard.sqlite3");
    let mut store = deckweave_core::store::Store::open(&path).unwrap();
    store.save(&configuration, 0).unwrap();
    drop(store);
    let mut c = Core::open(&path).unwrap();
    assert_eq!(
        c.request_lighting_preset(&id, &other.id, Some(100), "action")
            .unwrap_err()
            .code,
        "lighting_recovery"
    );
    assert!(c.take_provider_outputs().is_empty());
    c.request_lighting_preset(&id, &other.id, None, "action")
        .unwrap();
    assert_eq!(c.take_provider_outputs()[0].changes["brightness"], 80);
}
