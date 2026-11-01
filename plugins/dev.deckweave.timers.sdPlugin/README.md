# Timers

Optional DeckWeave action plugin, GPL-3.0-or-later. Node 22+, no dependencies,
network requests, sound or application launches. SDK 0.1.0 is vendored in `lib/`.

Countdown / Focus defaults to 25 minutes. Stopwatch counts upwards. Press a key,
dial or Panel area to start/pause; hold 0.8 seconds to reset. Rotating a paused
countdown changes its configured duration by a minute per detent and resets it.
The Property Inspector provides duration, label, language, Start/Pause and Reset.
Changing duration resets the timer. Each assigned action has independent state.

Transitions persist through host `setSettings`. While visible, elapsed time uses
the monotonic clock; on page return or app restart the saved wall-clock anchor
accounts for time away. A system clock change while hidden therefore affects
elapsed time (backward changes are clamped). Expired hidden timers show Done when
next visible; there is deliberately no background alert or sound. Completion on
a visible page turns green and remains until reset/restarted. No timing service
or polling process runs for hidden actions.

Tests: `node --test scripts/tests/starter-plugins.test.mjs` from the app repository.
Models cover navigation, pause, reset, expiration, stopwatch and rendering. The
backend demonstrates disposal-safe holds, persisted state, and Encoder feedback.
