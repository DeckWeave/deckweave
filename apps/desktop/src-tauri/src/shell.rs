use deckweave_core::{Error, Locale};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
};

pub struct Shell {
    pub settings_device: std::sync::Mutex<Option<String>>,
    pub quitting: AtomicBool,
    open: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    _tray: TrayIcon<tauri::Wry>,
    locale: AtomicU8,
    app: tauri::AppHandle,
}
impl Shell {
    pub fn install(app: &tauri::AppHandle) -> tauri::Result<Self> {
        let open = MenuItem::with_id(app, "registry", "Настройки", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Выйти из DeckWeave", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&open, &quit])?;
        let mut builder = TrayIconBuilder::with_id("deckweave")
            .tooltip("DeckWeave")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id.as_ref() {
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
                    show_registry(tray.app_handle());
                }
            });
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        let tray = builder.build(app)?;
        Ok(Self {
            settings_device: std::sync::Mutex::new(None),
            quitting: AtomicBool::new(false),
            open,
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
        let (open, quit) = match locale {
            Locale::Ru => ("Настройки", "Выйти из DeckWeave"),
            Locale::En => ("Settings", "Quit DeckWeave"),
        };
        let (open_item, quit_item) = (self.open.clone(), self.quit.clone());
        let _ = self.app.run_on_main_thread(move || {
            let _ = open_item.set_text(open);
            let _ = quit_item.set_text(quit);
        });
    }
}
pub fn show_registry(app: &tauri::AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = handle.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        } else {
            let mut builder = tauri::WebviewWindowBuilder::new(
                &handle,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            );
            if let Some(directory) = crate::directory_override() {
                builder = builder.data_directory(directory.join("web-editor"));
            }
            let _ = builder
                .title("DeckWeave — Settings")
                .inner_size(980.0, 720.0)
                .min_inner_size(820.0, 600.0)
                .build();
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
