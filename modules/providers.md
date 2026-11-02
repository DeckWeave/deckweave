# Hardware providers

Один provider обнаруживает и обслуживает независимые instances, описывает geometry/capabilities и input/output. N4 transport/codec/renderer — crates/deckweave-n4, desktop lifecycle — src-tauri/src/hardware.rs, модель — Core. Внешний process host — src-tauri/src/device_providers.rs; provisional SDK/шаблон находятся в соседних plugin-sdk/plugin-template. Контракт ещё не стабилизирован; [точный поддержанный объём](../docs/PROVIDER-CONTRACT.md), [lifecycle и настройки](../docs/DEVICE-EXTENSIONS.md).

Windows supervisor обнаруживает устройства раз в 2 секунды; отдельный worker каждого identity владеет HID handle, повторяет подключение и выполняет heartbeat. Stable hardware_key не меняет instance/назначения при reconnect; generation отсекает старые события/кадры. Disconnect очищает удержания и jobs без исполнения release действия. Окно не владеет session; tray оставляет workers активными.

Frames latest-wins, изменённые targets обходятся по кругу; одна атомарная BAT/BGPIC/chunks/STP транзакция между чтениями. Core lock не покрывает USB/render/file I/O. Preview получает тот же JPEG с page/binding identity. OUT считает успешные writes и не заменяет наблюдение дисплея.

Input key codes 1–10 независимы от BAT output addresses. N4 touch adapter использует X и optional Pointer Y, общие raster rectangles и захват исходного contact. Native widget владеет drag/swipe; обычный tap отменяется при движении/таймауте. Неизвестный Y не выбирает вертикальную область по догадке. Capability down/up/hold не заявляется по одному firmware click.

[Panel contract](../docs/PANEL-CONTRACT.md), [driver/provenance](../crates/deckweave-n4/README.md), [аппаратная приёмка](../docs/ACCEPTANCE-NEXT.md), [план](../docs/PLAN.md). Несколько физических N4 одновременно пока не проверены; software tests покрывают изоляцию instances и stale generation.
