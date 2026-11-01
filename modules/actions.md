# Действия

Ответственность: action instances, scheduler, ввод и feedback. Core actions.rs исполняет Counter/Toggle, навигацию, Multi Action/Delay/Switch, Key Logic, Dial Stack/Action Wheel и отдельные dial directions/push/tap. OS effects выполняет desktop engine после Core lock; FFI изолирован в deckweave-platform.

Elgato host: apps/desktop/src-tauri/src/plugins. Устойчивый context принадлежит binding, package/process отличается от action instance. Native/Node/HTML backend лениво запускается на пакет и обслуживает много contexts. Невидимые назначения получают willDisappear, retained holds/jobs завершаются в исходном context. Ошибка отменяет ещё не исполненные шаги владельца.

Оригинальный PI работает без editor IPC. Windows HTML backend — headless Edge с собственным Job/profile; Node/native backend обходятся без browser. [Матрица совместимости](../docs/PLUGIN-COMPATIBILITY.md), [runtime evidence](../docs/RUNTIME-PROGRESS.md), [план](../docs/PLAN.md).
