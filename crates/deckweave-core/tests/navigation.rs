use deckweave_core::*;

fn send(core: &mut Core, f: impl FnOnce(u64) -> Command) -> Snapshot {
    core.dispatch(f(core.revision()), "navigation-test")
        .unwrap()
}
fn device(core: &Core) -> Device {
    core.snapshot().configuration.devices[0].clone()
}
fn input(core: &mut Core, id: &str, event: InputEvent) {
    let d = device(core);
    send(core, |_| Command::Input {
        device_id: d.id,
        control_id: id.into(),
        event,
    });
}
fn select(core: &mut Core, page: &str) {
    let d = device(core);
    send(core, |r| Command::SelectPage {
        device_id: d.id,
        page_id: page.into(),
        expected_revision: r,
    });
}
fn counter(core: &mut Core, control: &str) -> String {
    let d = device(core);
    let s = send(core, |r| Command::ConfigureControl {
        device_id: d.id,
        page_id: d.profile.active_page_id,
        control_id: control.into(),
        kind: Some(ActionKind::Counter),
        label: "Counter".into(),
        expected_revision: r,
    });
    let d = &s.configuration.devices[0];
    d.profile
        .pages
        .iter()
        .find(|p| p.id == d.profile.active_page_id)
        .unwrap()
        .bindings[control]
        .id
        .clone()
}
fn folder(core: &mut Core) -> (String, String) {
    let d = device(core);
    let root = d.profile.active_page_id.clone();
    let s = send(core, |r| Command::AddFolder {
        device_id: d.id,
        control_id: "key-3".into(),
        name: "Folder".into(),
        expected_revision: r,
    });
    let id = s.configuration.devices[0]
        .profile
        .pages
        .iter()
        .find(|p| p.parent.as_deref() == Some(&root))
        .unwrap()
        .id
        .clone();
    select(core, &id);
    (root, id)
}
fn auto_close(core: &mut Core, id: &str) {
    let d = device(core);
    send(core, |r| Command::FolderBehavior {
        device_id: d.id,
        page_id: id.into(),
        close_after_action: true,
        expected_revision: r,
    });
}

#[test]
fn independent_pinned_dial_cycles_pages_without_overwriting_fullscreen_widget() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("navigation.sqlite3");
    let mut core = Core::open(&path).unwrap();
    let d = device(&core);
    let root = d.profile.active_page_id.clone();
    let region = PanelRegion {
        id: new_id(),
        name: "Full panel".into(),
        rect: [0., 0., 1., 1.],
        dials: (1..=4).map(|n| format!("dial-{n}")).collect(),
    };
    let control = region.control_id();
    send(&mut core, |r| Command::SetPanelLayout {
        device_id: d.id.clone(),
        page_id: root.clone(),
        layout: Some(PanelLayout {
            regions: vec![region],
            swipe_pages: true,
        }),
        expected_revision: r,
    });
    send(&mut core, |r| Command::ConfigurePlugin {
        device_id: d.id.clone(),
        page_id: root.clone(),
        control_id: control.clone(),
        plugin: PluginAction {
            package: "fixture".into(),
            action: "panel".into(),
            settings: serde_json::json!({}),
            controller: Some(Controller::Panel),
        },
        label: "Widget".into(),
        expected_revision: r,
    });
    send(&mut core, |r| Command::AddPage {
        device_id: d.id.clone(),
        expected_revision: r,
    });
    let second = device(&core).profile.active_page_id;
    select(&mut core, &root);
    send(&mut core, |r| Command::ConfigureProgram {
        device_id: d.id.clone(),
        page_id: root.clone(),
        control_id: "dial-1".into(),
        plan: Plan::PageCycle {
            direction: PageDirection::Dial,
        },
        label: "Pages".into(),
        expected_revision: r,
    });
    send(&mut core, |r| Command::SetPinned {
        device_id: d.id.clone(),
        control_id: "dial-1".into(),
        pinned: true,
        expected_revision: r,
    });
    let s = core.snapshot();
    let page = &s.configuration.devices[0].profile.pages[0];
    assert_eq!(page.bindings[&control].label, "Widget");
    assert!(
        !page.panel.as_ref().unwrap().regions[0]
            .dials
            .contains(&"dial-1".into())
    );
    input(&mut core, "dial-2", InputEvent::Rotate { ticks: 1 });
    assert_eq!(core.take_plugin_inputs()[0].binding.label, "Widget");
    input(&mut core, "dial-1", InputEvent::Rotate { ticks: 1 });
    assert_eq!(device(&core).profile.active_page_id, second);
    input(&mut core, "dial-1", InputEvent::Rotate { ticks: -1 });
    assert_eq!(device(&core).profile.active_page_id, root);
    drop(core);
    let mut core = Core::open(&path).unwrap();
    input(&mut core, "dial-1", InputEvent::Rotate { ticks: -1 });
    assert_eq!(device(&core).profile.active_page_id, second);
}

#[test]
fn folder_plugin_release_is_retained_after_automatic_exit() {
    let mut core = Core::memory().unwrap();
    let (root, folder) = folder(&mut core);
    auto_close(&mut core, &folder);
    let d = device(&core);
    send(&mut core, |r| Command::ConfigurePlugin {
        device_id: d.id,
        page_id: folder.clone(),
        control_id: "key-2".into(),
        label: "Plugin".into(),
        plugin: PluginAction {
            package: "fixture".into(),
            action: "key".into(),
            settings: serde_json::json!({}),
            controller: Some(Controller::Keypad),
        },
        expected_revision: r,
    });
    input(&mut core, "key-2", InputEvent::Down);
    assert_eq!(device(&core).profile.active_page_id, folder);
    input(&mut core, "key-2", InputEvent::Up);
    assert_eq!(device(&core).profile.active_page_id, root);
    assert!(!core.retained_actions().is_empty());
    assert_eq!(
        core.take_plugin_inputs()
            .iter()
            .map(|i| i.event.as_str())
            .collect::<Vec<_>>(),
        ["keyDown", "keyUp"]
    );
}

#[test]
fn folder_waits_for_key_logic_resolution_before_closing() {
    let mut core = Core::memory().unwrap();
    let (root, folder) = folder(&mut core);
    auto_close(&mut core, &folder);
    let d = device(&core);
    send(&mut core, |r| Command::ConfigureProgram {
        device_id: d.id,
        page_id: folder.clone(),
        control_id: "key-2".into(),
        label: "Key Logic".into(),
        plan: Plan::KeyLogic {
            single: Some(Box::new(Binding::new(ActionKind::Counter, "Single"))),
            double: Some(Box::new(Binding::new(ActionKind::Counter, "Double"))),
            hold: None,
            double_ms: 300,
            hold_ms: 700,
        },
        expected_revision: r,
    });
    input(&mut core, "key-2", InputEvent::Down);
    input(&mut core, "key-2", InputEvent::Up);
    assert_eq!(device(&core).profile.active_page_id, folder);
    core.advance(301);
    assert_eq!(device(&core).profile.active_page_id, root);
}

#[test]
fn page_dial_can_pin_and_unpin_on_another_page_with_empty_dial_slots() {
    let mut core = Core::memory().unwrap();
    let d = device(&core);
    let root = d.profile.active_page_id.clone();
    send(&mut core, |r| Command::SetPanelLayout {
        device_id: d.id.clone(),
        page_id: root.clone(),
        layout: Some(PanelLayout::dial_slots(&d.provider_id)),
        expected_revision: r,
    });
    send(&mut core, |r| Command::ConfigureProgram {
        device_id: d.id.clone(),
        page_id: root.clone(),
        control_id: "dial-1".into(),
        plan: Plan::PageCycle {
            direction: PageDirection::Dial,
        },
        label: "Pages".into(),
        expected_revision: r,
    });
    send(&mut core, |r| Command::SetPinned {
        device_id: d.id.clone(),
        control_id: "dial-1".into(),
        pinned: true,
        expected_revision: r,
    });
    send(&mut core, |r| Command::AddPage {
        device_id: d.id.clone(),
        expected_revision: r,
    });
    send(&mut core, |r| Command::SetPinned {
        device_id: d.id.clone(),
        control_id: "dial-1".into(),
        pinned: false,
        expected_revision: r,
    });
    input(&mut core, "dial-1", InputEvent::Rotate { ticks: -1 });
    assert_eq!(device(&core).profile.active_page_id, root);
    assert!(!device(&core).pinned.contains_key("dial-1"));
}

#[test]
fn shared_panel_accepts_builtin_actions_without_controller_error() {
    let mut core = Core::memory().unwrap();
    let d = device(&core);
    let region = PanelRegion {
        id: new_id(),
        name: "Shared".into(),
        rect: [0., 0., 1., 1.],
        dials: vec!["dial-1".into(), "dial-2".into()],
    };
    let control = region.control_id();
    send(&mut core, |r| Command::SetPanelLayout {
        device_id: d.id,
        page_id: d.profile.active_page_id,
        layout: Some(PanelLayout {
            regions: vec![region],
            swipe_pages: true,
        }),
        expected_revision: r,
    });
    let binding = counter(&mut core, &control);
    input(&mut core, "dial-1", InputEvent::Rotate { ticks: 2 });
    input(&mut core, "dial-2", InputEvent::Rotate { ticks: -1 });
    assert_eq!(
        core.snapshot()
            .feedback
            .iter()
            .find(|f| f.binding_id == binding)
            .unwrap()
            .value,
        1
    );
}

#[test]
fn folder_auto_close_is_optional_and_cancel_or_empty_key_never_closes_it() {
    let mut core = Core::memory().unwrap();
    let (root, folder) = folder(&mut core);
    let binding = counter(&mut core, "key-2");
    input(&mut core, "key-2", InputEvent::Down);
    input(&mut core, "key-2", InputEvent::Up);
    assert_eq!(device(&core).profile.active_page_id, folder);
    auto_close(&mut core, &folder);
    input(&mut core, "key-4", InputEvent::Down);
    input(&mut core, "key-4", InputEvent::Up);
    input(&mut core, "key-2", InputEvent::Down);
    input(&mut core, "key-2", InputEvent::Cancel);
    assert_eq!(device(&core).profile.active_page_id, folder);
    input(&mut core, "key-2", InputEvent::Down);
    input(&mut core, "key-2", InputEvent::Up);
    assert_eq!(device(&core).profile.active_page_id, root);
    assert_eq!(
        core.snapshot()
            .feedback
            .iter()
            .find(|f| f.binding_id == binding)
            .unwrap()
            .value,
        2
    );
}

#[test]
fn folder_exit_keeps_delayed_sequence_and_does_not_intercept_folder_navigation() {
    let mut core = Core::memory().unwrap();
    let (root, first) = folder(&mut core);
    auto_close(&mut core, &first);
    let (_, nested) = folder(&mut core);
    select(&mut core, &first);
    input(&mut core, "key-3", InputEvent::Click);
    assert_eq!(device(&core).profile.active_page_id, nested);
    select(&mut core, &first);
    let d = device(&core);
    let child = Binding::new(ActionKind::Counter, "Delayed");
    let child_id = child.id.clone();
    send(&mut core, |r| Command::ConfigureProgram {
        device_id: d.id,
        page_id: first.clone(),
        control_id: "key-2".into(),
        label: "Sequence".into(),
        plan: Plan::Sequence {
            steps: vec![Step {
                delay_ms: 100,
                action: child,
            }],
        },
        expected_revision: r,
    });
    input(&mut core, "key-2", InputEvent::Click);
    assert_eq!(device(&core).profile.active_page_id, root);
    core.advance(100);
    assert_eq!(
        core.snapshot()
            .feedback
            .iter()
            .find(|f| f.binding_id == child_id)
            .unwrap()
            .value,
        1
    );
}
