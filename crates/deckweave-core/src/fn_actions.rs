use super::*;

pub(crate) fn validate(
    device: &Device,
    profile: &Profile,
    id: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<()> {
    if profile.fn_bindings.len() > 12 {
        return Err(Error::new(
            "fn",
            "At most twelve Fn shortcuts are supported",
        ));
    }
    let mut triggers = BTreeSet::new();
    for (control, shortcut) in &profile.fn_bindings {
        let physical_fn = device
            .provider_descriptor
            .as_ref()
            .and_then(|d| d.definition["controls"].as_array())
            .and_then(|controls| controls.iter().find(|c| c["id"].as_str() == Some(control)))
            .is_some_and(|c| {
                c["role"] == "fn"
                    || c["label"]
                        .as_str()
                        .is_some_and(|label| label.trim().eq_ignore_ascii_case("fn"))
                    || c["label"].as_object().is_some_and(|labels| {
                        labels.values().any(|label| {
                            label
                                .as_str()
                                .is_some_and(|s| s.trim().eq_ignore_ascii_case("fn"))
                        })
                    })
            });
        if physical_fn {
            return Err(Error::new(
                "fn",
                "Fn itself is not an observable shortcut key; choose Fn plus another physical key",
            ));
        }
        let supports_fn = device.provider_descriptor.as_ref().is_some_and(|d| {
            d.definition
                .pointer("/keyboard/onboardMappings/layers")
                .and_then(|v| v.as_array())
                .is_some_and(|layers| layers.iter().any(|v| v == "fn"))
        });
        if !supports_fn
            || require_control(device, control)?.kind != ControlKind::Key
            || !(13..=24).contains(&shortcut.trigger)
            || !triggers.insert(shortcut.trigger)
        {
            return Err(Error::new(
                "fn",
                "Fn shortcuts need physical keys, a declared Fn layer, and distinct F13–F24 triggers",
            ));
        }
        fn one_shot(binding: &Binding) -> Result<()> {
            if matches!(binding.plan, Some(Plan::KeyLogic { .. })) {
                return Err(Error::new(
                    "fn",
                    "Fn shortcuts support one-shot activation only; Key Logic, hold and release are unavailable",
                ));
            }
            for child in binding.children() {
                one_shot(child)?;
            }
            Ok(())
        }
        one_shot(&shortcut.binding)?;
        actions::validate_binding(&shortcut.binding, 0, id)?;
        panels::validate_controller(&shortcut.binding, ControlKind::Key, true)?;
    }
    Ok(())
}
impl Core {
    pub fn test_fn_binding(&mut self, device: &str, control: &str) -> Result<()> {
        let binding = self
            .config
            .devices
            .iter()
            .find(|d| d.id == device)
            .and_then(|d| d.profile.fn_bindings.get(control))
            .map(|f| f.binding.clone())
            .ok_or_else(|| Error::new("fn", "Fn action is not configured"))?;
        self.run_binding(device, control, &binding, 1, false, &binding.id)?;
        self.sequence += 1;
        Ok(())
    }
    pub fn fn_routes(&self) -> Vec<FnRoute> {
        self.config
            .devices
            .iter()
            .flat_map(|device| {
                device
                    .profile
                    .fn_bindings
                    .iter()
                    .filter(|(control, _)| {
                        !device
                            .keyboard_aliases
                            .iter()
                            .any(|a| a.layer == KeyboardLayer::Fn && a.control_id == **control)
                    })
                    .map(|(control, binding)| FnRoute {
                        device_id: device.id.clone(),
                        profile_id: device.profile.id.clone(),
                        control_id: control.clone(),
                        binding_id: binding.binding.id.clone(),
                        trigger: binding.trigger,
                        lifecycle_epoch: self.device_lifecycle_epoch(&device.id),
                    })
            })
            .collect()
    }
    /// One logical activation. It never creates a physical held state or infers Fn.
    /// Plugin protocols receive the existing immediate logical tap compatibility pair.
    pub fn activate_fn(&mut self, route: &FnRoute, generation: u64) -> Result<bool> {
        if self.device_lifecycle_epoch(&route.device_id) != route.lifecycle_epoch
            || !self.connections.get(&route.device_id).is_some_and(|s| {
                s.state == ConnectionState::Connected && s.generation == generation
            })
        {
            return Ok(false);
        }
        let binding = self
            .config
            .devices
            .iter()
            .find(|d| d.id == route.device_id && d.profile.id == route.profile_id)
            .filter(|d| {
                !d.keyboard_aliases
                    .iter()
                    .any(|a| a.layer == KeyboardLayer::Fn && a.control_id == route.control_id)
            })
            .filter(|d| {
                // OS readiness was sampled before taking this Core lock. A completed
                // mapping edit can invalidate that sample while the old hotkey is queued.
                let requested = d
                    .provider_descriptor
                    .as_ref()
                    .and_then(|descriptor| {
                        descriptor
                            .definition
                            .pointer("/keyboard/onboardMappings/settingId")
                    })
                    .and_then(|id| id.as_str())
                    .and_then(|id| d.provider_settings.get(id))
                    .and_then(|mapping| mapping.get(&route.control_id))
                    .and_then(|key| key.pointer("/fn/usage"))
                    .and_then(|usage| usage.as_u64());
                !requested.is_some_and(|usage| usage != u64::from(route.trigger) + 91)
            })
            .and_then(|d| d.profile.fn_bindings.get(&route.control_id))
            .filter(|b| b.trigger == route.trigger && b.binding.id == route.binding_id)
            .map(|b| b.binding.clone());
        let Some(binding) = binding else {
            return Ok(false);
        };
        self.run_binding(
            &route.device_id,
            &route.control_id,
            &binding,
            1,
            false,
            &binding.id,
        )?;
        self.sequence += 1;
        Ok(true)
    }
}
