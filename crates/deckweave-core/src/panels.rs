use super::*;

pub(crate) fn validate_controller(binding: &Binding, kind: ControlKind, root: bool) -> Result<()> {
    if let Some(controller) = binding.plugin.as_ref().and_then(|p| p.controller)
        && ((controller == Controller::Panel && (!root || kind != ControlKind::Touch))
            || (controller == Controller::Encoder && kind == ControlKind::Key)
            || (controller == Controller::Keypad && kind == ControlKind::Dial))
    {
        return Err(Error::new(
            "controller",
            "Action controller is incompatible with this control",
        ));
    }
    let child_kind = if matches!(binding.plan, Some(Plan::DialStack { .. })) {
        ControlKind::Dial
    } else {
        ControlKind::Key
    };
    for child in binding.children() {
        validate_controller(child, child_kind, false)?;
    }
    Ok(())
}

pub(crate) fn relative_point(point: PointerPosition, bounds: [f64; 4]) -> PointerPosition {
    PointerPosition {
        x: ((point.x - bounds[0]) / bounds[2]).clamp(0.0, 1.0),
        y: point
            .y
            .map(|y| ((y - bounds[1]) / bounds[3]).clamp(0.0, 1.0)),
    }
}

impl Core {
    pub(crate) fn plugin_pointer(
        &mut self,
        device: &str,
        control: &str,
        binding: &Binding,
        event: &str,
        pointer: PointerData,
    ) {
        self.plugin_input(device, control, binding, event, 0);
        if let Some(input) = self.plugin_inputs.back_mut()
            && input.binding.id == binding.id
        {
            input.position = Some(pointer.position);
            input.hold = pointer.hold;
            input.canvas = pointer.canvas;
        }
    }

    pub(crate) fn touch_binding(
        &mut self,
        device: &str,
        control: &str,
        binding: &Binding,
        pointer: PointerData,
    ) -> Result<()> {
        if binding.kind == ActionKind::Plugin {
            match controller_for(binding, ControlKind::Touch) {
                Controller::Panel => {
                    self.plugin_pointer(device, control, binding, "panelUp", pointer)
                }
                Controller::Encoder => {
                    if pointer.position.y.is_none() {
                        self.action_error(
                            device,
                            &binding.id,
                            "Exact touch Y is unavailable; Encoder touch was not executed",
                        );
                    } else {
                        self.plugin_pointer(device, control, binding, "touchTap", pointer);
                    }
                }
                Controller::Keypad => {
                    self.plugin_input(device, control, binding, "keyDown", 0);
                    self.plugin_input(device, control, binding, "keyUp", 0);
                }
            }
        } else if let Some(Plan::DialBindings { tap, .. }) = &binding.plan {
            if let Some(child) = tap {
                self.run_binding(device, control, child, 1, false, &binding.id)?;
            }
        } else if let Some(Plan::DialStack { actions }) = &binding.plan {
            let index = self.action_state(device, &binding.id) as usize % actions.len();
            // Push changes the selected stack item; tapping invokes that item's push action.
            self.run_binding(device, control, &actions[index], 1, false, &binding.id)?;
        } else if matches!(binding.plan, Some(Plan::ActionWheel { .. })) {
            let direction = if pointer.position.x < 1.0 / 3.0 {
                -1
            } else {
                1
            };
            let rotate = !(1.0 / 3.0..=2.0 / 3.0).contains(&pointer.position.x);
            self.run_binding(device, control, binding, direction, rotate, &binding.id)?;
        } else {
            self.run_binding(device, control, binding, 1, false, &binding.id)?;
        }
        self.advance(self.actions.now);
        Ok(())
    }

    pub(crate) fn swipe(&mut self, device: &str, control: &str, right: bool) -> Result<()> {
        if let Some(held) = self.held.get(&(device.into(), control.into())).cloned()
            && let Some(binding) = held.binding
            && controller_for(&binding, held.control.kind) == Controller::Panel
        {
            self.plugin_pointer(device, control, &binding, "panelSwipe", held.pointer);
            if let Some(input) = self.plugin_inputs.back_mut() {
                input.ticks = if right { 1 } else { -1 };
            }
            return Ok(());
        }
        self.cancel_holds(|d, _, h| d == device && h.control.kind == ControlKind::Touch);
        self.actions.cancel_panel_gestures(device);
        let d = self
            .config
            .devices
            .iter()
            .find(|d| d.id == device)
            .ok_or_else(|| Error::new("not_found", "Device not found"))?;
        let page = d
            .profile
            .pages
            .iter()
            .find(|p| p.id == d.profile.active_page_id)
            .ok_or_else(|| Error::new("data", "Active page is missing"))?;
        if page.parent.is_some() || page.panel.as_ref().is_some_and(|p| !p.swipe_pages) {
            return Ok(());
        }
        let roots: Vec<_> = d
            .profile
            .pages
            .iter()
            .filter(|p| p.parent.is_none())
            .collect();
        if roots.len() < 2 {
            return Ok(());
        }
        let index = roots.iter().position(|p| p.id == page.id).unwrap_or(0) as i64;
        let next = (index + if right { -1 } else { 1 }).rem_euclid(roots.len() as i64) as usize;
        let page_id = roots[next].id.clone();
        self.dispatch(
            Command::SelectPage {
                device_id: device.into(),
                page_id,
                expected_revision: self.revision(),
            },
            "action",
        )?;
        Ok(())
    }
}
