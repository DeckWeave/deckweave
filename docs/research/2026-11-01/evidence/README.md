# Evidence исследования

Дата: 2026-11-01. JSON созданы диагностическими запусками, а не новым продуктом. В репозиторий не скопированы binaries и исходники сторонних плагинов. Настройки probe пустые; пользовательские tokens/аккаунты не выгружались.

## Группы файлов

| Файлы | Смысл |
|---|---|
| `installed-packages.json` | Manifest hash и тип установленного пакета; protected manifest не декодирован |
| `probed-artifacts.json` | Версии и хеши manifest/entrypoint реально запущенных копий |
| `probe-environment.json` | Пути тестовых binaries, SHA-256 и диагностический scratch directory |
| `source-hashes.json` | Идентичность изученных локальных source files |
| `io-type-84-sources.json` | Хеши выбранных файлов прототипа IO Type 84 на момент чтения; не новая аппаратная проверка |
| `streamdeck-ui-audit.json` | Журнал обхода настоящего Stream Deck, accessibility text и заметки |
| `streamdeck-ui-clock-modern-minimal.jpg` | Смена циферблата в отдельном лабораторном профиле: сетка и исходный HTML PI |
| `hardware.json` | ОС, модель CPU и RAM измерительного ПК |
| `installed-webview-runtimes.json` | Обнаруженный установленный WebView2 150.0.4078.65; executable фактически запущенного процесса отдельно не хешировался |
| `probe-scenarios.json` | Нейтральный HTML, пятисекундная performance-проба и виртуальные inputs sample |
| `cleanup.json` | Подтверждение удаления временных копий/профилей после проверки границ пути и отсутствия использующих их процессов |
| `browser-control-sandbox.json`, `browser-control-user.json` | Один контрольный HTML-case в двух средах; sandbox timeout и outside PASS |
| `onairclock-probe-failure.json` | Первоначальный sandbox timeout, не итоговая несовместимость |
| `clock-visible-debug-probe.json` | Неудачная диагностическая попытка; visible-флаг фактически не установлен из-за ошибки PS property assignment. Для выводов о видимом режиме не используется |
| `onairclock-probe.json`, `onairclock-summary.json` | Успешная outside-sandbox проба; image payload заменён длиной/SHA-256 |
| `clock-image-summary.json`, `onairclock-frame.png/json` | Статистика изображений и один декодированный PNG, просмотренный исследователем |
| `vscode-probe.json` | Регистрация native backend и settings; key actions не отправлялись |
| `tutorial-scan.json` | Отказ старого scanner на пустом имени установленного пакета |
| `obs-probe.json` | Архивный OBS backend, inputs=0; внешняя OBS-операция не испытана |
| `hello-world-input-probe.json` | Node sample и две виртуальные пары down/up; реальные settings/title changes |
| `perf-control-*.json`, `perf-clock-*.json` | Одиночные memory samples и event counts. Поле первоначального CPU-накопителя недостоверно, помечено false |
| `cpu-counter-validation.json`, `cpu-control.json`, `cpu-clock.json` | Проверенные raw Win32 CPU snapshots и интервальные расчёты |
| `two-clocks-isolated.json`, `two-clocks-shared-udf.json` | Сравнение process groups и памяти; shared-вариант дал ConnectionReset при завершении |
| `n4pro-existing-binary-tests.txt`, `n4pro-test-metadata.json` | 37 тестов существующего N4 binary, SHA-256; source HEAD не пересобирался |
| `scanner-existing-binary-tests.txt`, `scanner-test-metadata.json` | 3 теста существующего scanner test binary с тем же ограничением |

## Как читать измерения

Private commit и сумма working sets не взаимозаменяемы. Working sets могут дважды считать shared pages разных процессов. Пять секунд с cold profile не являются долгим steady-state/soak. Две точки CPU не доказывают разницу между архитектурами. Нового Tauri UI и GPUI-прототипа не было.

В shared-тесте намеренно использован один временный UDF только внутри дочерних процессов; глобальные переменные/настройки не менялись. Это диагностическая проверка механизма, не предлагаемый production способ isolation.

Пути scratch в metadata — исторический контекст запуска; temporary package copies не являются поддерживаемой поставкой. Детальная методика и ограничения — в [PERFORMANCE](../PERFORMANCE.md); границы функциональных результатов — в [COMPATIBILITY](../COMPATIBILITY.md).

Исследовательская временная папка удалена после сохранения evidence. Перед завершением проверены локальные Markdown-ссылки и разбор JSON; ошибок не найдено. Git commit и публикация не выполнялись.
