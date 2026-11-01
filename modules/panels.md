# Панели

Ответственность: независимые widgets, dial associations, preview/render/hit geometry. Core model.rs/panels.rs владеют нормализованными областями и input semantics; desktop touch.rs адаптирует N4; deckweave-n4 renderer/artwork композируют JPEG и Encoder feedback.

У страницы бывают произвольные непересекающиеся regions, fullscreen widget или области по числу dials. Один dial связан с одной областью; native Panel может владеть несколькими dials. Keypad, Encoder и native Panel не смешиваются в один протокол. Native pointer widget владеет drag/swipe; обычный tap отменяется при движении, неизвестный Y не превращается в выбранную вертикальную область.

Новые страницы/профили получают dial regions. Исходная тестовая раскладка остаётся доступной; явные Touch assignments имеют приоритет, свободная исходная область отображает соседний dial. Физический N4 preview использует тот же итоговый JPEG, что отправляется транспорту, с обратным поворотом в UI.

[Точный контракт](../docs/PANEL-CONTRACT.md), [совместимость layouts](../docs/PLUGIN-COMPATIBILITY.md), [неопределённая аппаратная часть](../docs/ACCEPTANCE-NEXT.md), [план](../docs/PLAN.md). Pixel-perfect всех Elgato styles ещё не заявляется.
