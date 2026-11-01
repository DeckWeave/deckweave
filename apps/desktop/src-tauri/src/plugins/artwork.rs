use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Default)]
pub(super) struct Visual {
    layout: Option<Value>,
    values: Value,
    image: Option<String>,
}
fn asset(package: &Package, path: &str) -> Result<String, String> {
    if path.starts_with("data:image/") {
        if path.len() > 2 * 1024 * 1024 {
            return Err("Image exceeds bounds".into());
        }
        let (header, data) = path.split_once(',').ok_or("Invalid image URI")?;
        if header.ends_with(";base64") {
            return Ok(path.into());
        }
        let mut decoded = Vec::new();
        let mut bytes = data.as_bytes().iter();
        while let Some(&b) = bytes.next() {
            if b == b'%' {
                let a = *bytes.next().ok_or("Bad image escape")?;
                let b = *bytes.next().ok_or("Bad image escape")?;
                let hex = std::str::from_utf8(&[a, b])
                    .map_err(|_| "Bad image escape")?
                    .to_owned();
                decoded.push(u8::from_str_radix(&hex, 16).map_err(|_| "Bad image escape")?);
            } else {
                decoded.push(b);
            }
        }
        return Ok(format!(
            "{};base64,{}",
            header.split(';').next().unwrap_or(header),
            STANDARD.encode(decoded)
        ));
    }
    let resolved = catalog::resolve(&package.root, path).or_else(|_| {
        ["png", "svg", "jpg", "webp"]
            .into_iter()
            .find_map(|ext| catalog::resolve(&package.root, &format!("{path}.{ext}")).ok())
            .ok_or_else(|| "Plugin image not found".to_string())
    })?;
    let bytes = std::fs::read(&resolved).map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Plugin image exceeds bounds".into());
    }
    let mime = match resolved.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "svg" => "svg+xml",
        "jpg" | "jpeg" => "jpeg",
        "webp" => "webp",
        _ => "png",
    };
    Ok(format!(
        "data:image/{mime};base64,{}",
        STANDARD.encode(bytes)
    ))
}
fn layout(package: &Package, name: &str) -> Result<Value, String> {
    if name.starts_with('$') {
        return builtin(name);
    }
    let path = catalog::resolve(&package.root, name)?;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Feedback layout exceeds 64 KiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn builtin(name: &str) -> Result<Value, String> {
    let mut items = vec![
        json!({"key":"title","type":"text","rect":[16,10,136,24],"alignment":"left","font":{"size":16}}),
    ];
    match name {
        "$X1" => items.push(json!({"key":"icon","type":"pixmap","rect":[76,40,48,48]})),
        "$A0" => {
            items[0]["zOrder"] = json!(1);
            items.push(json!({"key":"full-canvas","type":"pixmap","rect":[0,0,200,100]}));
            items.push(json!({"key":"canvas","type":"pixmap","rect":[16,34,136,54],"zOrder":1}));
        }
        "$A1" | "$B1" | "$B2" => {
            items.push(json!({"key":"icon","type":"pixmap","rect":[16,40,48,48]}));
            items.push(json!({"key":"value","type":"text","rect":[76,40,108,32],"alignment":"right","font":{"size":24}}));
            if name != "$A1" {
                items.push(json!({"key":"indicator","type":if name=="$B2"{"gbar"}else{"bar"},"rect":[76,74,108,if name=="$B2"{20}else{12}],"subtype":4,"border_w":0,"bar_h":12,"value":0}));
            }
        }
        "$C1" => {
            for (n, y) in [(1, 40), (2, 68)] {
                items.push(json!({"key":format!("icon{n}"),"type":"pixmap","rect":[16,y,24,24]}));
                items.push(json!({"key":format!("indicator{n}"),"type":"bar","rect":[48,y+6,136,12],"subtype":4,"border_w":0,"value":0}));
            }
        }
        _ => return Err("Unknown built-in Encoder layout".into()),
    }
    Ok(json!({"id":name,"items":items}))
}
pub(super) fn initial(
    session: &mut Session,
    slot: &Slot,
    core: &Mutex<Core>,
    app: &tauri::AppHandle,
    epoch: &AtomicU64,
) {
    let Some(action) = session
        .package
        .actions
        .iter()
        .find(|a| Some(&a.id) == slot.binding.plugin.as_ref().map(|p| &p.action))
        .cloned()
    else {
        return;
    };
    let mut visual = Visual::default();
    if let Some(image) = action.images.first() {
        visual.image = asset(&session.package, image).ok();
    }
    if slot.controller == "Encoder" {
        visual.layout = layout(
            &session.package,
            action.encoder_layout.as_deref().unwrap_or("$X1"),
        )
        .ok();
        visual.values = json!({"title":slot.binding.label,"icon":visual.image});
    }
    session.visuals.insert(slot.binding.id.clone(), visual);
    let _ = render(session, slot, core, app, epoch);
}
pub(super) fn update(
    session: &mut Session,
    slot: &Slot,
    event: &str,
    payload: &Value,
    core: &Mutex<Core>,
    app: &tauri::AppHandle,
    epoch: &AtomicU64,
) -> Result<(), String> {
    let context = &slot.binding.id;
    match event {
        "setImage" => {
            let image = payload["image"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| asset(&session.package, s))
                .transpose()?;
            session.visuals.entry(context.clone()).or_default().image = image;
            return render(session, slot, core, app, epoch);
        }
        "setFeedbackLayout" => {
            let layout = layout(
                &session.package,
                payload["layout"]
                    .as_str()
                    .ok_or("Missing feedback layout")?,
            )?;
            session.visuals.entry(context.clone()).or_default().layout = Some(layout);
        }
        "setFeedback" => {
            let object = payload.as_object().ok_or("Feedback must be an object")?;
            let visual = session.visuals.entry(context.clone()).or_default();
            if !visual.values.is_object() {
                visual.values = json!({});
            }
            for (key, value) in object {
                let old = &mut visual.values[key];
                if let (Some(old), Some(next)) = (old.as_object_mut(), value.as_object()) {
                    old.extend(next.clone());
                } else {
                    *old = value.clone();
                }
            }
            if serde_json::to_vec(&visual.values)
                .map_err(|e| e.to_string())?
                .len()
                > 65536
            {
                visual.values = json!({});
                return Err("Feedback values exceed 64 KiB".into());
            }
        }
        "setTitle" => {
            feedback(core, app, epoch, context, event, payload.clone());
            if slot.controller != "Encoder" {
                return Ok(());
            }
            let visual = session.visuals.entry(context.clone()).or_default();
            if !visual.values.is_object() {
                visual.values = json!({});
            }
            visual.values["title"] = payload["title"].clone();
        }
        "setState" => {
            feedback(core, app, epoch, context, event, payload.clone());
            let state = payload["state"].as_u64().unwrap_or(0) as usize;
            if let Some(path) = session
                .package
                .actions
                .iter()
                .find(|a| Some(&a.id) == slot.binding.plugin.as_ref().map(|p| &p.action))
                .and_then(|a| a.images.get(state))
            {
                let image = asset(&session.package, path)?;
                session.visuals.entry(context.clone()).or_default().image = Some(image);
            }
        }
        _ => {}
    }
    render(session, slot, core, app, epoch)
}
fn render(
    session: &mut Session,
    slot: &Slot,
    core: &Mutex<Core>,
    app: &tauri::AppHandle,
    epoch: &AtomicU64,
) -> Result<(), String> {
    let visual = session.visuals.entry(slot.binding.id.clone()).or_default();
    let mut image = if slot.controller == "Encoder" && visual.layout.is_some() {
        let mut layout = visual.layout.clone().unwrap_or_default();
        for item in layout["items"]
            .as_array_mut()
            .ok_or("Layout has no items")?
        {
            let key = item["key"].as_str().unwrap_or("").to_owned();
            let next = &visual.values[&key];
            if let Some(next) = next.as_object() {
                for (k, v) in next {
                    item[k] = v.clone();
                }
            } else if !next.is_null() {
                item["value"] = next.clone();
            }
            if item["type"] == "pixmap"
                && let Some(path) = item["value"].as_str().filter(|s| !s.is_empty())
            {
                item["value"] = json!(asset(&session.package, path)?);
            }
        }
        let bytes = session
            .renderer
            .as_ref()
            .map_err(Clone::clone)?
            .feedback_png(&layout, &json!({}))?;
        Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
    } else {
        visual.image.clone()
    };
    if let Some(svg) = image
        .as_ref()
        .filter(|s| s.starts_with("data:image/svg+xml"))
    {
        let bytes = session
            .renderer
            .as_ref()
            .map_err(Clone::clone)?
            .image_data_png(
                svg,
                if slot.controller == "Panel" {
                    slot.canvas
                } else {
                    [112, 112]
                },
            )?;
        image = Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)));
    }
    feedback(
        core,
        app,
        epoch,
        &slot.binding.id,
        "setImage",
        json!({"image":image}),
    );
    Ok(())
}
