# Граница hardware provider

Внешний process host реализован, контракт v1 предварительный. N4 остаётся встроенным. Mock проверяет изоляцию и lifecycle, локальный IO Type 84 — штатные настройки клавиатуры. Это ещё не стабильный публичный SDK с полным display/input-паритетом. [Практический workflow](DEVICE-EXTENSIONS.md), [модуль](../modules/providers.md), [панели](PANEL-CONTRACT.md).

Provider обслуживает много независимых instances и отвечает за discovery, транспорт, capabilities, геометрию и передачу кадров. Он не исполняет пользовательские действия и не владеет окном редактора.

| Операция | Данные и инвариант |
|---|---|
| Discover | provider/model ID, opaque instance identity и её confidence. USB path не объявляется постоянной identity; новое внешнее оборудование добавляется явно |
| Descriptor | logical body/surface rectangles, native surface pixels, стабильные control IDs, kind key/dial/touch и реально поддерживаемые события. Наличие dial click не означает наличие down/up/hold |
| Open | hardware identity → instance session с новым generation. Отсутствие устройства — Offline, отказ открытия — Error. Core instance ID и назначения сохраняются |
| Input | session/generation + возрастающий sequence; текущий внешний host принимает down/up/rotate и firmware click. Старые события отбрасываются, переполнение/ошибка останавливает процесс |
| Observation | optional keyTravel capability, явный observe и отдельная последовательность depth. Samples ограничены и устаревают через 600 мс; подписки окон и provider lease не меняют Core/configuration и не исполняют действия |
| Key lighting | optional экспериментальный RAM-выход `keyLighting` / `releaseKeyLighting`: живые session/generation, ограниченные lease/sequence/TTL, физические controlId/RGB, отдельный исходный снимок. ACK приёма не является readback или физическим подтверждением. Закрытие/expiry/settings освобождают владение; неизвестное восстановление остаётся pending |
| Present | instance ID + generation + display target + page/binding identity + bounded image. Последний желаемый кадр заменяет устаревшие; транзакция одного изображения не прерывается другим изображением |
| Close / loss | прекращение ввода/вывода этой session, освобождение handles, отмена контактов без исполнения действия. Sibling instances продолжают работу |
| Settings | явный intent из Core, валидация полей/диапазонов, requested/observed/evidence отдельно. Открытие, восстановление и rebind не проигрывают сохранённые SET |
| Recovery | pending intent до первого SET, исходный снимок вне пакета, preview token и подтверждение текущего экземпляра. После cold restart возможен recovery-only запуск без обычного ввода/SET |

Core сохраняет настройку, активные profile/page, binding IDs, scheduler и feedback. Host преобразует логические кадры в представление конкретного транспорта; preview показывает результат той же композиции. Provider не меняет профили и не интерпретирует Counter/Switch/OBS.

Discovery выполняется supervisor; одна session владеет одним открытым HID handle. Медленные USB, JPEG, файловые операции и внешние процессы выполняются вне Core lock. Устаревший generation не может отпустить новую кнопку или подтвердить её кадр. При reconnect текущая страница рисуется заново, действия не проигрываются повторно.

`apps/desktop/src-tauri/src/device_providers.rs` запускает Node или native entry без shell. JSON-lines ограничен размером, очередями и тайм-аутами. Управление деревом процессов предотвращает зависание из-за унаследованных stdout handles. Удаление support сначала сохраняет запрет запуска во всех scan roots; сломанный managed пакет не заменяется старым репозиторным кодом.

SDK/source distribution находятся в соседнем `plugin-sdk`: `DEVICE-PROTOCOL.md`, device runtime/types/validators. Пакет — `<id>.dwDevice/deckweave.device.json`, устанавливается bounded archive installer в отдельный каталог `device-plugins`. Шаблон генерируется с `--device`. Реестр различает action/device; старый action registry читается совместимо.

Текущий внешний host не передаёт LCD frames и touchStart/Move/End до Core; эти части предварительного SDK описывают будущую границу. Внешний IO не объявляет таких событий/дисплеев. Полный N4 input/render/present остаётся в `deckweave-n4` и desktop hardware worker. Core поддерживает сохранённые provider descriptors; встроенные N4 IDs/geometries ещё остаются.

Подсветка задаётся именованными пресетами и явными владельцами. Текущая арбитрация — на экземпляр, независимые output groups ещё не стабилизированы. Временные onboard overrides закрыты до полной поддержки восстановления. Пассивные decorations показывают физическую LED-панель, не добавляя Controls или output capability. Наблюдение глубины реализовано отдельно; захват клавиатуры, мышь, Android transport и синхронные группы не имитируются.

Отдельный временный RAM-канал не является обходом запрета onboard overrides. IO объявляет его экспериментальным только для статичного одноцветного фона с яркостью от 2, 1–14 клавиш, до 10 Гц и lease 100–3000 мс. Исходный цвет берётся свежим чтением, кадры сохраняют один состав клавиш; первый кадр сохраняет durable RAM intent. Потеря ответа не запускает повтор. Успешно восстановленная lease допускает идемпотентный release без USB; неизвестная или неопределённая lease — нет. Cold recovery требует выбранного живого экземпляра и preview token, без `keepCurrent` для непрочитываемого RAM-цвета. [Пользовательские сценарии и границы](KEYBOARD-ACTIONS.md).

## Экспериментальные аппаратные привязки клавиатуры

IO84 объявляет `keyboard.aliases.version: 1`. `observed.keyboardAliases` содержит обе полные таблицы по 128 четырёхбайтовых записей и SHA-256 revisions слоёв. Это частный предварительный codec IO84, а не универсальный формат всех клавиатур. Проекция `onboardKeyLabels` позволяет показывать считанные штатные функции без их перезаписи.

`installKeyboardAlias` / `restoreKeyboardAlias` / `reconcileKeyboardAlias` передают идентификаторы текущей операции и первоначальной установки, control/layer, исходную и установленную запись, ожидаемую revision и явно подтверждённый экземпляр. Они идут отдельной ограниченной очередью, сериализованной с чтением и обычными настройками. Установка пишет полный изменённый слой с сохранением соседей и собственным durable intent. Reconcile только читает. Неопределённый результат удерживает резерв; reconnect не повторяет SET. Host не принимает совпадение одной записи вместо успешного исхода операции и полного наблюдения.

`configureKeyboardInput` получает только подтверждённые источники текущего поколения и номер конфигурации. Адаптер проверяет доказательство установки, текущее назначение и отсутствие второго такого кода. Он сопоставляет управляющий интерфейс и Windows Raw Input collections через ContainerId внутри своего процесса; второй Raw Input target в процессе Tauri не создаётся. Неподтверждённое сопоставление не заменяется глобальными горячими клавишами.

`keyboardAlias` содержит session/generation, возрастающий sequence, configurationSequence, control/layer и настроенный trigger. Host сопоставляет событие с захваченным маршрутом, профилем, жизненным циклом и отпечатком полного действия; устаревшее событие не активирует изменённое действие даже при прежнем binding ID. `keyboardInputState` останавливает только источник клавиатурных действий, сохраняя доступ к чтению/настройкам устройства. Потеря последовательности и переполнение требуют явной проверки, без автоматического повторного исполнения.

Первый контракт выдаёт одну логическую активацию, отбрасывая повтор и дубликаты коллекций. Он не обещает физические hold/up, подавление F-кодов или самостоятельное событие Fn. После перезапуска сохранённое `verified` не даёт живого маршрута без явного чтения и подтверждения текущего сеанса. [Workflow, удаление против восстановления и физическая приёмка](KEYBOARD-ASSIGNMENTS.md).
