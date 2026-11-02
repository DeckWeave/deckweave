# Граница hardware provider

Внешний process host реализован, контракт v1 предварительный. N4 остаётся встроенным. Mock проверяет изоляцию и lifecycle, локальный IO Type 84 — штатные настройки клавиатуры. Это ещё не стабильный публичный SDK с полным display/input-паритетом. [Практический workflow](DEVICE-EXTENSIONS.md), [модуль](../modules/providers.md), [панели](PANEL-CONTRACT.md).

Provider обслуживает много независимых instances и отвечает за discovery, транспорт, capabilities, геометрию и передачу кадров. Он не исполняет пользовательские действия и не владеет окном редактора.

| Операция | Данные и инвариант |
|---|---|
| Discover | provider/model ID, opaque instance identity и её confidence. USB path не объявляется постоянной identity; новое внешнее оборудование добавляется явно |
| Descriptor | logical body/surface rectangles, native surface pixels, стабильные control IDs, kind key/dial/touch и реально поддерживаемые события. Наличие dial click не означает наличие down/up/hold |
| Open | hardware identity → instance session с новым generation. Отсутствие устройства — Offline, отказ открытия — Error. Core instance ID и назначения сохраняются |
| Input | session/generation + возрастающий sequence; текущий внешний host принимает down/up/rotate и firmware click. Старые события отбрасываются, переполнение/ошибка останавливает процесс |
| Present | instance ID + generation + display target + page/binding identity + bounded image. Последний желаемый кадр заменяет устаревшие; транзакция одного изображения не прерывается другим изображением |
| Close / loss | прекращение ввода/вывода этой session, освобождение handles, отмена контактов без исполнения действия. Sibling instances продолжают работу |
| Settings | явный intent из Core, валидация полей/диапазонов, requested/observed/evidence отдельно. Открытие, восстановление и rebind не проигрывают сохранённые SET |
| Recovery | pending intent до первого SET, исходный снимок вне пакета, preview token и подтверждение текущего экземпляра. После cold restart возможен recovery-only запуск без обычного ввода/SET |

Core сохраняет настройку, активные profile/page, binding IDs, scheduler и feedback. Host преобразует логические кадры в представление конкретного транспорта; preview показывает результат той же композиции. Provider не меняет профили и не интерпретирует Counter/Switch/OBS.

Discovery выполняется supervisor; одна session владеет одним открытым HID handle. Медленные USB, JPEG, файловые операции и внешние процессы выполняются вне Core lock. Устаревший generation не может отпустить новую кнопку или подтвердить её кадр. При reconnect текущая страница рисуется заново, действия не проигрываются повторно.

`apps/desktop/src-tauri/src/device_providers.rs` запускает Node или native entry без shell. JSON-lines ограничен размером, очередями и тайм-аутами. Управление деревом процессов предотвращает зависание из-за унаследованных stdout handles. Удаление support сначала сохраняет запрет запуска во всех scan roots; сломанный managed пакет не заменяется старым репозиторным кодом.

SDK/source distribution находятся в соседнем `plugin-sdk`: `DEVICE-PROTOCOL.md`, device runtime/types/validators. Пакет — `<id>.dwDevice/deckweave.device.json`, устанавливается bounded archive installer в отдельный каталог `device-plugins`. Шаблон генерируется с `--device`. Реестр различает action/device; старый action registry читается совместимо.

Текущий внешний host не передаёт LCD frames и touchStart/Move/End до Core; эти части предварительного SDK описывают будущую границу. Внешний IO не объявляет таких событий/дисплеев. Полный N4 input/render/present остаётся в `deckweave-n4` и desktop hardware worker. Core поддерживает сохранённые provider descriptors; встроенные N4 IDs/geometries ещё остаются.

Подсветка задаётся именованными пресетами и явными владельцами. Текущая арбитрация — на экземпляр, независимые output groups ещё не стабилизированы. Временные onboard overrides закрыты до полной поддержки восстановления. Аналоговые каналы, захват клавиатуры, мышь, Android transport и синхронные группы не имитируются.
