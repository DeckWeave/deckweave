# Источники и воспроизводимость

Дата проверки: 2026-11-01. Первичные ссылки ниже поддерживают конкретные наблюдения в соседних документах. Живой сайт/main может измениться; это не pin распространяемого executable. Локальные хеши относятся к исследованным файлам.

## Интерфейс и browser runtime

- [Tauri process model](https://v2.tauri.app/concept/process-model/) — граница Rust core и browser UI.
- [Tauri WebView versions](https://v2.tauri.app/reference/webview-versions/) — WebView2 на Windows, другие движки на desktop ОС.
- [Tauri 2.12.5, Windows builder](https://docs.rs/tauri/latest/x86_64-pc-windows-msvc/tauri/webview/struct.WebviewWindowBuilder.html) — несколько окон, data directory, with_environment, особенности Windows event loop.
- [Microsoft WebView2 process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model) — browser/renderer/helpers и связь с UDF.
- [Microsoft performance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance) — накладные расходы и измерение реальных сценариев.
- [Microsoft multiple profiles](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/multi-profile-support) — раздельные browser profiles в одном UDF.
- [Microsoft WebView2 environment API](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/webview2-idl) — override временного UDF в диагностике.
- [GPUI upstream README](https://raw.githubusercontent.com/zed-industries/zed/main/crates/gpui/README.md) и [пример окон](https://raw.githubusercontent.com/zed-industries/zed/main/crates/gpui/examples/window.rs) — platform support, lifecycle и pre-1.0 status.
- [GPUI Kit WebView guide](https://gpui-kit.com/docs/webview/) — экспериментальное встраивание и текущие ограничения. Это сторонняя интеграция, не гарантия upstream GPUI.

## Elgato, OpenAction и OpenDeck

- [Elgato manifest](https://docs.elgato.com/streamdeck/sdk/references/manifest/) — backend entrypoints, действия, OS и Node metadata.
- [Elgato WebSocket API](https://docs.elgato.com/streamdeck/sdk/references/websocket/plugin/) — регистрация, contexts и события.
- [Elgato Property Inspector](https://docs.elgato.com/streamdeck/sdk/guides/ui/) — HTML-формы настроек.
- [Elgato distribution](https://docs.elgato.com/streamdeck/sdk/introduction/distribution/) — упаковка, DRM/encryption и отличие обработанного пакета.
- [Elgato keys](https://docs.elgato.com/streamdeck/sdk/guides/keys/) и [dials](https://docs.elgato.com/streamdeck/sdk/guides/dials/) — states, изображения, encoder layouts/feedback.
- [Elgato profiles](https://docs.elgato.com/streamdeck/sdk/guides/profiles/), [pages](https://help.elgato.com/hc/en-us/articles/4410312027277-Elgato-Stream-Deck-Pages), [folders](https://help.elgato.com/hc/en-us/articles/360027957912-Elgato-Stream-Deck-Using-Folders), [folder guide](https://www.elgato.com/us/en/explorer/products/stream-deck/how-to-use-folders-stream-deck/).
- [Pinned actions](https://help.elgato.com/hc/en-us/articles/26638431612429-Elgato-Stream-Deck-Pinned-Actions-and-Folders), [Smart Profiles](https://www.elgato.com/ca/en/explorer/products/stream-deck/smart-profiles-stream-deck/), [Multi Actions](https://www.elgato.com/us/en/explorer/products/stream-deck/how-to-use-multi-actions/), [Key Logic](https://www.elgato.com/jp/en/explorer/products/stream-deck/key-logic-stream-deck/) — поведение для будущего списка полноты.
- [OpenAction](https://openaction.amankhanna.me/) — совместимый с Stream Deck API и расширения устройств.
- [OpenAction catalogue](https://raw.githubusercontent.com/OpenActionAPI/plugins/main/catalogue.json) — metadata доступных кандидатов, не результаты совместимости.
- [OpenDeck локальная исследованная версия](https://github.com/nekename/OpenDeck/tree/772189ca74c89a10bad219b67de620ecb07bf1f6) — основной pin source review.
- [OpenDeck device registration](https://raw.githubusercontent.com/nekename/OpenDeck/main/src-tauri/src/events/inbound/devices.rs), [shared types](https://raw.githubusercontent.com/nekename/OpenDeck/main/src-tauri/src/shared.rs), [plugin launch](https://raw.githubusercontent.com/nekename/OpenDeck/main/src-tauri/src/plugins/mod.rs), [manifest](https://raw.githubusercontent.com/nekename/OpenDeck/main/src-tauri/src/plugins/manifest.rs) — дополнительная сверка live source, не pin версии плагинов.

## Оборудование и последующие расширения

- [Mirabox SDK](https://github.com/MiraboxSpace/StreamDock-Device-SDK) — vendor reference; локальные N4 результаты дополнительно проверяются по собственным captures/tests, а не только SDK.
- [Windows Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input), [RAWINPUTDEVICE](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinputdevice) — per-device input, background delivery и flags.
- [Wooting Analog SDK](https://github.com/WootingKb/wooting-analog-sdk) — пример доступа к аналоговой клавиатуре; не подтверждает поддержку неизвестной модели владельца.
- [Android NSD](https://developer.android.com/develop/connectivity/wifi/use-nsd) — LAN discovery, отдельно от pairing.
- [OBS WebSocket 5.x](https://raw.githubusercontent.com/obsproject/obs-websocket/master/docs/generated/protocol.md) — альтернатива специфическому streamdeck-obs bridge.

## Локальные доказательства

- Дополнительное исследование сложных действий: [механика и первичные ссылки Elgato](ADVANCED-INTERACTIONS.md), [Ajazz/Mirabox с закреплёнными commits](REFERENCE-IMPLEMENTATIONS.md), [статус просмотра видео](VIDEO-REVIEW.md), [provenance](evidence/advanced-research-provenance.json). Прочитанный код отделён от исполненного поведения.
- Установленный StreamDock: [наблюдения UI и восстановление исходного режима](evidence/streamdock-ui-observations.json), [снимок](evidence/streamdock-dials-mode.jpg); аппаратный ввод не проверялся.

- Установленный Stream Deck 7.6.0.23012: подробный [UI-обход](STREAM-DECK-WALKTHROUGH.md), [журнал](evidence/streamdeck-ui-audit.json). Создан отдельный DeckWeave UX Lab; Default Profile восстановлен. Существующие назначения не менялись.
- [Elgato Marketplace / Plugins](https://marketplace.elgato.com/stream-deck/plugins), [поиск OBS](https://marketplace.elgato.com/search?q=OBS), [OBS Studio и история версий](https://marketplace.elgato.com/product/obs-studio-35615969-830f-45c9-ba0a-1a295bba7fec) — реально открыты в браузере; версия 3.0.3.31 и запись о переработке 3.0.0.25. Это не проверка её wire protocol.
- [Virtual Stream Deck: добавление и условия активации](https://help.elgato.com/hc/en-us/articles/35492041288337-Elgato-Stream-Deck-How-to-Add-and-Remove-a-Virtual-Device-Virtual-Stream-Deck) — поясняет недоступность добавления панели без разблокировки; в установленном UI действие Toggle есть, но настроенных VSD нет.
- IO Type 84: `G:\APPS\io_type_84`, HEAD `9a6553bb486a754a6cd82b379cfcdac3bdb7ebb1`, tracked/untracked status пуст при чтении. `docs/STATUS.md`, `docs/PROTOCOL.md`, `modules/exclusive-depth.md`, `docs/evidence/stock-capture-acceptance-v1.17.json`, выбранные `io-core` / `io-platform` исходники — [выводы](IO-TYPE-84.md). Сохранённая аппаратная приёмка не повторялась в этой сессии.
- [Хеши выбранных source files](evidence/source-hashes.json), [manifest установленных пакетов](evidence/installed-packages.json), [испытавшиеся entrypoints](evidence/probed-artifacts.json).
- [Окружение probe](evidence/probe-environment.json), [hardware](evidence/hardware.json), [пояснение evidence](evidence/README.md).
- [Сводка совместимости](COMPATIBILITY.md) отличает live observations, исполняемые пробы, статический просмотр и непроверенные части.

Локальные git commits и dirty-tree оговорки перечислены в [FINDINGS](FINDINGS.md). Нет оснований считать изменённые файлы Companion/Mirabox точным содержимым upstream commits. Старые binaries используются с собственными SHA-256; тесты не выдаются за новую сборку source HEAD.
