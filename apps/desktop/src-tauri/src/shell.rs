use deckweave_core::{Error, Locale};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
};

pub struct Shell {
    pub window_creation: std::sync::Mutex<()>,
    pub settings_device: std::sync::Mutex<Option<String>>,
    pub quitting: AtomicBool,
    open: MenuItem<tauri::Wry>,
    settings: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    _tray: TrayIcon<tauri::Wry>,
    locale: AtomicU8,
    app: tauri::AppHandle,
}
impl Shell {
    pub fn install(app: &tauri::AppHandle) -> tauri::Result<Self> {
        let open = MenuItem::with_id(app, "editor", "Открыть DeckWeave", true, None::<&str>)?;
        let settings = MenuItem::with_id(app, "registry", "Настройки", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Выйти из DeckWeave", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&open, &settings, &quit])?;
        let mut builder = TrayIconBuilder::with_id("deckweave")
            .tooltip("DeckWeave")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "editor" => show_editor(app),
                "registry" => show_registry(app),
                "quit" => exit(app),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                ) {
                    show_editor(tray.app_handle());
                }
            });
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        let tray = builder.build(app)?;
        Ok(Self {
            window_creation: std::sync::Mutex::new(()),
            settings_device: std::sync::Mutex::new(None),
            quitting: AtomicBool::new(false),
            open,
            settings,
            quit,
            _tray: tray,
            locale: AtomicU8::new(0),
            app: app.clone(),
        })
    }
    pub fn locale(&self, locale: Locale) {
        let value = if locale == Locale::Ru { 0 } else { 1 };
        if self.locale.swap(value, Ordering::Relaxed) == value {
            return;
        }
        let (open, settings, quit) = match locale {
            Locale::Ru => ("Открыть DeckWeave", "Настройки", "Выйти из DeckWeave"),
            Locale::En => ("Open DeckWeave", "Settings", "Quit DeckWeave"),
        };
        let (open_item, quit_item) = (self.open.clone(), self.quit.clone());
        let settings_item = self.settings.clone();
        let _ = self.app.run_on_main_thread(move || {
            let _ = open_item.set_text(open);
            let _ = settings_item.set_text(settings);
            let _ = quit_item.set_text(quit);
        });
    }
}
pub fn show_editor(app: &tauri::AppHandle) {
    let handle = app.clone();
    // WebView2 creation can deadlock in a Windows UI/event callback.
    std::thread::spawn(move || {
        let target = handle
            .try_state::<crate::Application>()
            .and_then(|state| {
                crate::locked(&state)
                    .ok()
                    .map(|core| initial_device(&core.snapshot().configuration))
            })
            .flatten();
        if let Some(id) = target {
            if crate::show_device(&handle, &id).is_err() {
                show_registry(&handle);
            }
        } else {
            show_registry(&handle);
        }
    });
}
fn initial_device(config: &deckweave_core::Configuration) -> Option<String> {
    config
        .preferences
        .last_device
        .as_ref()
        .filter(|id| config.devices.iter().any(|d| &d.id == *id))
        .cloned()
        .or_else(|| {
            config
                .devices
                .iter()
                .find(|d| d.provider_id != "deckweave.virtual")
                .map(|d| d.id.clone())
        })
        .or_else(|| config.devices.first().map(|d| d.id.clone()))
}
pub fn show_registry(app: &tauri::AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let shell = handle.state::<Shell>();
        let Ok(_creation) = shell.window_creation.lock() else {
            return;
        };
        if let Some(window) = handle.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        } else {
            if let Ok(mut core) = crate::locked(&handle.state::<crate::Application>()) {
                core.editor_opened("main");
            }
            let mut builder = tauri::WebviewWindowBuilder::new(
                &handle,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            );
            if let Some(directory) = crate::directory_override() {
                builder = builder.data_directory(directory.join("web-editor"));
            }
            if builder
                .title("DeckWeave — Settings")
                .inner_size(980.0, 720.0)
                .min_inner_size(820.0, 600.0)
                .build()
                .is_err()
                && let Ok(mut core) = crate::locked(&handle.state::<crate::Application>())
            {
                core.editor_closed("main");
            }
        }
        if handle.get_webview_window("main").is_some() {
            if let Ok(mut core) = crate::locked(&handle.state::<crate::Application>()) {
                core.editor_opened("main");
            }
            let _ = handle.emit("deckweave:changed", ());
        }
    });
}
pub fn exit(app: &tauri::AppHandle) {
    if let Some(shell) = app.try_state::<Shell>() {
        shell.quitting.store(true, Ordering::Relaxed);
    }
    app.exit(0);
}
#[tauri::command]
pub fn quit_application(window: tauri::WebviewWindow, app: tauri::AppHandle) -> Result<(), Error> {
    if window.label() != "main" {
        return Err(Error::new("window", "Quit from the device registry"));
    }
    exit(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use deckweave_core::{Core, Device};
    #[test]
    fn startup_prefers_last_device_then_physical_then_virtual() {
        let mut config = Core::memory().unwrap().snapshot().configuration;
        let virtual_id = config.devices[0].id.clone();
        let mut physical = Device::virtual_studio(2);
        physical.provider_id = "mirabox.n4".into();
        let physical_id = physical.id.clone();
        config.devices.push(physical);
        assert_eq!(initial_device(&config), Some(physical_id.clone()));
        config.preferences.last_device = Some(virtual_id.clone());
        assert_eq!(initial_device(&config), Some(virtual_id.clone()));
        config.preferences.last_device = Some("removed".into());
        assert_eq!(initial_device(&config), Some(physical_id));
        config.devices.truncate(1);
        assert_eq!(initial_device(&config), Some(virtual_id));
        config.devices.clear();
        assert_eq!(initial_device(&config), None);
    }
}
