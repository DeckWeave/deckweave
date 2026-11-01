# ADR-0002: Транзакционное сохранение в SQLite

Дата: 2026-11-01. Статус: принято в рамках делегированных технических решений.

Варианты: JSON-файл с platform-specific atomic replacement; SQLite. Выбрана SQLite через rusqlite с bundled SQLite: одна versioned JSON-конфигурация, schema version и транзакционная запись. ORM не требуется.

Изменение валидируется, commit выполняется до публикации новой конфигурации runtime. Ошибка не оставляет частично применённый документ. Неизвестная schema version или повреждение не приводят к автоматическому сбросу данных. Runtime feedback/pressed хранятся отдельно от конфигурации.

Последствия: появляется SQLite-зависимость, зато не нужен собственный Windows replacement/recovery protocol для каждого сохранения. Небольшой первый формат можно развивать через миграции собственных версий; импорт старого DeckWeave исключён. [SQLite](https://www.sqlite.org/transactional.html).
