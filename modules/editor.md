# Редактор

Ответственность: runtime snapshots, selection, немедленная настройка и отдельная проверка. Код apps/desktop/src: types.ts, client.svelte.ts, DeviceEditor, ActionEditor/PlanEditor, PanelEditor/TouchTest. Rust остаётся единственным runtime; browser-local fake store отсутствует.

Назначение и поля сохраняются без Apply. Библиотека различает Keys/Dials/Plugins и Controllers; оригинальные PI открываются отдельно. Вложенные действия показывают свой feedback, Switch — следующее действие, Stack/Wheel — выбранное. Main preview выбирает область; отдельное поле исполняет tap/drag. Panel form ограничен по высоте, preview имеет минимальный размер и правильный aspect ratio.

Собственный UI en/ru, имена пользователя не переводятся. Profiles/pages/folders пока покрывают создание/навигацию; полный CRUD и дальнейший UX остаются в v1 плане. [UI](../docs/UI.md), [проверки](../docs/RUNTIME-PROGRESS.md), [план](../docs/PLAN.md).
