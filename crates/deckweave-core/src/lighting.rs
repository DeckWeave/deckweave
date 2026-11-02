use super::*;
#[derive(Clone, Debug, Serialize)]
pub struct ProviderOutput {
    pub device_id: String,
    pub changes: BTreeMap<String, serde_json::Value>,
    pub lifecycle_epoch: u64,
    pub token: u64,
}
#[derive(Clone, Default, Debug, Serialize)]
pub struct LightingStatus {
    pub source: String,
    pub manual_editing: bool,
    pub temporary: bool,
    pub preset_id: Option<String>,
    pub error: Option<String>,
}
struct Temporary {
    token: u64,
    until: u64,
    profile: String,
    owner: Option<String>,
}
#[derive(Default)]
struct Owner {
    lock: Option<String>,
    temporary: Option<Temporary>,
    status: LightingStatus,
}
#[derive(Default)]
pub(crate) struct LightingRuntime {
    owners: BTreeMap<String, Owner>,
    outputs: VecDeque<ProviderOutput>,
    token: u64,
}
impl LightingRuntime {
    pub fn statuses(&self) -> BTreeMap<String, LightingStatus> {
        self.owners
            .iter()
            .map(|(id, o)| (id.clone(), o.status.clone()))
            .collect()
    }
    pub fn cancel_device(&mut self, id: &str) {
        self.outputs.retain(|o| o.device_id != id);
        if let Some(o) = self.owners.get_mut(id) {
            o.temporary = None;
            o.status.temporary = false;
            o.status.source = "offline".into();
        }
    }
}
pub(crate) fn lighting_setting(definition: &serde_json::Value, id: &str) -> bool {
    definition["settings"].as_array().is_some_and(|settings| {
        settings
            .iter()
            .any(|s| s["id"].as_str() == Some(id) && s["group"] == "lighting")
    }) || definition["lighting"].as_array().is_some_and(|groups| {
        groups.iter().any(|g| {
            g["settings"]
                .as_array()
                .is_some_and(|s| s.iter().any(|s| s.as_str() == Some(id)))
                || g["staticColors"]["settingId"].as_str() == Some(id)
        })
    })
}
pub(crate) fn validate_lighting_preset(device: &Device, preset: &LightingPreset) -> Result<()> {
    if preset.name.trim().is_empty()
        || preset.name.chars().count() > 80
        || preset.settings.is_empty()
    {
        return Err(Error::new(
            "lighting",
            "Preset needs a name and supported lighting settings",
        ));
    }
    let descriptor = device
        .provider_descriptor
        .as_ref()
        .ok_or_else(|| Error::new("lighting", "Lighting is not supported"))?;
    if preset
        .settings
        .keys()
        .any(|id| !lighting_setting(&descriptor.definition, id))
    {
        return Err(Error::new(
            "lighting",
            "Preset contains a non-lighting setting",
        ));
    }
    validate_provider_settings(&descriptor.definition, &preset.settings)
}
impl Core {
    pub fn take_provider_outputs(&mut self) -> Vec<ProviderOutput> {
        self.lighting.outputs.drain(..).collect()
    }
    fn queue_provider_output(
        &mut self,
        device: &str,
        changes: BTreeMap<String, serde_json::Value>,
        replace: bool,
    ) -> u64 {
        self.lighting.token += 1;
        let token = self.lighting.token;
        let mut merged = self
            .lighting
            .outputs
            .iter()
            .find(|o| o.device_id == device)
            .map(|o| o.changes.clone())
            .unwrap_or_default();
        if replace
            && let Some(desc) = self
                .config
                .devices
                .iter()
                .find(|d| d.id == device)
                .and_then(|d| d.provider_descriptor.as_ref())
        {
            merged.retain(|id, _| !lighting_setting(&desc.definition, id));
        }
        merge_provider_patch(&mut merged, changes);
        self.lighting.outputs.retain(|o| o.device_id != device);
        if !merged.is_empty() {
            self.lighting.outputs.push_back(ProviderOutput {
                device_id: device.into(),
                changes: merged,
                lifecycle_epoch: self.device_lifecycle_epoch(device),
                token,
            });
        }
        self.sequence += 1;
        token
    }
    pub(crate) fn lighting_preset_settings(
        &self,
        device: &str,
        preset: &str,
    ) -> Result<BTreeMap<String, serde_json::Value>> {
        self.config
            .devices
            .iter()
            .find(|d| d.id == device)
            .and_then(|d| d.lighting_presets.iter().find(|p| p.id == preset))
            .map(|p| p.settings.clone())
            .ok_or_else(|| Error::new("lighting", "Lighting preset is missing"))
    }
    pub(crate) fn manual_provider_change(
        &mut self,
        device: &str,
        mut changes: BTreeMap<String, serde_json::Value>,
        source: &str,
    ) -> Result<()> {
        let d = self
            .config
            .devices
            .iter()
            .find(|d| d.id == device)
            .ok_or_else(|| Error::new("not_found", "Device not found"))?;
        let lighting = d.provider_descriptor.as_ref().is_some_and(|desc| {
            changes
                .keys()
                .any(|id| lighting_setting(&desc.definition, id))
        });
        let restore_override = self.lighting.owners.get(device).is_some_and(|o| {
            o.temporary.is_some() || matches!(o.status.source.as_str(), "action" | "profile")
        });
        if lighting {
            if restore_override {
                changes.extend(
                    d.provider_settings
                        .iter()
                        .filter(|(id, _)| {
                            d.provider_descriptor
                                .as_ref()
                                .is_some_and(|desc| lighting_setting(&desc.definition, id))
                        })
                        .map(|(id, value)| (id.clone(), value.clone())),
                );
            }
            let owner = self.lighting.owners.entry(device.into()).or_default();
            owner.lock = Some(source.into());
            owner.temporary = None;
            owner.status = LightingStatus {
                source: "manual".into(),
                manual_editing: true,
                temporary: false,
                preset_id: d.manual_lighting_preset.clone(),
                error: None,
            };
        }
        self.queue_provider_output(device, changes, lighting);
        Ok(())
    }
    pub fn release_lighting_edit(&mut self, device: &str, source: &str) {
        if let Some(owner) = self.lighting.owners.get_mut(device)
            && owner.lock.as_deref() == Some(source)
        {
            owner.lock = None;
            owner.status.manual_editing = false;
            self.sequence += 1;
        }
    }
    pub(crate) fn release_lighting_source(&mut self, source: &str) {
        for owner in self.lighting.owners.values_mut() {
            if owner.lock.as_deref() == Some(source) {
                owner.lock = None;
                owner.status.manual_editing = false;
            }
        }
    }
    pub fn request_lighting_preset(
        &mut self,
        device: &str,
        preset: &str,
        temporary_ms: Option<u64>,
        source: &str,
    ) -> Result<u64> {
        self.request_lighting_preset_owned(device, preset, temporary_ms, source, None)
    }
    pub(crate) fn request_lighting_preset_owned(
        &mut self,
        device: &str,
        preset: &str,
        temporary_ms: Option<u64>,
        source: &str,
        binding_owner: Option<&str>,
    ) -> Result<u64> {
        if self
            .lighting
            .owners
            .get(device)
            .is_some_and(|o| o.lock.is_some())
        {
            let owner = self.lighting.owners.entry(device.into()).or_default();
            owner.status.error =
                Some("Lighting is owned by manual editing; request was not queued".into());
            self.sequence += 1;
            return Err(Error::new(
                "lighting_locked",
                "Lighting is being edited manually",
            ));
        }
        let changes = self.lighting_preset_settings(device, preset)?;
        let d = self
            .config
            .devices
            .iter()
            .find(|d| d.id == device)
            .ok_or_else(|| Error::new("not_found", "Device not found"))?;
        if temporary_ms.is_some_and(|ms| ms == 0 || ms > 86400000) {
            return Err(Error::new(
                "lighting",
                "Temporary preset duration must be 1..86400000 ms",
            ));
        }
        if temporary_ms.is_some()
            && d.provider_descriptor.as_ref().is_some_and(|desc| {
                changes.keys().any(|id| {
                    !desc.definition["settings"]
                        .as_array()
                        .is_some_and(|settings| {
                            settings.iter().any(|setting| {
                                setting["id"].as_str() == Some(id)
                                    && setting["persistence"] == "volatile"
                            })
                        })
                })
            })
        {
            return Err(Error::new(
                "lighting_recovery",
                "Temporary lighting requires verified volatile output; onboard restoration is not available",
            ));
        }
        if temporary_ms.is_some()
            && changes
                .keys()
                .any(|id| !d.provider_settings.contains_key(id))
        {
            return Err(Error::new(
                "lighting",
                "Choose a manual base covering this preset before temporary lighting",
            ));
        }
        let profile = d.profile.id.clone();
        let token = self.queue_provider_output(device, changes, true);
        let owner = self.lighting.owners.entry(device.into()).or_default();
        owner.temporary = temporary_ms.map(|ms| Temporary {
            token,
            until: self.actions.now.saturating_add(ms),
            profile,
            owner: binding_owner.map(str::to_owned),
        });
        owner.status = LightingStatus {
            source: source.into(),
            manual_editing: false,
            temporary: temporary_ms.is_some(),
            preset_id: Some(preset.into()),
            error: None,
        };
        Ok(token)
    }
    pub fn stop_lighting_request(&mut self, device: &str, token: u64) -> Result<bool> {
        if !self
            .lighting
            .owners
            .get(device)
            .and_then(|o| o.temporary.as_ref())
            .is_some_and(|t| t.token == token)
        {
            return Ok(false);
        }
        self.return_to_manual_lighting(device)?;
        Ok(true)
    }
    pub fn return_to_manual_lighting(&mut self, device: &str) -> Result<()> {
        let d = self
            .config
            .devices
            .iter()
            .find(|d| d.id == device)
            .ok_or_else(|| Error::new("not_found", "Device not found"))?;
        let descriptor = d
            .provider_descriptor
            .as_ref()
            .ok_or_else(|| Error::new("lighting", "No lighting descriptor"))?;
        let changes = d
            .provider_settings
            .iter()
            .filter(|(id, _)| lighting_setting(&descriptor.definition, id))
            .map(|(id, value)| (id.clone(), value.clone()))
            .collect();
        let preset = d.manual_lighting_preset.clone();
        let owner = self.lighting.owners.entry(device.into()).or_default();
        owner.temporary = None;
        owner.status = LightingStatus {
            source: "manual".into(),
            manual_editing: owner.lock.is_some(),
            temporary: false,
            preset_id: preset,
            error: None,
        };
        self.queue_provider_output(device, changes, true);
        Ok(())
    }
    pub(crate) fn lighting_obsolete_owners(
        &self,
        valid: &BTreeSet<(String, String)>,
    ) -> BTreeSet<(String, String)> {
        self.lighting
            .owners
            .iter()
            .filter_map(|(device, o)| {
                o.temporary
                    .as_ref()
                    .and_then(|t| t.owner.as_ref())
                    .filter(|owner| !valid.contains(&(device.clone(), (*owner).clone())))
                    .map(|owner| (device.clone(), owner.clone()))
            })
            .collect()
    }
    pub(crate) fn cancel_lighting_owners(&mut self, owners: &BTreeSet<(String, String)>) {
        let cancelled: Vec<_> = self
            .lighting
            .owners
            .iter()
            .filter_map(|(device, o)| {
                o.temporary
                    .as_ref()
                    .filter(|t| {
                        t.owner
                            .as_ref()
                            .is_some_and(|id| owners.contains(&(device.clone(), id.clone())))
                    })
                    .map(|t| (device.clone(), t.token))
            })
            .collect();
        for (device, token) in cancelled {
            let _ = self.stop_lighting_request(&device, token);
        }
    }
    pub(crate) fn cancel_device_temporary_lighting(&mut self, device: &str) {
        if let Some(token) = self
            .lighting
            .owners
            .get(device)
            .and_then(|o| o.temporary.as_ref())
            .map(|t| t.token)
        {
            let _ = self.stop_lighting_request(device, token);
        }
    }
    pub(crate) fn advance_lighting(&mut self, now: u64) {
        let expired: Vec<_> = self
            .lighting
            .owners
            .iter()
            .filter_map(|(device, owner)| {
                owner
                    .temporary
                    .as_ref()
                    .filter(|t| t.until <= now)
                    .map(|t| (device.clone(), t.token))
            })
            .collect();
        for (device, token) in expired {
            let _ = self.stop_lighting_request(&device, token);
        }
    }
    pub(crate) fn lighting_profile_changes(&mut self, previous: &BTreeMap<String, String>) {
        let changes: Vec<_> = self
            .config
            .devices
            .iter()
            .filter(|d| previous.get(&d.id).is_some_and(|p| p != &d.profile.id))
            .map(|d| {
                (
                    d.id.clone(),
                    d.profile.id.clone(),
                    d.profile.lighting_preset.clone(),
                )
            })
            .collect();
        for (device, profile, preset) in changes {
            let cancel = self
                .lighting
                .owners
                .get(&device)
                .and_then(|o| o.temporary.as_ref())
                .is_some_and(|t| t.profile != profile);
            if cancel {
                let _ = self.return_to_manual_lighting(&device);
            }
            if let Some(preset) = preset
                && let Err(error) = self.request_lighting_preset(&device, &preset, None, "profile")
            {
                self.lighting.owners.entry(device).or_default().status.error = Some(error.message);
            }
        }
    }
}
