# Personal workspace: intake

Date: 2026-11-03. Historical intake. The owner subsequently delegated initial
choices and implementation. Six reviews and implementation are complete; see
[review disposition](review_report.md) and [working setup](../../PERSONAL-WORKSPACE.md).
The original questions below are retained as intake history, not pending gates.

## Requested outcome

Use DeckWeave for daily report writing in VS Code and Typst, Codex, and the
browser. Provide a separate system-monitoring page with CPU load, RAM use,
GPU load and GPU memory use. Provide quick access to Telegram, Telemost and
Zotero. Keep weather, date/time and a Pomodoro timer easy to see. The Pomodoro
must have audible phase notifications. Widgets must remain legible at their
actual physical display size.

The next stage includes notes on device keys, recent Telegram notifications,
and recent unread email. These are later features, not reasons to delay the
first daily-use configuration. Access methods and the meaning of "recent"
and "unread" have not been selected.

## Verified local foundation

- The saved Mirabox N4 Pro has ten display keys, four dials and a touch strip.
  Its active profile is Tools; Default and an acceptance profile also exist.
  Preserve these until a separate personal configuration has been reviewed.
- The workspace also contains virtual devices and the saved IO Type 84.
  Do not rewrite keyboard mappings as a side effect of configuring the deck.
- CPU, RAM and uptime are implemented by System Monitor. GPU and VRAM are
  not implemented in that plugin or the inspected system helper.
- Weather supports keys and touch regions. The saved workspace contains
  Ekaterinburg and London examples; inspect the Mirabox-specific binding before
  reusing a location. Keep Elgato Weather disabled as previously instructed.
- Shared host audio and persistent countdown panels exist. The current Timers
  plugin is a countdown/stopwatch, not a complete work/break Pomodoro cycle.
  A device Pomodoro needs one owner for phase timing and sound even while its
  page/profile is hidden; two independent countdowns are not an acceptable substitute.
- Local VS Code has Typst Typewriter 0.3.10, Tinymist 0.15.10 and the OpenAI
  extension. Typewriter exposes commands for formatting/navigation and PDF
  export. Their presence does not establish a DeckWeave command bridge.

## Working direction, not a decision

Keep daily status in stable places while work commands change by context.
Use a dedicated monitoring page and task-oriented work pages/profiles. Assess
weather, clock and Pomodoro placement on the touch strip before spending
several of the ten keys on status. Pinned placement must remain consistent
across the personal pages; profile changes must not reset the Pomodoro.

Do not select work buttons just because an editor command exists. The user's
actual repeated operations should determine the limited physical layout.
Launching an application and running a command in its intended window are
different operations; a blind keystroke sent to the wrong app is a failure.

## Risks to resolve during design and review

- Four categories of GPU activity cannot be reduced to an unspecified number.
  Select the adapter and define load and dedicated/shared memory explicitly.
  Missing telemetry must show unavailable, not zero.
- A miniature desktop dashboard or an analog clock with tiny secondary text
  can look acceptable in a large preview and fail on the device. Give each
  display one primary value, limit secondary information and inspect artwork
  at native size through the existing JPEG output path.
- A complete Pomodoro needs work/short-break/long-break transitions, pause,
  resume, reset and once-per-transition sound. Existing timers must retain their
  current behavior unless deliberately adopted.
- "Frequently visible" weather is not necessarily permission to create another
  always-on-top desktop window. Distinguish device visibility from desktop overlays.
- Telegram notifications and Telegram chat history are different sources.
  Email previews need a selected source and privacy boundaries. Do not read
  private messages or mail during this initial layout discussion.

## First clarification

Identify the repeated VS Code/Typst, Codex and browser operations that should
become physical controls. Software discovery and implementation details can be
handled independently; which operations deserve the ten keys is a user workflow
decision. Clarify this before fixing a layout or starting fresh-context reviews.
