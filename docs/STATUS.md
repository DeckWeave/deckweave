# Текущее состояние DeckWeave

Обновлено 2026-11-01. Windows 11, Rust + Tauri 2 + Svelte 5. Продолжение первого коммита `85d242d` находится в ветке `codex/runtime-plugins-actions`. Это работающий следующий срез, а не завершённый v1 с полным паритетом Elgato.

## Что реализовано

- Реестр устройств, отдельное окно каждого instance, tray и фоновые действия после закрытия всех окон. Повторный запуск открывает существующий runtime; явный выход освобождает workers и процессы HTML plugins.
- N4 Pro: автообнаружение, identity/generation, отдельные workers, Offline/Error, ввод keys/dials/touch и JPEG на клавиши/полосу. Владелец ранее подтвердил K1–K10, крайние touch области и D1; эти проверки не нужно повторять без причины.
- Профили, страницы/папки, pinned keys, запуск программ и горячие клавиши. Multi Action/Delay, Switch A/B/A, Key Logic single/double/hold, независимые направления/push/tap крутилки, Dial Stack и Action Wheel. Выбор не запускает действие; проверка отдельная, сохранение без Apply.
- Smart Profiles: foreground executable, глобальная пауза при любом editor/PI. Без настроенных правил ручной профиль сохраняется; неизвестное foreground окно не переключает профиль.
- Elgato host: native/Node/HTML backend, lifecycle, настройки, оригинальные PI, title/state/image и Encoder feedback layouts. Установленный OnAirClock 1.3.0 прошёл сквозную проверку, включая изменение настройки через свой PI.
- Панель: независимые нормализованные области, fullscreen, связи с dials, композиция bitmap/SVG/feedback, отдельное поле tap/drag проверки. Controller Panel — собственное расширение DeckWeave; пример — Panel Lab. Новые страницы/профили получают области по числу крутилок; исходная диагностическая раскладка сохраняется.
- en/ru; SQLite сохраняет новую revision до её публикации. Ошибки не сбрасывают базу. Диагностика ограничена и исключает artwork blobs/настройки плагинов.

## Проверки и доказательства

Общий check: Svelte без ошибок/предупреждений, format, production web, Rust fmt/clippy с запретом предупреждений, 60 Rust-тестов и документация. Native release: `npm run build`; бинарник `target/release/deckweave-desktop.exe`.

Через Windows UI проверены native Panel drag, оригинальный PI своего плагина с немедленным сохранением и выполнением, Switch A/B/A на одной кнопке, оригинальный PI установленного OnAirClock и смена 24/12 часов. Закрытие всех окон, повторное открытие runtime и завершение всего собственного дерева Edge проверены исполнением. Подробности: [журнал результата](RUNTIME-PROGRESS.md), [совместимость](PLUGIN-COMPATIBILITY.md), [историческая аппаратная приёмка](N4-ACCEPTANCE.md).

OBS: собственный plugin по obs-websocket 5; challenge authentication и RPC проверены настоящими соединениями, установленный OBS ответил на read-only запросы. Запись и трансляция автоматически не переключались.

## Что ещё не готово

Физическая приёмка новых touch координат/жестов, D2–D4 и reconnect остаётся владельцу. Шаги будут собраны в [следующую приёмку](ACCEPTANCE-NEXT.md). Программные тесты и успешные HID writes не выдаются за наблюдение дисплея.

Внешний hardware process provider/публичный SDK ещё не реализован: N4 встроен, граница описана в [provider contract](PROVIDER-CONTRACT.md). Нет Marketplace installer, профилей из Elgato/старого проекта, полного SDK resources/app monitoring и pixel-perfect всех feedback styles. Редактор профилей/папок пока покрывает создание и навигацию, не полный CRUD и все удобства Elgato. Packaging/runtime distribution, длительные нагрузочные прогоны и macOS/Linux не завершены. IO84/MX4/Android и sync groups остаются отложенными по принятым решениям.

Это сохранённый объём оставшейся работы, а не исключение обязательных функций v1. [Полный план](PLAN.md), [ближайшее продолжение](NEXT.md), [решения владельца](DECISIONS.md).
