use super::*;

#[derive(Clone, Debug, Serialize)]
pub struct Effect {
    pub device_id: String,
    pub binding_id: String,
    pub plan: Plan,
}
struct Task {
    device: String,
    control: String,
    owner: String,
    binding: Binding,
    due: u64,
    delta: i64,
}
struct Pending {
    device: String,
    control: String,
    owner: String,
    action: Option<Binding>,
    due: u64,
    source: String,
    waiting: bool,
}
#[derive(Default)]
pub(crate) struct Runtime {
    pub now: u64,
    tasks: Vec<Task>,
    singles: Vec<Pending>,
    effects: VecDeque<Effect>,
}
impl Runtime {
    pub fn cancel_panel_gestures(&mut self, device: &str) {
        self.singles.retain(|p| {
            p.device != device
                || !(p.control.starts_with("touch-") || p.control.starts_with("panel-"))
        });
    }
    pub fn retained(&self) -> Vec<RetainedAction> {
        self.tasks
            .iter()
            .map(|t| RetainedAction {
                device_id: t.device.clone(),
                control_id: t.control.clone(),
                binding_id: t.owner.clone(),
            })
            .chain(self.singles.iter().map(|p| RetainedAction {
                device_id: p.device.clone(),
                control_id: p.control.clone(),
                binding_id: p.owner.clone(),
            }))
            .collect()
    }
    pub fn cancel_source(&mut self, source: &str) {
        self.singles.retain(|p| p.source != source);
    }
    pub fn cancel_gesture(&mut self, device: &str, control: &str) {
        self.singles
            .retain(|p| p.device != device || p.control != control);
    }
    pub fn down(&mut self, device: &str, id: &str) {
        for p in &mut self.singles {
            if p.device == device && p.owner == id && p.due >= self.now {
                p.waiting = true;
            }
        }
    }
    pub fn cancel(&mut self, device: &str) {
        self.tasks.retain(|t| t.device != device);
        self.singles.retain(|t| t.device != device);
        self.effects.retain(|t| t.device_id != device);
    }
}

pub(crate) fn select_profile(device: &mut Device, id: &str) -> Result<()> {
    if device.profile.id == id {
        return Ok(());
    }
    let index = device
        .profiles
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| Error::new("not_found", "Profile not found"))?;
    std::mem::swap(&mut device.profile, &mut device.profiles[index]);
    Ok(())
}
pub(crate) fn bindings(device: &Device) -> Vec<&Binding> {
    fn walk<'a>(b: &'a Binding, out: &mut Vec<&'a Binding>) {
        out.push(b);
        for child in b.children() {
            walk(child, out);
        }
    }
    let mut result = vec![];
    for binding in device.pinned.values().chain(
        std::iter::once(&device.profile)
            .chain(&device.profiles)
            .flat_map(|p| p.pages.iter())
            .flat_map(|p| p.bindings.values()),
    ) {
        walk(binding, &mut result);
    }
    result
}
pub(crate) fn find_mut<'a>(device: &'a mut Device, id: &str) -> Option<&'a mut Binding> {
    fn walk<'a>(b: &'a mut Binding, id: &str) -> Option<&'a mut Binding> {
        if b.id == id {
            return Some(b);
        }
        match b.plan.as_mut() {
            Some(Plan::DialBindings {
                left,
                right,
                press,
                tap,
            }) => [left, right, press, tap]
                .into_iter()
                .filter_map(|b| b.as_deref_mut())
                .find_map(|b| walk(b, id)),
            Some(Plan::Sequence { steps }) => {
                steps.iter_mut().find_map(|s| walk(&mut s.action, id))
            }
            Some(Plan::Switch { states }) => states.iter_mut().find_map(|b| walk(b, id)),
            Some(Plan::DialStack { actions } | Plan::ActionWheel { actions }) => {
                actions.iter_mut().find_map(|b| walk(b, id))
            }
            Some(Plan::KeyLogic {
                single,
                double,
                hold,
                ..
            }) => [single, double, hold]
                .into_iter()
                .filter_map(|b| b.as_deref_mut())
                .find_map(|b| walk(b, id)),
            _ => None,
        }
    }
    device
        .pinned
        .values_mut()
        .chain(
            std::iter::once(&mut device.profile)
                .chain(&mut device.profiles)
                .flat_map(|p| p.pages.iter_mut())
                .flat_map(|p| p.bindings.values_mut()),
        )
        .find_map(|b| walk(b, id))
}

impl Core {
    fn run_gesture(
        &mut self,
        device: &str,
        control: &str,
        binding: &Binding,
        owner: &str,
    ) -> Result<()> {
        self.run_binding(device, control, binding, 1, false, owner)?;
        let f = self.feedback_entry(device, owner);
        f.value = f.value.saturating_add(1);
        f.error = None;
        f.title = Some(binding.label.clone());
        f.display = Some(binding.label.clone());
        self.close_folder_after_action(device, owner)?;
        Ok(())
    }
    pub fn take_effects(&mut self) -> Vec<Effect> {
        self.actions.effects.drain(..).collect()
    }
    pub fn action_error(&mut self, device: &str, binding: &str, error: &str) {
        self.plugin_inputs
            .retain(|i| i.device_id != device || i.owner_id != binding);
        self.actions
            .effects
            .retain(|e| e.device_id != device || e.binding_id != binding);
        self.actions
            .tasks
            .retain(|t| !(t.device == device && t.owner == binding));
        self.feedback_entry(device, binding).error = Some(error.chars().take(512).collect());
        self.sequence += 1;
    }
    fn feedback_entry(&mut self, device: &str, id: &str) -> &mut Feedback {
        self.feedback
            .entry((device.into(), id.into()))
            .or_insert(Feedback {
                device_id: device.into(),
                binding_id: id.into(),
                value: 0,
                active: false,
                title: None,
                image: None,
                error: None,
                display: None,
                running: false,
            })
    }
    fn binding_exists(&self, device: &str, id: &str) -> bool {
        self.config
            .devices
            .iter()
            .find(|d| d.id == device)
            .is_some_and(|d| bindings(d).iter().any(|b| b.id == id))
    }
    /// Monotonic milliseconds supplied by the desktop timer, or an explicit fake clock in tests.
    pub fn advance(&mut self, now: u64) -> bool {
        if now < self.actions.now {
            return false;
        }
        self.actions.now = now;
        let sequence = self.sequence;
        let held: Vec<_> = self
            .held
            .iter()
            .filter_map(|((device, control), held)| {
                let b = held.binding.as_ref()?;
                if let Some(Plan::KeyLogic {
                    hold: Some(action),
                    hold_ms,
                    ..
                }) = &b.plan
                    && !held.hold_fired
                    && now.saturating_sub(held.started) >= *hold_ms
                {
                    Some((
                        device.clone(),
                        control.clone(),
                        b.id.clone(),
                        action.as_ref().clone(),
                    ))
                } else {
                    None
                }
            })
            .collect();
        for (device, control, owner, action) in held {
            if let Some(h) = self.held.get_mut(&(device.clone(), control.clone())) {
                h.hold_fired = true;
            }
            self.actions
                .singles
                .retain(|p| !(p.device == device && p.owner == owner));
            if self.binding_exists(&device, &owner)
                && let Err(e) = self.run_gesture(&device, &control, &action, &owner)
            {
                self.action_error(&device, &owner, &e.message);
            }
            self.sequence += 1;
        }
        let mut ready = vec![];
        let mut pending = vec![];
        for single in self.actions.singles.drain(..) {
            if single.due <= now && !single.waiting {
                ready.push(single);
            } else {
                pending.push(single);
            }
        }
        self.actions.singles = pending;
        for single in ready {
            if self.binding_exists(&single.device, &single.owner)
                && let Some(action) = single.action
                && let Err(e) =
                    self.run_gesture(&single.device, &single.control, &action, &single.owner)
            {
                self.action_error(&single.device, &single.owner, &e.message);
            }
            self.sequence += 1;
        }
        // Stable insertion order determines equal-deadline steps. Bound work per tick.
        for _ in 0..256 {
            // An OS effect must report success/failure before the next scheduled step runs.
            // Otherwise a zero-delay counter/plugin step could execute before a failed launch.
            if !self.actions.effects.is_empty() {
                break;
            }
            let index = self
                .actions
                .tasks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.due <= now)
                .min_by_key(|(_, t)| t.due)
                .map(|(i, _)| i);
            let Some(index) = index else {
                break;
            };
            let task = self.actions.tasks.remove(index);
            if self.binding_exists(&task.device, &task.owner) {
                if let Err(e) = self.run_binding(
                    &task.device,
                    &task.control,
                    &task.binding,
                    task.delta,
                    false,
                    &task.owner,
                ) {
                    self.action_error(&task.device, &task.owner, &e.message);
                }
                self.sequence += 1;
            }
        }
        self.sequence != sequence
    }
    pub fn foreground_application(&mut self, application: Option<&str>) -> Result<bool> {
        if !self.config.preferences.smart_profiles
            || !self.editors.is_empty()
            || application.is_none()
        {
            return Ok(false);
        }
        let mut next = self.config.clone();
        let mut changed = false;
        for device in &mut next.devices {
            if !std::iter::once(&device.profile)
                .chain(&device.profiles)
                .any(|p| p.application.is_some())
            {
                continue;
            }
            let target = application
                .and_then(|application| {
                    std::iter::once(&device.profile)
                        .chain(&device.profiles)
                        .find(|p| {
                            p.application.as_ref().is_some_and(|rule| {
                                rule.eq_ignore_ascii_case(application)
                                    || std::path::Path::new(application).file_name().is_some_and(
                                        |name| name.to_string_lossy().eq_ignore_ascii_case(rule),
                                    )
                            })
                        })
                        .map(|p| p.id.clone())
                })
                .or_else(|| device.default_profile.clone());
            if let Some(id) = target
                && id != device.profile.id
            {
                select_profile(device, &id)?;
                changed = true;
            }
        }
        if changed {
            let revision = next.revision;
            next.revision += 1;
            self.commit(next, revision)?;
        }
        Ok(changed)
    }
    pub(crate) fn release_binding(
        &mut self,
        device: &str,
        control: &str,
        held: Held,
    ) -> Result<()> {
        let Some(binding) = held.binding else {
            return Ok(());
        };
        if !self.binding_exists(device, &binding.id) {
            return Ok(());
        }
        if let Some(Plan::KeyLogic {
            single,
            double,
            hold: _,
            double_ms,
            ..
        }) = &binding.plan
        {
            if held.hold_fired {
                return Ok(());
            }
            if double.is_some() {
                if let Some(index) = self.actions.singles.iter().position(|p| {
                    p.device == device
                        && p.owner == binding.id
                        && (p.waiting || p.due >= self.actions.now)
                }) {
                    self.actions.singles.remove(index);
                    if let Some(action) = double {
                        self.run_gesture(device, control, action, &binding.id)?;
                    }
                } else {
                    self.actions.singles.push(Pending {
                        device: device.into(),
                        control: control.into(),
                        owner: binding.id.clone(),
                        action: single.as_deref().cloned(),
                        due: self.actions.now.saturating_add(*double_ms),
                        source: held.source,
                        waiting: false,
                    });
                }
            } else if let Some(action) = single {
                self.run_gesture(device, control, action, &binding.id)?;
            }
            return Ok(());
        }
        self.run_binding(device, control, &binding, 1, false, &binding.id)
    }
    pub(crate) fn run_binding(
        &mut self,
        device: &str,
        control: &str,
        binding: &Binding,
        delta: i64,
        rotation: bool,
        owner: &str,
    ) -> Result<()> {
        self.run_binding_inner(device, control, binding, delta, rotation, owner)?;
        if binding.id == owner
            && !matches!(
                (&binding.plan, rotation),
                (Some(Plan::ActionWheel { .. }), true) | (Some(Plan::DialStack { .. }), false)
            )
        {
            self.close_folder_after_action(device, owner)?;
        }
        Ok(())
    }
    pub(crate) fn close_folder_after_action(&mut self, device: &str, owner: &str) -> Result<()> {
        let parent = self
            .config
            .devices
            .iter()
            .find(|d| d.id == device)
            .and_then(|d| {
                let page = d
                    .profile
                    .pages
                    .iter()
                    .find(|p| p.id == d.profile.active_page_id)?;
                if !page.close_after_action {
                    return None;
                }
                let root = page
                    .bindings
                    .values()
                    .chain(d.pinned.values())
                    .find(|b| b.contains(owner))?;
                if matches!(
                    root.plan,
                    Some(
                        Plan::Page { .. }
                            | Plan::PageCycle { .. }
                            | Plan::Profile { .. }
                            | Plan::Back
                    )
                ) {
                    return None;
                }
                page.parent.clone()
            });
        if let Some(page_id) = parent {
            self.dispatch(
                Command::SelectPage {
                    device_id: device.into(),
                    page_id,
                    expected_revision: self.revision(),
                },
                "folder-auto-close",
            )?;
        }
        Ok(())
    }
    fn run_binding_inner(
        &mut self,
        device: &str,
        control: &str,
        binding: &Binding,
        delta: i64,
        rotation: bool,
        owner: &str,
    ) -> Result<()> {
        if binding.kind == ActionKind::Program {
            let stateful = matches!(
                binding.plan,
                Some(Plan::Switch { .. } | Plan::DialStack { .. } | Plan::ActionWheel { .. })
            );
            let entry = self.feedback_entry(device, &binding.id);
            entry.error = None;
            if !stateful {
                entry.value = entry.value.saturating_add(1);
            }
        }
        match binding.kind {
            ActionKind::Counter => {
                let f = self.feedback_entry(device, &binding.id);
                f.value = f.value.saturating_add(delta);
                f.error = None;
            }
            ActionKind::Toggle => {
                let f = self.feedback_entry(device, &binding.id);
                f.active = !f.active;
                f.error = None;
            }
            ActionKind::Plugin => {
                let dial = binding
                    .plugin
                    .as_ref()
                    .and_then(|p| p.controller)
                    .map_or_else(
                        || control.starts_with("dial-"),
                        |c| c == Controller::Encoder,
                    );
                if rotation {
                    self.plugin_input_owned(
                        device,
                        control,
                        binding,
                        "dialRotate",
                        delta as i32,
                        owner,
                    );
                } else {
                    self.plugin_input_owned(
                        device,
                        control,
                        binding,
                        if dial { "dialDown" } else { "keyDown" },
                        0,
                        owner,
                    );
                    self.plugin_input_owned(
                        device,
                        control,
                        binding,
                        if dial { "dialUp" } else { "keyUp" },
                        0,
                        owner,
                    );
                }
            }
            ActionKind::Program => {
                let plan = binding
                    .plan
                    .as_ref()
                    .ok_or_else(|| Error::new("action", "Missing action plan"))?;
                match plan {
                    Plan::PageCycle { direction } => {
                        let d = self
                            .config
                            .devices
                            .iter()
                            .find(|d| d.id == device)
                            .ok_or_else(|| Error::new("not_found", "Device not found"))?;
                        let mut current = d
                            .profile
                            .pages
                            .iter()
                            .find(|p| p.id == d.profile.active_page_id)
                            .expect("active page");
                        while let Some(parent) = &current.parent {
                            current = d
                                .profile
                                .pages
                                .iter()
                                .find(|p| &p.id == parent)
                                .expect("validated parent");
                        }
                        let roots: Vec<_> = d
                            .profile
                            .pages
                            .iter()
                            .filter(|p| p.parent.is_none())
                            .collect();
                        let index =
                            roots.iter().position(|p| p.id == current.id).unwrap_or(0) as i64;
                        let offset = match direction {
                            PageDirection::Dial if rotation => delta,
                            PageDirection::Dial => 0,
                            PageDirection::Previous => -if rotation { delta.abs() } else { 1 },
                            PageDirection::Next => {
                                if rotation {
                                    delta.abs()
                                } else {
                                    1
                                }
                            }
                        };
                        if offset != 0 {
                            let page_id = roots
                                [(index + offset).rem_euclid(roots.len() as i64) as usize]
                                .id
                                .clone();
                            self.dispatch(
                                Command::SelectPage {
                                    device_id: device.into(),
                                    page_id,
                                    expected_revision: self.revision(),
                                },
                                "action",
                            )?;
                        }
                    }
                    Plan::DialBindings {
                        left, right, press, ..
                    } => {
                        let selected = if rotation {
                            if delta < 0 { left } else { right }
                        } else {
                            press
                        };
                        if let Some(child) = selected {
                            for _ in 0..if rotation {
                                delta.unsigned_abs().min(100)
                            } else {
                                1
                            } {
                                self.run_binding(device, control, child, 1, false, owner)?;
                            }
                        }
                    }
                    Plan::Page { page_id } => {
                        let revision = self.revision();
                        self.dispatch(
                            Command::SelectPage {
                                device_id: device.into(),
                                page_id: page_id.clone(),
                                expected_revision: revision,
                            },
                            "action",
                        )?;
                    }
                    Plan::Profile { profile_id } => {
                        let revision = self.revision();
                        self.dispatch(
                            Command::SelectProfile {
                                device_id: device.into(),
                                profile_id: profile_id.clone(),
                                expected_revision: revision,
                            },
                            "action",
                        )?;
                    }
                    Plan::Back => {
                        let d = self
                            .config
                            .devices
                            .iter()
                            .find(|d| d.id == device)
                            .ok_or_else(|| Error::new("not_found", "Device not found"))?;
                        let parent = d
                            .profile
                            .pages
                            .iter()
                            .find(|p| p.id == d.profile.active_page_id)
                            .and_then(|p| p.parent.clone());
                        if let Some(page_id) = parent {
                            let revision = self.revision();
                            self.dispatch(
                                Command::SelectPage {
                                    device_id: device.into(),
                                    page_id,
                                    expected_revision: revision,
                                },
                                "action",
                            )?;
                        }
                    }
                    Plan::Launch { .. } | Plan::Hotkey { .. } => {
                        if self.actions.effects.len() >= 128 {
                            return Err(Error::new("limit", "System action queue is full"));
                        }
                        self.actions.effects.push_back(Effect {
                            device_id: device.into(),
                            binding_id: owner.into(),
                            plan: plan.clone(),
                        });
                    }
                    Plan::Sequence { steps } => {
                        self.actions
                            .tasks
                            .retain(|t| !(t.device == device && t.owner == owner));
                        if self.actions.tasks.len() + steps.len() > 1024 {
                            return Err(Error::new("limit", "Action scheduler is full"));
                        }
                        let mut due = self.actions.now;
                        for step in steps {
                            due = due.saturating_add(step.delay_ms);
                            self.actions.tasks.push(Task {
                                device: device.into(),
                                control: control.into(),
                                owner: owner.into(),
                                binding: step.action.clone(),
                                due,
                                delta: 1,
                            });
                        }
                    }
                    Plan::Switch { states } => {
                        let index = self.feedback_entry(device, &binding.id).value.max(0) as usize
                            % states.len();
                        self.run_binding(device, control, &states[index], 1, false, owner)?;
                        let next = (index + 1) % states.len();
                        let f = self.feedback_entry(device, &binding.id);
                        f.value = next as i64;
                        f.active = next != 0;
                        f.title = Some(states[next].label.clone());
                    }
                    Plan::DialStack { actions } | Plan::ActionWheel { actions } => {
                        let index = self
                            .feedback_entry(device, &binding.id)
                            .value
                            .rem_euclid(actions.len() as i64)
                            as usize;
                        let cycle = matches!(plan, Plan::DialStack { .. }) && !rotation
                            || matches!(plan, Plan::ActionWheel { .. }) && rotation;
                        let selected = if cycle {
                            (index as i64 + if rotation { delta } else { 1 })
                                .rem_euclid(actions.len() as i64)
                                as usize
                        } else {
                            index
                        };
                        if !cycle {
                            self.run_binding(
                                device,
                                control,
                                &actions[selected],
                                delta,
                                rotation,
                                owner,
                            )?;
                        }
                        let f = self.feedback_entry(device, &binding.id);
                        f.value = selected as i64;
                        f.title = Some(actions[selected].label.clone());
                        f.active = selected != 0;
                    }
                    Plan::KeyLogic { single, .. } => {
                        if let Some(action) = single {
                            self.run_binding(device, control, action, delta, rotation, owner)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Core {
    pub(crate) fn render_feedback(&self) -> Vec<Feedback> {
        fn project(core: &Core, device: &str, binding: &Binding) -> Option<Feedback> {
            let mut value = core
                .feedback
                .get(&(device.into(), binding.id.clone()))
                .cloned();
            if binding.kind != ActionKind::Program {
                return value;
            }
            let mut parent = value.take().unwrap_or(Feedback {
                device_id: device.into(),
                binding_id: binding.id.clone(),
                value: 0,
                active: false,
                title: None,
                image: None,
                error: None,
                display: None,
                running: false,
            });
            parent.running = core
                .actions
                .tasks
                .iter()
                .any(|t| t.device == device && t.owner == binding.id)
                || core
                    .actions
                    .singles
                    .iter()
                    .any(|t| t.device == device && t.owner == binding.id);
            let selected = match &binding.plan {
                Some(Plan::Switch { states }) => {
                    Some(&states[parent.value.max(0) as usize % states.len()])
                }
                Some(Plan::DialStack { actions } | Plan::ActionWheel { actions }) => {
                    Some(&actions[parent.value.rem_euclid(actions.len() as i64) as usize])
                }
                _ => None,
            };
            if let Some(child) = selected {
                let result = project(core, device, child);
                parent.title = Some(
                    result
                        .as_ref()
                        .and_then(|f| f.title.clone())
                        .unwrap_or_else(|| child.label.clone()),
                );
                parent.image = result.as_ref().and_then(|f| f.image.clone());
                parent.error = parent
                    .error
                    .or_else(|| result.as_ref().and_then(|f| f.error.clone()));
                parent.display =
                    result
                        .as_ref()
                        .and_then(|f| f.display.clone())
                        .or_else(|| match child.kind {
                            ActionKind::Counter => {
                                Some(result.as_ref().map_or(0, |f| f.value).to_string())
                            }
                            ActionKind::Toggle => Some(
                                if result.as_ref().is_some_and(|f| f.active) {
                                    "ON"
                                } else {
                                    "OFF"
                                }
                                .into(),
                            ),
                            _ => None,
                        });
            } else {
                parent.display = if parent.running {
                    Some("…".into())
                } else if parent.display.is_some() {
                    parent.display
                } else if parent.value > 0 {
                    Some("✓".into())
                } else {
                    None
                };
            }
            Some(parent)
        }
        let mut output = self.feedback.clone();
        for device in &self.config.devices {
            if let Some(page) = device
                .profile
                .pages
                .iter()
                .find(|p| p.id == device.profile.active_page_id)
            {
                for binding in page.bindings.values().chain(device.pinned.values()) {
                    if let Some(value) = project(self, &device.id, binding) {
                        output.insert((device.id.clone(), binding.id.clone()), value);
                    }
                }
            }
        }
        output.into_values().collect()
    }
}

pub(crate) fn validate_binding(
    binding: &Binding,
    depth: usize,
    id: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<()> {
    validate_binding_inner(binding, depth, false, id)
}
fn validate_binding_inner(
    binding: &Binding,
    depth: usize,
    in_sequence: bool,
    id: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<()> {
    if depth > 8 {
        return Err(Error::new("limit", "Action nesting exceeds 8 levels"));
    }
    id(&binding.id)?;
    if binding.label.chars().count() > 80 {
        return Err(Error::new(
            "validation",
            "Action title exceeds 80 characters",
        ));
    }
    if binding.kind == ActionKind::Plugin {
        let p = binding
            .plugin
            .as_ref()
            .ok_or_else(|| Error::new("plugin", "Plugin reference missing"))?;
        if p.package.is_empty()
            || p.package.len() > 255
            || !p
                .package
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || p.action.is_empty()
            || !p.settings.is_object()
            || p.settings.to_string().len() > 65536
        {
            return Err(Error::new("plugin", "Invalid plugin reference or settings"));
        }
    }
    if binding.kind == ActionKind::Program {
        match binding
            .plan
            .as_ref()
            .ok_or_else(|| Error::new("action", "Action plan missing"))?
        {
            Plan::Sequence { steps } => {
                if steps.is_empty()
                    || steps.len() > 64
                    || steps.iter().any(|s| s.delay_ms > 3600000)
                {
                    return Err(Error::new(
                        "limit",
                        "Sequence requires 1..64 steps and delays up to 1 hour",
                    ));
                }
                if in_sequence {
                    return Err(Error::new("action", "Nested Multi Action is not supported"));
                }
            }
            Plan::Switch { states }
            | Plan::DialStack { actions: states }
            | Plan::ActionWheel { actions: states } => {
                if states.is_empty() || states.len() > 32 {
                    return Err(Error::new("limit", "Require 1..32 actions"));
                }
            }
            Plan::KeyLogic {
                double_ms,
                hold_ms,
                single,
                double,
                hold,
            } => {
                if depth > 0
                    || *double_ms < 100
                    || *double_ms > 1000
                    || *hold_ms < 200
                    || *hold_ms > 5000
                    || [single, double, hold].iter().all(|b| b.is_none())
                {
                    return Err(Error::new(
                        "action",
                        "Invalid Key Logic thresholds or branches",
                    ));
                }
            }
            Plan::Launch { program, args } => {
                if program.trim().is_empty()
                    || program.len() > 32768
                    || args.len() > 64
                    || args.iter().any(|s| s.contains('\0') || s.len() > 32768)
                {
                    return Err(Error::new("action", "Invalid program or arguments"));
                }
            }
            Plan::Hotkey { keys }
                if keys.is_empty() || keys.len() > 8 || keys.iter().any(|k| !valid_hotkey(k)) =>
            {
                return Err(Error::new(
                    "action",
                    "Use Ctrl/Alt/Shift/Win, A–Z, 0–9, F1–F24, Enter, Space, Tab, Esc, arrows or media keys",
                ));
            }
            _ => {}
        }
    }
    for child in binding.children() {
        validate_binding_inner(
            child,
            depth + 1,
            in_sequence || matches!(binding.plan, Some(Plan::Sequence { .. })),
            id,
        )?;
    }
    Ok(())
}
fn valid_hotkey(key: &str) -> bool {
    matches!(
        key,
        "Ctrl"
            | "Alt"
            | "Shift"
            | "Win"
            | "Enter"
            | "Space"
            | "Tab"
            | "Esc"
            | "Left"
            | "Right"
            | "Up"
            | "Down"
            | "VolumeUp"
            | "VolumeDown"
            | "VolumeMute"
            | "PlayPause"
            | "NextTrack"
            | "PreviousTrack"
    ) || key.len() == 1
        && key
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        || key
            .strip_prefix('F')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n))
}
