use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Serialize)]
pub struct Action {
    pub id: String,
    pub name: String,
    pub controllers: Vec<String>,
    pub inspector: Option<String>,
    #[serde(skip)]
    pub encoder_layout: Option<String>,
    #[serde(skip)]
    pub images: Vec<String>,
}
#[derive(Clone, Serialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub actions: Vec<Action>,
    pub error: Option<String>,
    #[serde(skip)]
    pub root: PathBuf,
    #[serde(skip)]
    pub entry: Option<PathBuf>,
}
pub fn scan(roots: &[PathBuf]) -> Vec<Package> {
    let mut packages = vec![];
    let mut seen = BTreeSet::new();
    for root in roots {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("sdPlugin") {
                    continue;
                }
                let id = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                if seen.insert(id.clone()) {
                    packages.push(read_package(path, id));
                }
            }
        }
    }
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages
}
fn read_package(root: PathBuf, id: String) -> Package {
    let root = root.canonicalize().unwrap_or(root);
    let mut package = Package {
        name: id.clone(),
        id,
        version: String::new(),
        actions: vec![],
        error: None,
        root: root.clone(),
        entry: None,
    };
    let result = (|| -> Result<(), String> {
        let bytes = std::fs::read(root.join("manifest.json")).map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("Manifest exceeds 1 MiB".into());
        }
        let manifest: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "Protected or invalid manifest".to_string())?;
        package.name = manifest["Name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&package.id)
            .into();
        package.version = manifest["Version"].as_str().unwrap_or("0").into();
        let platform = if cfg!(windows) {
            "CodePathWin"
        } else if cfg!(target_os = "macos") {
            "CodePathMac"
        } else {
            "CodePathLin"
        };
        let entry = manifest[platform]
            .as_str()
            .or(manifest["CodePath"].as_str())
            .ok_or("No platform entry point")?;
        package.entry =
            Some(resolve(&root, entry).or_else(|_| resolve(&root, &format!("{entry}.exe")))?);
        let mut ids = BTreeSet::new();
        for action in manifest["Actions"].as_array().ok_or("No action list")? {
            let id = action["UUID"]
                .as_str()
                .ok_or("Missing action UUID")?
                .to_string();
            if !ids.insert(id.clone()) {
                return Err("Duplicate action UUID".into());
            }
            let inspector = action["PropertyInspectorPath"]
                .as_str()
                .or(manifest["PropertyInspectorPath"].as_str())
                .map(str::to_owned);
            if let Some(path) = &inspector {
                resolve(&root, path)?;
            }
            let controllers = action["Controllers"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_else(|| vec!["Keypad".into()]);
            package.actions.push(Action {
                id,
                name: action["Name"].as_str().unwrap_or("Action").into(),
                controllers,
                inspector,
                encoder_layout: action["Encoder"]["layout"].as_str().map(str::to_owned),
                images: action["States"]
                    .as_array()
                    .map(|states| {
                        states
                            .iter()
                            .filter_map(|s| s["Image"].as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
        Ok(())
    })();
    if let Err(error) = result {
        package.error = Some(error);
    }
    package
}
pub fn resolve(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        || relative.contains(['\\', ':'])
    {
        return Err("Path escapes plugin bundle".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let target = root.join(path).canonicalize().map_err(|e| e.to_string())?;
    if !target.starts_with(&root) || !target.is_file() {
        return Err("Path escapes plugin bundle".into());
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundle_paths_cannot_escape_or_open_directories() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for path in [
            "../Cargo.toml",
            "src/../../Cargo.toml",
            "C:/Windows/win.ini",
            "src",
        ] {
            assert!(resolve(root, path).is_err());
        }
        assert!(resolve(root, "Cargo.toml").is_ok());
    }
}
