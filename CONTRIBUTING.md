# Разработка DeckWeave

Первый рабочий срез: Rust Core, Tauri 2/Svelte, встроенный N4 adapter. Полный объём продукта и ограничения записаны в [статусе](docs/STATUS.md); [ближайший этап](docs/NEXT.md) отделён от всего [плана](docs/PLAN.md).

## Окружение и команды

Первая поддерживаемая среда — Windows 11, MSVC Build Tools с Windows SDK, WebView2, Node.js 22.12+ и Rust 1.98.1. Версия Rust и компоненты заданы в rust-toolchain.toml. На текущем ПК проверки выполнялись с Node.js 26; другие ОС ещё не проверены.

```powershell
npm ci
npm run check
npm run dev
```

`npm run check` выполняет Svelte check, Prettier, production frontend build, cargo fmt, clippy с запретом предупреждений, Rust tests и проверку локальных ссылок документации. Lock-файлы входят в Git; Rust-проверки используют --locked. При уже заполненном Cargo cache допускается CARGO_NET_OFFLINE=true.

`npm run build` собирает release executable в target/release без установщика. Браузерный dev:web предназначен для разработки оболочки и требует native runtime для действий. Проверки Core/codec не требуют физического устройства; полноценный аппаратный сценарий проверяется отдельно по [N4-ACCEPTANCE](docs/N4-ACCEPTANCE.md).

Для изолированного desktop-сеанса можно задать DECKWEAVE_DATA_DIR на отдельный каталог. Обычная БД находится в app data идентификатора app.deckweave.studio. Не копировать её в Git и не сбрасывать при ошибке чтения. StreamDock и другие управляющие программы перед аппаратной проверкой закрываются.

## Состав коммитов

В Git входят исходники, конфигурация сборки, lock-файлы, лицензии, документация и выбранные проверочные fixtures/evidence. Target, node_modules, dist, generated Tauri files, .local, .env и базы данных исключены. Исторические пути/хеши в evidence описывают конкретный проверенный запуск и не являются настройками нового компьютера.

Перед коммитом выполнить npm run check и git diff --cached --check, проверить список staged files. Изменение runtime/UI сопровождается актуальным STATUS и применимыми критериями приёмки. Новый протокол нельзя считать подтверждённым по одному совпадению теста с реализацией: нужны исходный raw capture и физическая проверка, когда она применима.

Сохранять границы Core, hardware transport, desktop lifecycle и UI. USB/JPEG/внешние plugin процессы не выполняются под Core lock. Выбор элемента не исполняет действие; draft/apply не возвращается. Не объявлять тестовую touch-раскладку готовым panel SDK.

Лицензия приложения — GPL-3.0-or-later. Происхождение включённых материалов — в [THIRD_PARTY](THIRD_PARTY.md); лицензирование будущих SDK/шаблонов описано в [DECISIONS](docs/DECISIONS.md).
