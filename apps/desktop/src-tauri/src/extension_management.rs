//! Source-aware action package management. No external installation is deleted.
use crate::{Application, plugin_lifecycle::Record, plugin_manager::Manager, plugins};
use deckweave_core::{Binding, Configuration};
use serde::Serialize;
use std::{collections::BTreeMap, fs, path::Path};
use tauri::{Emitter, Manager as _};

#[derive(Clone, Serialize)]
pub struct Source {
    kind: String,
    version: String,
    active: bool,
}
#[derive(Clone, Serialize)]
pub struct Installed {
    kind: String,
    id: String,
    name: String,
    version: String,
    source: String,
    sources: Vec<Source>,
    blocked: bool,
    phase: Option<String>,
    error: Option<String>,
    actions: Vec<plugins::catalog::Action>,
    resource_only: bool,
    development: bool,
    available: bool,
}
#[derive(Clone, Serialize)]
pub struct Dependency {
    device: String,
    profile: String,
    page: String,
    control: String,
    action: String,
    saved_device: bool,
}
#[derive(Serialize)]
pub struct RemovalPreview {
    kind: String,
    devices: Vec<String>,
    id: String,
    name: String,
    source: String,
    revision: u64,
    dependencies: Vec<Dependency>,
}

fn inventory(manager: &Manager, host: &plugins::Host) -> Result<Vec<Installed>, String> {
    let mut entries: BTreeMap<String, Installed> = BTreeMap::new();
    for (source, package) in host.candidates() {
        let entry = entries
            .entry(package.id.clone())
            .or_insert_with(|| Installed {
                kind: "action".into(),
                id: package.id.clone(),
                name: package.name.clone(),
                version: package.version.clone(),
                source: source.clone(),
                sources: vec![],
                blocked: false,
                phase: None,
                error: package.error.clone(),
                actions: package.actions.clone(),
                resource_only: package.resource_only,
                development: package.development,
                available: true,
            });
        entry.sources.push(Source {
            kind: source,
            version: package.version,
            active: entry.sources.is_empty(),
        });
    }
    let roots = [
        ("managed", manager.directory.join("device-plugins")),
        (
            "repository",
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../device-plugins"),
        ),
    ];
    for (source, root) in roots {
        let Ok(dirs) = fs::read_dir(root) else {
            continue;
        };
        for dir in dirs.flatten() {
            let path = dir.path();
            if path.extension().is_none_or(|e| e != "dwDevice") {
                continue;
            }
            let folder = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if !crate::plugin_manager::valid_id(folder) {
                continue;
            }
            let (id, name, version, error) = match crate::device_providers::inspect_package(&path) {
                Ok(manifest) => (manifest.id, manifest.name, manifest.version, None),
                Err(error) => (
                    folder.to_owned(),
                    folder.to_owned(),
                    String::new(),
                    Some(error),
                ),
            };
            let entry = entries
                .entry(format!("device:{id}"))
                .or_insert_with(|| Installed {
                    kind: "device".into(),
                    id: id.clone(),
                    name,
                    version: version.clone(),
                    source: source.into(),
                    sources: vec![],
                    blocked: false,
                    phase: None,
                    error,
                    actions: vec![],
                    resource_only: false,
                    development: false,
                    available: true,
                });
            entry.sources.push(Source {
                kind: source.into(),
                version,
                active: entry.sources.is_empty(),
            });
        }
    }
    for record in manager.lifecycle.records()? {
        let entry = entries
            .entry(if record.kind == "device" {
                format!("device:{}", record.id)
            } else {
                record.id.clone()
            })
            .or_insert_with(|| Installed {
                kind: record.kind,
                id: record.id.clone(),
                name: record.name,
                version: record.version,
                source: record.source,
                sources: vec![],
                blocked: true,
                phase: None,
                error: None,
                actions: vec![],
                resource_only: false,
                development: false,
                available: false,
            });
        entry.blocked = true;
        entry.phase = Some(record.phase);
        entry.error = record.error;
        for source in &mut entry.sources {
            source.active = false;
        }
    }
    let mut entries: Vec<_> = entries.into_values().collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn dependencies(config: &Configuration, package: &str) -> Vec<Dependency> {
    fn visit(binding: &Binding, package: &str, context: &Dependency, out: &mut Vec<Dependency>) {
        if binding
            .plugin
            .as_ref()
            .is_some_and(|p| p.package == package)
        {
            let mut hit = context.clone();
            hit.action = binding.label.clone();
            out.push(hit);
        }
        for child in binding.children() {
            visit(child, package, context, out);
        }
    }
    let mut result = vec![];
    for (device, saved) in config
        .devices
        .iter()
        .map(|d| (d, false))
        .chain(config.removed_devices.iter().map(|d| (d, true)))
    {
        let base = Dependency {
            device: device.name.clone(),
            profile: String::new(),
            page: String::new(),
            control: String::new(),
            action: String::new(),
            saved_device: saved,
        };
        for (control, binding) in &device.pinned {
            visit(
                binding,
                package,
                &Dependency {
                    profile: "Pinned".into(),
                    control: control.clone(),
                    ..base.clone()
                },
                &mut result,
            );
        }
        for profile in std::iter::once(&device.profile).chain(&device.profiles) {
            for page in &profile.pages {
                for (control, binding) in &page.bindings {
                    visit(
                        binding,
                        package,
                        &Dependency {
                            profile: profile.name.clone(),
                            page: page.name.clone(),
                            control: control.clone(),
                            ..base.clone()
                        },
                        &mut result,
                    );
                }
            }
        }
    }
    result
}

fn remove_managed(root: &Path, id: &str, kind: &str) -> Result<(), String> {
    if !crate::plugin_manager::valid_id(id) {
        return Err("Invalid package identity".into());
    }
    let suffix = if kind == "device" {
        "dwDevice"
    } else {
        "sdPlugin"
    };
    let target = root.join(format!("{id}.{suffix}"));
    let metadata = match fs::symlink_metadata(&target) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Managed package is not a regular directory".into());
    }
    let owned = root.canonicalize().map_err(|e| e.to_string())?;
    let resolved = target.canonicalize().map_err(|e| e.to_string())?;
    if resolved.parent() != Some(owned.as_path()) {
        return Err("Package path is outside managed installation".into());
    }
    fs::remove_dir_all(&target).map_err(|e| format!("Package stopped; file cleanup pending: {e}"))
}

fn allowed(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Manage extensions from application settings".into())
    }
}
#[tauri::command]
pub fn plugin_lifecycle_history(
    window: tauri::WebviewWindow,
    manager: tauri::State<'_, Manager>,
) -> Result<Vec<crate::plugin_lifecycle::Event>, String> {
    allowed(&window)?;
    manager.lifecycle.history()
}
#[tauri::command]
pub async fn plugin_inventory(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Vec<Installed>, String> {
    allowed(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Application>();
        let Application::Ready { plugins, .. } = state.inner() else {
            return Err("Plugin host unavailable".into());
        };
        inventory(&app.state::<Manager>(), plugins)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn plugin_removal_preview(
    id: String,
    kind: Option<String>,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<RemovalPreview, String> {
    allowed(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Application>();
        let Application::Ready { plugins, .. } = state.inner() else {
            return Err("Plugin host unavailable".into());
        };
        let entry = inventory(&app.state::<Manager>(), plugins)?
            .into_iter()
            .find(|p| p.id == id && p.kind == kind.as_deref().unwrap_or("action"))
            .ok_or("Package not found")?;
        let config = crate::locked(&state)
            .map_err(|e| e.to_string())?
            .snapshot()
            .configuration;
        Ok(RemovalPreview {
            kind: entry.kind.clone(),
            devices: if entry.kind == "device" {
                config
                    .devices
                    .iter()
                    .chain(&config.removed_devices)
                    .filter(|d| d.provider_id == id)
                    .map(|d| d.name.clone())
                    .collect()
            } else {
                vec![]
            },
            id,
            name: entry.name,
            source: entry.source,
            revision: config.revision,
            dependencies: if entry.kind == "action" {
                dependencies(&config, &entry.id)
            } else {
                vec![]
            },
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn plugin_remove(
    id: String,
    kind: Option<String>,
    expected_revision: u64,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    allowed(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        remove_package(
            &app,
            &id,
            kind.as_deref().unwrap_or("action"),
            expected_revision,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) fn remove_package(
    app: &tauri::AppHandle,
    id: &str,
    kind: &str,
    expected_revision: u64,
) -> Result<(), String> {
    let manager = app.state::<Manager>();
    let _operation = manager
        .install
        .lock()
        .map_err(|_| "Extension manager unavailable")?;
    let state = app.state::<Application>();
    let Application::Ready {
        plugins,
        device_providers,
        ..
    } = state.inner()
    else {
        return Err("Plugin host unavailable".into());
    };
    let entry = inventory(&manager, plugins)?
        .into_iter()
        .find(|p| p.id == id && p.kind == kind)
        .ok_or("Package not found")?;
    if crate::locked(&state).map_err(|e| e.to_string())?.revision() != expected_revision {
        return Err("Configuration changed; review affected assignments again".into());
    }
    let mut record = Record {
        kind: kind.into(),
        id: id.into(),
        name: entry.name,
        version: entry.version,
        source: entry.source,
        phase: "stopping".into(),
        error: None,
    };
    manager.lifecycle.block(record.clone())?;
    if kind == "action" {
        crate::locked(&state)
            .map_err(|e| e.to_string())?
            .cancel_package(id);
    }
    let stopped = if kind == "device" {
        device_providers.pause_package(id)
    } else {
        plugins.pause(id)
    };
    let result = match stopped {
        Ok(()) => {
            record.phase = "cleanup_pending".into();
            manager.lifecycle.block(record.clone())?;
            remove_managed(
                &manager.directory.join(if kind == "device" {
                    "device-plugins"
                } else {
                    "plugins"
                }),
                id,
                kind,
            )
        }
        Err(error) => {
            record.phase = "stop_pending".into();
            Err(error)
        }
    };
    if result.is_ok() {
        record.phase = "removed".into();
    }
    record.error = result.clone().err();
    manager.lifecycle.block(record)?;
    let reloaded = if kind == "device" {
        device_providers.resume_package(id);
        Ok(())
    } else {
        plugins.reload(id)
    };
    let _ = app.emit("deckweave:plugins", ());
    let _ = app.emit("deckweave:changed", ());
    result?;
    reloaded
}
#[tauri::command]
pub async fn plugin_reconnect(
    id: String,
    kind: Option<String>,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    allowed(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let manager = app.state::<Manager>();
        let _operation = manager
            .install
            .lock()
            .map_err(|_| "Extension manager unavailable")?;
        let state = app.state::<Application>();
        let Application::Ready {
            plugins,
            device_providers,
            ..
        } = state.inner()
        else {
            return Err("Plugin host unavailable".into());
        };
        let kind = kind.as_deref().unwrap_or("action");
        if !inventory(&manager, plugins)?
            .iter()
            .any(|p| p.id == id && p.kind == kind && p.available)
        {
            return Err("Install the package archive to restore this extension".into());
        }
        manager.lifecycle.reconnect(&if kind == "device" {
            format!("device:{id}")
        } else {
            id.clone()
        })?;
        if kind == "device" {
            device_providers.resume_package(&id);
        } else {
            plugins.reload(&id)?;
        }
        let _ = app.emit("deckweave:plugins", ());
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_is_bounded_to_one_managed_package() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("test.one.sdPlugin")).unwrap();
        fs::create_dir(dir.path().join("test.two.sdPlugin")).unwrap();
        assert!(remove_managed(dir.path(), "../escape", "action").is_err());
        remove_managed(dir.path(), "test.one", "action").unwrap();
        assert!(!dir.path().join("test.one.sdPlugin").exists());
        assert!(dir.path().join("test.two.sdPlugin").exists());
    }
    #[test]
    fn dependencies_include_nested_inactive_pinned_and_saved_devices() {
        let mut config = deckweave_core::Core::memory()
            .unwrap()
            .snapshot()
            .configuration;
        let mut plugin = Binding::new(deckweave_core::ActionKind::Plugin, "Nested plugin");
        plugin.plugin = Some(deckweave_core::PluginAction {
            package: "test.plugin".into(),
            action: "test.action".into(),
            settings: serde_json::json!({"keep":true}),
            controller: None,
        });
        let mut scenario = Binding::new(deckweave_core::ActionKind::Program, "Switch");
        scenario.plan = Some(deckweave_core::Plan::Switch {
            states: vec![plugin.clone()],
        });
        let mut inactive = config.devices[0].profile.clone();
        inactive.pages[0].bindings.clear();
        inactive.pages[0].bindings.insert("key-9".into(), scenario);
        config.devices[0].profiles.push(inactive);
        config.devices[0].pinned.insert("key-10".into(), plugin);
        let mut saved = config.devices[0].clone();
        saved.id = "saved".into();
        config.removed_devices.push(saved);
        let hits = dependencies(&config, "test.plugin");
        assert_eq!(hits.len(), 4);
        assert_eq!(hits.iter().filter(|d| d.saved_device).count(), 2);
        assert!(hits.iter().all(|d| d.action == "Nested plugin"));
        assert!(dependencies(&config, "unrelated.plugin").is_empty());
    }
}
