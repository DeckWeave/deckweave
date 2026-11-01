# Действия

Ответственность: action instances, ввод и feedback. Первая реализация: Counter/Toggle в Core. Внешние процессы, Elgato compatibility host, compose scheduler — следующие фазы.

Целевой интерфейс: appear/disappear/input/settings/feedback с устойчивым context. Процесс плагина отличается от пакета и action instance. Counter/Toggle демонстрационные и не выполняют OS-команды. Multi Action, Switch, Key Logic и состояния плагина — разные механизмы; userDesiredState сохраняется в шагах.

Elgato PI остаётся исходной формой пакета без доступа к desktop IPC. Scheduler не спит под глобальным mutex. Падение одного backend не уничтожает соседний browser session. [Архитектура](../docs/ARCHITECTURE.md), [план](../docs/PLAN.md).
