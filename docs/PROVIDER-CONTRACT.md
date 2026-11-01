# Граница hardware provider

Это внутренний контракт первого рабочего среза. N4 сейчас встроен; внешний process provider/публичный SDK не объявляются готовыми. Следующий реальный adapter должен проверить контракт до его стабилизации. [Текущий lifecycle](../modules/providers.md), [панели](PANEL-CONTRACT.md).

Provider обслуживает много независимых instances и отвечает за discovery, транспорт, capabilities, геометрию и передачу кадров. Он не исполняет пользовательские действия и не владеет окном редактора.

| Операция | Данные и инвариант |
|---|---|
| Discover | provider ID, model ID, стабильная opaque hardware identity, display name. Номера USB bus/path не используются как постоянная identity |
| Descriptor | logical body/surface rectangles, native surface pixels, стабильные control IDs, kind key/dial/touch и реально поддерживаемые события. Наличие dial click не означает наличие down/up/hold |
| Open | hardware identity → instance session с новым generation. Отсутствие устройства — Offline, отказ открытия — Error. Core instance ID и назначения сохраняются |
| Input | instance ID + generation + control ID + ordered event. Digital edges не сливаются. Rotation содержит signed ticks. Pointer содержит нормализованные X и optional Y; неизвестный Y не заменяется выдуманным |
| Present | instance ID + generation + display target + page/binding identity + bounded image. Последний желаемый кадр заменяет устаревшие; транзакция одного изображения не прерывается другим изображением |
| Close / loss | прекращение ввода/вывода этой session, освобождение handles, отмена контактов без исполнения действия. Sibling instances продолжают работу |

Core сохраняет настройку, активные profile/page, binding IDs, scheduler и feedback. Host преобразует логические кадры в представление конкретного транспорта; preview показывает результат той же композиции. Provider не меняет профили и не интерпретирует Counter/Switch/OBS.

Discovery выполняется supervisor; одна session владеет одним открытым HID handle. Медленные USB, JPEG, файловые операции и внешние процессы выполняются вне Core lock. Устаревший generation не может отпустить новую кнопку или подтвердить её кадр. При reconnect текущая страница рисуется заново, действия не проигрываются повторно.

Для N4 эти границы сейчас реализованы модулями `deckweave-n4`, desktop hardware worker и Core hardware API. В Core ещё остаются built-in provider IDs/geometries: их предстоит заменить descriptor registry при добавлении второго физического adapter. Аналоговые каналы IO84, device-wide configuration, мышь и Android transport добавляются через capabilities; их протоколы сейчас не имитируются. Синхронные группы не входят в этот контракт.
