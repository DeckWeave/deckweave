//! Profile editing never writes onboard keyboard settings.
use super::*;

pub(crate) fn require_selected(device: &Device, profile: &str) -> Result<()> {
    if device.profile.id != profile {
        return Err(Error::new(
            "conflict",
            "Selected profile changed; review it before editing",
        ));
    }
    Ok(())
}

fn visit(binding: &mut Binding, f: &mut impl FnMut(&mut Binding)) {
    f(binding);
    match &mut binding.plan {
        Some(Plan::DialBindings {
            left,
            right,
            press,
            tap,
        }) => {
            for child in [left, right, press, tap]
                .into_iter()
                .filter_map(|c| c.as_deref_mut())
            {
                visit(child, f);
            }
        }
        Some(Plan::KeyLogic {
            single,
            double,
            hold,
            ..
        }) => {
            for child in [single, double, hold]
                .into_iter()
                .filter_map(|c| c.as_deref_mut())
            {
                visit(child, f);
            }
        }
        Some(Plan::Sequence { steps }) => {
            for step in steps {
                visit(&mut step.action, f);
            }
        }
        Some(Plan::Switch { states }) => {
            for child in states {
                visit(child, f);
            }
        }
        Some(Plan::DialStack { actions } | Plan::ActionWheel { actions }) => {
            for child in actions {
                visit(child, f);
            }
        }
        _ => {}
    }
}

pub(crate) fn duplicate(device: &mut Device, name: &str) -> Result<()> {
    if device.profiles.len() >= 31 {
        return Err(Error::new("limit", "At most 32 profiles"));
    }
    let mut copy = device.profile.clone();
    let original = copy.id.clone();
    copy.id = new_id();
    copy.name = name.trim().into();
    // An explicit copy must not compete with its source for the same Smart Profile app.
    copy.application = None;
    let pages: BTreeMap<_, _> = copy
        .pages
        .iter()
        .map(|p| (p.id.clone(), new_id()))
        .collect();
    copy.active_page_id = pages[&copy.active_page_id].clone();
    let mut rewrite = |b: &mut Binding| {
        b.id = new_id();
        match &mut b.plan {
            Some(Plan::Page { page_id }) => {
                if let Some(id) = pages.get(page_id) {
                    *page_id = id.clone();
                }
            }
            Some(Plan::Profile { profile_id }) if *profile_id == original => {
                profile_id.clone_from(&copy.id);
            }
            _ => {}
        }
    };
    for page in &mut copy.pages {
        page.id = pages[&page.id].clone();
        page.parent = page.parent.as_ref().map(|id| pages[id].clone());
        if let Some(panel) = &mut page.panel {
            for region in &mut panel.regions {
                let old = region.control_id();
                region.id = new_id();
                if let Some(binding) = page.bindings.remove(&old) {
                    page.bindings.insert(region.control_id(), binding);
                }
            }
        }
        for binding in page.bindings.values_mut() {
            visit(binding, &mut rewrite);
        }
    }
    for binding in copy.fn_bindings.values_mut() {
        visit(&mut binding.binding, &mut rewrite);
    }
    if device.default_profile.is_none() {
        device.default_profile = Some(original);
    }
    let previous = std::mem::replace(&mut device.profile, copy);
    device.profiles.push(previous);
    Ok(())
}

pub(crate) fn delete(device: &mut Device) -> Result<()> {
    if device.profiles.is_empty() {
        return Err(Error::new("profile", "Keep at least one profile"));
    }
    let target = &device.profile.id;
    fn references(binding: &Binding, target: &str) -> bool {
        matches!(&binding.plan, Some(Plan::Profile { profile_id }) if profile_id == target)
            || binding
                .children()
                .iter()
                .any(|child| references(child, target))
    }
    if device
        .pinned
        .values()
        .chain(device.profiles.iter().flat_map(|p| {
            p.pages
                .iter()
                .flat_map(|page| page.bindings.values())
                .chain(p.fn_bindings.values().map(|f| &f.binding))
        }))
        .any(|b| references(b, target))
    {
        return Err(Error::new(
            "profile_in_use",
            "Another profile or pinned action switches to this profile; change that action before deleting",
        ));
    }
    let next = device
        .default_profile
        .as_ref()
        .and_then(|id| device.profiles.iter().position(|p| &p.id == id))
        .unwrap_or(0);
    let replacement = device.profiles.remove(next);
    if device.default_profile.as_ref() == Some(target) {
        device.default_profile = Some(replacement.id.clone());
    }
    device.profile = replacement;
    Ok(())
}
