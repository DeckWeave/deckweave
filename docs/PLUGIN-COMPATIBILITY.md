# Реализованная совместимость действий

Это проверяемый текущий срез, не сертификация Elgato и не обещание работы каждого установленного пакета. Основа: [официальный WebSocket API](https://docs.elgato.com/streamdeck/sdk/references/websocket/plugin/), [Dials & Touch Strip](https://docs.elgato.com/streamdeck/sdk/guides/dials/), [layouts](https://docs.elgato.com/streamdeck/sdk/references/layouts/).

| Часть | Текущее поведение |
|---|---|
| Catalog | `.sdPlugin` из каталога приложения, workspace plugins и установленного каталога Elgato. Platform entry point, Controllers, PI, state images и Encoder layout из manifest |
| Backend | Один процесс на пакет; несколько contexts/instances независимы. Запускается при первом использовании, остаётся, пока пакет назначен где-либо в конфигурации. `willDisappear` снимает невидимый context, не обрывая остальные действия |
| Keypad / Encoder | keyDown/keyUp, dialDown/dialUp/dialRotate, touchTap с hold и tapPos в canvas 200×100. N4 click не выдаётся за измеренную длительность удержания крутилки |
| Settings | set/getSettings, per-package global settings, didReceiveSettings, sendToPlugin/sendToPropertyInspector. Оригинальный PI имеет отдельное окно и глобальную editing pause |
| Feedback | title/state/image, state images, showAlert/showOk, setFeedback и смена layout. PNG/JPEG/WebP; standalone SVG растеризуется с bundled IBM Plex без доступа к внешним SVG images/files |
| Encoder layouts | Custom JSON и `$X1`, `$A0`, `$A1`, `$B1`, `$B2`, `$C1`; text/pixmap/bar/gbar, значения и object updates, bounds, z-order, text alignment/color/font size, basic bar styles/gradients. Это ещё не pixel-perfect реализация всех стилей/типографики Elgato |
| Native panel | Controller `Panel` — расширение DeckWeave, не часть Elgato SDK. Произвольные независимые области, fullscreen, несколько связанных dials, pointer down/move/up/cancel/swipe, normalized position и native canvas pixels |
| Ограничения | Защищённые manifests не расшифровываются. Нет Marketplace installer, profile import, полного SDK resources/app monitoring/deep-linking. Автоподбор нужной major Node runtime и поставка runtime в installer ещё не завершены |

Windows HTML backend использует установленный headless Edge с собственным профилем: на этой машине hidden WebView2 зависал при первой Canvas-отрисовке, а Edge прошёл реальную проверку OnAirClock. Edge descendants удерживаются в собственном Windows Job и завершаются при выходе/падении host. Native/Node plugins не создают browser. UI/PI работают через Tauri/WebView2; сторонние страницы не получают editor IPC. Нужны установленный Node для script plugins и Edge для HTML backend. macOS/Linux ветви пока не проверены.

Процесс с назначением вне активной страницы сохраняется, но его таймеры должен останавливать `willDisappear`, как требует lifecycle плагина. Отсутствующая регистрация, упавший backend и исключения HTML получают видимую ошибку. Нажатие до готовности не накапливается для неожиданного исполнения позже: показывается просьба повторить после запуска. Ошибка вложенного action отменяет оставшиеся отложенные шаги его запуска.

Проверки: [runtime evidence](RUNTIME-PROGRESS.md), [native Panel UI](evidence/2026-11-01/panel-native-ui.json). Installed OnAirClock тестировался без изменения его файлов; OBS — собственный пакет через obs-websocket 5, не утверждение о проверке всех сторонних OBS-плагинов.
