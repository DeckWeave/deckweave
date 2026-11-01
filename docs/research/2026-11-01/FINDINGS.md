# Выводы по проектам и кандидаты переноса

Дата: 2026-11-01. Предложения исследователя, не принятый дизайн. Продуктовые решения после обсуждения — в [DECISIONS](../../DECISIONS.md); они заменяют ранние открытые варианты.

## Состояние репозиториев

`G:\APPS\DW\deckweave`, `plugin-sdk`, `plugin-template`, `plugin-registry` до этого исследования содержали только Git metadata, без commits и исходников. Новый продукт не является уже существующей новой веткой старого DeckWeave.

| Проект | Версия исследования | Назначение |
|---|---|---|
| Старый DeckWeave | HEAD `b3098df5df16253031a8703c579fe9ce7e22e38f`; tracked modifications не обнаружены | Rust core, Tauri/Svelte editor, N4, Windows packages, compatibility spike |
| Локальный OpenDeck | `772189ca74c89a10bad219b67de620ecb07bf1f6` | Референс plugin runtime, lifecycle, profiles и device plugins |
| Companion surface | HEAD `32d44806e2eefb9ce4431300d13dfb0928007315` плюс локальные изменения | TypeScript Mirabox transport и интеграция в Companion |
| Mirabox panel | HEAD `c383ae6df9673bfa8d8c01a38cedaf5d50b5ef6d` плюс локальные изменения | Готовые панели, layout и bridges поверх Companion |

Хеши выбранных файлов — [source-hashes.json](evidence/source-hashes.json). Описание локальных изменений Companion/Mirabox нельзя приписывать только указанным commits. Upstream OpenDeck дополнительно просмотрен через первичные web-источники; живой main не отождествляется с локальным commit.

## Что объясняет проблемы старого редактора

В `ui/desktop/src/editor/session.svelte.ts` одновременно существуют draft, document, activeDocument, review, authoringSelection и navigationShown. Начальное `liveEnabled=false`, liveStatus=paused; сохранённая настройка конкретного профиля может это изменить.

В `navigation-follow.ts` переход на устройство требует доступной активной навигации и равенства active document документу редактора. Иначе появляется ошибка о необходимости применить конфигурацию/страницу. Следовательно, изменение authoring selection само по себе не гарантирует смену физической страницы. Это подтверждённое устройство кода; точная последовательность пользовательского отказа не восстановлена.

Тесты `navigation-acceptance.test.ts` включают проверки writers/fixtures и preview client. Они полезны для структуры данных, но не доказывают прохождение через USB, работающий backend и реальный пользовательский экран. Ошибкой следующего проекта будет засчитывать такой тест вместо сквозной приёмки.

Новая модель должна скрыть техническую координацию записи/активации внутри приложения. Валидация, атомарная запись и восстановление после сбоя всё равно нужны. Удаление пользовательской кнопки Apply не означает отказ от консистентности. Конкретный store и формат пока не выбраны.

## Что стоит забрать

| Компонент | Оценка | Граница переноса |
|---|---|---|
| N4 `report.rs`, `input.rs`, `session.rs` | Сильный кандидат | Декодеры/пакеты/fixtures; новые общие event DTO |
| N4 `display.rs`, RGB/haptic, serialized USB transactions | Сильный кандидат после изоляции | BAT/BGPIC/JPEG, keepalive, порядок передачи; убрать привязку к старому document/apply protocol |
| N4 runtime workers | Переработать | Уже различает несколько configured device IDs и workers; перепроверить поколения, ошибки одного экземпляра и reconnect |
| Windows Audio/window/media adapters | Выборочно | Низкоуровневые Win32/COM вызовы и tests; новый простой inspector/action surface |
| Raw Input reader | Выборочно | Использует device handle внутри; публикуемый источник в старом bridge — aggregate keyboard |
| Elgato scanner/session/launch | Испытательная основа | Добавить PI, persistence, реальные device descriptors, installation и общий browser lifecycle |
| Старые widgets | По одному сценарию | Брать понятные алгоритмы/API, не переносить всю модель presentation/automation |
| Canvas/scene editor, compiler graph, grants/apply UX | Не переносить как базовую модель продукта | Геометрия устройства остаётся полезной, свободный редактор сцены откладывается |
| Companion panel | Сценарии и визуальные идеи | Зависит от Companion, не является самостоятельным драйвером нового приложения |

Существующий N4 test executable от 2026-10-22: **37 passed**; scanner executable: **3 passed**. Журналы и SHA-256 в evidence. Binaries не пересобирались на HEAD, поэтому эти результаты подтверждают выбранные артефакты, а не сегодняшнюю сборку исходников. Hardware acceptance заново не выполнялась.

## N4: полезные ограничения

Подтверждённая старым кодом модель: 10 keys, четыре энкодера/нажатия, четыре RGB-кольца, глобальное включение встроенной вибрации, экран с общим framebuffer 800×480. Физические кнопочные изображения 112×112; отдельные исторические strip paths имеют различные размеры/маски. Нельзя смешивать transport coordinates, видимые окна корпуса и touch coordinates.

Rust decoder различает обычный ACK/OK и ACK/ARX с big-endian X/Y. В этом коде энкодерное нажатие является click event, а touch — только sample координат. Down/up/hold/swipe/multitouch нельзя дописывать в descriptor без доказательств. Для tap по sample-потоку возможна явная эвристика, но её нельзя выдавать за аппаратный contact lifecycle.

Для редактора как у Elgato не нужен свободный scene canvas. Достаточно фиксированной геометрии плагина и назначения картинок/подписей/feedback областям. При этом внутренний compositor может собирать единый аппаратный кадр, чтобы особенности LCD не попадали в пользовательскую модель.

Из старого transport стоит сохранить сериализацию целой передачи header/chunks/STP, bounded latest-frame-wins и кеш неизменённых изображений. Нажатия и отпускания не должны теряться из-за кадров или объединяться по правилу latest-wins. Heartbeat важен даже без открытого редактора.

## Два разных расширения

**Плагин оборудования** обнаруживает несколько экземпляров, управляет их соединениями, читает ввод, принимает display/light outputs и описывает неизменяемую геометрию. **Плагин действий** предоставляет команды/feedback и их настройки. Один пакет может содержать обе роли, но роли не следует смешивать в модели редактора.

Пакет ≠ процесс ≠ модель оборудования ≠ физический экземпляр ≠ открытое окно. Принятый владельцем смысл «один обработчик — несколько устройств»: один provider обслуживает несколько instances, у каждого стабильная идентичность, собственный профиль, навигация и окно. Отключение одного instance не выключает siblings. Синхронные группы отложены в [Q-07](QUESTIONS.md).

Не следует делать SDK зависимым от Tauri или GPUI. Описание контролов должно иметь стабильные IDs, kind, geometry и capabilities, а не Rust UI closures или DOM fragments. Это позволит одному host отображать N4, клавиатуру и удалённую панель. Точный transport/serialization/versioning ещё не утверждены; старый большой Protobuf-протокол не является обязательным выбором.

OpenDeck уже умеет registerDevice/deregisterDevice и события keys/encoders от device plugin. Его `DeviceInfo` преимущественно описывает rows/columns/encoders/touchpoints/infobars; произвольные формы клавиатуры и analog axes требуют расширения. Поэтому совместимость action-плагинов Elgato и собственный hardware SDK разумно развивать отдельно. [OpenDeck source](https://github.com/nekename/OpenDeck/tree/772189ca74c89a10bad219b67de620ecb07bf1f6/src-tauri/src).

## Клавиатура и мышь

Windows Raw Input различает источник клавиатуры/мыши и поддерживает фоновое получение. Это основа наблюдения, а не обещание системного переназначения с блокировкой исходного ввода. `RIDEV_NOLEGACY` относится к legacy messages зарегистрированного приложения; он не заменяет универсальное подавление ввода во всех остальных приложениях. [Microsoft overview](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input), [flags](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinputdevice).

Старый `platform-windows/src/input/bridge.rs` публикует `system-keyboard-aggregate`. Несмотря на внутренние device handles, его нельзя обещать как готовый драйвер для настройки каждой физической клавиатуры отдельно.

Владелец указал IO Type 84 и существующий прототип `G:\APPS\io_type_84`. Исходники и сохранённая аппаратная проверка подтверждают наблюдение глубины, в том числе полный ход PgUp/PgDn на White 1.17; вся матрица не принята. Есть отдельные модели onboard bindings, lighting и performance. Текущий USB open отказывает при нескольких устройствах, поэтому provider нужно адаптировать. [Подробный разбор и границы](IO-TYPE-84.md). Wooting остаётся лишь сторонним референсом API, не основанием поддержки этой модели.

Для будущей аналоговой поддержки полезно предусмотреть значение оси, диапазон/единицы и sample time отдельно от digital edge. Порог и hysteresis для запуска действия — отдельная политика, а не притворный keyDown в драйвере. Владелец отложил глубокие настройки IO Type 84 и Logitech MX4; первый общий срез — кнопки/колесо и наблюдение. Настройки всего устройства должны быть отдельно от назначения действий на контролы.

## Android-панель

Телефон должен появляться как device instance сетевого provider с тем же назначением действий. На телефоне можно рисовать готовые key images, не отправляя ему исполняемый action-плагин. Начальная поддержка desktop не требует сейчас писать Android-клиент.

Стоит заранее оставить различие transport, session identity, input sequence и display output. При reconnect нельзя повторять старые launch/recording actions; накопленные устаревшие картинки можно заменять последними, а потерю pressed state нужно явно сбрасывать. Это тот же класс задач, что USB unplug, а не отдельный automation engine.

Android NSD даёт обнаружение сервисов в LAN, но не заменяет pairing/authentication. Интернет-доступ, фоновые Android-ограничения и конфигурирование layout на телефоне остаются последующими задачами. [Android NSD](https://developer.android.com/develop/connectivity/wifi/use-nsd).

## Репозитории и происхождение

В `deckweave` предлагается держать host, UI и встроенные обязательные возможности; в соседних репозиториях — публичный SDK, минимальный шаблон и каталог устанавливаемых пакетов. Необязательный плагин можно развивать отдельно под `G:\APPS\DW`. Каталог должен отличать «обнаружен», «есть Windows artifact» и «проверен в exact version».

Владелец подтвердил прежнюю схему лицензирования: GPL-3.0-or-later для приложения, MIT OR Apache-2.0 для SDK/шаблонов. У Companion surface — MIT. Исходники/manifest других авторов в новый repo не переносились; в evidence лежат результаты и изображения проверок. Публикация не выполнялась. [Q-10](QUESTIONS.md).
