# План реализации

Дата: 2026-11-01. Новый проект, без миграции старых конфигураций. Технические решения владелец делегировал; [требования](DECISIONS.md) сохраняют приоритет. Отметки готовности ставятся после проверки, не после создания файла.

## Фаза 1 — Запускаемый скелет

Цель: Rust workspace, Tauri/Svelte и сборка. Трудоёмкость S. Статус: [x] Готово.

Контекст: репозиторий содержит исследование; код создаётся заново, старые приложения не изменяются.

- [x] Сборка Core/Desktop, Vite и ru/en ([desktop](../modules/desktop.md), [editor](../modules/editor.md)).
- [x] Native build и открытие окна ([desktop](../modules/desktop.md)).

Проверки: cargo check, Svelte check, production web build, native запуск. Результат: настоящее окно приложения.

## Фаза 2 — Первый сквозной виртуальный сценарий

Цель: назначение → «Проверить» → feedback → сохранение. Трудоёмкость M. Статус: [x] Готово для встроенных Counter/Toggle; внешние плагины и аппаратный вывод не входят в эту фазу.

Контекст: скелет запускается. Один Core обслуживает независимые виртуальные instances; это рабочие устройства приложения, а не статический макет.

- [x] Instances, страницы, назначения, down/up, исходный адресат release ([runtime](../modules/runtime.md)).
- [x] Транзакционная запись и восстановление, конфликт revisions ([storage](../modules/storage.md)).
- [x] Реестр/отдельные окна, библиотека и инспектор, выбор отдельно от выполнения ([editor](../modules/editor.md), [desktop](../modules/desktop.md)).

Проверки: изоляция двух instances, release после навигации, invalid input без частичной записи, restart конфигурации, действия через UI. Результат: безопасные builtins Counter/Toggle, без обещания готовых внешних плагинов.

## Фаза 3 — Hardware provider и N4

Цель: тот же сценарий на железе. Трудоёмкость M. Статус: [ ] В работе: кнопки/полоса/D1 подтверждены владельцем; остаются tray, provider contracts и оставшаяся аппаратная приёмка. Ближайший срез детализирован в [NEXT](NEXT.md).

Контекст: назначения и input router проверены на виртуальном provider. Старые низкоуровневые N4 codec/fixtures используются выборочно с сохранением происхождения.

- [ ] Provider boundary, capabilities и lifecycle нескольких instances ([providers](../modules/providers.md)).
- [ ] N4 transport, framebuffer, reconnect и независимость от окон ([providers](../modules/providers.md), [panels](../modules/panels.md)).
- [ ] Tray/фоновая работа и остановка приложения ([desktop](../modules/desktop.md)).

Сделано: отдельный transport crate, автообнаружение/постоянная identity, workers с generation, down/up/Click/rotation adapter, bounded key renderer, общие JPEG preview/output, очистка held и автоматические regression tests. На подключённом PID 1021 успешно переданы 10 начальных кадров, изменение Counter и два перехода страниц (32 кадра без reconnect/error). Закрытие редактора оставляет соединение активным. Эти результаты не закрывают аппаратную приёмку: [следующая проверка владельцем](N4-ACCEPTANCE.md).

По первой физической проверке: изображения, нижний ряд и первая крутилка подтверждены владельцем. Исправлена путаница input codes и BAT output map верхнего ряда; сохранены raw fixtures, replay проверяет действие всех 10 keys. Подключены strip JPEG/BGPIC и первый touch adapter с отменой движения. Повторная физическая проверка верхнего ряда, полосы, крайних touch областей и D1 на полосе подтверждена владельцем. Остаются D2–D4, USB reconnect/жесты и tray. Полноценные panel plugins, Dial Stack и gesture logic этой заготовкой не закрываются.

Проверки: собственные capture/fixtures, затем hardware acceptance при единственном владельце HID, disconnect и held cleanup. Результат: назначение и картинка на N4. Неподтверждённые аппаратные release/hold не выдумываются.

## Фаза 4 — Elgato compatibility host

Цель: реальные native/Node/HTML действия и PI. Трудоёмкость L; разбить на отдельные срезы при начале. Статус: [ ] Не начата.

Контекст: runtime и provider независимы. Существуют проверенные диагностические пакеты и журнал совместимости, но не готовый production host.

- [ ] Scanner, process supervision, WebSocket contexts, настройки и feedback ([actions](../modules/actions.md)).
- [ ] PI isolation, browser owner и lifecycle HTML backend ([desktop](../modules/desktop.md), [actions](../modules/actions.md)).
- [ ] Диагностический native/HTML пакет, часы и полезная OBS-интеграция ([actions](../modules/actions.md)).

Проверки: down/up, setImage/settings после restart, закрытие PI не останавливает backend, падение одного плагина не затрагивает соседей. Результат: проверенные exact artifacts уровня OpenDeck. Исходный импорт профилей исключён.

## Фаза 5 — Полная навигация и составные действия

Цель: profiles/folders/pinned, Multi Action/Delay/Switch и Key Logic. Трудоёмкость L; отдельные этапы внутри фазы. Статус: [ ] Не начата.

Контекст: действия имеют устойчивые context IDs и lifecycle. UI работает с единой runtime navigation.

- [ ] Профили/папки/закрепление ([runtime](../modules/runtime.md), [editor](../modules/editor.md)).
- [ ] Scheduler, отмена, desired state, Switch A/B/A, gesture resolver ([actions](../modules/actions.md)).
- [ ] Smart Profiles с глобальной паузой настроек ([desktop](../modules/desktop.md), [runtime](../modules/runtime.md)).

Проверки: fake clock для задержек, повторный запуск, ошибка, navigation/restart, single/double/hold без лишних ветвей. Результат: проверяемые сложные сценарии, смена самой кнопки видна.

## Фаза 6 — Панели, Dial Stack и Action Wheel

Цель: полноценная сенсорная поверхность и крутилки. Трудоёмкость L; разделить panel SDK, compositor и редактор при начале. Статус: [ ] Не начата.

Контекст: control geometry не привязана к прямоугольной сетке, action instances и feedback уже существуют.

- [ ] Native panel plugin, layouts/regions, размещение совместимых действий ([panels](../modules/panels.md)).
- [ ] Связь с dials, Elgato layouts, tap/hold/swipe, hit testing ([panels](../modules/panels.md), [providers](../modules/providers.md)).
- [ ] Dial Stack и Action Wheel с различием выбора/исполнения ([actions](../modules/actions.md), [editor](../modules/editor.md)).

Проверки: одна геометрия preview/hardware/input, координаты и масштаб, отсутствие tap+swipe, произвольное число виджетов, два instances. Результат: оба сценария панели, а не только полоса громкости.

## Фаза 7 — Устойчивость и приёмка выпуска

Цель: ошибки, lifecycle, производительность и packaging. Трудоёмкость M. Статус: [ ] Не начата.

Контекст: обязательные функции v1 реализованы предыдущими срезами; теперь требуется сквозная приёмка всего приложения.

- [ ] Сбои/восстановление/диагностика и резервная копия ([storage](../modules/storage.md), [runtime](../modules/runtime.md)).
- [ ] Установка и измерение всего дерева процессов ([desktop](../modules/desktop.md), [editor](../modules/editor.md)).
- [ ] Выполнить применимые UX acceptance, описать непроверенное ([панели](../modules/panels.md), [приёмка](research/2026-11-01/UX-ACCEPTANCE.md)).

Проверки: длительная работа, reconnect, падение плагинов, очередь кадров, окна и память. Результат: первый полноценный Windows 11 выпуск. macOS/Linux сохраняют предметную переносимость, но не объявляются проверенными без сборки на этих ОС.
