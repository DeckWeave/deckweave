# ADR-0005: Внешние плагины через процессы и локальный WebSocket

Дата: 2026-11-01. Статус: принято для реализации следующих фаз; внешний transport ещё не написан.

## Контекст и варианты

Нужно подключать драйверы, действия и панели, написанные на разных языках; один provider обслуживает несколько instances. Варианты: Rust dynamic ABI; отдельные процессы со stdio; процессы с локальным WebSocket. Выбраны процессы и WebSocket. Rust ABI связывает плагины с компилятором, stdio неудобно смешивать с диагностическим выводом и бинарными кадрами.

## Решение

Listener принадлежит host, привязан к loopback и запускается при первом внешнем плагине. После остановки последнего освобождается. Native protocol имеет версию, role и session nonce; handshake проверяет пакет и разрешённую роль. Control plane — JSON-RPC 2.0 с request ID, ответами и notifications; display payload — отдельные binary WebSocket frames с versioned header и frame ID. Точные поля header фиксируются с fixtures при начале provider SDK, до публикации пакета.

Elgato использует свой исходный WebSocket protocol через отдельный adapter, без преобразования пакета в native manifest. Его registration, context и lifecycle не смешиваются с native RPC. Общая библиотека сетевого IO допустима, общий неконтролируемый broadcast — нет.

Один provider process может публиковать несколько независимых instance IDs. Host выдаёт session generation и проверяет адресата всех событий/feedback. Disconnect удаляет session, но не пользовательскую конфигурацию. Reconnect создаёт новое поколение без replay старых input/actions. Configured offline instance остаётся доступным для редактирования по сохранённому descriptor; наличие драйвера не должно быть условием чтения всей конфигурации.

Control/input queue ordered и bounded; переполнение приводит к явному unhealthy/disconnect и cleanup удержаний, а не к молчаливой потере up. Display queue latest-frame-wins на output region; транспорт передаёт целую транзакцию кадра. Первый N4 adapter может быть in-process модулем с тем же предметным интерфейсом, до вынесения в provider process.

Process supervisor владеет только запущенными им процессами. Browser owner владеет общим WebView environment; остановка одного HTML backend не убивает общее дерево браузера. stdout/stderr — диагностика с ограничением размера, без session nonce в журнале. Исходный PI не получает доступ к desktop IPC или workspace store.

## Последствия

Добавляются handshake, versioning и supervisor, зато SDK независим от Tauri, языка плагина и OS process internals. Первый срез встроенных действий не требует стартовать listener или browser backend. Полный device/panel SDK публикуется после проверки минимум двух instances и reconnect, а не по черновому протоколу.
