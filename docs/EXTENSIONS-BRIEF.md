# Extension work brief — 2026-11-02

Owner request: make the action library predictable, reduce physical page latency,
and provide a practical starting point for open-source plugin authors. Technical
choices are delegated; preserve existing profiles and the tested N4 JPEG format.

## Outcomes and boundaries

1. Library editing beside search: show/hide and order groups and individual
   actions, persist globally, restore defaults. Hiding never disables assigned
   actions. Respect Elgato's `VisibleInActionsList`; resource-only icon packages
   belong in installed-package information, not in the action list. Development
   examples start hidden and remain available through an explicit switch.
2. Instrument the output pipeline and remove idle input waits while output is
   pending. Keep input servicing between complete HID frames, a bounded artwork
   cache, bounded timing records, and the physically accepted JPEG encoder mode.
   Measure CPU and USB separately; software completion does not prove LCD latency.
3. An action SDK and template must work without private paths, unpublished npm
   dependencies, or a build of DeckWeave. Node 22+, ESM, per-context disposal and
   stale-result protection. A generated bundle must validate and package locally.
   Hardware providers remain a separate, unfinished API, documented honestly.
4. A versioned optional-plugin registry provides metadata, validation and
   contribution instructions. Local development entries and published immutable
   archives are distinct. No automatic download, installation or execution.

## Starter plugin selection

- System Audio: master volume/mute, dial adjustment, observed system state.
  Native CoreAudio helper with bounded JSON-line requests; no shell per detent.
  No automatic volume changes on startup or polling. Per-application mixing waits.
- System Monitor: CPU, RAM and uptime, local read-only Node OS APIs. It demonstrates
  sampling without network access and responsive artwork for keys and panel areas.
  CPU is whole-system utilization over a sample interval, not process CPU.
- Timers: countdown/focus timer and stopwatch, explicit start/pause/reset,
  independent instances, monotonic time and readable small artwork. No surprise
  sound or external application launches. Hidden pages stop plugin contexts; timer
  semantics must explicitly account for page switches and process restarts.
- Existing Weather remains the network example with explicit city selection.
  Elgato Weather remains disabled, as requested; do not enable IP geolocation.

## Risks to review with fresh EGM lenses

Library visibility versus execution; stale settings writes across windows; SDK
disposal and async races; native helper lifecycle and packaging; timer behavior
across page navigation; registry trust versus executable code; USB input fairness
under fast output. Review artifacts and implementation, fix concrete defects, and
record unresolved product choices rather than inventing acceptance chores.

## Validation and owner acceptance

Automate model/protocol tests, validate all bundles, generate and package a fresh
plugin, run UI/static/Rust checks, and test real host lifecycle. Native UI confirms
library editing and settings persistence. Hardware instrumentation reports page
completion and cache hits. Owner acceptance is limited to perceived LCD transition
quality/input responsiveness and readability/usefulness of the new widgets.
