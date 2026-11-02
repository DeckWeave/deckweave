//! Device-owned keyboard trigger reservations. These APIs never perform or replay hardware I/O.
use super::*;
use serde::{Deserialize, Serialize};

pub type RawKeyboardRecord = [u8; 4];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardLayer {
    Main,
    Fn,
}
impl KeyboardLayer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Fn => "fn",
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardAliasPhase {
    Pending,
    Verified,
    Uncertain,
    Conflict,
    LegacyUnverified,
    Restoring,
    Released,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardAliasProvenance {
    Deckweave,
    Legacy,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyboardAlias {
    pub version: u32,
    pub operation_id: String,
    /// Driver proof of the original installation; retained across restoration attempts.
    pub installation_id: String,
    pub control_id: String,
    pub layer: KeyboardLayer,
    pub alias: u8,
    pub original: Option<RawKeyboardRecord>,
    pub installed: RawKeyboardRecord,
    pub phase: KeyboardAliasPhase,
    pub provenance: KeyboardAliasProvenance,
    pub outcome: String,
    pub identity: Option<String>,
    pub generation: Option<u64>,
    pub profile_id: Option<String>,
    pub page_id: Option<String>,
}

/// Complete freshly observed layer blocks, including non-physical and currently unused slots.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyboardObservation {
    pub main: Vec<RawKeyboardRecord>,
    pub fn_layer: Vec<RawKeyboardRecord>,
    pub control_indices: BTreeMap<String, usize>,
    pub layer_revisions: BTreeMap<KeyboardLayer, String>,
}
impl KeyboardObservation {
    fn validate(&self, device: &Device) -> Result<()> {
        if self.main.len() != 128 || self.fn_layer.len() != 128 {
            return Err(Error::new(
                "keyboard_observation",
                "Read both complete 128-record keyboard layers before assigning an action",
            ));
        }
        let mut indices = BTreeSet::new();
        for (control, index) in &self.control_indices {
            if *index >= 128
                || !indices.insert(*index)
                || require_control(device, control)?.kind != ControlKind::Key
            {
                return Err(Error::new(
                    "keyboard_observation",
                    "Invalid physical keyboard source index",
                ));
            }
        }
        for layer in [KeyboardLayer::Main, KeyboardLayer::Fn] {
            if self.layer_revisions.get(&layer).is_none_or(|revision| {
                revision.len() != 64 || !revision.bytes().all(|b| b.is_ascii_hexdigit())
            }) {
                return Err(Error::new(
                    "keyboard_observation",
                    "Fresh complete-layer revisions are required",
                ));
            }
        }
        Ok(())
    }
    fn records(&self, layer: KeyboardLayer) -> &[RawKeyboardRecord] {
        match layer {
            KeyboardLayer::Main => &self.main,
            KeyboardLayer::Fn => &self.fn_layer,
        }
    }
    fn record(&self, layer: KeyboardLayer, control: &str) -> Result<RawKeyboardRecord> {
        self.control_indices
            .get(control)
            .and_then(|index| self.records(layer).get(*index))
            .copied()
            .ok_or_else(|| {
                Error::new(
                    "keyboard_observation",
                    "Selected control has no observed record",
                )
            })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyboardAssignmentRequest {
    pub device_id: String,
    pub profile_id: String,
    pub page_id: Option<String>,
    pub control_id: String,
    pub layer: KeyboardLayer,
    pub binding: Binding,
    pub observation: KeyboardObservation,
    pub generation: u64,
    /// Confirmed session identity, never inferred from VID/PID alone by Core.
    pub identity: String,
    pub expected_revision: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyboardRestoreRequest {
    pub device_id: String,
    pub control_id: String,
    pub layer: KeyboardLayer,
    pub observation: KeyboardObservation,
    pub generation: u64,
    pub identity: String,
    pub expected_revision: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyboardAssignmentOperation {
    pub operation_id: String,
    pub installation_id: String,
    pub device_id: String,
    pub control_id: String,
    pub layer: KeyboardLayer,
    pub alias: u8,
    pub original: RawKeyboardRecord,
    pub installed: RawKeyboardRecord,
    pub generation: u64,
    pub identity: String,
    pub profile_id: Option<String>,
    pub page_id: Option<String>,
    pub expected_layer_revision: String,
    pub requires_write: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyboardAssignmentReadback {
    pub operation_id: String,
    pub generation: u64,
    pub identity: String,
    pub record: RawKeyboardRecord,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KeyboardAliasRoute {
    pub device_id: String,
    pub profile_id: String,
    pub control_id: String,
    pub layer: KeyboardLayer,
    pub binding_id: String,
    /// Content freshness only; never contains plugin settings or program arguments.
    pub execution_fingerprint: String,
    pub alias: u8,
    pub operation_id: String,
    pub installation_id: String,
    pub page_id: Option<String>,
    pub lifecycle_epoch: u64,
    pub generation: u64,
    pub identity: String,
}
#[derive(Default)]
pub(crate) struct KeyboardSession {
    generation: u64,
    identity: String,
    verified_operations: BTreeSet<String>,
}

fn alias_record(alias: u8) -> RawKeyboardRecord {
    [2, 0, alias + 91, 0]
}
fn record_alias(record: RawKeyboardRecord) -> Option<u8> {
    (record[0] == 2 && (104..=115).contains(&record[2])).then_some(record[2].saturating_sub(91))
}
fn replaceable(record: RawKeyboardRecord) -> bool {
    record[0] == 2
        && record[1] == 0
        && record[3] == 0
        && ((4..=103).contains(&record[2]) || (224..=231).contains(&record[2]))
}
fn validate_one_shot(binding: &Binding) -> Result<()> {
    if matches!(binding.plan, Some(Plan::KeyLogic { .. })) {
        return Err(Error::new(
            "keyboard_input",
            "Keyboard trigger actions currently support one-shot activation; physical hold/release acceptance is pending",
        ));
    }
    for child in binding.children() {
        validate_one_shot(child)?;
    }
    Ok(())
}
fn profile_mut<'a>(device: &'a mut Device, profile_id: &str) -> Result<&'a mut Profile> {
    std::iter::once(&mut device.profile)
        .chain(&mut device.profiles)
        .find(|p| p.id == profile_id)
        .ok_or_else(|| Error::new("not_found", "Original profile no longer exists"))
}
fn active_binding<'a>(device: &'a Device, alias: &KeyboardAlias) -> Option<&'a Binding> {
    match alias.layer {
        KeyboardLayer::Fn => device
            .profile
            .fn_bindings
            .get(&alias.control_id)
            .filter(|reference| reference.trigger == alias.alias)
            .map(|f| &f.binding),
        KeyboardLayer::Main => device.pinned.get(&alias.control_id).or_else(|| {
            device
                .profile
                .pages
                .iter()
                .find(|p| p.id == device.profile.active_page_id)
                .and_then(|p| p.bindings.get(&alias.control_id))
        }),
    }
}
fn execution_fingerprint(binding: &Binding) -> Option<String> {
    // Deterministic content equality guard, not an authentication/identity hash.
    // Hash the entire tree so edits preserving IDs also retire captured routes.
    let bytes = serde_json::to_vec(binding).ok()?;
    let mut first = 0xcbf2_9ce4_8422_2325u64;
    let mut second = 0x8422_2325_cbf2_9ce4u64;
    for byte in bytes {
        first = (first ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        second = (second ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
    }
    Some(format!("{first:016x}{second:016x}"))
}
fn record_requested_mapping(
    device: &mut Device,
    control: &str,
    layer: KeyboardLayer,
    record: RawKeyboardRecord,
) {
    let setting = device
        .provider_descriptor
        .as_ref()
        .and_then(|d| d.definition.pointer("/keyboard/onboardMappings/settingId"))
        .and_then(|id| id.as_str())
        .map(str::to_owned);
    if let Some(setting) = setting {
        merge_provider_patch(
            &mut device.provider_settings,
            BTreeMap::from([(
                setting,
                serde_json::json!({control: {layer.as_str(): {"usage":record[2]}}}),
            )]),
        );
    }
}
fn legacy_references(device: &Device) -> impl Iterator<Item = (&String, &FnBinding)> {
    std::iter::once(&device.profile)
        .chain(&device.profiles)
        .flat_map(|p| p.fn_bindings.iter())
}
fn allocate(
    device: &Device,
    observation: &KeyboardObservation,
    control: &str,
    layer: KeyboardLayer,
) -> Result<u8> {
    let mut used = BTreeSet::new();
    for record in observation.main.iter().chain(&observation.fn_layer) {
        if let Some(alias) = record_alias(*record) {
            used.insert(alias);
        }
    }
    for owner in &device.keyboard_aliases {
        if owner.phase != KeyboardAliasPhase::Released
            && !(owner.provenance == KeyboardAliasProvenance::Legacy
                && owner.layer == layer
                && owner.control_id == control)
        {
            used.insert(owner.alias);
        }
    }
    // Requested mappings can survive disconnection or a failed write and are reservations too.
    let setting = device
        .provider_descriptor
        .as_ref()
        .and_then(|d| d.definition.pointer("/keyboard/onboardMappings/settingId"))
        .and_then(|v| v.as_str());
    if let Some(mapping) = setting
        .and_then(|id| device.provider_settings.get(id))
        .and_then(|v| v.as_object())
    {
        for changes in mapping.values() {
            for candidate_layer in [KeyboardLayer::Main, KeyboardLayer::Fn] {
                if let Some(usage) = changes
                    .get(candidate_layer.as_str())
                    .and_then(|v| v.get("usage"))
                    .and_then(|v| v.as_u64())
                    .filter(|v| (104..=115).contains(v))
                {
                    used.insert((usage - 91) as u8);
                }
            }
        }
    }
    for (source, reference) in legacy_references(device) {
        if layer == KeyboardLayer::Fn && source == control {
            continue;
        }
        // A released owned source is dormant, not an independent legacy reservation.
        if !device.keyboard_aliases.iter().any(|a| {
            a.layer == KeyboardLayer::Fn
                && a.control_id == *source
                && a.phase == KeyboardAliasPhase::Released
                && a.alias == reference.trigger
        }) {
            used.insert(reference.trigger);
        }
    }
    (13..=24).find(|a| !used.contains(a)).ok_or_else(|| Error::new("keyboard_capacity", "All twelve keyboard triggers are occupied or retained. Restore an unused keyboard assignment before adding another source"))
}

pub(crate) fn validate_ledger(device: &Device) -> Result<()> {
    if device.keyboard_aliases.len() > 1024 {
        return Err(Error::new(
            "keyboard_ownership",
            "Too many retained keyboard operations",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut owners = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    for entry in &device.keyboard_aliases {
        if entry.version != 1
            || !(13..=24).contains(&entry.alias)
            || entry.installed != alias_record(entry.alias)
            || entry.operation_id.len() != 32
            || !entry.operation_id.bytes().all(|b| b.is_ascii_hexdigit())
            || !ids.insert(&entry.operation_id)
            || entry.installation_id.len() != 32
            || !entry.installation_id.bytes().all(|b| b.is_ascii_hexdigit())
            || entry.control_id.is_empty()
            || entry.control_id.len() > 256
            || entry.outcome.len() > 512
            || entry
                .identity
                .as_ref()
                .is_some_and(|id| id.is_empty() || id.len() > 4096)
        {
            return Err(Error::new(
                "keyboard_ownership",
                "Invalid keyboard ownership record",
            ));
        }
        if entry.provenance == KeyboardAliasProvenance::Deckweave {
            if entry.original.is_none() || entry.identity.is_none() || entry.generation.is_none() {
                return Err(Error::new(
                    "keyboard_ownership",
                    "Owned trigger has no original record or identity evidence",
                ));
            }
            if entry.phase != KeyboardAliasPhase::Released
                && (!owners.insert((entry.layer, &entry.control_id))
                    || !aliases.insert(entry.alias))
            {
                return Err(Error::new(
                    "keyboard_ownership",
                    "Overlapping keyboard ownership records",
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_owned_mapping_changes(
    device: &Device,
    changes: &BTreeMap<String, serde_json::Value>,
) -> Result<()> {
    let setting = device
        .provider_descriptor
        .as_ref()
        .and_then(|d| d.definition.pointer("/keyboard/onboardMappings/settingId"))
        .and_then(|v| v.as_str());
    let Some(mapping) = setting.and_then(|id| changes.get(id)) else {
        return Ok(());
    };
    for owner in device.keyboard_aliases.iter().filter(|a| {
        a.provenance == KeyboardAliasProvenance::Deckweave
            && a.phase != KeyboardAliasPhase::Released
    }) {
        if let Some(proposed) = mapping
            .get(&owner.control_id)
            .and_then(|key| key.get(owner.layer.as_str()))
            && (owner.phase != KeyboardAliasPhase::Verified
                || proposed.get("usage").and_then(|v| v.as_u64())
                    != Some(u64::from(owner.alias) + 91))
        {
            return Err(Error::new(
                "keyboard_ownership",
                "Restore the retained keyboard assignment before replacing its owned trigger",
            ));
        }
        if let Some(changes) = mapping.as_object() {
            for (control, patch) in changes {
                for layer in [KeyboardLayer::Main, KeyboardLayer::Fn] {
                    if control == &owner.control_id && layer == owner.layer {
                        continue;
                    }
                    if patch
                        .get(layer.as_str())
                        .and_then(|record| record.get("usage"))
                        .and_then(|usage| usage.as_u64())
                        == Some(u64::from(owner.alias) + 91)
                    {
                        return Err(Error::new(
                            "keyboard_ownership",
                            "This trigger is retained by another keyboard source; restore its keyboard assignment before reusing it",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// A verified owned mapping may be submitted as an identical editor value, but
/// must not become a generic SET: the physical record may have changed elsewhere.
pub(crate) fn strip_owned_mapping_noops(
    device: &Device,
    changes: &mut BTreeMap<String, serde_json::Value>,
) {
    let setting = device
        .provider_descriptor
        .as_ref()
        .and_then(|d| d.definition.pointer("/keyboard/onboardMappings/settingId"))
        .and_then(|id| id.as_str());
    let Some(setting) = setting else {
        return;
    };
    let Some(mapping) = changes
        .get_mut(setting)
        .and_then(|mapping| mapping.as_object_mut())
    else {
        return;
    };
    for owner in device.keyboard_aliases.iter().filter(|a| {
        a.provenance == KeyboardAliasProvenance::Deckweave
            && a.phase == KeyboardAliasPhase::Verified
    }) {
        if let Some(layers) = mapping
            .get_mut(&owner.control_id)
            .and_then(|patch| patch.as_object_mut())
        {
            if layers
                .get(owner.layer.as_str())
                .and_then(|record| record.get("usage"))
                .and_then(|usage| usage.as_u64())
                == Some(u64::from(owner.alias) + 91)
            {
                layers.remove(owner.layer.as_str());
            }
            if layers.is_empty() {
                mapping.remove(&owner.control_id);
            }
        }
    }
    if mapping.is_empty() {
        changes.remove(setting);
    }
}

/// Migration only captures references. It does not establish ownership or emit a write.
pub(crate) fn migrate_legacy(config: &mut Configuration) {
    for device in config.devices.iter_mut().chain(&mut config.removed_devices) {
        let references: Vec<_> = std::iter::once(&device.profile)
            .chain(&device.profiles)
            .flat_map(|p| {
                p.fn_bindings
                    .iter()
                    .map(move |(control, binding)| (control.clone(), binding.trigger, p.id.clone()))
            })
            .collect();
        for (control, trigger, profile) in references {
            if !(13..=24).contains(&trigger)
                || device.keyboard_aliases.iter().any(|a| {
                    a.layer == KeyboardLayer::Fn && a.control_id == control && a.alias == trigger
                })
            {
                continue;
            }
            let operation = new_id();
            device.keyboard_aliases.push(KeyboardAlias {
                version: 1,
                operation_id: operation.clone(),
                installation_id: operation,
                control_id: control,
                layer: KeyboardLayer::Fn,
                alias: trigger,
                original: None,
                installed: alias_record(trigger),
                phase: KeyboardAliasPhase::LegacyUnverified,
                provenance: KeyboardAliasProvenance::Legacy,
                outcome: "Legacy host binding preserved; original keyboard assignment is unknown"
                    .into(),
                identity: None,
                generation: None,
                profile_id: Some(profile),
                page_id: None,
            });
        }
    }
}

impl Core {
    /// Check the complete observation before establishing an input session. An alias
    /// appearing on two controls/layers in one keyboard cannot identify its source.
    pub fn validate_keyboard_alias_observation(
        &self,
        device_id: &str,
        observation: &KeyboardObservation,
    ) -> Result<()> {
        let device = self
            .config
            .devices
            .iter()
            .find(|d| d.id == device_id)
            .ok_or_else(|| Error::new("not_found", "Keyboard not found"))?;
        observation.validate(device)?;
        for owner in device.keyboard_aliases.iter().filter(|a| {
            a.provenance == KeyboardAliasProvenance::Deckweave
                && a.phase != KeyboardAliasPhase::Released
        }) {
            let current = observation.record(owner.layer, &owner.control_id)?;
            let expected_occurrences = usize::from(current == owner.installed);
            let occurrences = observation
                .main
                .iter()
                .chain(&observation.fn_layer)
                .filter(|r| record_alias(**r) == Some(owner.alias))
                .count();
            if occurrences != expected_occurrences {
                return Err(Error::new(
                    "keyboard_alias_conflict",
                    "A retained trigger appears on another keyboard source; reconcile it before enabling host actions",
                ));
            }
        }
        Ok(())
    }
    fn keyboard_guard(
        &self,
        device: &str,
        generation: u64,
        identity: &str,
        expected: u64,
    ) -> Result<()> {
        if self.revision() != expected {
            return Err(Error::new(
                "conflict",
                "Configuration changed; reload before editing the keyboard",
            ));
        }
        if identity.is_empty()
            || identity.len() > 4096
            || !self.connections.get(device).is_some_and(|c| {
                c.state == ConnectionState::Connected && c.generation == generation
            })
        {
            return Err(Error::new(
                "keyboard_identity",
                "A confirmed current keyboard connection is required",
            ));
        }
        Ok(())
    }
    fn save_keyboard_configuration(&mut self, mut next: Configuration) -> Result<()> {
        let expected = self.revision();
        next.revision = expected
            .checked_add(1)
            .ok_or_else(|| Error::new("limit", "Revision overflow"))?;
        validate(&next)?;
        self.commit(next, expected)?;
        Ok(())
    }
    fn mark_keyboard_session(
        &mut self,
        device: &str,
        generation: u64,
        identity: &str,
        operation: &str,
    ) {
        let session = self.keyboard_sessions.entry(device.into()).or_default();
        if session.generation != generation || session.identity != identity {
            *session = KeyboardSession {
                generation,
                identity: identity.into(),
                verified_operations: BTreeSet::new(),
            };
        }
        session.verified_operations.insert(operation.into());
    }
    /// Persist the reservation and captured profile binding before the caller sends any SET.
    pub fn prepare_keyboard_assignment(
        &mut self,
        request: KeyboardAssignmentRequest,
    ) -> Result<KeyboardAssignmentOperation> {
        self.keyboard_guard(
            &request.device_id,
            request.generation,
            &request.identity,
            request.expected_revision,
        )?;
        validate_one_shot(&request.binding)?;
        let mut next = self.config.clone();
        let device = device_mut(&mut next, &request.device_id)?;
        request.observation.validate(device)?;
        let descriptor = device.provider_descriptor.as_ref().ok_or_else(|| {
            Error::new(
                "keyboard_control",
                "Device has no declared keyboard mapping support",
            )
        })?;
        if !descriptor
            .definition
            .pointer("/keyboard/onboardMappings/layers")
            .and_then(|v| v.as_array())
            .is_some_and(|layers| {
                layers
                    .iter()
                    .any(|v| v.as_str() == Some(request.layer.as_str()))
            })
        {
            return Err(Error::new(
                "keyboard_control",
                "Selected onboard layer is not declared by this device",
            ));
        }
        let physical_fn = descriptor.definition["controls"]
            .as_array()
            .and_then(|controls| {
                controls
                    .iter()
                    .find(|c| c["id"].as_str() == Some(&request.control_id))
            })
            .is_some_and(|c| {
                c["role"] == "fn"
                    || c["label"]
                        .as_str()
                        .is_some_and(|label| label.trim().eq_ignore_ascii_case("fn"))
                    || c["label"].as_object().is_some_and(|labels| {
                        labels.values().any(|v| {
                            v.as_str()
                                .is_some_and(|s| s.trim().eq_ignore_ascii_case("fn"))
                        })
                    })
            });
        if physical_fn {
            return Err(Error::new(
                "keyboard_control",
                "Fn itself is not an observable host trigger; choose Fn plus another key",
            ));
        }
        let control = require_control(device, &request.control_id)?;
        if control.kind != ControlKind::Key {
            return Err(Error::new(
                "keyboard_control",
                "Choose a physical keyboard key",
            ));
        }
        let native = request
            .observation
            .record(request.layer, &request.control_id)?;
        let existing = device
            .keyboard_aliases
            .iter()
            .find(|a| {
                a.control_id == request.control_id
                    && a.layer == request.layer
                    && a.provenance == KeyboardAliasProvenance::Deckweave
                    && a.phase != KeyboardAliasPhase::Released
            })
            .cloned();
        let (owner, requires_write) = if let Some(mut owner) = existing {
            self.validate_keyboard_alias_observation(&request.device_id, &request.observation)?;
            if owner.phase != KeyboardAliasPhase::Verified
                || owner.identity.as_deref() != Some(&request.identity)
                || native != owner.installed
            {
                return Err(Error::new(
                    "keyboard_reconcile",
                    "Read and reconcile the retained keyboard operation before editing its action",
                ));
            }
            owner.generation = Some(request.generation);
            owner.profile_id = Some(request.profile_id.clone());
            owner.page_id = request.page_id.clone();
            owner.outcome =
                "Existing keyboard trigger readback verified; host action updated".into();
            (owner, false)
        } else {
            if !replaceable(native) {
                return Err(Error::new(
                    "keyboard_read_only",
                    "This native assignment is unknown, paired, mode-changing or already an unowned trigger; it is read-only",
                ));
            }
            let alias = allocate(
                device,
                &request.observation,
                &request.control_id,
                request.layer,
            )?;
            let operation_id = new_id();
            (
                KeyboardAlias {
                    version: 1,
                    operation_id: operation_id.clone(),
                    installation_id: operation_id,
                    control_id: request.control_id.clone(),
                    layer: request.layer,
                    alias,
                    original: Some(native),
                    installed: alias_record(alias),
                    phase: KeyboardAliasPhase::Pending,
                    provenance: KeyboardAliasProvenance::Deckweave,
                    outcome: "Action saved; keyboard installation pending verification".into(),
                    identity: Some(request.identity.clone()),
                    generation: Some(request.generation),
                    profile_id: Some(request.profile_id.clone()),
                    page_id: request.page_id.clone(),
                },
                true,
            )
        };
        if request.layer == KeyboardLayer::Main && device.pinned.contains_key(&request.control_id) {
            return Err(Error::new(
                "keyboard_pinned",
                "Unpin this key before assigning a profile-specific keyboard action",
            ));
        }
        let profile = profile_mut(device, &request.profile_id)?;
        match request.layer {
            KeyboardLayer::Fn => {
                profile.fn_bindings.insert(
                    request.control_id.clone(),
                    FnBinding {
                        trigger: owner.alias,
                        binding: request.binding,
                    },
                );
            }
            KeyboardLayer::Main => {
                let page = profile
                    .pages
                    .iter_mut()
                    .find(|p| Some(&p.id) == request.page_id.as_ref())
                    .ok_or_else(|| {
                        Error::new("not_found", "Choose a page in the original profile")
                    })?;
                page.bindings
                    .insert(request.control_id.clone(), request.binding);
            }
        }
        // One common physical source uses one alias in every profile; preserve every action.
        if request.layer == KeyboardLayer::Fn {
            for profile in std::iter::once(&mut device.profile).chain(&mut device.profiles) {
                if let Some(binding) = profile.fn_bindings.get_mut(&request.control_id) {
                    binding.trigger = owner.alias;
                }
            }
        }
        for legacy in device.keyboard_aliases.iter_mut().filter(|a| {
            a.provenance == KeyboardAliasProvenance::Legacy
                && a.control_id == request.control_id
                && a.layer == request.layer
                && a.phase != KeyboardAliasPhase::Released
        }) {
            legacy.phase = KeyboardAliasPhase::Released;
            legacy.outcome = "Legacy host reference superseded by explicit common-source ownership; saved actions retained".into();
        }
        if let Some(entry) = device
            .keyboard_aliases
            .iter_mut()
            .find(|a| a.operation_id == owner.operation_id)
        {
            *entry = owner.clone();
        } else {
            device.keyboard_aliases.push(owner.clone());
        }
        let operation = KeyboardAssignmentOperation {
            operation_id: owner.operation_id.clone(),
            installation_id: owner.installation_id.clone(),
            device_id: request.device_id.clone(),
            control_id: request.control_id,
            layer: request.layer,
            alias: owner.alias,
            original: owner.original.unwrap(),
            installed: owner.installed,
            generation: request.generation,
            identity: request.identity.clone(),
            profile_id: owner.profile_id,
            page_id: owner.page_id,
            expected_layer_revision: request.observation.layer_revisions[&request.layer].clone(),
            requires_write,
        };
        self.save_keyboard_configuration(next)?;
        if !requires_write {
            self.mark_keyboard_session(
                &request.device_id,
                request.generation,
                &request.identity,
                &operation.operation_id,
            );
        }
        Ok(operation)
    }
    /// Verify a completed installation. A stale generation or different identity cannot commit it.
    pub fn complete_keyboard_assignment(
        &mut self,
        readback: KeyboardAssignmentReadback,
    ) -> Result<KeyboardAliasPhase> {
        self.apply_keyboard_readback(readback, false, false)
    }
    /// Explicit read-only reconciliation. Opening/reconnecting never calls a hardware SET here.
    pub fn reconcile_keyboard_alias(
        &mut self,
        readback: KeyboardAssignmentReadback,
    ) -> Result<KeyboardAliasPhase> {
        self.apply_keyboard_readback(readback, true, false)
    }
    /// Explicit user-confirmed reattachment only. The caller must verify the driver's
    /// retained installation proof against the selected current physical instance first.
    /// A matching VID/PID or a startup discovery is not sufficient authorization.
    pub fn reconcile_keyboard_alias_confirmed(
        &mut self,
        readback: KeyboardAssignmentReadback,
    ) -> Result<KeyboardAliasPhase> {
        self.apply_keyboard_readback(readback, true, true)
    }
    fn apply_keyboard_readback(
        &mut self,
        readback: KeyboardAssignmentReadback,
        reconcile: bool,
        confirmed_identity: bool,
    ) -> Result<KeyboardAliasPhase> {
        let mut next = self.config.clone();
        let device = next
            .devices
            .iter_mut()
            .find(|d| {
                d.keyboard_aliases
                    .iter()
                    .any(|a| a.operation_id == readback.operation_id)
            })
            .ok_or_else(|| Error::new("not_found", "Keyboard operation not found"))?;
        let device_id = device.id.clone();
        self.keyboard_guard(
            &device_id,
            readback.generation,
            &readback.identity,
            self.revision(),
        )?;
        let owner = device
            .keyboard_aliases
            .iter_mut()
            .find(|a| a.operation_id == readback.operation_id)
            .unwrap();
        let same_identity = owner.identity.as_deref() == Some(&readback.identity);
        let expected_record =
            readback.record == owner.installed || owner.original == Some(readback.record);
        if (!same_identity && !(confirmed_identity && expected_record))
            || (!reconcile && owner.generation != Some(readback.generation))
        {
            return Err(Error::new(
                "keyboard_identity",
                "Keyboard operation belongs to a different confirmed connection",
            ));
        }
        if owner.provenance == KeyboardAliasProvenance::Legacy {
            return Err(Error::new(
                "keyboard_reconcile",
                "Legacy trigger references have no provable original keyboard record",
            ));
        }
        if owner.phase == KeyboardAliasPhase::Released {
            return Ok(KeyboardAliasPhase::Released);
        }
        owner.identity = Some(readback.identity.clone());
        owner.generation = Some(readback.generation);
        let restoring = owner.phase == KeyboardAliasPhase::Restoring;
        owner.phase = if restoring && owner.original == Some(readback.record) {
            KeyboardAliasPhase::Released
        } else if !restoring && readback.record == owner.installed {
            KeyboardAliasPhase::Verified
        } else if owner.original == Some(readback.record) {
            KeyboardAliasPhase::Uncertain
        } else {
            KeyboardAliasPhase::Conflict
        };
        owner.outcome = match owner.phase {
            KeyboardAliasPhase::Verified => {
                "Stored keyboard trigger verified; input route requires confirmed source readiness"
            }
            KeyboardAliasPhase::Released => {
                "Original keyboard assignment readback verified; host actions retained inactive"
            }
            KeyboardAliasPhase::Uncertain => {
                "Original assignment observed; installation unresolved and not replayed"
            }
            _ => "Observed assignment differs; external edit preserved and trigger retained",
        }
        .into();
        let phase = owner.phase;
        let control = owner.control_id.clone();
        let layer = owner.layer;
        if matches!(
            phase,
            KeyboardAliasPhase::Verified | KeyboardAliasPhase::Released
        ) {
            record_requested_mapping(device, &control, layer, readback.record);
        }
        self.save_keyboard_configuration(next)?;
        if phase == KeyboardAliasPhase::Verified {
            self.mark_keyboard_session(
                &device_id,
                readback.generation,
                &readback.identity,
                &readback.operation_id,
            );
        } else if let Some(session) = self.keyboard_sessions.get_mut(&device_id) {
            session.verified_operations.remove(&readback.operation_id);
        }
        Ok(phase)
    }
    pub fn fail_keyboard_assignment(&mut self, operation_id: &str, outcome: &str) -> Result<()> {
        let mut next = self.config.clone();
        let device = next
            .devices
            .iter_mut()
            .find(|d| {
                d.keyboard_aliases
                    .iter()
                    .any(|a| a.operation_id == operation_id)
            })
            .ok_or_else(|| Error::new("not_found", "Keyboard operation not found"))?;
        let device_id = device.id.clone();
        let owner = device
            .keyboard_aliases
            .iter_mut()
            .find(|a| a.operation_id == operation_id)
            .unwrap();
        if owner.phase == KeyboardAliasPhase::Released {
            return Err(Error::new(
                "keyboard_operation",
                "Released keyboard operation cannot fail",
            ));
        }
        // A failed restoration remains a restoration obligation so read-only reconciliation
        // can still recognize a completed restore after a timeout or process interruption.
        if owner.phase != KeyboardAliasPhase::Restoring {
            owner.phase = KeyboardAliasPhase::Uncertain;
        }
        owner.outcome = outcome.to_owned();
        while owner.outcome.len() > 512 {
            owner.outcome.pop();
        }
        self.save_keyboard_configuration(next)?;
        if let Some(session) = self.keyboard_sessions.get_mut(&device_id) {
            session.verified_operations.remove(operation_id);
        }
        Ok(())
    }
    pub fn prepare_keyboard_restore(
        &mut self,
        request: KeyboardRestoreRequest,
    ) -> Result<KeyboardAssignmentOperation> {
        self.keyboard_guard(
            &request.device_id,
            request.generation,
            &request.identity,
            request.expected_revision,
        )?;
        let mut next = self.config.clone();
        let device = device_mut(&mut next, &request.device_id)?;
        request.observation.validate(device)?;
        let current = request
            .observation
            .record(request.layer, &request.control_id)?;
        let observed_aliases: BTreeSet<_> = request
            .observation
            .main
            .iter()
            .chain(&request.observation.fn_layer)
            .filter_map(|record| record_alias(*record))
            .collect();
        let owner = device
            .keyboard_aliases
            .iter_mut()
            .find(|a| {
                a.control_id == request.control_id
                    && a.layer == request.layer
                    && a.provenance == KeyboardAliasProvenance::Deckweave
                    && a.phase != KeyboardAliasPhase::Released
            })
            .ok_or_else(|| {
                Error::new(
                    "keyboard_ownership",
                    "No owned keyboard assignment can be restored",
                )
            })?;
        let cancelled_before_install = owner.phase == KeyboardAliasPhase::Uncertain
            && owner.original == Some(current)
            && !observed_aliases.contains(&owner.alias);
        if owner.identity.as_deref() != Some(&request.identity)
            || !(current == owner.installed || cancelled_before_install)
            || owner.original.is_none()
            || matches!(
                owner.phase,
                KeyboardAliasPhase::Pending | KeyboardAliasPhase::Restoring
            )
        {
            return Err(Error::new(
                "keyboard_restore_conflict",
                "Read and reconcile first; the current assignment must exactly match the retained installed record",
            ));
        }
        let old_operation = owner.operation_id.clone();
        owner.operation_id = new_id();
        owner.generation = Some(request.generation);
        owner.phase = if cancelled_before_install {
            KeyboardAliasPhase::Released
        } else {
            KeyboardAliasPhase::Restoring
        };
        owner.outcome = if cancelled_before_install { "Uninstalled operation explicitly cancelled after complete readback; original assignment retained" } else { "Original keyboard restoration pending verification" }.into();
        let operation = KeyboardAssignmentOperation {
            operation_id: owner.operation_id.clone(),
            installation_id: owner.installation_id.clone(),
            device_id: request.device_id.clone(),
            control_id: owner.control_id.clone(),
            layer: owner.layer,
            alias: owner.alias,
            original: owner.original.unwrap(),
            installed: owner.installed,
            generation: request.generation,
            identity: request.identity,
            profile_id: owner.profile_id.clone(),
            page_id: owner.page_id.clone(),
            expected_layer_revision: request.observation.layer_revisions[&request.layer].clone(),
            requires_write: !cancelled_before_install,
        };
        if cancelled_before_install {
            record_requested_mapping(
                device,
                &operation.control_id,
                operation.layer,
                operation.original,
            );
        }
        self.save_keyboard_configuration(next)?;
        if let Some(session) = self.keyboard_sessions.get_mut(&request.device_id) {
            session.verified_operations.remove(&old_operation);
        }
        Ok(operation)
    }
    pub fn keyboard_alias_routes(&self) -> Vec<KeyboardAliasRoute> {
        self.config
            .devices
            .iter()
            .flat_map(|device| {
                device.keyboard_aliases.iter().filter_map(|owner| {
                    let session = self.keyboard_sessions.get(&device.id)?;
                    if owner.phase != KeyboardAliasPhase::Verified
                        || owner.identity.as_deref() != Some(&session.identity)
                        || !session.verified_operations.contains(&owner.operation_id)
                        || !self.connections.get(&device.id).is_some_and(|c| {
                            c.state == ConnectionState::Connected
                                && c.generation == session.generation
                        })
                    {
                        return None;
                    }
                    let binding = active_binding(device, owner)?;
                    if validate_one_shot(binding).is_err() {
                        return None;
                    }
                    Some(KeyboardAliasRoute {
                        device_id: device.id.clone(),
                        profile_id: device.profile.id.clone(),
                        control_id: owner.control_id.clone(),
                        layer: owner.layer,
                        binding_id: binding.id.clone(),
                        execution_fingerprint: execution_fingerprint(binding)?,
                        alias: owner.alias,
                        operation_id: owner.operation_id.clone(),
                        installation_id: owner.installation_id.clone(),
                        page_id: (owner.layer == KeyboardLayer::Main)
                            .then(|| device.profile.active_page_id.clone()),
                        lifecycle_epoch: self.device_lifecycle_epoch(&device.id),
                        generation: session.generation,
                        identity: session.identity.clone(),
                    })
                })
            })
            .collect()
    }
    /// One immediate logical activation, with no inference of a standalone physical Fn event.
    pub fn activate_keyboard_alias(
        &mut self,
        route: &KeyboardAliasRoute,
        generation: u64,
    ) -> Result<bool> {
        if generation != route.generation || !self.keyboard_alias_routes().contains(route) {
            return Ok(false);
        }
        let device = self
            .config
            .devices
            .iter()
            .find(|d| d.id == route.device_id)
            .unwrap();
        let owner = device
            .keyboard_aliases
            .iter()
            .find(|a| a.operation_id == route.operation_id)
            .unwrap();
        let binding = active_binding(device, owner).unwrap().clone();
        validate_one_shot(&binding)?;
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
