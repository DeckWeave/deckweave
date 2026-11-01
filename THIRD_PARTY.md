# Происхождение включённых материалов

Код приложения распространяется по GPL-3.0-or-later, полный текст — [LICENSE](LICENSE).

| Материал | Происхождение | Условия |
|---|---|---|
| Низкоуровневые N4 packet layouts и часть подхода к рендерингу | Старый DeckWeave, commit b3098df5df16253031a8703c579fe9ce7e22e38f; адаптация для новой реализации | GPL-3.0-or-later; подробности в [README драйвера](crates/deckweave-n4/README.md) |
| IBM Plex Sans, assets/IBMPlexSans.ttf | IBM Corp., файл без изменения перенесён из старого проекта | SIL Open Font License 1.1; copyright и Reserved Font Name сохранены в [OFL.txt](crates/deckweave-n4/assets/OFL.txt) |
| Пакеты Rust/npm | Зависимости через соответствующие package managers, версии зафиксированы lock-файлами | Лицензии отдельных зависимостей задаются их авторами; этот документ не перелицензирует их |
| image / resvg / usvg / tiny-skia | Rust crates для bounded bitmap/SVG rendering; версии в Cargo.lock | image: MIT OR Apache-2.0; resvg/usvg/tiny-skia: MIT OR Apache-2.0; embedded artwork чужих плагинов не включён |
| Скриншоты и диагностические журналы в docs | Материалы локального исследования/приёмки, включая предоставленные владельцем изображения | Проверочные материалы, не UI assets приложения и не поставка сторонних программ |

SDK Mirabox использован как документированный референс структуры протокола; Python runtime/исходники SDK в поставку не включены. Адреса входа и выхода проверяются отдельно, по raw capture владельца.

Binaries и исходники исследованных сторонних Elgato-плагинов в репозиторий не включены. Наличие названий, версий и хешей в документации не означает их поставку или гарантию совместимости.

Node.js и Microsoft Edge/WebView2 используются как установленные runtime; их binaries не включены в этот репозиторий. Собственные diagnostic/Panel/OBS пакеты в plugins являются исходниками DeckWeave под GPL-3.0-or-later, не копиями установленных сторонних пакетов. Declarative Encoder layout keys/bounds реализованы по [официальному контракту SDK](https://docs.elgato.com/streamdeck/sdk/guides/dials/); сторонние UI assets Elgato не поставляются.

Новые Audio/Monitor/Timers и vendored SDK 0.1.0 — собственные исходники DeckWeave под GPL-3.0-or-later; полный текст лицензии включён в каждый bundle. CoreAudio использует Windows API через windows-rs; native helper собирается из исходников, его binary не хранится в Git. Шаблон создаёт SVG кодом и не включает чужие иконки.
