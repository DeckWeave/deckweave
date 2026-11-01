# Сенсорные панели

Ответственность: surface geometry, native panel layouts, области, hit testing, feedback и dial associations. Первый срез описывает независимую сенсорную поверхность и фиксированные тестовые touch-контролы виртуального устройства. Полный panel plugin/compositor реализуется фазой 6.

В аппаратном срезе N4 добавлены 800×128 strip renderer/BGPIC output и single-contact adapter. Пять тестовых областей T1–T5 берут назначения из Core; первые четыре также показывают значения D1–D4 как проверочный индикатор. Это временная раскладка приёмки, не постоянное правило связывания действий/крутилок. Preview показывает тот же JPEG, а hit testing использует те же преобразованные pixel rectangles. Сейчас адресат Down определяется по X в областях полной высоты; ARX X/Y позволяет отменять движение. Незавершённый контакт отменяется через 3 секунды. Полноценные произвольные 2D widgets, hold, gesture arbitration и публичный plugin API ещё не реализованы.

Целевой контракт: panel layout, regions с action bindings, локальные координаты и feedback. Собственная панель поддерживает произвольное число виджетов, включая раскладку целиком от плагина. Подготовленные области могут содержать совместимые Keypad/Encoder actions; dial association не является обязательным условием существования области.

Preview/hardware/hit test используют одну geometry. Свайп и tap не исполняются одновременно. Elgato adapter соблюдает исходную logical area и event semantics; native panel API не наследует ограничение одной области на dial. Не определяет raw hardware contact lifecycle — он приходит из подтверждённых capabilities provider. [Исследование](../docs/research/2026-11-01/ADVANCED-INTERACTIONS.md), [архитектура](../docs/ARCHITECTURE.md), [план](../docs/PLAN.md).
