# Host feedback: engineering dispositions

**Date:** 2026-11-03. **Status:** Root-selected engineering directions, pending reconciliation with the completed six-lens report. These are proposed implementation requirements, not deployed behavior or measured performance. The owner's confirmed persistence and profile semantics are unchanged.

## Save and close feedback

Persistent content edits, placement changes and explicit close use a durable inventory revision. Close has a short pending state; it hides the panel only after committing closed intent. If the save fails, the panel stays visible and reports that it could not save the close request. The global panel inventory exposes the same error and Retry. It must not look successfully closed and return unexpectedly after restart. Failed content or geometry edits keep the last committed value and retain the unsaved proposal for retry without resetting other panels.

Malformed or unsupported saved records produce per-panel unavailable entries in the inventory. Preserve their raw records for recovery. A bad record does not erase the entire inventory or prevent unrelated valid panels from restoring. Do not log note text or image bytes in diagnostics.

## What a sound result means

Do not return an unqualified success boolean. The host response identifies a request and whether it was accepted, deduplicated, rejected, cancelled, or unsupported. A later playback event reports started, finished, or failed when the backend supports that observation. Started means the playback backend accepted and began processing the sound; it does not prove that speakers are connected or that the user heard it. Preview uses a separate request identity from a timer completion.

Record schedule lateness independently from audio start latency. Use stable reason codes and a bounded local diagnostic buffer plus current status. A network tracing service is unnecessary. Owner names, run IDs, request IDs and revisions provide correlation without payload dumps or authentication tokens.

## One timer authority

Extract and generalize the existing in-memory deadline planner from key-lighting notification code. A run gets a stable identity and revision; changing duration, pausing, resetting or restarting updates that run explicitly. Repainting a key, opening a page or restoring a widget never creates a new completion. Sound, panel and lighting are output adapters of one completion event, each with a recorded disposition. A slow renderer or missing keyboard must not cancel an unrelated sound output.

Profile selection and device disconnection do not cancel the run. Explicit stop/reset and owner disable do. A process restart restores saved timer state: a valid run with a future deadline re-arms only its remaining time; an already elapsed historical completion restores as finished without playing an old sound. A timer still running in the same process when Windows resumes may report a late completion once; sleep behavior and the chosen clock must be tested explicitly before release. No promise is made here about waking Windows from sleep or delivering alarms while DeckWeave is not running.

Completion delivery cannot be an exactly-once transaction with the physical audio device. Select and document the persistence boundary and crash semantics before implementation. Prefer avoiding unexpected old sounds after restart over replaying an ambiguous pre-crash attempt; diagnostics must distinguish suppressed historical output from successful playback. This does not relax ordinary running-session one-completion behavior.

### Existing timer assignments keep their current behavior

Installing an update must not add sound or open panels for existing timer assignments. Preserve their current runtime and configured lighting notifications. Adopt the new common scheduler on a deliberate subsequent start or explicit service opt-in, not by mistaking a loaded running state for a new start. Give adoption a durable ownership version and run identity outside the legacy runtime serializer; after adoption, an older visible-context settings write cannot become a second authority. Timers not yet adopted retain their documented existing limits rather than claiming new background guarantees.

Capability negotiation is separate for sound, panels and timer scheduling. A new host keeps old plugins' visibility behavior. A new plugin on an older host keeps its ordinary controls usable, explicitly reports the unsupported service and does not advertise a working background alarm. Release validation includes actual bundled `lib/sdk.mjs` copies, the sibling SDK, templates and installed package versions; updating only the SDK repository is insufficient.

## Separate commands from replaceable display updates

Commands such as close, configure, start, pause and button activation use bounded ordered queues and explicit acknowledgments. Do not coalesce two button presses or silently discard close intent. Continuous widget values may replace an older pending value for the same panel. Bound both per-owner ingress and global work so one plugin cannot starve other widgets, timer outputs or device input. Reject oversized content before allocating or decoding it.

The exact first-release caps are engineering choices to set in the renderer experiment. Exercise one, four and eight static panels; a live CPU widget; a timer notification; oversized images; a noisy owner; and repeated close/reopen cycles. Count browser child processes if a webview candidate is measured. Report incremental private memory and CPU against the same application's baseline, update age, audio start latency, device-input latency and retained resources after close. A single memory reading or total host CPU during unrelated work is not a renderer benchmark.

## Recoverable plugin ownership

### Explicit open, update and close

Replace the provisional single `upsert` operation with separate intent. `open` creates or deliberately reopens an identified instance; repeating the same request does not duplicate a panel. `update` changes an existing open panel's content or live values and never reopens a closed instance. `close` commits closed intent. A stale periodic update cannot reverse the user's close. Names remain subject to the formal SDK contract, but these three semantics are required.

The host assigns a stable panel instance ID under a plugin and an owning service-instance ID. A local name such as `main` is scoped inside that owner, not directly under the plugin package: two actions from the same plugin can create independent panels. A new independent panel is an explicit create-new operation; a refresh or reconnect is not one.

For the first Show/Open action, pressing it again opens or brings back the same instance without toggling it closed or creating another copy. A copied device action has a fresh creation identity and does not automatically open a panel or acquire another action's saved instance. Copying defaults and deliberately linking an existing instance are distinct operations. If a Toggle action is later provided, it must be labelled as Toggle rather than hiding that behavior inside Show.

The saved panel instance owns its widget configuration after creation. The creating device action is a provenance link and an entry point to its inspector, not its lifetime. Editing action defaults must not silently replace another saved instance's content or geometry. The inspector must clearly distinguish editing that saved panel from editing defaults for future instances, and provide a direct route to existing instances. Global inventory access remains available if the original action was removed or its device is offline. No second custom plugin settings implementation is required; the same inspector can receive an explicit panel settings scope.

Persistent panel identity is independent of the current page and profile. Authenticate the package connection and bind every update and click to a fresh service generation and expected panel revision. Reject queued events from a previous connection. Keep live widgets visibly unavailable after plugin loss; keep static content readable. A profile change alone never makes a panel unavailable.

Closing a timer panel dismisses its presentation, while Stop/Reset are explicit timer operations. A fully accepted host-owned run continues after its plugin process crashes and may deliver its already validated native outputs without restarting that plugin just to play a sound. No dead plugin callback can run. Explicit owner disable/removal cancels that owner's jobs; a transient process failure is different from the user disabling it. A host-rendered timer can show its authoritative job state while plugin-specific functions are unavailable.

Use the existing single package supervisor. Visible actions and live services contribute separate process-demand reasons; saved static content contributes none. Disable/removal and upgrade pause override demand. Do not launch a second copy of a package because its service and visible action both need it. Cold-start restoration must establish service demand before expecting a hidden-source plugin to register.

Do not implement this by extending the existing retired visible-context grace period. That grace period protects late settings persistence; it is not a background execution contract. Do not send invented Elgato visibility events to legacy plugins. New DeckWeave service registration and service-instance events are opt-in, versioned SDK capabilities.

## Storage and rendering are pending bounded experiments

Use a separately versioned host-service database, leaving the existing configuration format and profiles untouched. Older builds leave that service database alone; returning to a newer build restores compatible saved instances without replaying old completion delivery. That is preservation on rollback, not a promise that an old build can display the new panels. The design must define transaction boundaries, asset ownership and per-record recovery. Test upgrade, panel edit/close, an unrelated profile edit in the older build, return to the new build, unsupported records and asset loss using isolated databases rather than the owner's live workspace.

Benchmark a Tauri-backed panel renderer before selecting many webviews or adding a second UI framework. Existing Tauri controls make a prototype possible; they do not prove low resource use or acceptable mouse hit testing. The shared content and service contracts must not depend on that backend. A native Windows candidate remains an option if the experiment fails, with its additional rendering and accessibility maintenance cost recorded.
