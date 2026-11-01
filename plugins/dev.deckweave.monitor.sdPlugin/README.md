# System Monitor

Optional DeckWeave action plugin, GPL-3.0-or-later. Node 22+, no dependencies,
network access or elevated privileges. SDK 0.1.0 is vendored in `lib/`.

Drag CPU, Memory or Uptime onto a key, dial area or Panel region. Settings change
the label and language. CPU is the whole system average over a two-second sample,
not this process's CPU; the first sample is a warm-up. Memory means used physical
RAM (OS caches may be included), uptime is since OS boot. A shared sampler runs
only while an instance is visible. Pressing these read-only widgets does nothing.

The same SVG adapts to key, Encoder `$A0` feedback and actual Panel canvas sizes.
Tests: `node --test scripts/tests/starter-plugins.test.mjs` from the app repository.
This is a useful small example of shared sampling and independent settings.
