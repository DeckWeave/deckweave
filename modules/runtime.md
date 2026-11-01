# Runtime

Ответственность: единое состояние независимых instances, навигация и адресация ввода. Расположение: `crates/deckweave-core/src/lib.rs`, `model.rs`.

Публичный интерфейс: `Core`, `Command`, `Snapshot`, `InputEvent`. Зависимости: serde DTO и [Store](storage.md). Конфигурация меняется только после commit; revision конфигурации не является счётчиком runtime feedback. Выбор UI не является командой ввода. Down/up адресуются исходному action instance.

Не обрабатывает OS foreground monitoring, внешние процессы и USB. Их добавляют adapters через будущую provider/action границу. Первая реализация содержит безопасные builtins и виртуальные устройства. [Архитектура](../docs/ARCHITECTURE.md), [план](../docs/PLAN.md).
