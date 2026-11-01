# Desktop host

Ответственность: Tauri lifecycle, IPC, device windows, tray и adapters. Код apps/desktop/src-tauri. get_snapshot/dispatch/open_device/list_plugins/open_inspector/restart_plugin/quit_application проверяют scope окна. Одно окно на instance; уничтожение editor/PI снимает его editing session и только его удержания.

Окно main теперь содержит общие настройки и реестр устройств. open_settings/settings_context сохраняют целевое устройство между созданием и загрузкой WebView, чтобы переход из editor открывал именно его параметры. Окно main участвует в общей паузе Smart Profiles. [Новый срез](../docs/SETTINGS-AND-WEATHER.md).

Закрытие всех окон оставляет runtime в tray, повторный запуск открывает существующий реестр, явный Quit завершает workers/backends. HTML PI имеет отдельный browser data directory и не получает editor IPC. Headless HTML backend не зависит от PI/window lifecycle. Engine и hardware/plugin workers уведомляют UI событием deckweave:changed; snapshots не запрашиваются интервалом UI.

В Windows boundary изолированы foreground, SendInput и Process Job. Платформы macOS/Linux пока не проверены. [Проверки](../docs/RUNTIME-PROGRESS.md), [совместимость](../docs/PLUGIN-COMPATIBILITY.md), [план](../docs/PLAN.md).
