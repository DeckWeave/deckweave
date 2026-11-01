# Runtime, плагины и составные действия

Дата 2026-11-01, ветка `codex/runtime-plugins-actions`, базовый commit `85d242d`. Три согласованных направления реализованы как следующий рабочий срез; полный v1 ещё требует UX, совместимости и длительной приёмки. [Статус](STATUS.md), [продолжение](NEXT.md).

## Реализация

- Tray и повторное создание реестра, один runtime, явный выход. Hardware session независима от окон; Offline отделён от ошибки открытия, stale generation отсекается.
- Elgato catalog/process/socket/PI host. Пакет запускается при первом активном использовании и остаётся, пока назначен где-либо; невидимые contexts получают willDisappear. Settings/global settings, stable contexts, Keypad/Encoder, title/state/image, feedback layouts и ошибки. [Матрица и ограничения](PLUGIN-COMPATIBILITY.md).
- Windows HTML backend работает в отдельном headless Edge с собственным профилем и Job для завершения descendants. UI/PI — Tauri. Native/Node backend не создаёт browser. Hidden WebView2 на этой машине воспроизводимо зависал при первой Canvas отрисовке; Edge прошёл тот же сценарий.
- Навигация profiles/folders/pages/pinned keys; Multi Action/Delay, Switch A/B/A, single/double/hold Key Logic, отдельные dial left/right/push/tap, Dial Stack и Action Wheel. Scheduler монотонный; ожидания и OS/USB/file I/O вне Core lock. Ошибка запуска отменяет ещё не исполненные шаги, в том числе следующие с нулевой задержкой. Вложенные feedback видны в редакторе.
- Smart Profiles по foreground executable; глобальная editing pause. Без правил ручной выбор не сбрасывается, неизвестное foreground observation ничего не переключает.
- Native panel regions/fullscreen/dial associations, единые raster bounds для hit/render, bitmap/SVG и Encoder layouts. Новый Panel Lab показывает pointer phases/optional Y и несколько dials. [Контракт](PANEL-CONTRACT.md). Без точного Y вертикальная область не выбирается по догадке; Encoder tap с внутренней Y-логикой не выполняется с выдуманной координатой.
- Windows FFI изолирован в deckweave-platform; прочие crates сохраняют unsafe_code=forbid. Диагностика не копирует изображения и настройки плагинов.

## Проверено

Общий прогон: 60 Rust-тестов (31 Core, 17 desktop/protocol/router, 10 N4 unit, captured replay и Windows Job test), Svelte 0 errors/warnings, fmt/clippy -D warnings, production web и docs links. Native release собирается через npm run build. QA setup example проходит clippy и использует Core commands, не прямую правку SQLite.

Изолированные debug сценарии не использовали базу владельца или USB:

1. [Node backend после закрытия окон](evidence/2026-11-01/runtime-node-background.json): settings PI round-trip и второе действие дали SavedFromPI 2 при editing_windows=0.
2. [HTML OnAirClock](evidence/2026-11-01/runtime-clock-background.json): 12 изменений PNG за 15 секунд без окон. Пакет установленный, его файлы не изменены.
3. [Native Panel через настоящий UI](evidence/2026-11-01/panel-native-ui.json): назначение показало 50%, drag в отдельном тестовом поле дал 86%, panelUp и Y=0.49.
4. [Оригинальный PI своего пакета](evidence/2026-11-01/plugin-pi-ui.json): поле Button title сразу дало UI verified 0, отдельная проверка — UI verified 1.
5. [Switch через UI](evidence/2026-11-01/switch-ui.json): одна кнопка A0 → B0 → A1, инспектор показывает следующее действие.
6. [Установленный OnAirClock 1.3.0 и его оригинальный PI](evidence/2026-11-01/onair-pi-ui.json): смена 24/12 Hour изменила живое изображение, закрытие PI не остановило часы.
7. [Lifecycle окон/процессов](evidence/2026-11-01/runtime-window-lifecycle-ui.json): ноль окон при живом runtime, повторный запуск открыл реестр; явный Quit завершил приложение и все девять его процессов Edge.
8. OBS: два настоящих WebSocket соединения в тесте, включая challenge authentication; ранее установленный OBS 32.2.2 / obs-websocket 5.7.4 ответил на GetVersion/GetRecordStatus. Запись/стрим не запускались. Собственный OBS plugin не выдаётся за тест всех сторонних OBS пакетов.

[Ресурсы](evidence/2026-11-01/runtime-clock-debug-resources.json): короткая debug проба OnAirClock без окон — 10 процессов, private commit 218.1 MiB, сумма working sets 539.3 MiB, CPU 0.0625 s за 5.282 s. Скрипт учитывает всё дерево, включая Node/Edge, и смену процессов. Это не release budget; shared pages могут суммироваться повторно.

## Подготовка аппаратной приёмки

Отдельный профиль Acceptance 2026-11-01 создан Core utility с резервной копией базы; прежние профили не менялись. Подготовка не исполняет действия. Утилита идемпотентна и отказывается перезаписывать уже существующий acceptance profile/backup. Полный перечень — [инструкция](ACCEPTANCE-NEXT.md).

Окончательная release-сборка запущена на прежнем N4 instance, Connected, без ошибок плагинов. [Build/hash](evidence/2026-11-01/runtime-release-build.json), [software session](evidence/2026-11-01/n4-runtime-release-session.json), [общий кадр Panel Lab](evidence/2026-11-01/n4-runtime-panel-release.png), [страница Encoder/Stack/Wheel](evidence/2026-11-01/n4-runtime-dials-release.png). Это preview переданных JPEG, не замена физической приёмки. Резервная копия проверена через Core на отдельной локальной копии.

Остаётся физически подтвердить только новую неопределённость: D2–D4, pointer Y/drag/swipe и reconnect. Исходные K1–K10, T1/T5, D1 и ориентация уже подтверждены владельцем. Общий provider contract описан, N4 всё ещё встроен; внешний hardware SDK не объявлен готовым.

Runtime selection Switch/Stack/Wheel сбрасывается при restart, конфигурация сохраняется. Повтор Multi Action заменяет его отложенные шаги. Вложенный Multi Action запрещён; Sequence в состоянии Switch разрешён. Нет profile import. Незавершённые v1/packaging/UX задачи перечислены в STATUS и PLAN.
