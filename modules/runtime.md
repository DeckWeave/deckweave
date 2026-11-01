# Runtime

Ответственность: единое состояние независимых instances, navigation и адресация ввода. Код: crates/deckweave-core, включая model.rs, actions.rs и panels.rs. API: Core, Command, Snapshot, InputEvent; транзакции — [Store](storage.md).

Revision конфигурации отличается от runtime sequence. Выбор UI не является вводом; release адресуется исходному binding/source. Snapshot проецирует pinned и связанные dial/panel aliases; persistent binding остаётся один. Scheduler монотонный, ошибки/отмена очищают ещё не исполненные шаги. OS/USB/процессы не исполняются под Core lock.

Desktop adapters поставляют foreground observations и hardware events. Editing pause глобальна; без Smart rules ручной выбор сохраняется, неизвестное окно не вызывает fallback. Native panel использует нормализованную geometry и optional Y. [Контракт панели](../docs/PANEL-CONTRACT.md), [результаты](../docs/RUNTIME-PROGRESS.md), [план](../docs/PLAN.md).
