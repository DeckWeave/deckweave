# Action plugins and open-source workflow

The application remains a development release for Windows 11. The action SDK is
usable today; the external **hardware provider SDK is not implemented**. N4 is
built in. Do not present the action template as a USB/device-driver template.

## Repositories and quick start

Keep the four repositories beside one another for local workspace validation:

```text
DW/
  deckweave/          application and starter bundles
  plugin-sdk/        dependency-free ESM runtime and protocol/types
  plugin-template/   standalone generator, PI, tests and ZIP tools
  plugin-registry/   optional-plugin metadata, schema and validation
```

An author needs Node 22+; a Rust build is unnecessary for ordinary action plugins:

```powershell
node ../plugin-template/bin/create.mjs ./my-plugin.sdPlugin dev.example.myplugin "My plugin"
cd my-plugin.sdPlugin
npm test
npm run validate
npm run pack
```

The generated directory includes its SDK, license, bilingual settings page,
tests and deterministic ZIP packager. It needs no sibling repository or npm
download afterward. Update `lib/sdk.mjs`, its matching types and `SDK-VERSION`
together when intentionally upgrading. The SDK is source-distributed, not an
unpublished npm dependency. It is GPL-3.0-or-later, as requested for this project.

To load a development plugin, put its complete `<reverse.dns.id>.sdPlugin` folder
inside the application's app-data `plugins` directory, then restart DeckWeave.
The host also scans this repository's `plugins` during source development. It
does not watch folders for newly installed packages. Do not overwrite the Elgato
installation. The registry is metadata only: no automatic download/install/run.
ZIP archives currently require manual extraction into the same plugins directory.
Settings/assignments live in the host database, outside the plugin bundle.

## Starter bundles

| Bundle | Controls | Useful extension example |
| --- | --- | --- |
| System Audio | Key, dial, Panel | Windows CoreAudio helper, observed state, ordered/coalesced input |
| System Monitor | Key, dial, Panel | Shared local CPU/RAM/uptime sampling, no network |
| Timers | Key, dial, Panel | Independent persisted countdown/stopwatch, hold and lifecycle |
| Weather | Key, Panel | Explicit city, bounded Open-Meteo requests/cache, original hand-written transport |

For Audio, run `./scripts/build-audio.ps1` from the application root before
distribution. `npm run pack` inside the audio bundle refuses a missing native
helper. The helper executable is generated and excluded from Git; its source is
in `crates/deckweave-platform/src/audio` and `src/bin/deckweave-system.rs`.
Monitor and Timers package with `npm run pack` inside their directories directly.
Archives have SHA-256 sidecars and contain a complete named `.sdPlugin` directory.
The registry documents the distinction between local workspace source and a
future immutable HTTPS release archive; no public release URLs are invented.

Built-in demo packages Canvas/Diagnostics/Panel Lab and test actions are hidden
from the ordinary library. Turn on “Show examples and diagnostics” in its display
editor to use them. Existing assigned examples keep working. Elgato's hidden
tutorial actions and resource-only Icon Library are not assignable library rows.
Unavailable packages remain visible with their reason in Settings → Plugins.
Original Elgato Weather remains disabled under the owner's existing preference.

## Lifecycle rules

One package process can own many independent action contexts. Use context state
for transient data and host settings for persistence; do not key data by screen
position or assume a context is visible forever. Dispose intervals, pending work
and child processes. SDK timers stop on disappearance; stale context feedback is
suppressed. Async handlers are concurrent, so plugins must own request sequencing.
Feedback sends are not execution acknowledgements. Encoder full-canvas images
use `$A0` and `setFeedback`; keys and native Panel use `setImage`.

Timer transitions persist immediately. Retirement snapshots re-anchor monotonic
elapsed time to the wall clock for navigation. Time while hidden or the app is
closed uses that wall clock; completion is silent and visible when returning.
This is a desk timer, not a background alarm service.

## Verification

`npm run check` covers app static checks, Rust regression/real-WebSocket tests,
Weather models, starter models and Audio's fake-helper lifecycle fixtures.
The native helper is checked read-only. Companion repositories have their own
`npm test` commands; registry adds `npm run validate:workspace` and strict archive
packaging checks. Windows CI runs the main checks and creates the three new
starter archives. CI configuration is checked in; a hosted run has not been
claimed before publishing the repositories.
