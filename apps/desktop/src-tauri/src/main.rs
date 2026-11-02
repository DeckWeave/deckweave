#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deckweave_core::{Command, Core, Error, Snapshot};
mod engine;
mod hardware;
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
    },
    Failed(Error),
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
fn get_snapshot(
    window: WebviewWindow,
    state: State<'_, Application>,
) -> Result<LiveSnapshot, Error> {
    if window.label() != "main" && !window.label().starts_with("device-") {
        return Err(Error::new(
            "window",
            "This window cannot read the workspace",
        ));
    }
    let snapshot = locked(&state)?.snapshot();
    live(&state, snapshot)
}

fn live(state: &Application, snapshot: Snapshot) -> Result<LiveSnapshot, Error> {
    match state {
        Application::Ready { hardware, .. } => Ok(hardware.snapshot(snapshot)),
        Application::Failed(error) => Err(error.clone()),
    }
}

#[tauri::command]
fn dispatch(
    command: Command,
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, Application>,
) -> Result<LiveSnapshot, Error> {
    let label = window.label();
    if !permitted(label, &command) {
        return Err(Error::new(
            "window",
            "This window cannot change the requested device",
        ));
    }
    let snapshot = locked(&state)?.dispatch(command, label)?;
    if let Some(shell) = app.try_state::<shell::Shell>() {
        shell.locale(snapshot.configuration.locale);
    }
    if let Application::Ready { hardware, .. } = state.inner() {
        hardware.changed();
    }
    let _ = app.emit("deckweave:changed", snapshot.event_sequence);
    live(&state, snapshot)
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
        )
    } else if let Some(id) = label.strip_prefix("device-") {
        matches!(
            command,
            Command::SetLocale { .. } | Command::SetPreferences { .. }
        ) || command.device_id() == Some(id)
    } else {
        false
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
    .data_directory(directory.join("plugin-inspectors").join(package))
    .inner_size(540.0, 720.0)
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
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()));
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
) -> Result<(), Error> {
    if window.label() != "main" && !window.label().starts_with("device-") {
        return Err(Error::new("window", "Not an editor window"));
    }
    if let Some(shell) = app.try_state::<shell::Shell>() {
        *shell
            .settings_device
            .lock()
            .map_err(|_| Error::new("window", "Settings are unavailable"))? = device_id.clone();
    }
    shell::show_registry(&app);
    let _ = app.emit("deckweave:settings", device_id);
    Ok(())
}

#[tauri::command]
fn settings_context(window: WebviewWindow, app: tauri::AppHandle) -> Result<Option<String>, Error> {
    if window.label() != "main" {
        return Err(Error::new("window", "Not a settings window"));
    }
    app.state::<shell::Shell>()
        .settings_device
        .lock()
        .map(|v| v.clone())
        .map_err(|_| Error::new("window", "Settings are unavailable"))
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
                        directory,
                        hardware.invalidation(),
                    );
                    startup("plugins");
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
                    }
                }
                Err(error) => Application::Failed(error),
            });
            startup("managed");
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
                if let Application::Ready { hardware, .. } = state.inner() {
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
                    ..
                } = state.inner()
            {
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
