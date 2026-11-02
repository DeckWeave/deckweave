use super::*;

pub(super) fn validate_provider_descriptor(descriptor: &ProviderDescriptor) -> Result<()> {
    if serde_json::to_vec(descriptor)?.len() > 262144 {
        return Err(Error::new("descriptor", "Descriptor exceeds limit"));
    }
    let g = &descriptor.geometry;
    if !g.width.is_finite()
        || !g.height.is_finite()
        || g.width <= 0.0
        || g.height <= 0.0
        || g.width > 100000.0
        || g.height > 100000.0
        || g.controls.len() > 512
    {
        return Err(Error::new("descriptor", "Invalid device geometry"));
    }
    let mut ids = BTreeSet::new();
    for control in &g.controls {
        let [x, y, w, h] = control.rect;
        if control.id.is_empty()
            || control.id.len() > 128
            || !ids.insert(&control.id)
            || !control.rect.iter().all(|n| n.is_finite())
            || x < 0.0
            || y < 0.0
            || w <= 0.0
            || h <= 0.0
            || x + w > g.width + 1e-9
            || y + h > g.height + 1e-9
        {
            return Err(Error::new(
                "descriptor",
                "Invalid or duplicate device control",
            ));
        }
    }
    if let Some(observation) = descriptor.definition.get("observation") {
        let observation = observation
            .as_object()
            .filter(|value| value.len() == 1 && value.contains_key("keyTravel"))
            .ok_or_else(|| {
                Error::new(
                    "descriptor",
                    "Observation must declare the supported keyTravel capability",
                )
            })?;
        let range = observation["keyTravel"]
            .as_object()
            .filter(|value| {
                value.len() == 5
                    && value.keys().all(|key| {
                        ["minUm", "maxUm", "staleAfterMs", "normalTyping", "evidence"]
                            .contains(&key.as_str())
                    })
            })
            .ok_or_else(|| Error::new("descriptor", "Key travel capability has invalid fields"))?;
        let maximum = range["maxUm"].as_f64();
        if range["minUm"].as_f64() != Some(0.0)
            || maximum.is_none_or(|value| {
                !value.is_finite() || value.fract() != 0.0 || value <= 0.0 || value > 10000.0
            })
            || range["staleAfterMs"].as_f64() != Some(600.0)
            || range["normalTyping"].as_bool().is_none()
            || !matches!(range["evidence"].as_str(), Some("partial" | "verified"))
        {
            return Err(Error::new(
                "descriptor",
                "Invalid key travel observation range or evidence",
            ));
        }
    }
    if let Some(decorations) = descriptor.definition.get("decorations") {
        let decorations = decorations
            .as_array()
            .filter(|d| d.len() <= 32)
            .ok_or_else(|| {
                Error::new(
                    "descriptor",
                    "Decorations must be an array of at most 32 passive elements",
                )
            })?;
        let mut decoration_ids = BTreeSet::new();
        for decoration in decorations {
            let object = decoration
                .as_object()
                .ok_or_else(|| Error::new("descriptor", "Decoration must be an object"))?;
            if object.keys().any(|key| {
                !["id", "kind", "label", "x", "y", "width", "height"].contains(&key.as_str())
            }) {
                return Err(Error::new(
                    "descriptor",
                    "Passive decorations cannot declare events, actions or output capabilities",
                ));
            }
            let id = decoration["id"]
                .as_str()
                .filter(|id| !id.trim().is_empty() && id.len() <= 128)
                .ok_or_else(|| Error::new("descriptor", "Invalid decoration ID"))?;
            if !decoration_ids.insert(id)
                || g.controls.iter().any(|c| c.id == id)
                || decoration["kind"] != "light-strip"
            {
                return Err(Error::new(
                    "descriptor",
                    "Decoration IDs must be unique and separate from controls; only passive light-strip is supported",
                ));
            }
            let [Some(x), Some(y), Some(w), Some(h)] = ["x", "y", "width", "height"]
                .map(|key| decoration[key].as_f64().filter(|v| v.is_finite()))
            else {
                return Err(Error::new(
                    "descriptor",
                    "Decoration bounds must be finite numbers",
                ));
            };
            if x < 0.0
                || y < 0.0
                || w <= 0.0
                || h <= 0.0
                || x + w > g.width + 1e-9
                || y + h > g.height + 1e-9
            {
                return Err(Error::new(
                    "descriptor",
                    "Decoration lies outside device geometry",
                ));
            }
            if let Some(label) = decoration.get("label") {
                let label = label.as_object().ok_or_else(|| {
                    Error::new(
                        "descriptor",
                        "Decoration labels must be localized plain strings",
                    )
                })?;
                if label.iter().any(|(locale, value)| {
                    !["en", "ru"].contains(&locale.as_str())
                        || value.as_str().is_none_or(|text| text.chars().count() > 128)
                }) {
                    return Err(Error::new(
                        "descriptor",
                        "Decoration labels support bounded en/ru plain strings",
                    ));
                }
            }
        }
    }
    let absent_surface = g.surface == [0.0; 4] && g.surface_pixels == [0; 2];
    if !absent_surface
        && (!g.surface.iter().all(|n| n.is_finite())
            || g.surface[0] < 0.0
            || g.surface[1] < 0.0
            || g.surface[2] <= 0.0
            || g.surface[3] <= 0.0
            || g.surface[0] + g.surface[2] > g.width + 1e-9
            || g.surface[1] + g.surface[3] > g.height + 1e-9
            || g.surface_pixels.iter().any(|n| *n == 0 || *n > 8192))
    {
        return Err(Error::new("descriptor", "Invalid display surface"));
    }
    if let Some(groups) = descriptor.definition.get("lighting") {
        let groups = groups
            .as_array()
            .filter(|g| g.len() <= 32)
            .ok_or_else(|| Error::new("descriptor", "Lighting groups must be a bounded array"))?;
        let mut group_ids = BTreeSet::new();
        for group in groups {
            let name = group["id"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 128)
                .ok_or_else(|| Error::new("descriptor", "Invalid lighting group ID"))?;
            if !group_ids.insert(name) {
                return Err(Error::new("descriptor", "Duplicate lighting group"));
            }
            if let Some(controls) = group.get("controls") {
                let controls = controls
                    .as_array()
                    .filter(|c| c.len() <= 512)
                    .ok_or_else(|| Error::new("descriptor", "Invalid lighting controls"))?;
                if controls.iter().any(|c| {
                    c.as_str()
                        .is_none_or(|id| !g.controls.iter().any(|control| control.id == id))
                }) {
                    return Err(Error::new(
                        "descriptor",
                        "Lighting control is absent from geometry",
                    ));
                }
            }
        }
    }
    let settings = descriptor
        .definition
        .get("settings")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::new("descriptor", "Device settings must be an array"))?;
    if settings.len() > 128 {
        return Err(Error::new("descriptor", "Too many settings"));
    }
    let mut ids = BTreeSet::new();
    for setting in settings {
        let id = setting
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|id| !id.is_empty() && id.len() <= 128)
            .ok_or_else(|| Error::new("descriptor", "Invalid setting ID"))?;
        if !ids.insert(id)
            || !matches!(
                setting.get("type").and_then(|v| v.as_str()),
                Some("number" | "boolean" | "choice")
            )
        {
            return Err(Error::new("descriptor", "Invalid or duplicate setting"));
        }
        if setting["type"] == "number" {
            let min = setting["min"]
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or_else(|| Error::new("descriptor", "Number setting needs a finite minimum"))?;
            let max = setting["max"]
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or_else(|| Error::new("descriptor", "Number setting needs a finite maximum"))?;
            if min > max
                || setting
                    .get("step")
                    .is_some_and(|v| v.as_f64().is_none_or(|n| !n.is_finite() || n <= 0.0))
            {
                return Err(Error::new("descriptor", "Invalid setting range"));
            }
        }
        if setting["type"] == "choice"
            && setting
                .get("choices")
                .and_then(|v| v.as_array())
                .is_none_or(|c| c.is_empty() || c.len() > 128)
        {
            return Err(Error::new(
                "descriptor",
                "Choice setting needs bounded choices",
            ));
        }
    }
    validate_provider_presentation(&descriptor.definition, settings)?;
    Ok(())
}
/// Presentation only selects controls; it cannot introduce new writable settings.
fn validate_provider_presentation(
    definition: &serde_json::Value,
    settings: &[serde_json::Value],
) -> Result<()> {
    let declared: BTreeMap<_, _> = settings
        .iter()
        .map(|s| (s["id"].as_str().unwrap(), s))
        .collect();
    let mut dependencies = BTreeMap::new();
    for setting in settings {
        let id = setting["id"].as_str().unwrap();
        if let Some(presentation) = setting.get("presentation")
            && (presentation != "slider" || setting["type"] != "number")
        {
            return Err(Error::new(
                "descriptor",
                "Only numeric settings can use slider presentation",
            ));
        }
        if let Some(condition) = setting.get("visibleWhen") {
            let reference = validate_visibility(condition, &declared)?;
            if reference == id {
                return Err(Error::new(
                    "descriptor",
                    "A setting cannot control its own visibility",
                ));
            }
            dependencies.insert(id, reference);
        }
    }
    for start in dependencies.keys() {
        let mut visited = BTreeSet::new();
        let mut current = *start;
        while let Some(next) = dependencies.get(current) {
            if !visited.insert(current) {
                return Err(Error::new(
                    "descriptor",
                    "Setting visibility contains a cycle",
                ));
            }
            current = next;
        }
    }
    for group in definition
        .get("lighting")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        if let Some(condition) = group.pointer("/staticColors/visibleWhen") {
            validate_visibility(condition, &declared)?;
        }
        let Some(colors) = group.get("colorChannels") else {
            continue;
        };
        let colors = colors
            .as_array()
            .filter(|v| v.len() <= 16)
            .ok_or_else(|| Error::new("descriptor", "Color channels must be a bounded array"))?;
        let owned = group["settings"].as_array().ok_or_else(|| {
            Error::new(
                "descriptor",
                "Color channels need a declared lighting settings group",
            )
        })?;
        let mut ids = BTreeSet::new();
        for color in colors {
            let object = color
                .as_object()
                .ok_or_else(|| Error::new("descriptor", "Color channel must be an object"))?;
            if object
                .keys()
                .any(|key| !["id", "label", "r", "g", "b", "visibleWhen"].contains(&key.as_str()))
            {
                return Err(Error::new("descriptor", "Unknown color presentation field"));
            }
            let id = color["id"]
                .as_str()
                .filter(|id| !id.trim().is_empty() && id.len() <= 128)
                .ok_or_else(|| Error::new("descriptor", "Invalid color channel ID"))?;
            if !ids.insert(id) {
                return Err(Error::new("descriptor", "Duplicate color channel ID"));
            }
            if let Some(label) = color.get("label") {
                let label = label
                    .as_object()
                    .filter(|label| !label.is_empty() && label.len() <= 2)
                    .ok_or_else(|| {
                        Error::new("descriptor", "Color labels must be bounded en/ru strings")
                    })?;
                if label.iter().any(|(locale, value)| {
                    !["en", "ru"].contains(&locale.as_str())
                        || value
                            .as_str()
                            .is_none_or(|s| s.trim().is_empty() || s.chars().count() > 120)
                }) {
                    return Err(Error::new(
                        "descriptor",
                        "Color labels must be bounded en/ru strings",
                    ));
                }
            }
            let mut channels = BTreeSet::new();
            for component in ["r", "g", "b"] {
                let reference = color[component].as_str().ok_or_else(|| {
                    Error::new("descriptor", "Color component must reference a setting")
                })?;
                let setting = declared.get(reference).ok_or_else(|| {
                    Error::new("descriptor", "Color component setting is not declared")
                })?;
                if !channels.insert(reference)
                    || !owned.iter().any(|value| value.as_str() == Some(reference))
                    || setting["type"] != "number"
                    || setting["min"].as_f64() != Some(0.0)
                    || setting["max"].as_f64() != Some(255.0)
                    || setting
                        .get("step")
                        .is_some_and(|step| step.as_f64() != Some(1.0))
                {
                    return Err(Error::new(
                        "descriptor",
                        "RGB components need distinct group-owned integer settings from 0 to 255",
                    ));
                }
            }
            if let Some(condition) = color.get("visibleWhen") {
                validate_visibility(condition, &declared)?;
            }
        }
    }
    Ok(())
}
fn validate_visibility<'a>(
    condition: &'a serde_json::Value,
    declared: &BTreeMap<&str, &serde_json::Value>,
) -> Result<&'a str> {
    let object = condition
        .as_object()
        .filter(|value| {
            value.len() == 2 && value.contains_key("settingId") && value.contains_key("values")
        })
        .ok_or_else(|| {
            Error::new(
                "descriptor",
                "Visibility needs settingId and bounded values",
            )
        })?;
    let id = object["settingId"]
        .as_str()
        .ok_or_else(|| Error::new("descriptor", "Invalid visibility setting reference"))?;
    let target = declared
        .get(id)
        .ok_or_else(|| Error::new("descriptor", "Visibility setting is not declared"))?;
    if !matches!(target["type"].as_str(), Some("choice" | "boolean")) {
        return Err(Error::new(
            "descriptor",
            "Visibility must reference a choice or boolean setting",
        ));
    }
    let values = object["values"]
        .as_array()
        .filter(|values| !values.is_empty() && values.len() <= 32)
        .ok_or_else(|| Error::new("descriptor", "Visibility needs 1 to 32 choice values"))?;
    let mut unique = BTreeSet::new();
    for value in values {
        let scalar = match value {
            serde_json::Value::Null | serde_json::Value::Bool(_) => true,
            serde_json::Value::Number(number) => number.as_f64().is_some_and(|n| n.is_finite()),
            serde_json::Value::String(text) => text.chars().count() <= 256,
            _ => false,
        };
        let allowed = if target["type"] == "boolean" {
            value.is_boolean()
        } else {
            target["choices"].as_array().is_some_and(|choices| {
                choices
                    .iter()
                    .any(|choice| choice.get("value").unwrap_or(choice) == value)
            })
        };
        if !scalar || !allowed || !unique.insert(value.to_string()) {
            return Err(Error::new(
                "descriptor",
                "Visibility contains invalid, duplicate or undeclared choice values",
            ));
        }
    }
    Ok(id)
}
pub(super) fn validate_provider_settings(
    definition: &serde_json::Value,
    changes: &BTreeMap<String, serde_json::Value>,
) -> Result<()> {
    if changes.len() > 128 {
        return Err(Error::new("settings", "Too many device settings"));
    }
    let declared = definition
        .get("settings")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::new("settings", "No settings are declared"))?;
    for (id, value) in changes {
        if let Some(group) = definition
            .get("lighting")
            .and_then(|v| v.as_array())
            .and_then(|groups| {
                groups.iter().find(|g| {
                    g["staticColors"]["settingId"].as_str() == Some(id)
                        && g["staticColors"]["address"] == "controlId"
                })
            })
        {
            let colors = value
                .as_object()
                .filter(|v| !v.is_empty() && v.len() <= 512)
                .ok_or_else(|| {
                    Error::new("settings", "Static colors need a non-empty control map")
                })?;
            let controls = group["controls"]
                .as_array()
                .ok_or_else(|| Error::new("settings", "Lighting controls are not declared"))?;
            for (control, color) in colors {
                if !controls.iter().any(|id| id.as_str() == Some(control))
                    || color.as_object().is_none_or(|c| c.len() != 3)
                    || ["r", "g", "b"]
                        .iter()
                        .any(|channel| color[*channel].as_u64().is_none_or(|n| n > 255))
                {
                    return Err(Error::new(
                        "settings",
                        "Invalid lighting control or RGB value",
                    ));
                }
            }
            continue;
        }
        if validate_keyboard_change(definition, id, value)? {
            continue;
        }
        let setting = declared
            .iter()
            .find(|s| s["id"].as_str() == Some(id))
            .ok_or_else(|| Error::new("settings", "Setting is not declared by the provider"))?;
        let valid = match setting["type"].as_str() {
            Some("boolean") => value.is_boolean(),
            Some("number") => value.as_f64().is_some_and(|n| {
                n.is_finite()
                    && setting["min"].as_f64().is_some_and(|min| n >= min)
                    && setting["max"].as_f64().is_some_and(|max| n <= max)
                    && setting
                        .get("step")
                        .and_then(|v| v.as_f64())
                        .is_none_or(|step| {
                            let q = (n - setting["min"].as_f64().unwrap_or(0.0)) / step;
                            (q - q.round()).abs() < 1e-6
                        })
            }),
            Some("choice") => setting["choices"].as_array().is_some_and(|choices| {
                choices
                    .iter()
                    .any(|choice| choice == value || choice.get("value") == Some(value))
            }),
            _ => false,
        };
        if !valid {
            return Err(Error::new(
                "settings",
                format!("Invalid value for device setting {id}"),
            ));
        }
    }
    Ok(())
}

fn validate_keyboard_change(
    definition: &serde_json::Value,
    id: &str,
    value: &serde_json::Value,
) -> Result<bool> {
    let keyboard = &definition["keyboard"];
    let mappings = keyboard["onboardMappings"]["settingId"].as_str() == Some(id)
        && keyboard["onboardMappings"]["bindingFormat"] == "keyboard-usage";
    let actuation = keyboard["actuation"]["settingId"].as_str() == Some(id);
    if !mappings && !actuation {
        return Ok(false);
    }
    let patch = value
        .as_object()
        .filter(|m| !m.is_empty() && m.len() <= 512)
        .ok_or_else(|| Error::new("settings", "Select a bounded set of physical keys"))?;
    let controls = definition["controls"]
        .as_array()
        .ok_or_else(|| Error::new("settings", "Keyboard controls are not declared"))?;
    for (control, changes) in patch {
        if !controls
            .iter()
            .any(|c| c["id"].as_str() == Some(control) && c["kind"] == "key")
        {
            return Err(Error::new("settings", "Keyboard control is not declared"));
        }
        let changes = changes
            .as_object()
            .filter(|m| !m.is_empty() && m.len() <= 6)
            .ok_or_else(|| Error::new("settings", "Key settings must be a bounded object"))?;
        if mappings {
            let layers = keyboard["onboardMappings"]["layers"]
                .as_array()
                .ok_or_else(|| Error::new("settings", "Keyboard layers are not declared"))?;
            for (layer, binding) in changes {
                if !layers.iter().any(|l| l.as_str() == Some(layer))
                    || binding.as_object().is_none_or(|b| b.len() != 1)
                    || binding["usage"]
                        .as_u64()
                        .is_none_or(|u| !(4..=115).contains(&u) && !(224..=231).contains(&u))
                {
                    return Err(Error::new(
                        "settings",
                        "Only declared standard keyboard assignments are writable",
                    ));
                }
            }
        } else {
            let descriptor = &keyboard["actuation"];
            let minimum = descriptor["minUm"]
                .as_u64()
                .ok_or_else(|| Error::new("settings", "Actuation range is not declared"))?;
            let maximum = descriptor["maxUm"]
                .as_u64()
                .filter(|n| *n >= minimum && *n <= 100000)
                .ok_or_else(|| Error::new("settings", "Invalid actuation range"))?;
            let step = descriptor["stepUm"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(|| Error::new("settings", "Invalid actuation step"))?;
            let rapid = descriptor["rapidMinUm"]
                .as_u64()
                .filter(|n| *n > 0 && *n <= maximum)
                .unwrap_or(minimum);
            for (field, value) in changes {
                if descriptor
                    .get("fields")
                    .and_then(|v| v.as_array())
                    .is_none_or(|fields| !fields.iter().any(|f| f.as_str() == Some(field)))
                {
                    return Err(Error::new(
                        "settings",
                        "Actuation field is not declared writable",
                    ));
                }
                let valid = match field.as_str() {
                    "rapidTrigger" | "wholeTravel" | "rampage" => value.is_boolean(),
                    "triggerUm" => value
                        .as_u64()
                        .is_some_and(|n| n >= minimum && n <= maximum && n % step == 0),
                    "pressUm" | "releaseUm" => value
                        .as_u64()
                        .is_some_and(|n| n >= rapid && n <= maximum && n % step == 0),
                    _ => false,
                };
                if !valid {
                    return Err(Error::new(
                        "settings",
                        "Unsupported actuation field or out-of-range value",
                    ));
                }
            }
        }
    }
    Ok(true)
}

pub fn merge_provider_patch(
    target: &mut BTreeMap<String, serde_json::Value>,
    patch: BTreeMap<String, serde_json::Value>,
) {
    fn merge(target: &mut serde_json::Value, patch: serde_json::Value) {
        if let (Some(current), Some(next)) = (target.as_object_mut(), patch.as_object()) {
            for (key, value) in next {
                if let Some(old) = current.get_mut(key) {
                    merge(old, value.clone());
                } else {
                    current.insert(key.clone(), value.clone());
                }
            }
        } else {
            *target = patch;
        }
    }
    for (key, value) in patch {
        if let Some(old) = target.get_mut(&key) {
            merge(old, value);
        } else {
            target.insert(key, value);
        }
    }
}
