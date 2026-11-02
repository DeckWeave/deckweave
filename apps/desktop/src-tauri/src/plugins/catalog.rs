use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Serialize)]
pub struct Action {
    pub id: String,
    pub name: String,
    pub controllers: Vec<String>,
    pub visible: bool,
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
    pub resource_only: bool,
    pub development: bool,
    #[serde(skip)]
    pub root: PathBuf,
    #[serde(skip)]
    pub entry: Option<PathBuf>,
    #[serde(skip)]
    pub isolated_weather: bool,
}
const WEATHER_ID: &str = "com.elgato.weather";
const WEATHER_VERSION: &str = "1.2.1.103";
const WEATHER_BACKEND_SHA256: &str =
    "d4b0b6ec32dee0de0eb9ee0ef33e62f1ce9fa7bc32521b75c6941653be267827";
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
pub(crate) fn read_package(root: PathBuf, id: String) -> Package {
    let root = root.canonicalize().unwrap_or(root);
    let mut package = Package {
        name: id.clone(),
        id,
        version: String::new(),
        actions: vec![],
        error: None,
        resource_only: false,
        development: false,
        root: root.clone(),
        entry: None,
        isolated_weather: false,
    };
    let result = (|| -> Result<(), String> {
        let bytes = std::fs::read(root.join("manifest.json")).map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("Manifest exceeds 1 MiB".into());
        }
        let manifest: Value = match serde_json::from_slice(&bytes) {
            Ok(manifest) => manifest,
            Err(_) if package.id == WEATHER_ID && bytes.starts_with(b"ELGATO") => {
                // This descriptor describes the readable backend and PI, not the protected
                // manifest. Keep the vendor package intact; its SDK accepts a protected
                // manifest itself. Unknown backend versions must be reviewed separately.
                verify_weather_backend(&root)?;
                package.isolated_weather = true;
                weather_descriptor()
            }
            Err(_) => return Err("Protected or invalid manifest".into()),
        };
        package.name = manifest["Name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&package.id)
            .into();
        package.version = manifest["Version"].as_str().unwrap_or("0").into();
        package.development = manifest["DeckWeave"]["DevelopmentOnly"]
            .as_bool()
            .unwrap_or(false);
        // Elgato's icon editor has no backend or actions. It is a resource,
        // not a broken executable plugin. Never launch it as an action backend.
        if manifest["IconEditorPath"].is_string() && manifest.get("Actions").is_none() {
            package.resource_only = true;
            resolve(&root, manifest["IconEditorPath"].as_str().unwrap())?;
            return Ok(());
        }
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
                visible: action["VisibleInActionsList"].as_bool().unwrap_or(true),
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

fn verify_weather_backend(root: &Path) -> Result<(), String> {
    let entry = resolve(root, "bin/plugin.js")?;
    if std::fs::metadata(&entry).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("Elgato Weather backend exceeds the tested size".into());
    }
    let bytes = std::fs::read(entry).map_err(|e| e.to_string())?;
    if format!("{:x}", Sha256::digest(&bytes)) != WEATHER_BACKEND_SHA256 {
        return Err(format!(
            "Elgato Weather backend is not the tested version {WEATHER_VERSION}; compatibility descriptor needs review"
        ));
    }
    Ok(())
}

fn weather_descriptor() -> Value {
    // Identities and capabilities are observable in the installed readable ea/*,
    // en.json, ui/*.html, layouts/weather.json and bin/plugin.js. No vendor code
    // or decrypted manifest is distributed by DeckWeave.
    json!({
        "Name": "Elgato Weather",
        "Version": WEATHER_VERSION,
        "CodePath": "bin/plugin.js",
        "Actions": [
            {"UUID":"com.elgato.weather.weather","Name":"Weather", "Controllers":["Keypad","Encoder"],"PropertyInspectorPath":"ui/weather-info.html", "Encoder":{"layout":"layouts/weather.json"}, "States":[{"Image":"imgs/actions/weather/icons/key.svg"}]},
            {"UUID":"com.elgato.weather.forecast","Name":"Forecast", "Controllers":["Keypad","Encoder"],"PropertyInspectorPath":"ui/weather-info.html", "Encoder":{"layout":"layouts/weather.json"}, "States":[{"Image":"imgs/actions/weather/icons/key-forecast.svg"}]},
            {"UUID":"com.elgato.weather.aqi","Name":"Air Quality", "Controllers":["Keypad","Encoder"],"PropertyInspectorPath":"ui/air-quality.html", "Encoder":{"layout":"layouts/weather.json"}, "States":[{"Image":"imgs/actions/weather/icons/aqi-key.svg"}]}
        ]
    })
}

pub struct RuntimeEnvironment {
    pub home: PathBuf,
    pub roaming: PathBuf,
    pub local: PathBuf,
    pub temp: PathBuf,
}

/// Weather's original backend stores data relative to cwd, APPDATA and homedir.
/// Run an unchanged private copy so it never writes into the Elgato installation.
pub fn prepare_runtime(
    package: &mut Package,
    directory: &Path,
) -> Result<Option<RuntimeEnvironment>, String> {
    if !package.isolated_weather {
        return Ok(None);
    }
    verify_weather_backend(&package.root)?;
    let home = directory
        .join("plugin-runtime")
        .join(WEATHER_ID)
        .join(WEATHER_VERSION)
        .join("home");
    let roaming = home.join("AppData/Roaming");
    let local = home.join("AppData/Local");
    let temp = home.join("Temp");
    let root = roaming.join("Elgato/StreamDeck/Plugins/com.elgato.weather.sdPlugin");
    for path in [&root, &local, &temp] {
        std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    }
    let mut remaining = 16 * 1024 * 1024;
    copy_weather_assets(&package.root, &root, 0, &mut remaining)?;
    std::fs::create_dir_all(root.join("storage")).map_err(|e| e.to_string())?;
    verify_weather_backend(&root)?;
    package.entry = Some(resolve(&root, "bin/plugin.js")?);
    package.root = root;
    Ok(Some(RuntimeEnvironment {
        home,
        roaming,
        local,
        temp,
    }))
}

fn copy_weather_assets(
    source: &Path,
    target: &Path,
    depth: u8,
    remaining: &mut u64,
) -> Result<(), String> {
    if depth > 16 {
        return Err("Elgato Weather asset tree is too deep".into());
    }
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        if depth == 0 && matches!(name.to_str(), Some("logs" | "storage")) {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let destination = target.join(name);
        if kind.is_symlink()
            || std::fs::symlink_metadata(&destination).is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err("Elgato Weather asset symlinks are unsupported".into());
        }
        if kind.is_dir() {
            std::fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
            copy_weather_assets(&entry.path(), &destination, depth + 1, remaining)?;
        } else if kind.is_file() {
            *remaining = remaining
                .checked_sub(entry.metadata().map_err(|e| e.to_string())?.len())
                .ok_or("Elgato Weather assets exceed 16 MiB")?;
            std::fs::copy(entry.path(), destination).map_err(|e| e.to_string())?;
        } else {
            return Err("Elgato Weather has an unsupported asset type".into());
        }
    }
    Ok(())
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
    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("deckweave-weather-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }
    fn remove_fixture(root: &Path) {
        let root = root.canonicalize().unwrap();
        let temporary = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(root.parent(), Some(temporary.as_path()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("deckweave-weather-")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
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
    #[test]
    fn resources_and_hidden_actions_do_not_pollute_action_library() {
        let root = fixture();
        std::fs::write(root.join("icons.html"), "icons").unwrap();
        std::fs::write(
            root.join("manifest.json"),
            r#"{"Name":"Icons","Version":"1","IconEditorPath":"icons.html"}"#,
        )
        .unwrap();
        let resource = read_package(root.clone(), "example.icons".into());
        assert!(resource.resource_only);
        assert!(resource.entry.is_none());
        assert!(resource.error.is_none());
        std::fs::write(root.join("plugin.mjs"), "export {};").unwrap();
        std::fs::write(root.join("manifest.json"), r#"{"Name":"Examples","CodePath":"plugin.mjs","DeckWeave":{"DevelopmentOnly":true},"Actions":[{"UUID":"example.hidden","VisibleInActionsList":false},{"UUID":"example.visible"}]}"#).unwrap();
        let package = read_package(root.clone(), "example.actions".into());
        assert!(package.error.is_none());
        assert!(package.development);
        assert!(!package.actions[0].visible);
        assert!(package.actions[1].visible);
        // Keep descriptors to execute existing assignments.
        assert_eq!(package.actions.len(), 2);
        remove_fixture(&root);
    }
    #[test]
    fn protected_weather_requires_the_tested_backend_and_never_decodes_manifest() {
        let root = fixture();
        std::fs::create_dir(root.join("bin")).unwrap();
        let protected = b"ELGATO\x01\x00\x00unreadable-protected-manifest";
        std::fs::write(root.join("manifest.json"), protected).unwrap();
        std::fs::write(root.join("bin/plugin.js"), b"different backend").unwrap();
        let package = read_package(root.clone(), WEATHER_ID.into());
        assert!(package.actions.is_empty());
        assert!(!package.isolated_weather);
        assert!(package.error.unwrap().contains("not the tested version"));
        assert_eq!(
            std::fs::read(root.join("manifest.json")).unwrap(),
            protected
        );
        let other = read_package(root.clone(), "other.protected.plugin".into());
        assert_eq!(
            other.error.as_deref(),
            Some("Protected or invalid manifest")
        );
        remove_fixture(&root);
    }
    #[test]
    fn weather_descriptor_exposes_only_documented_production_actions() {
        let descriptor = weather_descriptor();
        assert_eq!(descriptor["Version"], WEATHER_VERSION);
        assert_eq!(descriptor["CodePath"], "bin/plugin.js");
        let actions = descriptor["Actions"].as_array().unwrap();
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0]["UUID"], "com.elgato.weather.weather");
        assert_eq!(actions[1]["UUID"], "com.elgato.weather.forecast");
        assert_eq!(actions[2]["UUID"], "com.elgato.weather.aqi");
        for action in actions {
            assert_eq!(action["Controllers"], json!(["Keypad", "Encoder"]));
            assert_eq!(action["Encoder"]["layout"], "layouts/weather.json");
        }
    }
    #[test]
    fn runtime_copy_preserves_vendor_bytes_and_keeps_our_storage_separate() {
        let root = fixture();
        let source = root.join("vendor");
        let target = root.join("runtime");
        std::fs::create_dir_all(source.join("bin")).unwrap();
        std::fs::create_dir_all(source.join("storage")).unwrap();
        std::fs::create_dir_all(source.join("logs")).unwrap();
        std::fs::create_dir_all(target.join("storage")).unwrap();
        std::fs::write(source.join("manifest.json"), b"ELGATO\x01vendor bytes").unwrap();
        std::fs::write(source.join("bin/plugin.js"), b"unchanged executable").unwrap();
        std::fs::write(source.join("storage/private.json"), b"vendor settings").unwrap();
        std::fs::write(source.join("logs/private.log"), b"vendor logs").unwrap();
        std::fs::write(target.join("storage/ours.json"), b"DeckWeave settings").unwrap();
        let mut budget = 4096;
        copy_weather_assets(&source, &target, 0, &mut budget).unwrap();
        assert_eq!(
            std::fs::read(target.join("manifest.json")).unwrap(),
            std::fs::read(source.join("manifest.json")).unwrap()
        );
        assert_eq!(
            std::fs::read(target.join("bin/plugin.js")).unwrap(),
            b"unchanged executable"
        );
        assert!(!target.join("storage/private.json").exists());
        assert!(!target.join("logs").exists());
        assert_eq!(
            std::fs::read(target.join("storage/ours.json")).unwrap(),
            b"DeckWeave settings"
        );
        assert!(copy_weather_assets(&source, &target, 0, &mut 0).is_err());
        remove_fixture(&root);
    }
}
