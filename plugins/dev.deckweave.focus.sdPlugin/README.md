# Pomodoro

Durable host-owned Pomodoro for hardware keys, touch regions and encoders. Distinct
from legacy countdown/stopwatch actions. No desktop overlay is created.

`dev.deckweave.focus.pomodoro` and `dev.deckweave.focus.control` both display the
shared state and perform the configured `operation` on press. Defaults are 25/5/15
minutes, four work rounds, manual next-phase start, complete cue at 35% volume.
Long press resets after 800 ms when `holdReset` is true (default).

All bindings owned by this plugin with the same `workspace` (`personal` by default)
share one job, including across pages and profiles. `operation` accepts `toggle`,
`start`, `pause`, `reset`, or `skip`. `skip` advances one phase and waits for manual
start, including when skipping work; the round counter describes the cycle's
position. It is not a count of successfully completed work periods.

Editable settings: `workspace`, `operation`, `holdReset`, `workMinutes`,
`shortMinutes`, `longMinutes`, `rounds` (1–16), `sound` (`complete`, `notice`,
`warning`, or null), `volume` (0–100), `language` (`en`/`ru`). Shared timing and sound
settings are authoritative in the host. New bindings only supply defaults for a
new workspace; opening an old page never overwrites the job. Inspector edits save
automatically with a revision check. A running/paused phase retains its remaining
duration when settings change; new durations apply to subsequent phases.

The plugin has no clock, periodic polling, local persistence of run state, or
sound playback. The bounded native scheduler persists transitions, sends visible
controls current phase/remaining time, and attempts the host cue once. Audio queue
or device failure can lose a cue: host service `outcomes` contains queued/finished/
failed diagnostics with the stable `focus-<run UUID>-<transition>` event identity.
Restart silently consumes one elapsed transition and leaves the next phase ready;
it neither replays old audio nor runs through missed breaks. Disabling this plugin
pauses its jobs. Hidden running jobs retain the plugin process without sending it
display ticks.

Host API (`focus:1`): `focus_get` takes `{workspace, config?}` and creates only if
absent; `focus_control` takes `{workspace, revision, commandId, command, config?}`.
Configuration uses seconds (`work_seconds`, `short_seconds`, `long_seconds`),
`rounds`, `sound`, `volume`; command `configure` applies it. Command IDs are UUIDs;
the latest 64 commands are deduplicated, and older retries fail their stale
revision. A conflict refreshes display state and asks the user to press again.
Host event `deckweaveFocusDidChange` carries `{jobs:[...]}`. These additive records
use a separate versioned `focus_jobs` table and preserve overlay/timer schema v1.

Install this source directory as a local plugin in DeckWeave, or run
`node tools/package.mjs pack` and import its archive. No third-party dependencies.
Run `node --test` and `node tools/package.mjs validate` for local checks.
