//! Explicit, bounded plugin acquisition. Downloads are inspected in private staging;
//! only a reviewed token can publish files. No downloaded backend is executed here.
use crate::plugins;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager as _};

const CATALOGUE: &str =
    "https://raw.githubusercontent.com/OpenActionAPI/plugins/main/catalogue.json";
const MAX_ARCHIVE: u64 = 128 * 1024 * 1024;
const MAX_TOTAL: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 32768;

#[derive(Clone, Serialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub repository: String,
}
#[derive(Clone, Serialize)]
pub struct Asset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub version: String,
}
#[derive(Clone, Serialize)]
pub struct Preview {
    pub kind: String,
    pub token: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub actions: usize,
    pub sha256: String,
    pub source: String,
    pub installed_version: Option<String>,
}
struct Pending {
    directory: tempfile::TempDir,
    bundle: PathBuf,
    preview: Preview,
    created: Instant,
}
pub struct Manager {
    pub(crate) directory: PathBuf,
    pending: Mutex<BTreeMap<String, Pending>>,
    catalogue: Mutex<Option<(Instant, Vec<Entry>)>>,
    pub(crate) install: Mutex<()>,
    pub(crate) lifecycle: Arc<crate::plugin_lifecycle::Store>,
}
impl Manager {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            lifecycle: Arc::new(crate::plugin_lifecycle::Store::open(
                directory.join("plugin-lifecycle.json"),
            )),
            directory,
            pending: Mutex::new(BTreeMap::new()),
            catalogue: Mutex::new(None),
            install: Mutex::new(()),
        }
    }
    pub fn catalogue(&self) -> Result<Vec<Entry>, String> {
        if let Some((at, entries)) = &*self.catalogue.lock().map_err(|_| "Catalogue unavailable")?
            && at.elapsed() < Duration::from_secs(300)
        {
            return Ok(entries.clone());
        }
        let bytes = download(CATALOGUE, 2 * 1024 * 1024)?;
        let entries = parse_catalogue(&bytes)?;
        *self.catalogue.lock().map_err(|_| "Catalogue unavailable")? =
            Some((Instant::now(), entries.clone()));
        Ok(entries)
    }
    pub fn assets(&self, id: &str) -> Result<Vec<Asset>, String> {
        let entry = self
            .catalogue()?
            .into_iter()
            .find(|e| e.id == id)
            .ok_or("Plugin is not in the catalogue")?;
        release_assets(&entry)
    }
    pub fn prepare_asset(
        &self,
        id: &str,
        asset_id: u64,
        packages: &[plugins::catalog::Package],
    ) -> Result<Preview, String> {
        let entry = self
            .catalogue()?
            .into_iter()
            .find(|e| e.id == id)
            .ok_or("Plugin is not in the catalogue")?;
        // Re-read the selected release so callers cannot supply arbitrary download URLs.
        let release = release(&entry)?;
        let asset = release["assets"]
            .as_array()
            .and_then(|a| a.iter().find(|a| a["id"].as_u64() == Some(asset_id)))
            .ok_or("Release asset no longer exists")?;
        let name = asset["name"].as_str().ok_or("Missing asset name")?;
        if !archive_name(name) || asset["size"].as_u64().unwrap_or(u64::MAX) > MAX_ARCHIVE {
            return Err("Select a plugin ZIP / streamDeckPlugin archive".into());
        }
        let url = asset["browser_download_url"]
            .as_str()
            .ok_or("Missing download URL")?;
        let parsed = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
        if parsed.scheme() != "https"
            || parsed.host_str() != Some("github.com")
            || !parsed.path().starts_with(&format!(
                "/{}/releases/download/",
                repository(&entry.repository)?
            ))
        {
            return Err("Unexpected release download URL".into());
        }
        let bytes = download(url, MAX_ARCHIVE)?;
        if let Some(digest) = asset["digest"]
            .as_str()
            .and_then(|d| d.strip_prefix("sha256:"))
            && digest != format!("{:x}", Sha256::digest(&bytes))
        {
            return Err("Release checksum mismatch".into());
        }
        self.prepare(
            &bytes,
            format!("{} · {}", entry.repository, name),
            Some(id),
            packages,
        )
    }
    pub fn prepare_file(
        &self,
        path: &Path,
        packages: &[plugins::catalog::Package],
    ) -> Result<Preview, String> {
        if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_ARCHIVE {
            return Err("Archive exceeds 128 MiB".into());
        }
        let mut bytes = vec![];
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_ARCHIVE + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        self.prepare(
            &bytes,
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            None,
            packages,
        )
    }
    fn prepare(
        &self,
        bytes: &[u8],
        source: String,
        expected: Option<&str>,
        packages: &[plugins::catalog::Package],
    ) -> Result<Preview, String> {
        if bytes.len() as u64 > MAX_ARCHIVE {
            return Err("Archive exceeds 128 MiB".into());
        }
        let staging = self.directory.join("plugin-staging");
        fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        let directory = tempfile::Builder::new()
            .prefix("incoming-")
            .tempdir_in(&staging)
            .map_err(|e| e.to_string())?;
        let (bundle, id) = extract(bytes, directory.path())?;
        if expected.is_some_and(|expected| expected != id) {
            return Err("Archive plugin identity differs from catalogue entry".into());
        }
        if packages
            .iter()
            .any(|p| p.id != id && p.id.eq_ignore_ascii_case(&id))
        {
            return Err("Plugin identity conflicts with an installed package's letter case".into());
        }
        let (kind, name, version, actions) = if bundle.extension().is_some_and(|e| e == "dwDevice")
        {
            let manifest = crate::device_providers::inspect_package(&bundle)?;
            if manifest.id != id {
                return Err("Device manifest identity mismatch".into());
            }
            ("device".to_owned(), manifest.name, manifest.version, 0)
        } else {
            let package = plugins::catalog::read_package(bundle.clone(), id.clone());
            if let Some(error) = package.error {
                return Err(format!("Unsupported plugin: {error}"));
            }
            if package.resource_only || package.actions.is_empty() {
                return Err("This archive does not contain a supported action plugin".into());
            }
            (
                "action".to_owned(),
                package.name,
                package.version,
                package.actions.len(),
            )
        };
        let installed_version = if kind == "device" {
            let roots = [
                self.directory.join("device-plugins"),
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../device-plugins"),
            ];
            let mut installed = None;
            for root in roots {
                if let Ok(existing) =
                    crate::device_providers::inspect_package(&root.join(format!("{id}.dwDevice")))
                {
                    if existing.id != id {
                        return Err(
                            "Device identity conflicts with an installed package's letter case"
                                .into(),
                        );
                    }
                    installed = Some(existing.version);
                    break;
                }
            }
            installed
        } else {
            packages
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.version.clone())
        };
        let token = uuid::Uuid::new_v4().simple().to_string();
        let preview = Preview {
            kind,
            token: token.clone(),
            id: id.clone(),
            name,
            version,
            actions,
            sha256: format!("{:x}", Sha256::digest(bytes)),
            source,
            installed_version,
        };
        let mut pending = self.pending.lock().map_err(|_| "Installer unavailable")?;
        pending.retain(|_, p| p.created.elapsed() < Duration::from_secs(600));
        if pending.len() >= 4 {
            return Err("Finish or cancel an installation before preparing another".into());
        }
        pending.insert(
            token,
            Pending {
                directory,
                bundle,
                preview: preview.clone(),
                created: Instant::now(),
            },
        );
        Ok(preview)
    }
    pub fn discard(&self, token: &str) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(token);
        }
    }
    pub fn commit(
        &self,
        token: &str,
        host: &plugins::Host,
        devices: &crate::device_providers::Host,
    ) -> Result<Preview, String> {
        let _lock = self.install.lock().map_err(|_| "Installer unavailable")?;
        // Do not replace an installation while its durable removal intent is unreadable.
        self.lifecycle.records()?;
        let mut pending = self.take_pending(token)?;
        let device = pending.preview.kind == "device";
        let stopped = if device {
            devices.pause_package(&pending.preview.id)
        } else {
            host.pause(&pending.preview.id)
        };
        if let Err(error) = stopped {
            if device {
                devices.resume_package(&pending.preview.id);
            } else {
                let _ = host.reload(&pending.preview.id);
            }
            return Err(error);
        }
        let result = self.publish(&mut pending);
        if result.is_ok() {
            // A reviewed explicit installation is the only install path that clears removal.
            let restored = self.lifecycle.reconnect(&if device {
                format!("device:{}", pending.preview.id)
            } else {
                pending.preview.id.clone()
            });
            if let Err(error) = restored {
                pending.directory.disable_cleanup(true);
                return Err(format!(
                    "Package published but activation was not recorded: {error}. Previous files retained at {}",
                    pending.directory.path().join("previous").display()
                ));
            }
        }
        let resumed = if device {
            devices.resume_package(&pending.preview.id);
            Ok(())
        } else {
            host.reload(&pending.preview.id)
        };
        result?;
        resumed?;
        Ok(pending.preview)
    }
    fn take_pending(&self, token: &str) -> Result<Pending, String> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| "Installer unavailable")?
            .remove(token)
            .ok_or("Installation preview expired; select the archive again")?;
        if pending.created.elapsed() > Duration::from_secs(600) {
            return Err("Installation preview expired".into());
        }
        Ok(pending)
    }
    fn publish(&self, pending: &mut Pending) -> Result<(), String> {
        let device = pending.preview.kind == "device";
        let root = self
            .directory
            .join(if device { "device-plugins" } else { "plugins" });
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let destination = root.join(format!(
            "{}.{}",
            pending.preview.id,
            if device { "dwDevice" } else { "sdPlugin" }
        ));
        if fs::symlink_metadata(&destination)
            .is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir())
        {
            return Err("Existing plugin is not a regular managed directory".into());
        }
        let backup = pending.directory.path().join("previous");
        let existed = destination.exists();
        if existed {
            fs::rename(&destination, &backup)
                .map_err(|e| format!("Cannot stop/update existing plugin: {e}"))?;
        }
        if let Err(error) = fs::rename(&pending.bundle, &destination) {
            if existed && let Err(rollback) = fs::rename(&backup, &destination) {
                pending.directory.disable_cleanup(true);
                return Err(format!(
                    "Install failed: {error}; restore failed: {rollback}. Previous plugin preserved at {}",
                    backup.display()
                ));
            }
            return Err(error.to_string());
        }
        Ok(())
    }
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(45))
        .user_agent("DeckWeave/0.1 plugin-manager")
        .build()
        .map_err(|e| e.to_string())
}
fn download(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let response = client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| format!("Download failed (GitHub rate limit or unavailable resource): {e}"))?;
    if response.content_length().is_some_and(|n| n > limit) {
        return Err("Download exceeds size limit".into());
    }
    let mut bytes = vec![];
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Download exceeds size limit".into());
    }
    Ok(bytes)
}
pub(crate) fn valid_id(id: &str) -> bool {
    id.len() <= 200
        && id.contains('.')
        && id.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
}
fn repository(url: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
    let path = url.path().trim_matches('/');
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || path.split('/').count() != 2
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("Unsupported repository URL".into());
    }
    Ok(path.into())
}
fn parse_catalogue(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let mut entries: Vec<_> = value
        .as_object()
        .ok_or("Invalid OpenDeck catalogue")?
        .iter()
        .filter_map(|(id, v)| {
            let repo = v["repository"].as_str()?;
            if !valid_id(id) || repository(repo).is_err() {
                return None;
            }
            Some(Entry {
                id: id.clone(),
                name: v["name"].as_str().unwrap_or(id).chars().take(160).collect(),
                author: v["author"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(160)
                    .collect(),
                description: v["description"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(2000)
                    .collect(),
                repository: repo.into(),
            })
        })
        .take(4096)
        .collect();
    entries.sort_by_key(|e| e.name.to_lowercase());
    Ok(entries)
}
fn release(entry: &Entry) -> Result<Value, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        repository(&entry.repository)?
    );
    serde_json::from_slice(&download(&url, 4 * 1024 * 1024)?).map_err(|e| e.to_string())
}
fn archive_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".zip") || name.ends_with(".streamdeckplugin")
}
fn release_assets(entry: &Entry) -> Result<Vec<Asset>, String> {
    let release = release(entry)?;
    let version = release["tag_name"].as_str().unwrap_or("");
    Ok(release["assets"]
        .as_array()
        .ok_or("Release has no assets")?
        .iter()
        .filter_map(|a| {
            let name = a["name"].as_str()?;
            let size = a["size"].as_u64()?;
            (archive_name(name) && size <= MAX_ARCHIVE).then(|| Asset {
                id: a["id"].as_u64().unwrap_or(0),
                name: name.into(),
                size,
                version: version.into(),
            })
        })
        .take(128)
        .collect())
}

fn safe_path(name: &str) -> Result<PathBuf, String> {
    if name.len() > 1024 || name.starts_with(['/', '\\']) {
        return Err("Unsafe archive path".into());
    }
    let normal = name.replace('\\', "/");
    let mut path = PathBuf::new();
    for part in normal.trim_end_matches('/').split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || part.chars().any(|c| c < ' ' || "<>:\"|?*".contains(c))
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err("Unsafe archive path".into());
        }
        path.push(part);
    }
    Ok(path)
}
fn extract(bytes: &[u8], root: &Path) -> Result<(PathBuf, String), String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "Expected a ZIP / unprotected streamDeckPlugin archive")?;
    if zip.len() > MAX_ENTRIES {
        return Err("Too many archive entries".into());
    }
    let mut paths = vec![];
    let mut names = BTreeSet::new();
    let mut candidates = vec![];
    let mut total = 0u64;
    for i in 0..zip.len() {
        let file = zip.by_index(i).map_err(|e| e.to_string())?;
        let path = safe_path(file.name())?;
        if !names.insert(path.to_string_lossy().to_lowercase()) {
            return Err("Duplicate archive paths".into());
        }
        if file.unix_mode().is_some_and(|m| {
            m & 0o170000 != 0 && m & 0o170000 != 0o100000 && m & 0o170000 != 0o040000
        }) {
            return Err("Archive links and special files are unsupported".into());
        }
        total = total
            .checked_add(file.size())
            .ok_or("Archive size overflow")?;
        if total > MAX_TOTAL || file.size() > MAX_ARCHIVE {
            return Err("Unpacked archive exceeds limits".into());
        }
        if path
            .file_name()
            .is_some_and(|s| s == "manifest.json" || s == "deckweave.device.json")
        {
            candidates.push(path.clone());
        }
        paths.push(path);
    }
    let candidates: Vec<_> = candidates
        .into_iter()
        .filter(|p| {
            let parent = p.parent().unwrap_or(Path::new(""));
            parent.as_os_str().is_empty()
                || parent
                    .extension()
                    .is_some_and(|e| e == "sdPlugin" || e == "dwDevice")
        })
        .collect();
    if candidates.len() != 1 {
        return Err("Archive must contain exactly one action or device bundle".into());
    }
    let manifest_path = &candidates[0];
    let prefix = manifest_path.parent().unwrap_or(Path::new(""));
    let device = manifest_path
        .file_name()
        .is_some_and(|s| s == "deckweave.device.json");
    let suffix = if device { "dwDevice" } else { "sdPlugin" };
    if !prefix.as_os_str().is_empty() && prefix.extension().is_none_or(|e| e != suffix) {
        return Err("Bundle type differs from manifest type".into());
    }
    let manifest_index = paths
        .iter()
        .position(|p| p == manifest_path)
        .ok_or("Missing manifest")?;
    let manifest: Value = {
        let file = zip.by_index(manifest_index).map_err(|e| e.to_string())?;
        if file.size() > 1024 * 1024 {
            return Err("Manifest exceeds 1 MiB".into());
        }
        serde_json::from_reader(file)
            .map_err(|_| "Protected or invalid plugin manifest; obtain an open release archive")?
    };
    let id = prefix
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|_| prefix.extension().is_some_and(|e| e == suffix))
        .or(manifest[if device { "id" } else { "UUID" }].as_str())
        .ok_or("Bare plugin manifest needs UUID")?
        .to_string();
    if !valid_id(&id)
        || manifest[if device { "id" } else { "UUID" }]
            .as_str()
            .is_some_and(|v| v != id)
    {
        return Err("Invalid or conflicting plugin identity".into());
    }
    let platform = if cfg!(target_os = "macos") {
        "mac"
    } else {
        std::env::consts::OS
    };
    if let Some(platforms) = manifest["OS"].as_array()
        && !platforms.is_empty()
        && !platforms
            .iter()
            .any(|o| o["Platform"].as_str() == Some(platform))
    {
        return Err("Plugin does not support this operating system".into());
    }
    let bundle = root.join(format!("{id}.{suffix}"));
    fs::create_dir(&bundle).map_err(|e| e.to_string())?;
    for (i, path) in paths.iter().enumerate() {
        let Ok(relative) = path.strip_prefix(prefix) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        let target = bundle.join(relative);
        if file.is_dir() {
            fs::create_dir_all(target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let expected = file.size();
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| e.to_string())?;
        let written = std::io::copy(&mut file.by_ref().take(expected + 1), &mut output)
            .map_err(|e| e.to_string())?;
        if written != expected {
            return Err("Archive entry size mismatch".into());
        }
        output.flush().map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                &target,
                fs::Permissions::from_mode(file.unix_mode().unwrap_or(0o644) & 0o755 | 0o600),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    if let Some(required) = manifest["DeckWeave"]["RequiredFiles"].as_array() {
        for file in required {
            plugins::catalog::resolve(&bundle, file.as_str().ok_or("Invalid required file")?)?;
        }
    }
    Ok((bundle, id))
}

fn allowed(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Manage plugins from application settings".into())
    }
}
fn packages(app: &tauri::AppHandle) -> Result<Vec<plugins::catalog::Package>, String> {
    match app.state::<crate::Application>().inner() {
        crate::Application::Ready { plugins, .. } => Ok(plugins.packages()),
        crate::Application::Failed(e) => Err(e.to_string()),
    }
}
#[tauri::command]
pub async fn plugin_catalogue(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Vec<Entry>, String> {
    allowed(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.try_state::<Manager>()
            .ok_or("Installer unavailable")?
            .catalogue()
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn plugin_release_assets(
    id: String,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Vec<Asset>, String> {
    allowed(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.try_state::<Manager>()
            .ok_or("Installer unavailable")?
            .assets(&id)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn plugin_pick_archive(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Option<Preview>, String> {
    allowed(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let installed = packages(&app)?;
        let russian = crate::locked(&app.state::<crate::Application>())
            .map_err(|e| e.to_string())?
            .snapshot()
            .configuration
            .locale
            == deckweave_core::Locale::Ru;
        let Some(path) = rfd::FileDialog::new()
            .set_parent(&window)
            .set_title(if russian {
                "DeckWeave — Установить плагин"
            } else {
                "DeckWeave — Install plugin"
            })
            .add_filter(
                if russian {
                    "Архивы плагинов"
                } else {
                    "Plugin archives"
                },
                &["zip", "streamDeckPlugin"],
            )
            .pick_file()
        else {
            return Ok(None);
        };
        app.try_state::<Manager>()
            .ok_or("Installer unavailable")?
            .prepare_file(&path, &installed)
            .map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn plugin_prepare_asset(
    id: String,
    asset_id: u64,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Preview, String> {
    allowed(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.try_state::<Manager>()
            .ok_or("Installer unavailable")?
            .prepare_asset(&id, asset_id, &packages(&app)?)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn plugin_commit(
    token: String,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Preview, String> {
    allowed(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<crate::Application>();
        let crate::Application::Ready {
            plugins,
            device_providers,
            ..
        } = state.inner()
        else {
            return Err("Plugin host unavailable".into());
        };
        let preview = app
            .try_state::<Manager>()
            .ok_or("Installer unavailable")?
            .commit(&token, plugins, device_providers)?;
        let _ = app.emit("deckweave:plugins", ());
        Ok(preview)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn plugin_discard(
    token: String,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    allowed(&window)?;
    if let Some(manager) = app.try_state::<Manager>() {
        manager.discard(&token);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use zip::write::SimpleFileOptions;

    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in files {
            writer
                .start_file(
                    *name,
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    fn manifest(version: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({"UUID":"dev.example.test","Name":"Installer test","Version":version,"CodePath":"backend.js","PropertyInspectorPath":"settings.html","Actions":[{"UUID":"dev.example.test.action","Name":"Test","Controllers":["Keypad"]}],"DeckWeave":{"RequiredFiles":["backend.js","settings.html"]}})).unwrap()
    }
    fn bundle(version: &str) -> Vec<u8> {
        archive(&[
            (
                "dev.example.test.sdPlugin/manifest.json",
                &manifest(version),
            ),
            (
                "dev.example.test.sdPlugin/backend.js",
                b"backend-test-sentinel",
            ),
            ("dev.example.test.sdPlugin/settings.html", b"<p>Test</p>"),
        ])
    }
    #[test]
    fn device_archives_use_the_same_preview_but_separate_owned_installation() {
        let root = tempfile::tempdir().unwrap();
        let manager = Manager::new(root.path().into());
        let manifest=serde_json::to_vec(&json!({"kind":"device","protocol":1,"id":"dev.example.keyboard","name":"Keyboard fixture","version":"1.0.0","runtime":"node","entry":"provider.mjs","platforms":["windows","macos","linux"],"models":["dev.example.keyboard.model"]})).unwrap();
        let bytes = archive(&[
            (
                "dev.example.keyboard.dwDevice/deckweave.device.json",
                &manifest,
            ),
            (
                "dev.example.keyboard.dwDevice/provider.mjs",
                b"throw Error('preview must never execute');",
            ),
        ]);
        let preview = manager
            .prepare(&bytes, "device.zip".into(), None, &[])
            .unwrap();
        assert_eq!(preview.kind, "device");
        assert_eq!(preview.actions, 0);
        assert!(!root.path().join("device-plugins").exists());
        let mut pending = manager.take_pending(&preview.token).unwrap();
        manager.publish(&mut pending).unwrap();
        assert!(!root.path().join("plugins").exists());
        assert!(
            root.path()
                .join("device-plugins/dev.example.keyboard.dwDevice/provider.mjs")
                .is_file()
        );
        let next = manager
            .prepare(&bytes, "device.zip".into(), None, &[])
            .unwrap();
        assert_eq!(next.installed_version.as_deref(), Some("1.0.0"));
        let mixed = archive(&[
            (
                "dev.example.keyboard.dwDevice/deckweave.device.json",
                &manifest,
            ),
            (
                "dev.example.test.sdPlugin/manifest.json",
                &super::tests::manifest("1"),
            ),
        ]);
        assert!(
            manager
                .prepare(&mixed, "mixed.zip".into(), None, &[])
                .is_err()
        );
    }
    #[test]
    fn preview_cancel_single_use_and_expiry_never_publish_a_bundle() {
        let root = tempfile::tempdir().unwrap();
        let manager = Manager::new(root.path().into());
        let bytes = bundle("1");
        let preview = manager
            .prepare(&bytes, "local.zip".into(), None, &[])
            .unwrap();
        assert_eq!(preview.id, "dev.example.test");
        assert_eq!(preview.actions, 1);
        assert_eq!(preview.sha256, format!("{:x}", Sha256::digest(&bytes)));
        assert!(!root.path().join("plugins").exists());
        let stage = manager.pending.lock().unwrap()[&preview.token]
            .directory
            .path()
            .to_path_buf();
        manager.discard(&preview.token);
        assert!(!stage.exists());
        assert!(manager.take_pending(&preview.token).is_err());
        let preview = manager
            .prepare(&bytes, "local.zip".into(), None, &[])
            .unwrap();
        let pending = manager.take_pending(&preview.token).unwrap();
        assert!(manager.take_pending(&preview.token).is_err());
        drop(pending);
        let preview = manager
            .prepare(&bytes, "local.zip".into(), None, &[])
            .unwrap();
        manager
            .pending
            .lock()
            .unwrap()
            .get_mut(&preview.token)
            .unwrap()
            .created = Instant::now() - Duration::from_secs(601);
        assert!(manager.take_pending(&preview.token).is_err());
        assert!(!root.path().join("plugins").exists());
    }
    #[test]
    fn publish_installs_and_updates_without_touching_configuration_or_vendor_packages() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("deckweave.sqlite3"), b"configuration").unwrap();
        let manager = Manager::new(root.path().into());
        let preview = manager
            .prepare(&bundle("1"), "local.zip".into(), None, &[])
            .unwrap();
        let mut pending = manager.take_pending(&preview.token).unwrap();
        manager.publish(&mut pending).unwrap();
        drop(pending);
        let installed = plugins::catalog::scan(&[root.path().join("plugins")]);
        assert_eq!(installed[0].version, "1");
        let preview = manager
            .prepare(&bundle("2"), "update.zip".into(), None, &installed)
            .unwrap();
        assert_eq!(preview.installed_version.as_deref(), Some("1"));
        assert_eq!(
            plugins::catalog::scan(&[root.path().join("plugins")])[0].version,
            "1"
        );
        let mut pending = manager.take_pending(&preview.token).unwrap();
        manager.publish(&mut pending).unwrap();
        let stage = pending.directory.path().to_path_buf();
        assert!(stage.join("previous").exists());
        drop(pending);
        assert!(!stage.exists());
        assert_eq!(
            plugins::catalog::scan(&[root.path().join("plugins")])[0].version,
            "2"
        );
        assert_eq!(
            fs::read(root.path().join("deckweave.sqlite3")).unwrap(),
            b"configuration"
        );
    }
    #[test]
    fn failed_publish_restores_previous_bundle() {
        let root = tempfile::tempdir().unwrap();
        let manager = Manager::new(root.path().into());
        let preview = manager
            .prepare(&bundle("1"), "local.zip".into(), None, &[])
            .unwrap();
        let mut pending = manager.take_pending(&preview.token).unwrap();
        manager.publish(&mut pending).unwrap();
        drop(pending);
        let preview = manager
            .prepare(&bundle("2"), "update.zip".into(), None, &[])
            .unwrap();
        let mut pending = manager.take_pending(&preview.token).unwrap();
        fs::rename(
            &pending.bundle,
            pending
                .directory
                .path()
                .join("moved-to-force-publish-failure"),
        )
        .unwrap();
        assert!(manager.publish(&mut pending).is_err());
        assert_eq!(
            plugins::catalog::scan(&[root.path().join("plugins")])[0].version,
            "1"
        );
        assert!(!pending.directory.path().join("previous").exists());
    }
    #[test]
    fn rejects_paths_that_could_escape_or_alias_on_windows() {
        for path in [
            "../escape",
            "a/../../escape",
            "/absolute",
            "\\absolute",
            "C:/escape",
            "a\\..\\escape",
            "a:stream",
            "AUX.txt",
            "x/COM1.exe",
            "nul",
            "bad. /f",
            "a//b",
            "a/./b",
            "tail.",
            "x\0bad",
        ] {
            assert!(safe_path(path).is_err(), "{path:?}");
        }
        assert_eq!(
            safe_path("dev.example.test.sdPlugin\\bin\\main.exe").unwrap(),
            PathBuf::from("dev.example.test.sdPlugin/bin/main.exe")
        );
        let root = tempfile::tempdir().unwrap();
        let bytes = archive(&[
            ("../escaped", b"no"),
            ("dev.example.test.sdPlugin/manifest.json", &manifest("1")),
        ]);
        assert!(extract(&bytes, root.path()).is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[test]
    fn rejects_links_case_collisions_multiple_bundles_and_protected_manifests() {
        let root = tempfile::tempdir().unwrap();
        for files in [
            vec![
                ("dev.example.test.sdPlugin/manifest.json", manifest("1")),
                ("dev.example.test.sdPlugin/MANIFEST.JSON", manifest("1")),
            ],
            vec![
                ("a.sdPlugin/manifest.json", manifest("1")),
                ("b.sdPlugin/manifest.json", manifest("1")),
            ],
            vec![(
                "dev.example.test.sdPlugin/manifest.json",
                b"ELGATO protected".to_vec(),
            )],
        ] {
            let files: Vec<_> = files
                .iter()
                .map(|(name, data)| (*name, data.as_slice()))
                .collect();
            assert!(extract(&archive(&files), root.path()).is_err());
        }
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.add_symlink(
            "dev.example.test.sdPlugin/link",
            "../../outside",
            SimpleFileOptions::default(),
        )
        .unwrap();
        assert!(extract(&zip.finish().unwrap().into_inner(), root.path()).is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[test]
    fn unsupported_identity_platform_or_missing_files_do_not_enter_the_install_queue() {
        let root = tempfile::tempdir().unwrap();
        let manager = Manager::new(root.path().into());
        assert!(
            manager
                .prepare(
                    &bundle("1"),
                    "wrong.zip".into(),
                    Some("dev.other.plugin"),
                    &[]
                )
                .is_err()
        );
        let missing = archive(&[("dev.example.test.sdPlugin/manifest.json", &manifest("1"))]);
        assert!(
            manager
                .prepare(&missing, "missing.zip".into(), None, &[])
                .is_err()
        );
        let mut data: Value = serde_json::from_slice(&manifest("1")).unwrap();
        data["UUID"] = json!("dev.different.plugin");
        let conflicting = archive(&[(
            "dev.example.test.sdPlugin/manifest.json",
            &serde_json::to_vec(&data).unwrap(),
        )]);
        assert!(
            manager
                .prepare(&conflicting, "id.zip".into(), None, &[])
                .is_err()
        );
        data["UUID"] = json!("dev.example.test");
        data["OS"] = json!([{"Platform":"not-a-real-os"}]);
        let wrong_os = archive(&[(
            "dev.example.test.sdPlugin/manifest.json",
            &serde_json::to_vec(&data).unwrap(),
        )]);
        assert!(
            manager
                .prepare(&wrong_os, "os.zip".into(), None, &[])
                .is_err()
        );
        assert!(manager.pending.lock().unwrap().is_empty());
        assert_eq!(
            fs::read_dir(root.path().join("plugin-staging"))
                .unwrap()
                .count(),
            0
        );
    }
    #[test]
    fn catalogue_accepts_only_expected_github_repositories() {
        let value = json!({"dev.good.plugin":{"name":"Good","repository":"https://github.com/owner/repo"},"dev.bad.plugin":{"repository":"https://example.org/owner/repo"},"../bad":{"repository":"https://github.com/owner/repo"}});
        let entries = parse_catalogue(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "dev.good.plugin");
        for repo in [
            "http://github.com/o/r",
            "https://github.com/o/r/tree/main",
            "https://user:password@github.com/o/r",
            "https://github.com/o/r?x=y",
            "https://github.com/o/r#x",
            "https://github.com.evil.example/o/r",
        ] {
            assert!(repository(repo).is_err(), "{repo}");
        }
    }
    #[test]
    #[ignore = "Explicit opt-in: downloads/inspects a public Counter archive; never installs or runs it"]
    fn live_catalogue_and_counter_archive_can_be_inspected() {
        let root = tempfile::tempdir().unwrap();
        let manager = Manager::new(root.path().into());
        let entries = manager.catalogue().unwrap();
        assert!(entries.len() > 20);
        let entry = entries
            .iter()
            .find(|p| p.id == "me.amankhanna.oacounter")
            .unwrap();
        let assets = release_assets(entry).unwrap();
        eprintln!(
            "Counter release files: {}",
            serde_json::to_string(&assets).unwrap()
        );
        let asset = assets
            .iter()
            .find(|a| {
                let name = a.name.to_ascii_lowercase();
                (name.contains("win") && !name.contains("darwin")) || assets.len() == 1
            })
            .expect("No Windows release asset");
        let preview = manager.prepare_asset(&entry.id, asset.id, &[]).unwrap();
        assert_eq!(preview.id, entry.id);
        assert!(preview.actions > 0);
        assert!(!root.path().join("plugins").exists());
        manager.discard(&preview.token);
    }
}
