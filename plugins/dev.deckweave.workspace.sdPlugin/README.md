# Personal Workspace commands

One configurable action: `dev.deckweave.workspace.command`.
Settings: `{ "operation": "report.preview", "label": "Просмотр", "icon": "preview", "language": "ru" }`.
`operation` must be listed in `operations.mjs`; optional `label` is at most 24
characters, `icon` is a named built-in SVG. Native operations are `launch.*`,
`browser.*` and `codex.*`. `report.*` and `vscode.*` require the local VS Code
companion from `integrations/vscode`.

The action runs only on a key/dial release or completed touch press. Assigning,
opening the inspector, changing settings, displaying a page or starting the
plugin never executes it. The inspector cannot run commands. A navigation
command first focuses its intended executable and fails visibly if Windows
cannot confirm focus. Browser search/address actions only focus an input;
they do not submit it. Desktop Codex and its VS Code extension are distinct
operation groups.

Build and copy the native helper before installation:

```powershell
cargo build -p deckweave-workspace --release --locked
New-Item -ItemType Directory plugins/dev.deckweave.workspace.sdPlugin/bin -Force
Copy-Item target/release/deckweave-workspace.exe plugins/dev.deckweave.workspace.sdPlugin/bin/
node plugins/dev.deckweave.workspace.sdPlugin/tools/package.mjs validate
```

Source-tree development can locate the built helper in `target/release` or
`target/debug`. Distributed packages require `bin/deckweave-workspace.exe`.
SDK and property-inspector appearance files are bundled, with the same compact
neutral settings surface as other DeckWeave plugins. Hardware output uses a
large consistent SVG icon and a short label; error icons stay visible until the
next successful press/settings change. Full recovery instructions appear in
the inspector and companion notification. Nothing is sent to a telemetry server.

Action failures retain the image and assignment. The hardware adds a warning
badge; the plugin renders a short cause, and opening its inspector also shows
the last full cause. A successful repeat restores the ordinary image. No command
is retried automatically. Real SDK tests cover failure, success, duplicate presses
while pending and disappearing contexts for keys, encoders and Panel controls.
