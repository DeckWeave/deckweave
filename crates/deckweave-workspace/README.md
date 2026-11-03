# Workspace native helper

For an explicit hardware command, a denied ordinary activation gets one bounded
attempt with a temporary shared foreground input queue. The queue is detached
before result verification or keyboard injection. The fallback is skipped when
modifiers are held or the foreground changed during the attempt. It changes no
foreground-lock setting and synthesizes no activation keystrokes. The exact
target HWND and executable must still be verified before a command is sent.
See [SetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow)
and [AttachThreadInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-attachthreadinput).

`DECKWEAVE_WORKSPACE_TRACE=1` adds bounded class/size/handle diagnostics to an
activation error. It includes no window title or document text. This is an opt-in
developer diagnostic, not a user setting or a network report.

`deckweave-workspace.exe` is a pooled private child of the Workspace plugin.
Build with `cargo build -p deckweave-workspace --release --locked`, then copy
`target/release/deckweave-workspace.exe` into the plugin's `bin/` directory.
No global command endpoint is created.

One UTF-8 JSON line on stdin, at most 1024 bytes, produces one JSON line on stdout:

```json
{"id":1,"operation":"launch.code"}
```

Success: `{"id":1,"ok":true}`. Failure additionally includes an actionable
`error`. Unknown fields and operations are rejected. No caller paths, arguments,
keycodes, URLs or text are accepted. Editor commands use the separate VS Code
companion; they are absent from this helper's allowlist.

Known application paths use the current user's AppData and Program Files.
Codex uses its stable installed AppID and validates the resulting packaged
executable identity. The default HTTPS browser comes from Windows' UserChoice
association and is restricted to supported browser executable names. No fallback
silently changes the selected browser.

Existing foreground windows are preferred. Otherwise the helper finds a visible
window belonging to the verified executable, opens the application if necessary,
and asks Windows to focus it. It verifies the exact foreground HWND and executable
again immediately before a single fixed-chord SendInput batch. A focus failure,
held modifier or privilege restriction returns an error. It never injects into
an arbitrary foreground application, submits text, or presses Enter. Windows can
still switch focus between the final check and input dispatch; these controls
are navigation-only and never submission/approval chords.

Unit checks do not launch applications or inject input. Actual launch/focus and
shortcut behavior must be checked explicitly with disposable application state.
