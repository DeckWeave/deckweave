# DeckWeave Audio

GPL-3.0-or-later. Optional Windows master-output plugin; uses the DeckWeave action SDK.
No network access or dependencies beyond the host's Node 22+ runtime and the native helper.

## Use / Использование

Drag **System Volume · Громкость системы** onto a key, dial or Panel area. A key or dial
press toggles mute. Turning a dial changes master volume by the configured step (default 2%).
On a Panel, touch or drag horizontally to choose 0–100%. Settings save immediately.
The displayed value is observed from Windows, including changes made outside DeckWeave;
muted output has an explicit MUTE/ВЫКЛ label. Every request follows the current default
multimedia output, including after unplugging a headset or changing the default device.

Перетащите действие на кнопку, крутилку или область панели. Нажатие кнопки/крутилки
переключает звук, вращение меняет громкость, касание панели задаёт громкость по
горизонтальной позиции. В настройках доступны шаг и подпись. Изменения из Windows
отображаются автоматически. Пока поддерживается только Windows; macOS/Linux показывают
недоступность, без имитации результата.

## Build and distribution

From the application source root:

```powershell
cargo build -p deckweave-platform --bin deckweave-system --release
New-Item -ItemType Directory -Force plugins/dev.deckweave.audio.sdPlugin/bin
Copy-Item target/release/deckweave-system.exe plugins/dev.deckweave.audio.sdPlugin/bin/
```

Distribute the complete plugin folder with `bin/deckweave-system.exe` and `lib/sdk.mjs`.
The latter is vendored from the companion `plugin-sdk` repository, version 0.1.0.
Run `npm run validate` and `npm run pack` inside this bundle after copying the helper.
Both refuse a missing helper; the archive includes it. No absolute checkout path is required. The source checkout can also use `target/release` or `target/debug`
without copying the helper. An installed plugin never searches the executable PATH.

The helper is a persistent JSON-lines process, not a shell process per detent. It resolves the
default Core Audio endpoint per request, maintains COM on one thread, caps input at 1024
bytes and supports only `status`, `set_volume`, `adjust_volume`, `toggle_mute`. The plugin
caps pending requests and response size, times out a stuck helper, and kills its child on
host disconnect. Fast detents and Panel moves are coalesced for 30ms in an ordered queue;
absolute/relative commands and direction changes retain their order. Disappearing contexts
cancel queued mutations, including inputs awaiting helper startup. Polling is once per
second while at least one action is visible; identical images are not resent.

Read-only helper check (does not change audio):

```powershell
'{"id":1,"command":"status"}' | & target/release/deckweave-system.exe
node --test plugins/dev.deckweave.audio.sdPlugin/audio.test.mjs
```

## Acceptance still requiring real use

After assigning the action, compare the percentage/mute indicator with Windows volume.
Turn the assigned dial, then press it: confirm audible volume and mute follow. If using the
Panel, touch its left/right edges and confirm 0%/100%; start with quiet playback. These
physical interactions are not simulated by the read-only helper check.
