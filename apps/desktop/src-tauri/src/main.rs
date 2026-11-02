#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deckweave_core::{Command, Core, Error, Snapshot};
mod device_providers;
mod engine;
mod extension_management;
mod hardware;
mod key_lighting;
mod keyboard_assignments;
mod plugin_lifecycle;
mod plugin_manager;
mod plugins;
mod shell;
#[cfg(debug_assertions)]
mod smoke;
mod touch;
use hardware::{Hardware, LiveSnapshot};
use std::sync::{Arc, Mutex};
use tauri::{
    Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

enum Application {
    Ready {
        core: Arc<Mutex<Core>>,
        hardware: Hardware,
        plugins: plugins::Host,
        engine: engine::Engine,
        device_providers: device_providers::Host,
    },
    Failed(Error),
}

#[derive(serde::Serialize)]
struct AppSnapshot {
    #[serde(flatten)]
    hardware: LiveSnapshot,
    provider_inventory: device_providers::ProviderSnapshot,
}

fn locked(state: &Application) -> Result<std::sync::MutexGuard<'_, Core>, Error> {
    match state {
        Application::Ready { core, .. } => core
            .lock()
            .map_err(|_| Error::new("runtime", "Runtime state is unavailable")),
        Application::Failed(error) => Err(error.clone()),
    }
}

#[tauri::command]
async fn get_snapshot(window: WebviewWindow, app: tauri::AppHandle) -> Result<AppSnapshot, Error> {
    if window.label() != "main" && !window.label().starts_with("device-") {
        return Err(Error::new(
            "window",
            "This window cannot read the workspace",
        ));
    }
    blocking_application(app, |state| {
        let snapshot = locked(state)?.snapshot();
        live(state, snapshot)
    })
    .await
}

// Synchronous commands run on Tauri's window/event thread. Even a cached read can
// wait behind a durable Core transaction; keep that wait away from the WebView.
async fn blocking_application<T, F>(app: tauri::AppHandle, operation: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce(&Application) -> Result<T, Error> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Application>();
        operation(state.inner())
    })
    .await
    .map_err(|error| Error::new("runtime", error.to_string()))?
}

fn live(state: &Application, snapshot: Snapshot) -> Result<AppSnapshot, Error> {
    match state {
        Application::Ready {
            hardware,
            device_providers,
            ..
        } => Ok(AppSnapshot {
            hardware: hardware.snapshot(snapshot),
            provider_inventory: device_providers.snapshot(),
        }),
        Application::Failed(error) => Err(error.clone()),
    }
}

#[tauri::command]
async fn dispatch(
    command: Command,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<AppSnapshot, Error> {
    let label = window.label().to_owned();
    if !permitted(&label, &command) {
        return Err(Error::new(
            "window",
            "This window cannot change the requested device",
        ));
    }
    let worker_app = app.clone();
    blocking_application(app, move |state| {
        dispatch_blocking(command, &label, &worker_app, state)
    })
    .await
}

fn dispatch_blocking(
    command: Command,
    label: &str,
    app: &tauri::AppHandle,
    state: &Application,
) -> Result<AppSnapshot, Error> {
    let closing = if let Command::RemoveDevice { device_id, .. } = &command {
        let config = locked(state)?.snapshot().configuration;
        let mut contexts = std::collections::BTreeSet::new();
        fn collect(
            binding: &deckweave_core::Binding,
            out: &mut std::collections::BTreeSet<String>,
        ) {
            out.insert(binding.id.clone());
            for child in binding.children() {
                collect(child, out);
            }
        }
        if let Some(device) = config.devices.iter().find(|d| d.id == *device_id) {
            for binding in device.pinned.values().chain(
                std::iter::once(&device.profile)
                    .chain(&device.profiles)
                    .flat_map(|p| &p.pages)
                    .flat_map(|p| p.bindings.values()),
            ) {
                collect(binding, &mut contexts);
            }
            for shortcut in std::iter::once(&device.profile)
                .chain(&device.profiles)
                .flat_map(|p| p.fn_bindings.values())
            {
                collect(&shortcut.binding, &mut contexts);
            }
        }
        Some((device_id.clone(), contexts))
    } else {
        None
    };
    let appearance_change = matches!(command, Command::SetPreferences { .. });
    let snapshot = locked(state)?.dispatch(command, label)?;
    if appearance_change {
        let theme = window_theme(snapshot.configuration.preferences.theme);
        for window in app.webview_windows().values() {
            let _ = window.set_theme(Some(theme));
        }
    }
    if let Some((id, contexts)) = closing {
        if let Some(window) = app.get_webview_window(&format!("device-{id}")) {
            let _ = window.close();
        }
        for context in contexts {
            if let Some(window) = app.get_webview_window(&format!("pi-{context}")) {
                let _ = window.close();
            }
        }
    }
    if let Some(shell) = app.try_state::<shell::Shell>() {
        shell.locale(snapshot.configuration.locale);
    }
    if let Application::Ready { hardware, .. } = state {
        hardware.changed();
    }
    let _ = app.emit("deckweave:changed", snapshot.event_sequence);
    live(state, snapshot)
}

fn permitted(label: &str, command: &Command) -> bool {
    if label == "main" {
        matches!(
            command,
            Command::AddDevice { .. }
                | Command::SetLocale { .. }
                | Command::SetPreferences { .. }
                | Command::DevicePreferences { .. }
                | Command::RenameDevice { .. }
                | Command::RemoveDevice { .. }
                | Command::RestoreDevice { .. }
                | Command::ForgetDevice { .. }
                | Command::AllowHardware { .. }
                | Command::SetDeviceHardwareSettings { .. }
                | Command::SetProviderSettings { .. }
                | Command::SetFnBinding { .. }
                | Command::TestFnBinding { .. }
                | Command::SaveLightingPreset { .. }
                | Command::DeleteLightingPreset { .. }
                | Command::ProfileLightingPreset { .. }
                | Command::SelectLightingPreset { .. }
                | Command::ReleaseLightingEdit { .. }
                | Command::ReturnToManualLighting { .. }
        )
    } else if let Some(id) = label.strip_prefix("device-") {
        if matches!(
            command,
            Command::RemoveDevice { .. }
                | Command::RestoreDevice { .. }
                | Command::ForgetDevice { .. }
                | Command::AllowHardware { .. }
        ) {
            return false;
        }
        matches!(
            command,
            Command::SetLocale { .. } | Command::SetPreferences { .. }
        ) || command.device_id() == Some(id)
    } else {
        false
    }
}

#[tauri::command]
async fn device_provider_status(
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<device_providers::ProviderSnapshot, Error> {
    if window.label() != "main" && !window.label().starts_with("device-") {
        return Err(Error::new("window", "Not an editor window"));
    }
    blocking_application(app, |state| match state {
        Application::Ready {
            device_providers, ..
        } => Ok(device_providers.snapshot()),
        Application::Failed(e) => Err(e.clone()),
    })
    .await
}
#[tauri::command]
async fn device_provider_session(
    device_id: String,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Option<device_providers::SessionStatus>, Error> {
    provider_window(&window, &device_id).map_err(|error| Error::new("window", error))?;
    blocking_application(app, move |state| match state {
        Application::Ready {
            device_providers, ..
        } => Ok(device_providers.session(&device_id)),
        Application::Failed(error) => Err(error.clone()),
    })
    .await
}
#[tauri::command]
async fn add_provider_device(
    provider_id: String,
    instance_id: String,
    device_id: Option<String>,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<String, Error> {
    if window.label() != "main" {
        return Err(Error::new(
            "window",
            "Add devices from application settings",
        ));
    }
    blocking_application(app, move |state| match state {
        Application::Ready {
            device_providers, ..
        } => match device_id {
            Some(id) => device_providers.rebind(&id, &provider_id, &instance_id),
            None => device_providers.add(&provider_id, &instance_id),
        }
        .map_err(|e| Error::new("provider", e)),
        Application::Failed(e) => Err(e.clone()),
    })
    .await
}
#[tauri::command]
async fn refresh_provider_state(
    device_id: String,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), Error> {
    if window.label() != "main" && window.label() != format!("device-{device_id}") {
        return Err(Error::new("window", "Device belongs to another editor"));
    }
    blocking_application(app, move |state| match state {
        Application::Ready {
            device_providers, ..
        } => device_providers
            .refresh(&device_id)
            .map_err(|e| Error::new("provider", e)),
        Application::Failed(e) => Err(e.clone()),
    })
    .await
}
#[tauri::command]
async fn set_provider_observation(
    device_id: String,
    enabled: bool,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<device_providers::ObservationSnapshot, String> {
    provider_window(&window, &device_id)?;
    let owner = window.label().to_owned();
    blocking_application(app, move |state| match state {
        Application::Ready {
            device_providers, ..
        } => device_providers
            .set_observation(&device_id, &owner, enabled)
            .map_err(|error| Error::new("provider", error)),
        Application::Failed(error) => Err(error.clone()),
    })
    .await
    .map_err(|error| error.to_string())
}
#[tauri::command]
fn provider_observation(
    device_id: String,
    window: WebviewWindow,
    state: State<'_, Application>,
) -> Result<device_providers::ObservationSnapshot, String> {
    provider_window(&window, &device_id)?;
    match state.inner() {
        Application::Ready {
            device_providers, ..
        } => device_providers.observation(&device_id, window.label()),
        Application::Failed(error) => Err(error.to_string()),
    }
}
#[tauri::command]
fn list_plugins(
    window: WebviewWindow,
    state: State<'_, Application>,
) -> Result<Vec<plugins::catalog::Package>, Error> {
    if window.label() != "main" && !window.label().starts_with("device-") {
        return Err(Error::new("window", "Not an editor window"));
    }
    match state.inner() {
        Application::Ready { plugins, .. } => Ok(plugins.packages()),
        Application::Failed(e) => Err(e.clone()),
    }
}

fn provider_window(window: &WebviewWindow, device_id: &str) -> Result<(), String> {
    if window.label() == "main" || window.label() == format!("device-{device_id}") {
        Ok(())
    } else {
        Err("Device belongs to another editor".into())
    }
}
#[tauri::command]
async fn keyboard_assignment_status(
    device_id: String,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<serde_json::Value, Error> {
    provider_window(&window, &device_id).map_err(|e| Error::new("window", e))?;
    blocking_application(app,move |state| match state {
        Application::Ready {device_providers,..} => Ok(serde_json::json!({"session":device_providers.session(&device_id),"routes":keyboard_assignments::statuses(&*locked(state)?,device_providers,&device_id)})),
        Application::Failed(e) => Err(e.clone()),
    }).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn keyboard_assign_binding(
    device_id: String,
    control_id: String,
    layer: deckweave_core::KeyboardLayer,
    binding: Option<deckweave_core::Binding>,
    expected_revision: u64,
    replace_native: bool,
    expected_native: Option<[u8; 4]>,
    expected_generation: Option<u64>,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<AppSnapshot, Error> {
    provider_window(&window, &device_id).map_err(|e| Error::new("window", e))?;
    let notify = app.clone();
    blocking_application(app, move |state| {
        if let Application::Ready {
            core,
            device_providers,
            hardware,
            ..
        } = state
        {
            let result = keyboard_assignments::assign(
                core,
                device_providers,
                keyboard_assignments::AssignmentEdit {
                    device_id,
                    control_id,
                    layer,
                    binding,
                    expected_revision,
                    replace_native,
                    expected_native,
                    expected_generation,
                },
            );
            hardware.changed();
            let _ = notify.emit("deckweave:changed", ());
            result?;
        }
        live(state, locked(state)?.snapshot())
    })
    .await
}

#[tauri::command]
async fn keyboard_restore_assignment(
    device_id: String,
    control_id: String,
    layer: deckweave_core::KeyboardLayer,
    expected_revision: u64,
    confirmed: bool,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<AppSnapshot, Error> {
    provider_window(&window, &device_id).map_err(|e| Error::new("window", e))?;
    if !confirmed {
        return Err(Error::new(
            "keyboard_preview",
            "Review affected profiles before restoring the keyboard assignment",
        ));
    }
    let notify = app.clone();
    blocking_application(app, move |state| {
        if let Application::Ready {
            core,
            device_providers,
            hardware,
            ..
        } = state
        {
            let result = keyboard_assignments::restore(
                core,
                device_providers,
                device_id,
                control_id,
                layer,
                expected_revision,
            );
            hardware.changed();
            let _ = notify.emit("deckweave:changed", ());
            result?;
        }
        live(state, locked(state)?.snapshot())
    })
    .await
}

#[tauri::command]
async fn keyboard_reconcile_assignment(
    device_id: String,
    control_id: String,
    layer: deckweave_core::KeyboardLayer,
    expected_revision: u64,
    confirmed: bool,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<AppSnapshot, Error> {
    provider_window(&window, &device_id).map_err(|e| Error::new("window", e))?;
    if !confirmed {
        return Err(Error::new(
            "keyboard_identity",
            "Confirm the currently selected keyboard before reconciliation",
        ));
    }
    let notify = app.clone();
    blocking_application(app, move |state| {
        if let Application::Ready {
            core,
            device_providers,
            hardware,
            ..
        } = state
        {
            let result = keyboard_assignments::reconcile(
                core,
                device_providers,
                device_id,
                control_id,
                layer,
                expected_revision,
            );
            hardware.changed();
            let _ = notify.emit("deckweave:changed", ());
            result?;
        }
        live(state, locked(state)?.snapshot())
    })
    .await
}
#[tauri::command]
async fn provider_recovery_preview(
    device_id: String,
    snapshot_id: Option<String>,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    provider_window(&window, &device_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Application>();
        match state.inner() {
            Application::Ready {
                device_providers, ..
            } => device_providers.recovery_preview(&device_id, snapshot_id.as_deref()),
            Application::Failed(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn recover_provider_settings(
    device_id: String,
    preview_token: String,
    confirmed_instance_id: String,
    resolution: Option<String>,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    provider_window(&window, &device_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Application>();
        match state.inner() {
            Application::Ready {
                device_providers, ..
            } => match resolution.as_deref() {
                None => {
                    device_providers.recover(&device_id, &preview_token, &confirmed_instance_id)
                }
                Some(resolution) => device_providers.resolve_recovery(
                    &device_id,
                    &preview_token,
                    &confirmed_instance_id,
                    Some(resolution),
                ),
            },
            Application::Failed(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn open_inspector(
    context: String,
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, Application>,
) -> Result<(), Error> {
    let device_id = window
        .label()
        .strip_prefix("device-")
        .ok_or_else(|| Error::new("window", "Open action settings from its device"))?;
    let owned = locked(&state)?
        .snapshot()
        .configuration
        .devices
        .iter()
        .find(|d| d.id == device_id)
        .is_some_and(|d| {
            d.profile
                .pages
                .iter()
                .flat_map(|p| p.bindings.values())
                .chain(d.profile.fn_bindings.values().map(|f| &f.binding))
                .any(|b| b.contains(&context))
        });
    if !owned {
        return Err(Error::new("window", "Action belongs to another device"));
    }
    let label = format!("pi-{context}");
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    let host = app.clone();
    let target = context.clone();
    let (url, package) = tauri::async_runtime::spawn_blocking(move || {
        let state = host.state::<Application>();
        match state.inner() {
            Application::Ready { plugins, .. } => plugins.inspector(target),
            Application::Failed(e) => Err(e.clone()),
        }
    })
    .await
    .map_err(|e| Error::new("plugin", e.to_string()))??;
    let directory = directory_override()
        .map(Ok)
        .unwrap_or_else(|| app.path().app_data_dir())
        .map_err(|e| Error::new("plugin", e.to_string()))?;
    let built = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::External(
            url.parse()
                .map_err(|e| Error::new("plugin", format!("Invalid inspector URL: {e}")))?,
        ),
    )
    .title("DeckWeave — Plugin settings")
    .theme(Some(application_theme(&app)))
    .data_directory(directory.join("plugin-inspectors").join(package))
    .inner_size(600.0, 480.0)
    .build()
    .map_err(|e| Error::new("window", e.to_string()))?;
    locked(&state)?.editor_opened(built.label());
    let _ = app.emit("deckweave:changed", ());
    Ok(())
}

#[tauri::command]
fn restart_plugin(
    package: String,
    window: WebviewWindow,
    state: State<'_, Application>,
) -> Result<(), Error> {
    if !window.label().starts_with("device-") {
        return Err(Error::new("window", "Not an editor window"));
    }
    if let Application::Ready { plugins, .. } = state.inner() {
        plugins.restart(package);
    }
    Ok(())
}

#[tauri::command]
async fn open_device(
    device_id: String,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), Error> {
    if window.label() != "main" && !window.label().starts_with("device-") {
        return Err(Error::new("window", "Open devices from the registry"));
    }
    show_device(&app, &device_id)
}

pub(crate) fn show_device(app: &tauri::AppHandle, device_id: &str) -> Result<(), Error> {
    let shell = app.state::<shell::Shell>();
    let _creation = shell
        .window_creation
        .lock()
        .map_err(|_| Error::new("window", "Window creation unavailable"))?;
    let name = {
        let state = app.state::<Application>();
        let mut core = locked(&state)?;
        let config = core.snapshot().configuration;
        let name = config
            .devices
            .iter()
            .find(|d| d.id == device_id)
            .map(|d| d.name.clone())
            .ok_or_else(|| Error::new("not_found", "Device not found"))?;
        if config.preferences.last_device.as_deref() != Some(device_id) {
            let mut preferences = config.preferences;
            preferences.last_device = Some(device_id.into());
            core.dispatch(
                Command::SetPreferences {
                    preferences,
                    expected_revision: config.revision,
                },
                "window",
            )?;
        }
        name
    };
    let label = format!("device-{device_id}");
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.show();
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        let _ = app.emit("deckweave:changed", ());
        return Ok(());
    }
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .theme(Some(application_theme(app)));
    if let Some(directory) = directory_override() {
        builder = builder.data_directory(directory.join("web-editor"));
    }
    locked(&app.state::<Application>())?.editor_opened(&label);
    let built = builder
        .disable_drag_drop_handler()
        .title(format!("DeckWeave — {name}"))
        .inner_size(1180.0, 860.0)
        .min_inner_size(920.0, 720.0)
        .build();
    if let Err(error) = built {
        locked(&app.state::<Application>())?.editor_closed(&label);
        let _ = app.emit("deckweave:changed", ());
        return Err(Error::new("window", error.to_string()));
    }
    let _ = app.emit("deckweave:changed", ());
    Ok(())
}

#[tauri::command]
async fn open_settings(
    window: WebviewWindow,
    app: tauri::AppHandle,
    device_id: Option<String>,
    section: Option<shell::SettingsSection>,
) -> Result<(), Error> {
    if window.label() != "main" && !window.label().starts_with("device-") {
        return Err(Error::new("window", "Not an editor window"));
    }
    let context = shell::SettingsContext {
        section: section.unwrap_or(if device_id.is_some() {
            shell::SettingsSection::Devices
        } else {
            shell::SettingsSection::General
        }),
        device_id,
    };
    if let Some(shell) = app.try_state::<shell::Shell>() {
        *shell
            .settings_context
            .lock()
            .map_err(|_| Error::new("window", "Settings are unavailable"))? = context.clone();
    }
    shell::show_registry(&app);
    let _ = app.emit("deckweave:settings", context);
    Ok(())
}

#[tauri::command]
fn settings_context(
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<shell::SettingsContext, Error> {
    if window.label() != "main" {
        return Err(Error::new("window", "Not a settings window"));
    }
    app.state::<shell::Shell>()
        .settings_context
        .lock()
        .map(|v| v.clone())
        .map_err(|_| Error::new("window", "Settings are unavailable"))
}

fn window_theme(theme: deckweave_core::Theme) -> tauri::Theme {
    match theme {
        deckweave_core::Theme::Dark => tauri::Theme::Dark,
        deckweave_core::Theme::Light => tauri::Theme::Light,
    }
}
fn application_theme(app: &tauri::AppHandle) -> tauri::Theme {
    locked(&app.state::<Application>())
        .map(|core| window_theme(core.snapshot().configuration.preferences.theme))
        .unwrap_or(tauri::Theme::Dark)
}

fn main() {
    startup("builder");
    let mut context = tauri::generate_context!();
    if let Some(directory) = directory_override() {
        for window in &mut context.config_mut().app.windows {
            window.data_directory = Some(std::path::PathBuf::from(&directory).join("web-editor"));
        }
    }
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            shell::show_editor(app);
        }))
        .setup(|app| {
            startup("setup");
            app.manage(shell::Shell::install(app.handle())?);
            startup("tray");
            let directory = directory_override()
                .map(Ok)
                .unwrap_or_else(|| app.path().app_data_dir())
                .map_err(|error| Error::new("storage", error.to_string()));
            let core = directory.and_then(|directory| {
                std::fs::create_dir_all(&directory)
                    .map_err(|error| Error::new("storage", error.to_string()))?;
                Core::open(&directory.join("deckweave.sqlite3")).map(|core| (core, directory))
            });
            app.manage(match core {
                Ok((core, directory)) => {
                    app.manage(plugin_manager::Manager::new(directory.clone()));
                    startup("core");
                    let core = Arc::new(Mutex::new(core));
                    let hardware =
                        Hardware::start(Arc::clone(&core), app.handle().clone(), directory.clone());
                    let plugins = plugins::Host::start(
                        Arc::clone(&core),
                        app.handle().clone(),
                        directory.clone(),
                        hardware.invalidation(),
                        Arc::clone(&app.state::<plugin_manager::Manager>().lifecycle),
                    );
                    startup("plugins");
                    let device_providers = device_providers::Host::start(
                        Arc::clone(&core),
                        app.handle().clone(),
                        hardware.invalidation(),
                        directory,
                        Arc::clone(&app.state::<plugin_manager::Manager>().lifecycle),
                    );
                    let engine = engine::Engine::start(
                        Arc::clone(&core),
                        app.handle().clone(),
                        hardware.invalidation(),
                    );
                    Application::Ready {
                        core,
                        hardware,
                        plugins,
                        engine,
                        device_providers,
                    }
                }
                Err(error) => Application::Failed(error),
            });
            startup("managed");
            app.manage(keyboard_assignments::Runtime::start(app.handle().clone()));
            app.manage(key_lighting::Runtime::start(app.handle().clone()));
            if let Ok(core) = locked(&app.state::<Application>()) {
                app.state::<shell::Shell>()
                    .locale(core.snapshot().configuration.locale);
            }
            shell::show_editor(app.handle());
            startup("ready");
            #[cfg(debug_assertions)]
            smoke::start(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::Destroyed)
                && (window.label() == "main"
                    || window.label().starts_with("device-")
                    || window.label().starts_with("pi-"))
                && let Some(state) = window.app_handle().try_state::<Application>()
            {
                if let Ok(mut core) = locked(&state) {
                    core.editor_closed(window.label());
                }
                if let Application::Ready {
                    hardware,
                    device_providers,
                    ..
                } = state.inner()
                {
                    device_providers.stop_observer(window.label());
                    hardware.changed();
                }
                let _ = window.app_handle().emit("deckweave:changed", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            dispatch,
            open_device,
            open_settings,
            settings_context,
            list_plugins,
            open_inspector,
            restart_plugin,
            plugin_manager::plugin_catalogue,
            plugin_manager::plugin_release_assets,
            plugin_manager::plugin_pick_archive,
            plugin_manager::plugin_prepare_asset,
            plugin_manager::plugin_commit,
            plugin_manager::plugin_discard,
            extension_management::plugin_inventory,
            extension_management::plugin_lifecycle_history,
            extension_management::plugin_removal_preview,
            extension_management::plugin_remove,
            extension_management::plugin_reconnect,
            device_provider_status,
            device_provider_session,
            add_provider_device,
            refresh_provider_state,
            set_provider_observation,
            provider_observation,
            provider_recovery_preview,
            recover_provider_settings,
            keyboard_assignment_status,
            keyboard_assign_binding,
            keyboard_restore_assignment,
            keyboard_reconcile_assignment,
            key_lighting::key_lighting_test,
            key_lighting::key_lighting_stop,
            key_lighting::key_lighting_status,
            shell::quit_application
        ])
        .build(context);
    match result {
        Ok(app) => app.run(|app, event| {
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = &event
                && let Some(shell) = app.try_state::<shell::Shell>()
                && !shell.quitting.load(std::sync::atomic::Ordering::Relaxed)
            {
                api.prevent_exit();
            }
            if matches!(event, tauri::RunEvent::Exit)
                && let Some(state) = app.try_state::<Application>()
                && let Application::Ready {
                    hardware,
                    plugins,
                    engine,
                    device_providers,
                    ..
                } = state.inner()
            {
                if let Some(runtime) = app.try_state::<keyboard_assignments::Runtime>() {
                    runtime.stop();
                }
                if let Some(runtime) = app.try_state::<key_lighting::Runtime>() {
                    runtime.stop();
                }
                device_providers.stop();
                plugins.stop();
                engine.stop();
                hardware.stop();
            }
        }),
        Err(error) => eprintln!("DeckWeave startup failed: {error}"),
    }
}

fn startup(marker: &str) {
    #[cfg(debug_assertions)]
    {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::env::temp_dir().join("deckweave-startup.log"))
        {
            let _ = writeln!(file, "{} {marker}", std::process::id());
        }
    }
    #[cfg(not(debug_assertions))]
    let _ = marker;
}

fn directory_override() -> Option<std::path::PathBuf> {
    let from_env = std::env::var_os("DECKWEAVE_DATA_DIR").map(std::path::PathBuf::from);
    #[cfg(debug_assertions)]
    return from_env.or_else(|| smoke::configuration().map(|c| c.directory.clone()));
    #[cfg(not(debug_assertions))]
    from_env
}

#[cfg(test)]
mod tests {
    use super::*;
    use deckweave_core::InputEvent;

    #[test]
    fn window_scope_prevents_cross_device_execution_and_registry_execution() {
        let command = Command::Input {
            device_id: "one".into(),
            control_id: "key-1".into(),
            event: InputEvent::Down,
        };
        assert!(permitted("device-one", &command));
        assert!(!permitted("device-two", &command));
        assert!(!permitted("main", &command));
        assert!(!permitted("property-inspector", &command));
    }

    #[test]
    fn only_registry_can_add_instances() {
        let command = Command::AddDevice {
            expected_revision: 0,
        };
        assert!(permitted("main", &command));
        assert!(!permitted("device-one", &command));
    }
}
