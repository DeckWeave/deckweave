# Hardware providers

Ответственность: один provider обнаруживает и обслуживает независимые instances, описывает geometry/capabilities и input/output. Виртуальная модель находится в `crates/deckweave-core/src/model.rs`; N4 transport/codec/renderer — в `crates/deckweave-n4`, desktop lifecycle — в `apps/desktop/src-tauri/src/hardware.rs`. Внешний process provider и публичный SDK ещё не реализованы.

Windows supervisor обнаруживает устройства раз в 2 секунды; отдельный worker каждого identity владеет HID handle, повторяет подключение и выполняет heartbeat. Identity сохраняется как hardware_key, instance ID/назначения не создаются заново при reconnect. Generation отсекает старые input/frame completions. Disconnect очищает удержания без запуска действия. Окно настроек не владеет аппаратной сессией.

Желаемые кадры пересчитываются по изменениям Core; очередь не накапливает старые картинки. Между чтениями отправляется одна полная транзакция, изменённые кнопки обходятся по кругу. Core lock не покрывает USB/JPEG/файлы. Preview получает тот же JPEG, который передан транспорту, с page/binding identity. Счётчик OUT подтверждает успешные HID writes, не визуальную приёмку дисплея. Диагностика ограничена последними 256 входами и пишется отдельно от конфигурации.

После первой физической приёмки исправлена входящая карта N4: key codes 1–10 отличаются от BAT output addresses. Добавлены BGPIC output полосы и single-contact adapter `src-tauri/src/touch.rs`. Touch down/up опираются на захваченные code 00/40–43 и X; firmware zone hints не определяют native widget layout. Hit testing и strip renderer используют одни raster rectangles. Движение/таймаут отменяет контакт без tap. Остальные gestures и внешние произвольные layouts остаются следующими этапами.

Целевой контракт: discover/open/input/present/close. Instance identity, generation и control IDs обязательны. Один disconnect не завершает siblings. Images bounded/latest-wins, digital edges ordered. Hardware down/up/hold не заявляются без подтверждения устройства.

Не отвечает за пользовательские actions и UI windows. Низкоуровневые N4 codec/fixtures можно переносить выборочно; старый apply protocol не переносится. [Архитектура](../docs/ARCHITECTURE.md), [план](../docs/PLAN.md).

Текущие ограничения/карта событий и лицензии: [N4 adapter](../crates/deckweave-n4/README.md). Аппаратная приёмка: [чеклист](../docs/N4-ACCEPTANCE.md). Несколько физических N4 одновременно пока не проверялись; независимость instances и stale generation проверяется автоматическими тестами.
