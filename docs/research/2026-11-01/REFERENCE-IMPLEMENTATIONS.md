# Ajazz, StreamDock и публичные SDK Mirabox

Дата: 2026-11-01. Исходники прочитаны через GitHub; сторонние проекты не собирались и их тесты здесь не запускались. Просмотр спецификации или unit test не означает аппаратную приёмку.

## Ajazz Control Center

Исследована ветка develop, commit [03a29f7a90dd32057ca7cd0d6e6fa8f13d5400f3](https://github.com/Aiacos/ajazz-control-center/commit/03a29f7a90dd32057ca7cd0d6e6fa8f13d5400f3). Приложение на C++/Qt, источники с GPL-заголовками; это отдельная реализация, не исходники самого StreamDock.

Полезная продуктовая модель: один Encoder владеет своим участком touch strip; назначение плагина даёт feedback и маршрутизацию rotate/press/touch одному context. Независимый drop target TouchZone в этой спецификации убран. Одинаковый результат рендера предназначен для preview и LCD. Это хороший референс связи UI с железом, но спецификация не доказывает полноту реализации. [Контракт dial feedback](https://github.com/Aiacos/ajazz-control-center/blob/03a29f7a90dd32057ca7cd0d6e6fa8f13d5400f3/specs/002-streamdeck-plugin-ui/contracts/dial-feedback-ui.md).

Обнаруженные ограничения исследованного кода:

| Участок | Наблюдение | Значение для DeckWeave |
|---|---|---|
| Адаптер Multi Action | Test прямо фиксирует потерю userDesiredState при преобразовании дочернего action в ActionChain | Копирование цепочки потеряет смысл «включить конкретное состояние» |
| ActionEngine | Шаги обходятся по порядку, задержки планирует Executor; plugin callback сам по себе не подтверждает завершение внешней команды | Не обещать ожидание фактического результата только по порядку вызовов |
| Touch bridge | В payload жёстко записаны hold=false и tapPos=[x,0] | Длительное касание и полные координаты этим кодом не подтверждены |
| Touch bridge | Каждый TouchUp отправляется как touchTap; комментарий оставляет swipe-классификацию другому сервису | Нужна сквозная проверка, что свайп не вызывает дополнительный action tap |
| Автоматические состояния | Два состояния меняются на release, если не задан DisableAutomaticStates | Это поведение данного исходника, а не измеренная трасса официального Stream Deck |
| Encoder rotate | Признак нажатой крутилки берётся из набора по индексу | Изоляцию одинаковых индексов разных instances нужно проверять отдельно |

Источники: [тест потери состояния, строки 133+](https://github.com/Aiacos/ajazz-control-center/blob/03a29f7a90dd32057ca7cd0d6e6fa8f13d5400f3/tests/unit/test_multiaction_dispatch.cpp#L133), [ActionEngine](https://github.com/Aiacos/ajazz-control-center/blob/03a29f7a90dd32057ca7cd0d6e6fa8f13d5400f3/src/core/src/action_engine.cpp), [bridge, строки 1548+](https://github.com/Aiacos/ajazz-control-center/blob/03a29f7a90dd32057ca7cd0d6e6fa8f13d5400f3/src/app/src/plugin_device_bridge.cpp#L1548).

Описание AKP05E содержит provisional-константы и требует живой проверки. Его предположения о byte-9 touch-кодах и однобайтовой X нельзя переносить в N4 Pro, где уже есть другой подтверждённый формат ACK ARX. Документ прочитан как внешний источник; извлечение бинарного кода StreamDock в этой работе не выполнялось. [Оговорки протокола AKP05E](https://github.com/Aiacos/ajazz-control-center/blob/03a29f7a90dd32057ca7cd0d6e6fa8f13d5400f3/docs/protocols/streamdeck/akp05_input_corrections.md).

## Что публично доступно у Mirabox

В проверенных репозиториях MiraboxSpace найдены Plugin SDK, Device SDK, Plugin Builder, примеры плагинов и OBS plugin. Полный исходный код редактора StreamDock среди этих результатов не найден. Это ограниченный результат поиска, а не доказательство отсутствия исходников где-либо.

- [Plugin SDK](https://github.com/MiraboxSpace/StreamDock-Plugin-SDK/tree/a3fb38490a36f507a09af8b78bdddac49c10668f): шаблоны и API для action-плагинов, README с MIT. Это не весь host/editor.
- [Device SDK](https://github.com/MiraboxSpace/StreamDock-Device-SDK/tree/324d498de6981ee5d9d437bf05818599b3c20781): Python/CPP/WebSocket для устройств, README с MIT.
- [OBS Plugin](https://github.com/MiraboxSpace/StreamDock-OBS-Plugin): отдельный OBS-side компонент, не исходники приложения StreamDock. Его совместимость с нашей будущей интеграцией не проверена.

В официальном [StreamDockN4Pro.py](https://github.com/MiraboxSpace/StreamDock-Device-SDK/blob/324d498de6981ee5d9d437bf05818599b3c20781/Python-SDK/src/StreamDock/Devices/StreamDockN4Pro.py) ACK ARX декодируется в TOUCH_POINT: big-endian X/Y из bytes 10–13. Это независимо подтверждает направление старого N4 decoder. SDK также перечисляет отдельные swipe-коды 0x38/0x39. Но список в SDK не доказывает, что наша firmware выдаёт все такие события, настоящие release и удержание; нужна сверка сохранённых/новых аппаратных отчётов. Размеры framebuffer и полосы N4 не равны логической области Elgato.

## Установленный StreamDock: фактически просмотрено

Приложение уже работало с N4 Pro. Исходно: сценарий 1, страница 1, режим панели управления, фильтр Keys, выбрано существующее Mail. Назначения и системная громкость не менялись; аппаратные нажатия не выполнялись.

1. Фильтр Dials показал отдельный набор: Hotkey, Super Hotkeys, Mouse Events, Multimedia, System Volume, Control Trigger, Action Wheel, Dial action group.
2. В режиме крутилок полоса стала показывать связанные с ними элементы и навигационные маркеры. Видны варианты свайпа страницы/сценария; их фактическое исполнение не проверялось.
3. Выбор Master на полосе открыл настройки громкости с подсказками rotate/press. Выбор кольца крутилки по другому пути открыл RGB/оформление. Это наблюдение путей выбора, не доказательство неисправности.
4. Контекстное меню участка содержало copy/paste/delete и оформление. «Проверить» в этом меню не обнаружено; всё приложение на наличие такой команды не проверено.
5. Восстановлены исходные режим, фильтр и выбор Mail; сценарий и страница сохранены.

Свидетельства: [журнал](evidence/streamdock-ui-observations.json), [снимок режима крутилок](evidence/streamdock-dials-mode.jpg). Это сравнение организации интерфейса. Оно не подтверждает качественное выполнение сложных сценариев.

Вывод: брать отдельные проверяемые идеи и низкоуровневые знания, а соответствие Elgato оценивать по собственным сквозным пробам. Ни название функции, ни наличие её формы или теста в чужом проекте не заменяет эту проверку.
