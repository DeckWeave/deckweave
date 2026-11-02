# Host feedback: first implementation contract

**Date:** 2026-11-03. Implements the reviewed concepts incrementally. The owner delegated technical decisions and authorized implementation; no additional section-by-section approval is required. Runtime capability versions describe implemented slices only.

## Boundaries

Create `crates/deckweave-services` for validated records, independent SQLite storage, explicit panel open/update/close, native timer jobs and event deduplication. Modify `deckweave-platform` with an in-process Windows notification playback boundary. Create desktop `host_services` for a bounded worker, rendering adapter, authenticated plugin requests, restoration and scoped Tauri commands. The existing application configuration format and hardware writes stay outside this service.

The initial renderer is one lightweight Tauri window per open panel, with no editor client or full configuration access. It receives only its own sanitized view. Static notes have no refresh interval. A timer render refreshes at one second; plugin metric values arrive from one shared sampler. Maximum eight open windows in this measured first slice, 128 saved instances, 8 KiB text per panel, 32 queued commands, 64 recent outcomes. Images and screenshot acquisition follow the managed-asset slice; unsupported content is rejected rather than silently approximated.

## Durable identity and data

`host-services.sqlite3` has its own schema version, WAL and full synchronous commits. A panel has a generated UUID, plugin owner, generated service-instance UUID, origin action ID as provenance, local key, persisted content/configuration, placement and open state. The unique `(owner, origin, localKey)` creation mapping makes repeated Open idempotent while copied actions get independent instances. An origin ID is not an authorization token. Live values and connection readiness remain in memory. A closed record survives for intentional reopen.

Records deserialize independently. Unsupported or malformed records remain stored and appear as unavailable entries; a bad record does not delete other panels. A newer service schema fails closed for service writes without changing the main configuration. Updates and close are revision-checked. Commit close before hiding. Renderer movement is coalesced and saved after the gesture settles; ordinary content updates cannot overwrite placement or open state.

## API and authorization

The host advertises separate `sound`, `panels` and `timers` version numbers in `info.deckweave.services`. Requests use `deckweaveService` with a bounded request ID, operation and payload; responses use `deckweaveServiceResult` with typed accepted/duplicate/failed outcomes and a stable error code. SDK helpers return promises with a timeout; legacy boolean methods retain their behavior. Preview is distinct from timer delivery.

Visible action calls must belong to an authenticated current plugin slot. Durable service calls must belong to that package's current authenticated service generation and own service-instance ID. A plugin cannot choose another package owner. Background service appearance/disappearance events are opt-in and separate from Elgato `willAppear`. The same package supervisor combines visible demand and saved live-service demand. Disabled packages override both. No second package launcher is introduced.

Only settings windows can enumerate/manage all panels. `overlay-<id>` windows can read their own view, close their own panel and invoke explicit native timer controls, without reading the workspace. Host windows render escaped text, never plugin HTML. Placement includes monitor, anchor, logical offsets, width/height, background/text colors and click-through. Panels show without taking focus; a Panels settings section provides recovery and position reset.

## Sound

Bundle three short original synthesized PCM cues generated deterministically by repository code, with offline provenance. Scale sample amplitude for per-request volume, never system master volume. A dedicated bounded playback worker uses Windows multimedia playback in process; no player subprocess or webview is needed. The initial backend serializes short cues. WinMM reports accepted and finished/failed; it has no reliable separate started acknowledgement. Completion is a backend result, not proof that a person heard it. Other platforms report playback failure until an adapter exists.

## Timer authority and adoption

Host timer jobs have their own durable run ID, status, remaining duration, deadline and revision. Native output events are consumed once per run before sending to the playback queue; restart suppresses elapsed historical alarms and re-arms valid future deadlines. Pause/reset/stop are explicit. Closing a panel does not cancel the run. Profile changes and device loss do not cancel it. Plugin process loss cannot cancel an accepted native job; explicit disable/removal does.

The first new host-owned timer is separate from legacy timer runs. Legacy assignments remain silent and retain their previous lighting behavior unless explicitly adopted. Once the bundled Timers plugin uses a host run, its view consumes host state; its legacy draw path and key-lighting planner must not also own that run. Extract reusable deadline logic as the adoption slice lands, preserving existing tests. No host capability is advertised for an unimplemented bridge.

## Verification and staging

First prove isolated-store restart, explicit close/update races, cross-owner denial, copied origins, stale revisions, bounded input and timer replay rules. Then verify SDK promises, timeout and absent-capability behavior with actual bundled SDK copies. Build and inspect the native panels, placement and global inventory. Measure baseline and one/four/eight panel process trees, static versus live updates and close cleanup before accepting the renderer for the full release. Audio audibility, focus, click-through and physical monitor/DPI behavior remain distinct acceptance checks. Record actual measured outcomes without calling source tests physical proof.
