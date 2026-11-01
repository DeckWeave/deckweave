# Архитектура DeckWeave

Дата: 2026-11-01. Технический дизайн на основе [принятых решений](DECISIONS.md). Владелец разрешил самостоятельно принимать технические решения, готовить архитектуру и переходить к коду. Повторное согласование стека, хранения и разбиения модулей не требуется. Реализованный объём отслеживается отдельно в [STATUS](STATUS.md).

## Системный контекст

```text
Реестр / окна устройств (Svelte)
             | типизированные команды / snapshots / события
             v
         Desktop host (Tauri 2)
             |
             v
        Runtime приложения ------ SQLite / assets
         |       |       |
         |       |       +---- panel compositor / layout / hit testing
         |       +------------ action executors / Elgato compatibility host
         +-------------------- hardware providers -> независимые instances
```

Модульный монолит с изолированными внешними плагинами. Нет обязательного локального HTTP-сервера, микросервисов и распределённой БД. SDK не зависит от Tauri/Svelte. Первые builtins и виртуальный provider работают в Rust-процессе; внешние provider/action процессы подключаются позднее через те же предметные границы.

## Компоненты

| Компонент | Ответственность | Контракт / расположение |
|---|---|---|
| Core | Модели, валидация, независимые instances, навигация, ввод и feedback | `crates/deckweave-core`, [модуль](../modules/runtime.md) |
| Store | Схема SQLite, атомарная запись конфигурации, версии | `crates/deckweave-core/src/store.rs`, [модуль](../modules/storage.md) |
| Desktop | Окна, IPC, глобальная editing session, lifecycle | `apps/desktop/src-tauri`, [модуль](../modules/desktop.md) |
| Editor | Реестр, геометрия, инспектор, список действий, ru/en | `apps/desktop/src`, [модуль](../modules/editor.md) |
| Hardware boundary | Обнаружение, geometry/capabilities, input/output для многих instances | `crates/deckweave-n4`, `src-tauri/src/hardware.rs`, [контракт и этап](../modules/providers.md) |
| Actions boundary | Builtins, составные действия, external plugin lifecycle | [контракт и этап](../modules/actions.md) |
| Panels boundary | Раскладка поверхности, области, plugin feedback, касания и связь с крутилками | [контракт и этап](../modules/panels.md) |

Core/Store/Desktop/Editor работают в ограниченном первом срезе. Встроенный N4 adapter уже подключён; внешний hardware plugin SDK, external actions и panel plugins ещё предстоит реализовать. Наличие документа не считается работающей загрузкой сторонних плагинов.

## Состояние и data flow

1. Provider или виртуальная команда передаёт ввод с instance/control identity.
2. Runtime определяет активный профиль/страницу и сохраняет адресата down до соответствующего up.
3. Action executor меняет runtime feedback либо посылает событие плагину.
4. Один snapshot описывает активную навигацию, pressed и feedback. N4 worker строит из него key frames вне Core lock; preview получает те же JPEG после успешной передачи HID. Полный panel compositor ещё предстоит реализовать.
5. Команда редактирования строит и валидирует новую конфигурацию. Store записывает её транзакцией; только после успеха runtime публикует новую revision. При ошибке прежняя конфигурация остаётся активной.

Selection существует только в окне редактора: выбор не вызывает executor. Активная страница хранится в Rust, не дублируется скрытым authoring cursor. Конфигурация отличается от краткоживущего pressed, счётчика демонстрационного действия и внешнего состояния программы. Последние не выдаются за подтверждённый аппаратный кадр или сохранённую настройку.

## Ключевые интерфейсы

Первый IPC: `snapshot`, `add_device`, `rename_device`, `configure_control`, `add_page`, `select_page`, `input`, `set_locale`; каждая изменяющая команда возвращает актуальный snapshot. Запись конфигурации принимает ожидаемую revision; конфликт возвращается явно. Инвалидация остальных окон идёт через `deckweave:changed`. Событие — повод получить snapshot, не второй источник состояния.

Будущие plugin contracts:

```text
HardwareProvider: discover -> InstanceDescriptor[]; open(instance); input; present; close
ActionExecutor: appear/disappear; input(context); settings; feedback
PanelProvider: layout(surface); region bindings; input(zone, local coordinates); feedback
```

Control имеет стабильный ID, тип, прямоугольник в логических координатах и capabilities. Instance ID отличается от provider/model/control ID. Action instance ID отличается от координат: смена страницы не превращает release старой кнопки в нажатие новой. Пакет плагина и процесс также разные сущности.

## Навигация, ввод и составные действия

Активная навигация — профиль/страница/папка конкретного instance. Папки, pinned, Smart Profiles и составные действия добавляются по [плану](PLAN.md); первая модель не объявляет их реализованными. Press latch хранит исходный action instance; отпускание после перехода не исполняет назначение новой страницы. Disconnect завершает удержания и инвалидирует поколение сессии, без повторного воспроизведения старых команд.

Scheduler составных действий будет последовательным по run, с отменой и монотонным временем. Delay не блокирует UI или глобальную блокировку. Switch, action state, pressed и Key Logic gesture — отдельные значения. `userDesiredState` шага не теряется при преобразовании. На необъяснённые микродетали Elgato заводятся conformance fixtures; отсутствие эталонного устройства не блокирует основу приложения.

## Сенсорная панель

Surface не ограничена количеством крутилок. Поддерживаем собственный panel plugin, расположение виджетов и областей, а также шаблон областей по крутилкам. В область можно поместить совместимый action; связь с dial задаётся отдельно. Отображение и hit testing используют одну геометрию. При масштабировании сохраняется преобразование из device координат в surface и локальные координаты области.

Elgato Encoder adapter сохраняет логическую область feedback, тип событий и context; его ограничения не становятся ограничениями native panel API. Keypad action в сенсорной области получает осмысленную пару down/up, а не ложную Encoder capability. Самостоятельный panel plugin может компоновать несколько областей и свои элементы. Приоритеты жестов/оверлеев должен разрешать host, чтобы один свайп не вызывал одновременно навигацию и tap. Первое приложение показывает геометрию поверхности; собственные внешние panel plugins и полный compositor ещё предстоит реализовать.

## Окна и Smart Profiles

Реестр открывает одно окно на instance или поднимает существующее. Настройки любого устройства удерживают глобальную паузу Smart Profiles. Последнее закрытое окно настроек снимает паузу; закрытие одного из нескольких не снимает. Обычные действия продолжают работать. Владелец окна берётся из фактического desktop lifecycle, не из произвольного boolean, присланного UI.

Runtime принадлежит приложению, не отдельному editor. В первом срезе закрытие всех окон завершает приложение; tray и корректная фоновая работа включены отдельной обязательной фазой до полноценного выпуска. Не считать существование счётчика editing sessions реализацией foreground monitoring.

## Хранение и конкурентность

SQLite хранит versioned JSON-конфигурацию в одной строке `configuration`, schema version — в `PRAGMA user_version`. Это позволяет начать с небольшого формата без ORM и одновременно сохранить транзакционный commit. Новая schema version не затирается старым приложением. Ошибка чтения/декодирования показывается, а не заменяет пользовательские данные пустыми настройками.

В первом срезе короткие команды сериализуются одним владельцем Core под mutex; IO плагинов и задержки под ним запрещены. Дальше runtime получает отдельную очередь команд и асинхронные executors, при сохранении одного владельца состояния. Очереди кадров bounded/latest-frame-wins, down/up — ordered и не объединяются. Подробности: [ADR хранения](adr/0002-storage.md), [ADR конкурентности](adr/0003-runtime.md).

## Плагины и HTML lifecycle

Hardware plugins, action plugins и panel plugins имеют разные роли и versioned manifests. Публичные SDK/шаблоны размещаются в соседних репозиториях; сейчас не публикуем нестабильный wire protocol как готовый SDK.

Elgato adapter: manifest scan, native/Node/HTML backend, localhost WebSocket registration, contexts, PI, настройки и feedback. PI отделён от редактора и не получает desktop command bridge. Закрытие PI не останавливает backend. Общий browser environment имеет одного владельца: завершение плагина не уничтожает разделяемое дерево WebView. Совместимость уровня OpenDeck, без обхода защиты закрытых пакетов. Импорт чужих профилей и миграция старого DeckWeave исключены.

Выбранные способы внешнего расширения и композиции описаны в [ADR процессов](adr/0005-plugin-processes.md) и [ADR панелей](adr/0006-panel-composition.md). Их реализация идёт последующими фазами; эти контракты не объявляются уже работающим публичным SDK.

## Режимы отказа

| Сбой | Поведение |
|---|---|
| Ошибка записи | Прежняя revision активна, ошибка видна у команды; никаких полуприменённых назначений |
| Устаревшая revision | Отклонить изменение, получить свежий snapshot; не затирать молча |
| Поздний feedback | Проверить instance/action/session generation; не перерисовать новую страницу |
| Плагин завершился | Пометить его назначения недоступными, сохранить конфигурацию; siblings продолжают работать |
| USB disconnect | Offline, очистка удержаний, reconnect без повторения старых действий |
| Повреждённая/новая БД | Понятная ошибка и сохранение файла; без скрытого reset |
| Ошибка layout | Сохранить прежнюю работающую раскладку, показать конкретную ошибку |

## Технологические решения и границы v1

Rust + Tauri 2 + Svelte 5/Vite; SQLite через rusqlite, serde DTO; en/ru в оболочке. Зависимости фиксируются lock-файлами. Нет постоянного UI polling в простое. Расход всего дерева процессов измеряется на работающих сценариях, без придуманного лимита.

В полноценном v1 обязательны отдельные окна, N4, совместимые Elgato-плагины, независимые instances, навигация, Multi Action/Delay/Switch, Smart Profiles, Key Logic, pinned, Dial Stack/Action Wheel и native panels. За его пределами: Android, синхронные группы, расширенные IO84/MX4 настройки, импорт старых данных. Первая реализация — часть v1, а не переопределение его объёма.

Первичные технические источники: [Tauri IPC](https://v2.tauri.app/develop/calling-rust/), [state management](https://v2.tauri.app/develop/state-management/), [capabilities](https://v2.tauri.app/security/capabilities/), [SQLite transactions](https://www.sqlite.org/transactional.html), [rusqlite](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html). Продуктовые источники и границы доказательств — в [исследовании](research/2026-11-01/README.md).
