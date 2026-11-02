# Host feedback: independent source follow-up

**Date:** 2026-11-03. **Author:** root agent. This is a separate source investigation during the six-lens review, not a Goldfish report or implementation acceptance. The reviewers receive the frozen hypothesis and source paths, not these conclusions.

## Existing timer scheduling changes the reuse plan

The shared service should extract the existing timer deadline logic instead of creating a second scheduler alongside it. [TimerPlanner::update and due](../../../apps/desktop/src-tauri/src/key_lighting.rs) already arm an in-memory monotonic deadline after a newly accepted running-state transition, preserve that deadline across later settings updates, bound armed jobs to 128 and remove a due job before delivery. Its unit test `hidden_countdown_uses_one_monotonic_deadline_and_never_replays_saved_running_state` covers hidden-page handoff, wall-clock change, one delivery and no startup rearm.

This is not a general sound or background-widget service. `timer_settings_accepted` only accepts the bundled Countdown action with an enabled `deckweaveKeyLightingAlert`. `timer_current` requires the originating profile to remain current. [Core::current_plugin_profile](../../../crates/deckweave-core/src/lib.rs) searches all pages of the selected profile, so hiding a page within that profile can retain the deadline; selecting another profile retires it. A new service must separate schedule ownership from the physical lighting target and current device profile while retaining checks for explicit stop, settings change and plugin disable.

The existing lighting worker wakes every 100 ms. That is evidence of current behavior, not a suitable idle-load target for the new service. An empty common scheduler can wait until a command or nearest due time. The physical lighting adapter retains its own existing bounded frame cadence and recovery rules.

## Optional Tauri renderer has relevant APIs, not performance proof

The installed `tauri-2.12.3` crate source contains `focused`, `always_on_top`, `skip_taskbar`, `focusable`, `transparent`, `set_ignore_cursor_events` and `set_focusable` in `src/webview/webview_window.rs`. Window creation is documented there as deadlocking from a synchronous Windows command or event handler. The renderer must be created through an appropriate asynchronous/main-thread path; no synchronous plugin request may wait while holding Core's mutex for window creation.

The public [builder documentation](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html) and [window documentation](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html) describe those window controls. The browsed latest version was 2.12.5; installed source, rather than the latest page, is the availability evidence for the locked 2.12.3 dependency. These controls do not establish memory use, idle CPU, or the correctness of multi-window mouse behavior.

A candidate Tauri renderer can reuse Svelte and existing appearance tokens. A native Windows renderer can avoid webviews but adds text, image, input, accessibility and platform maintenance work. Neither should be declared lightweight without measurements. Compare baseline, one static panel, several static panels, one live widget, click-through and close/reopen cycles, including the application's browser child processes. A baseline reading of a running editor is not a panel benchmark.

## Native alternatives are technically possible, with narrower evidence

Microsoft documents mixing audio through source, submix and mastering voices, with volume adjustment on each voice. This supports evaluating a single in-process Windows playback engine with per-playback volume, independent of the existing master-volume controls. It does not select a Rust implementation or prove output-device-loss handling. See [XAudio2 Voices](https://learn.microsoft.com/en-us/windows/win32/xaudio2/xaudio2-voices).

Microsoft documents mouse hit testing for layered windows, including pass-through for fully transparent areas and the effect of `WS_EX_TRANSPARENT`. This is a possible native rendering/input boundary, not evidence that a full-screen transparent Tauri webview will give correct independent interactive panels. See [Window Features: Layered Windows](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows).

## Persistence and isolation need explicit ownership

The current store checks configuration format 3 on open, load and write and merges unknown fields from the last known document. A new panel store must not rely on an older writer preserving fields that it cannot execute or interpret. Choose and document a versioned host-service storage boundary, atomic updates and local asset ownership before rollout; test restart, malformed records and rollback without touching the owner's live database for experiments.

The desktop's `get_snapshot` command currently restricts access to settings and device-editor windows. A panel renderer must receive only its own sanitized render model and fresh button tokens. Giving it the full editor client would be a new privilege expansion and would also copy unrelated workspace data into every panel.

## Proposed resolution boundaries

1. Reuse and generalize the existing timer planner, with output adapters for sound, panel and key lighting. A single timer run must have one authoritative deadline and event identity.
2. Keep host-service ownership independent of page visibility and profile selection. Keep physical input ownership and keyboard writes unchanged.
3. Make renderer work, sound playback and durable user edits independent queues so a slow panel cannot stall device input or timer state transitions.
4. Benchmark a bounded renderer prototype before choosing many webviews or a new native UI stack. No performance numbers are claimed yet.
5. Keep screenshot capture as a visible user command after the shared image panel and asset lifecycle work; do not conflate displaying an image with acquiring one.
