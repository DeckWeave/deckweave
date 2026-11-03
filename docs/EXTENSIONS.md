# Action plugins and open-source workflow

The application remains a development release for Windows 11. Action plugins and
device support are separate package kinds. The external device process SDK,
template and host are implemented provisionally; N4 remains built in. See
[device workflow and acceptance](DEVICE-EXTENSIONS.md) and the
[provider boundary](PROVIDER-CONTRACT.md). The action template does not drive USB.

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

In Settings → Plugins, **Install from file…** accepts the generated ZIP or an
unprotected `.streamDeckPlugin` archive. Review the bundle identity, version,
action count and SHA-256, then choose Install/Update. Cancel leaves the installed
files unchanged. A successful install refreshes all editors without restarting
DeckWeave; assigned contexts of an updated package restart. Settings and action
assignments remain outside the bundle, in the host data directory.

**OpenDeck catalogue** reads the public [OpenAction catalogue](https://github.com/OpenActionAPI/plugins).
Search for a plugin and choose Install… (or Update…). A modal opens immediately at any
catalogue scroll position; loading, release selection, download review, errors
and installation stay in this same modal. Select an author-provided archive for
your OS, then Download and inspect → Install. Closing returns focus to the
original row without resetting search, loaded entries or scroll position. Search
and catalogue controls remain visible while scrolling. [Native UI verification](evidence/2026-11-02/plugin-manager-usability.json). The latest stable GitHub release
is used; source code is not built automatically. Missing releases, API limits,
protected manifests, unsupported platforms and incomplete bundles show errors.
Catalogue inclusion alone is not proof of full DeckWeave compatibility. Hardware
providers are installed from their own local `.dwDevice` ZIPs, not from the
OpenDeck action catalogue. Resource-only archives are not assignable actions.

Managed copies go to the application's app-data `plugins/<id>.sdPlugin`.
They take precedence over the repository development bundles and locally installed
Elgato packages. Original Elgato files are never overwritten. Disable/enable is a
generic per-package control in Settings → Plugins, independent of library display
preferences. Disabling stops the backend and inspectors while preserving assignments.

For manual source development, put the complete `<id>.sdPlugin` folder in the
app-data plugins directory and restart; arbitrary filesystem changes are not
watched. The host also scans this repository's `plugins`. The sibling DeckWeave
registry remains author metadata/validation, separate from the public OpenAction
catalogue; no release URLs are invented or unpublished packages downloaded.

## Starter bundles

| Bundle | Controls | Useful extension example |
| --- | --- | --- |
| System Audio | Key, dial, Panel | Windows CoreAudio helper, observed state, ordered/coalesced input |
| System Monitor | Key, dial, Panel | Shared CPU/RAM/GPU/VRAM/uptime sampling, native PDH/DXGI, no network |
| Timers | Key, dial, Panel | Independent persisted countdown/stopwatch, hold and lifecycle |
| Weather | Key, Panel | Explicit city, bounded Open-Meteo requests/cache, original hand-written transport |
| Date & Time | Key, Panel | Offline digital clock, native-sized date and weekday |
| Pomodoro | Key, dial, Panel | Shared host job, persistent work/break phases and host sounds without overlays |
| Personal Workspace | Key, dial, Panel | Targeted Windows apps and authenticated local VS Code commands |

For Audio, run `./scripts/build-audio.ps1` from the application root before
distribution. `npm run pack` inside the audio bundle refuses a missing native
helper. The helper executable is generated and excluded from Git; its source is
in `crates/deckweave-platform/src/audio` and `src/bin/deckweave-system.rs`.
For Audio, Monitor and Personal Workspace together, run `npm run prepare:workspace`
from the application root. Monitor and Workspace archives refuse missing native
helpers. Timers, Clock and Pomodoro need no native helper in their bundle.
Each bundle packages with `npm run pack` inside its directory.
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

Archive regression tests cover unsafe paths, links/case collisions, bundle identity,
platform/required files, preview cancellation/expiry, updates and rollback. The
public Counter v1.0.1 ZIP was downloaded/inspected without executing its backend.
For an opt-in native host install → assigned action → live update → disable/enable
regression, close other DeckWeave instances, build debug and run:

```powershell
cargo build -p deckweave-desktop --locked
node scripts/smoke-installer.mjs
```

This generates first-party v1/v2 fixture archives under `.local`, uses a fresh
isolated database, disables physical hardware discovery and exits on completion.
It verifies new feedback after the update and retention of the assigned settings.
On Windows, run it in an ordinary desktop process: restricted execution can block
WebView2 initialization. [Recorded result](evidence/2026-11-02/plugin-manager.json).
