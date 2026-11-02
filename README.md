# DeckWeave

Новая реализация настольного редактора устройств управления. Основа продукта — привычное поведение Elgato Stream Deck, расширяемое поддержкой стороннего оборудования и плагинов действий.

Первая целевая ОС — Windows 11; в перспективе — macOS и Linux.

**Статус: следующий рабочий срез с Mirabox N4 Pro.** Реестр/отдельные окна, tray/фон, Elgato native/Node/HTML backend и оригинальные PI, навигация и составные действия, Smart Profiles, native panel widgets/regions, Dial Stack/Action Wheel, en/ru. Установленный OnAirClock прошёл сквозную проверку. Полный v1 ещё не завершён: [возможности и ограничения](docs/STATUS.md), [доказательства](docs/RUNTIME-PROGRESS.md).

## Запуск

Нужны Node.js 22.12+ и Rust 1.98.1, а на Windows — MSVC Build Tools и WebView2. Windows HTML plugins используют установленный Edge; Node plugins — установленный Node. Runtime distribution в installer ещё не завершена. Из корня репозитория:

```powershell
npm ci
npm run dev
```

Первая проверка: открыть Virtual Studio → выбрать кнопку → назначить действие справа → «Проверить». Выбор не выполняет действие. Изменение названия сохраняется без Apply. Counter/Toggle не выполняют системных команд; их runtime значения сбрасываются при restart, конфигурация сохраняется.

Для Mirabox закройте StreamDock и старый DeckWeave, затем откройте карточку N4. Исходный диагностический профиль содержит K1–K10, D1–D4 и T1–T5; новые страницы получают связанные dial regions. Через «Сенсорная панель» доступны fullscreen и произвольные области. Собственный пример — DeckWeave Panel Lab. [Границы драйвера](crates/deckweave-n4/README.md), [следующая аппаратная проверка](docs/ACCEPTANCE-NEXT.md).

```powershell
npm run check
npm run build
```

`npm run build` собирает native release без установщика. Проверенный executable: `target/release/deckweave-desktop.exe` (также собран debug). Запуск только `npm run dev:web` показывает сообщение о необходимости desktop runtime, без подмены Rust локальным браузерным fake store.

Данные хранятся в app data идентификатора `app.deckweave.studio`, файл `deckweave.sqlite3`. Старый DeckWeave не изменяется. Для изоляции разработки предусмотрен `DECKWEAVE_DATA_DIR`. Закрытие всех окон оставляет runtime в tray; «Выйти» завершает workers/backends. Реестр открывается из tray или повторным запуском executable.

## Документация

- [Ближайший результат и порядок дальнейшей работы](docs/NEXT.md).
- [Правила разработки и проверки коммитов](CONTRIBUTING.md).
- [Лицензии и происхождение включённых материалов](THIRD_PARTY.md).

- [Архитектура и границы компонентов](docs/ARCHITECTURE.md).
- [Этапы реализации и проверки](docs/PLAN.md).
- [Интерфейс](docs/UI.md).
- [Первые измерения ресурсов новой сборки](docs/PERFORMANCE.md).

- [Текущий статус и продолжение работы](docs/STATUS.md).
- [Принятые решения: Rust + Tauri 2 + Svelte и поведение продукта](docs/DECISIONS.md).
- [Подробный обход установленного Stream Deck](docs/research/2026-11-01/STREAM-DECK-WALKTHROUGH.md).
- [Multi Action, Switch, состояния кнопок, крутилки и touch](docs/research/2026-11-01/ADVANCED-INTERACTIONS.md).
- [Ajazz/StreamDock и ограничения их использования как референса](docs/research/2026-11-01/REFERENCE-IMPLEMENTATIONS.md).
- [Ограничения исследования и дальнейшие проверки](docs/research/2026-11-01/RESEARCH-GAPS.md).
- [Результаты исследования от 2026-11-01](docs/research/2026-11-01/README.md).
- [Вопросы и полученные ответы владельца](docs/research/2026-11-01/QUESTIONS.md).
- [Проверки плагинов](docs/research/2026-11-01/COMPATIBILITY.md).
- [Производительность и выбор интерфейса](docs/research/2026-11-01/PERFORMANCE.md).
- [Поведение редактора и будущая приёмка](docs/research/2026-11-01/UX-ACCEPTANCE.md).
- [Изученная основа IO Type 84](docs/research/2026-11-01/IO-TYPE-84.md).

Соседние репозитории `../plugin-sdk`, `../plugin-template` и `../plugin-registry` содержат SDK действий, предварительный device protocol, отдельные генераторы action/device и типизированный реестр. Готовы System Audio, System Monitor и Timers; Weather остаётся сетевым примером. [Установка из ZIP и каталога OpenDeck](docs/EXTENSIONS.md), [удаление расширений, настройки Mirabox, локальный IO Type 84 и создание собственного драйвера](docs/DEVICE-EXTENSIONS.md). Внешний device SDK ещё не стабилизирован; LCD/touch output/input остаются ограничением текущего host.
