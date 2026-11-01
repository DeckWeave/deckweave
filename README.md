# DeckWeave

Новая реализация настольного редактора устройств управления. Основа продукта — привычное поведение Elgato Stream Deck, расширяемое поддержкой стороннего оборудования и плагинов действий.

Рабочий репозиторий: `G:\APPS\DW\deckweave`. Первая целевая ОС — Windows 11; в перспективе — macOS и Linux.

**Статус: desktop-срез с Mirabox N4 Pro.** Реестр устройств, отдельные окна, страницы, назначения Counter/Toggle, «Проверить», pressed/feedback, сохранение и en/ru. Владелец подтвердил кнопки после исправления входящей карты, изображения, крайние touch области и первую крутилку с индикатором на полосе. Сторонние плагины, полноценные panels и сложные действия ещё предстоит подключить; это не готовый выпуск.

## Запуск

Нужны Node.js 22.12+ и Rust 1.98.1, а на Windows — MSVC Build Tools и WebView2. Из корня репозитория:

```powershell
npm ci
npm run dev
```

Первая проверка: открыть Virtual Studio → выбрать кнопку → назначить действие справа → «Проверить». Выбор не выполняет действие. Изменение названия сохраняется без Apply. Counter/Toggle не выполняют системных команд; их runtime значения сбрасываются при restart, конфигурация сохраняется.

Для Mirabox закройте StreamDock и старый DeckWeave, затем откройте карточку N4 в реестре. Первый запуск создаёт безопасные K1–K10, D1–D4 и T1–T5. Полоса показывает тестовые области и значения крутилок; произвольные panel plugins ещё не реализованы. [Границы драйвера и происхождение](crates/deckweave-n4/README.md), [аппаратная проверка](docs/N4-ACCEPTANCE.md).

```powershell
npm run check
npm run build
```

`npm run build` собирает native release без установщика. Проверенный executable: `target/release/deckweave-desktop.exe` (также собран debug). Запуск только `npm run dev:web` показывает сообщение о необходимости desktop runtime, без подмены Rust локальным браузерным fake store.

Новые данные хранятся в app data каталоге идентификатора `app.deckweave.studio`, файл `deckweave.sqlite3`. Старый DeckWeave не изменяется. Для изолированного запуска можно задать `DECKWEAVE_DATA_DIR`. Закрытие всех окон пока завершает процесс; tray/фоновая работа входят в следующий этап.

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

Соседние репозитории `../plugin-sdk`, `../plugin-template` и `../plugin-registry` предусмотрены владельцем для SDK, шаблонов и каталога необязательных плагинов. Их содержимое и публичные контракты пока не созданы.
