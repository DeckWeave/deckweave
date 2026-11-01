use deckweave_core::*;
use serde_json::json;
fn device(core: &Core) -> Device {
    core.snapshot().configuration.devices[0].clone()
}
fn layout(core: &mut Core, regions: Vec<PanelRegion>) -> Result<Snapshot> {
    let d = device(core);
    core.dispatch(
        Command::SetPanelLayout {
            device_id: d.id,
            page_id: d.profile.active_page_id,
            layout: Some(PanelLayout {
                regions,
                swipe_pages: true,
            }),
            expected_revision: core.revision(),
        },
        "test",
    )
}
fn region(rect: [f64; 4], dials: &[&str]) -> PanelRegion {
    PanelRegion {
        id: new_id(),
        name: "Widget".into(),
        rect,
        dials: dials.iter().map(|s| (*s).into()).collect(),
    }
}
fn assign(core: &mut Core, id: &str, controller: Controller) -> Binding {
    let d = device(core);
    let s = core
        .dispatch(
            Command::ConfigurePlugin {
                device_id: d.id,
                page_id: d.profile.active_page_id,
                control_id: id.into(),
                plugin: PluginAction {
                    package: "fixture".into(),
                    action: "fixture.action".into(),
                    controller: Some(controller),
                    settings: json!({}),
                },
                label: "Widget".into(),
                expected_revision: core.revision(),
            },
            "test",
        )
        .unwrap();
    s.configuration.devices[0].profile.pages[0].bindings[id].clone()
}
fn input(core: &mut Core, id: &str, event: InputEvent) {
    let d = device(core);
    core.dispatch(
        Command::Input {
            device_id: d.id,
            control_id: id.into(),
            event,
        },
        "test",
    )
    .unwrap();
}
fn pointer(phase: PointerPhase, x: f64, y: Option<f64>) -> InputEvent {
    InputEvent::Pointer { phase, x, y }
}

#[test]
fn layout_is_atomic_normalized_and_dial_aliases_share_one_context() {
    let mut core = Core::memory().unwrap();
    let region = region([0., 0., 1., 1.], &["dial-1"]);
    let id = region.control_id();
    layout(&mut core, vec![region.clone()]).unwrap();
    let b = assign(&mut core, &id, Controller::Encoder);
    let s = core.snapshot();
    let page = &s.configuration.devices[0].profile.pages[0];
    assert_eq!(page.bindings["dial-1"].id, b.id);
    let g = &s.geometries[&device(&core).id];
    assert!((g.surface[2] / g.surface[3] - 800.0 / 128.0).abs() < 1e-9);
    input(&mut core, "dial-1", InputEvent::Rotate { ticks: -3 });
    let events = core.take_plugin_inputs();
    assert_eq!(events[0].binding.id, b.id);
    assert_eq!(events[0].ticks, -3);
    input(
        &mut core,
        &id,
        pointer(PointerPhase::Down, 0.75, Some(0.25)),
    );
    core.advance(700);
    input(&mut core, &id, pointer(PointerPhase::Up, 0.75, Some(0.25)));
    let events = core.take_plugin_inputs();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event, "touchTap");
    assert!(events[0].hold);
    assert_eq!(events[0].position.unwrap().y, Some(0.25));
    let revision = core.revision();
    assert!(layout(&mut core, vec![region.clone(), region]).is_err());
    assert_eq!(core.revision(), revision);
}
#[test]
fn missing_y_does_not_guess_encoder_tap_but_keypad_cancel_remains_silent() {
    let mut core = Core::memory().unwrap();
    let r = region([0., 0., 1., 1.], &[]);
    let id = r.control_id();
    layout(&mut core, vec![r]).unwrap();
    let b = assign(&mut core, &id, Controller::Encoder);
    input(&mut core, &id, pointer(PointerPhase::Down, 0.4, None));
    input(&mut core, &id, pointer(PointerPhase::Up, 0.4, None));
    assert!(core.take_plugin_inputs().is_empty());
    assert!(
        core.snapshot()
            .feedback
            .iter()
            .find(|f| f.binding_id == b.id)
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("Y")
    );
    assign(&mut core, &id, Controller::Keypad);
    input(&mut core, &id, pointer(PointerPhase::Down, 0.4, None));
    input(&mut core, &id, InputEvent::Cancel);
    assert!(core.take_plugin_inputs().is_empty());
    input(&mut core, &id, pointer(PointerPhase::Down, 0.4, None));
    input(&mut core, &id, pointer(PointerPhase::Up, 0.4, None));
    assert_eq!(
        core.take_plugin_inputs()
            .iter()
            .map(|i| i.event.as_str())
            .collect::<Vec<_>>(),
        ["keyDown", "keyUp"]
    );
}
#[test]
fn native_widget_owns_drag_and_swipe_and_layout_edit_cancels_contact() {
    let mut core = Core::memory().unwrap();
    let r = region([0.25, 0., 0.75, 1.], &["dial-1", "dial-2"]);
    let id = r.control_id();
    layout(&mut core, vec![r.clone()]).unwrap();
    assign(&mut core, &id, Controller::Panel);
    input(&mut core, &id, pointer(PointerPhase::Down, 0.625, None));
    input(&mut core, &id, pointer(PointerPhase::Move, 1., Some(0.9)));
    let page = device(&core).profile.active_page_id;
    input(&mut core, &id, InputEvent::Swipe { right: false });
    assert_eq!(device(&core).profile.active_page_id, page);
    let events = core.take_plugin_inputs();
    assert_eq!(
        events.iter().map(|i| i.event.as_str()).collect::<Vec<_>>(),
        ["panelDown", "panelMove", "panelSwipe"]
    );
    assert_eq!(events[0].position.unwrap().x, 0.5);
    assert_eq!(events[0].canvas, [600, 128]);
    layout(&mut core, vec![r]).unwrap();
    let events = core.take_plugin_inputs();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event, "panelCancel");
    assert!(core.snapshot().pressed.is_empty());
    input(&mut core, "dial-2", InputEvent::Click);
    assert_eq!(
        core.take_plugin_inputs()
            .iter()
            .map(|i| i.event.as_str())
            .collect::<Vec<_>>(),
        ["dialDown", "dialUp"]
    );
}
#[test]
fn swipe_cancels_keypad_tap_and_changes_only_root_page() {
    let mut core = Core::memory().unwrap();
    let r = region([0., 0., 1., 1.], &[]);
    let id = r.control_id();
    layout(&mut core, vec![r]).unwrap();
    assign(&mut core, &id, Controller::Keypad);
    let d = device(&core);
    core.dispatch(
        Command::AddPage {
            device_id: d.id,
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    // AddPage selects the new page; return to the panel page first.
    let d = device(&core);
    let first = d.profile.pages[0].id.clone();
    core.dispatch(
        Command::SelectPage {
            device_id: d.id,
            page_id: first.clone(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    input(&mut core, &id, pointer(PointerPhase::Down, 0.4, Some(0.5)));
    input(&mut core, &id, InputEvent::Swipe { right: false });
    input(&mut core, &id, pointer(PointerPhase::Up, 0.4, Some(0.5)));
    assert!(core.take_plugin_inputs().is_empty());
    assert_ne!(device(&core).profile.active_page_id, first);
    assert!(core.snapshot().pressed.is_empty());
}

#[test]
fn dial_directions_push_and_touch_execute_independent_actions() {
    let mut core = Core::memory().unwrap();
    let r = region([0., 0., 1., 1.], &["dial-1"]);
    let id = r.control_id();
    layout(&mut core, vec![r]).unwrap();
    let children: [Binding; 4] =
        ["Left", "Right", "Press", "Tap"].map(|name| Binding::new(ActionKind::Counter, name));
    let [left, right, press, tap] = children.clone().map(|b| Some(Box::new(b)));
    let d = device(&core);
    core.dispatch(
        Command::ConfigureProgram {
            device_id: d.id,
            page_id: d.profile.active_page_id,
            control_id: id.clone(),
            plan: Plan::DialBindings {
                left,
                right,
                press,
                tap,
            },
            label: "Dial".into(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    input(&mut core, "dial-1", InputEvent::Rotate { ticks: -2 });
    input(&mut core, "dial-1", InputEvent::Rotate { ticks: 3 });
    input(&mut core, "dial-1", InputEvent::Click);
    input(&mut core, &id, InputEvent::Click);
    let s = core.snapshot();
    let values: Vec<_> = children
        .iter()
        .map(|b| {
            s.feedback
                .iter()
                .find(|f| f.binding_id == b.id)
                .unwrap()
                .value
        })
        .collect();
    assert_eq!(values, [2, 3, 1, 1]);
}

#[test]
fn stack_tap_executes_selected_push_while_wheel_edges_select_without_execution() {
    for wheel in [false, true] {
        let mut core = Core::memory().unwrap();
        let r = region([0., 0., 1., 1.], &["dial-1"]);
        let id = r.control_id();
        layout(&mut core, vec![r]).unwrap();
        let actions = vec![
            Binding::new(ActionKind::Counter, "A"),
            Binding::new(ActionKind::Counter, "B"),
        ];
        let child = actions[1].id.clone();
        let d = device(&core);
        core.dispatch(
            Command::ConfigureProgram {
                device_id: d.id,
                page_id: d.profile.active_page_id,
                control_id: id.clone(),
                plan: if wheel {
                    Plan::ActionWheel { actions }
                } else {
                    Plan::DialStack { actions }
                },
                label: "Select".into(),
                expected_revision: core.revision(),
            },
            "test",
        )
        .unwrap();
        if wheel {
            input(&mut core, &id, pointer(PointerPhase::Down, 0.9, Some(0.5)));
            input(&mut core, &id, pointer(PointerPhase::Up, 0.9, Some(0.5)));
        } else {
            input(&mut core, "dial-1", InputEvent::Click);
        }
        assert!(
            core.snapshot()
                .feedback
                .iter()
                .all(|f| f.binding_id != child)
        );
        input(&mut core, &id, InputEvent::Click);
        assert_eq!(
            core.snapshot()
                .feedback
                .iter()
                .find(|f| f.binding_id == child)
                .unwrap()
                .value,
            1
        );
    }
}

#[test]
fn new_pages_pair_dials_and_regions_without_duplicating_persistent_bindings() {
    let mut core = Core::memory().unwrap();
    let d = device(&core);
    core.dispatch(
        Command::AddPage {
            device_id: d.id,
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    let d = device(&core);
    let p = d
        .profile
        .pages
        .iter()
        .find(|p| p.id == d.profile.active_page_id)
        .unwrap();
    let region = p.panel.as_ref().unwrap().regions[0].control_id();
    core.dispatch(
        Command::ConfigurePlugin {
            device_id: d.id.clone(),
            page_id: p.id.clone(),
            control_id: "dial-1".into(),
            plugin: PluginAction {
                package: "fixture".into(),
                action: "fixture.action".into(),
                controller: Some(Controller::Encoder),
                settings: json!({}),
            },
            label: "Encoder".into(),
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
    let s = core.snapshot();
    let p = s.configuration.devices[0]
        .profile
        .pages
        .iter()
        .find(|p| p.id == d.profile.active_page_id)
        .unwrap();
    assert_eq!(p.bindings["dial-1"].id, p.bindings[&region].id);
    input(&mut core, &region, InputEvent::Click);
    assert_eq!(core.take_plugin_inputs()[0].event, "touchTap");
    // A further transactional edit validates the underlying configuration, which has only one binding.
    core.dispatch(
        Command::SetLocale {
            locale: Locale::En,
            expected_revision: core.revision(),
        },
        "test",
    )
    .unwrap();
}
