# Desktop host

Ответственность: Tauri lifecycle, IPC и окна instances. Расположение: `apps/desktop/src-tauri`.

Интерфейс: `get_snapshot`, `dispatch`, `open_device`. Core в managed state; команда проверяет адресата окна. Окно устройства не должно изменять чужой instance. Одно окно на instance, reopening поднимает существующее. Событие `deckweave:changed` инвалидирует snapshots. Реальное уничтожение окна очищает editing session и удержания этого источника.

Первый срез не содержит tray и внешние HTML PI; до их реализации закрытие всех окон завершает процесс. Они обязательны в дальнейшей фазе. [Архитектура](../docs/ARCHITECTURE.md), [план](../docs/PLAN.md).
