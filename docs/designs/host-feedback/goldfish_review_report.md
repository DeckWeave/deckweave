# Goldfish Review Report

**Mode:** BRAINSTORM, six independent lenses. **Date:** 2026-11-03.

**Target:** `G:/APPS/DW/deckweave/docs/designs/host-feedback/working_hypothesis.md`, lines 1–89. SHA256: `14B2854797631DF75411411AAE2B32E3E50117C2D09502B812611DB6762023ED`.

The target is unchanged. All line references below refer to that frozen target unless a source path is given. Reviewers assessed a concept, not a completed implementation plan. Missing proposed APIs are expected future work. This review establishes neither implementation readiness nor physical Windows acceptance.

Source paths are relative to `G:/APPS/DW/deckweave` unless stated otherwise. In shortened citations, `plugins/mod.rs` means `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs`, `key_lighting.rs` means the sibling `key_lighting.rs`, and `sdk.mjs` means `G:/APPS/DW/plugin-sdk/sdk.mjs`. Raw manifests give the exact paths and read ranges.

## Reviewer status and isolation

| Reviewer | BRAINSTORM lens | Status | Raw findings |
| --- | --- | --- | --- |
| `egm-design` | Architecture and resource economics | completed | 4 Major, 1 Nit |
| `egm-codebase-details` | Observability and source grounding | completed | 4 Major |
| `egm-zoom-out` | Reuse and YAGNI | completed | 2 Major, 3 Nit |
| `egm-comprehension` | User and developer ergonomics | completed | 5 Major, 1 Nit |
| `egm-implementation-readiness` | Migration and rollout | completed | 5 Major, 1 Nit |
| `egm-structural` | Maintenance and lifecycle coupling | completed | 6 Major |

The raw count is **0 Blocker, 26 Major, 6 Nit**, including explicitly severity-labelled product clarifications. After combining overlaps, this report contains **0 Blocker, 11 Major, 3 Nit**. Ten consolidated Major findings are engineering requirements; M11 groups user-visible policy gaps in the frozen target, now given root-selected implementation defaults. A Major is a decision or safeguard required before the affected implementation stage, not evidence that the concept is infeasible.

Each reviewer was a real general-purpose agent with `fork_turns: "none"`. Each read its bundled persona, the target and every supplied standard and returned a read manifest. Feature Specification Standard applied to all; design also read Heilmeier, zoom-out read Zoom-Out, and readiness read Implementation Readiness. That last standard's zero-decision test was reserved for a later implementation plan.

Four global agent slots included root and synthesizer, so concurrency was limited to three waves of two: design/codebase-details, zoom-out/comprehension, readiness/structural. Later reviewers received the same immutable target and optional source list, including `key_lighting.rs`, with no earlier findings. No reviewer failed or required a retry. A comprehension manifest hash transcription typo was corrected by actual read-only verification; the original report and correction are preserved together.

Raw reports are saved verbatim in [design](reviewer_design_raw.md), [codebase details](reviewer_codebase_details_raw.md), [zoom-out](reviewer_zoom_out_raw.md), [comprehension](reviewer_comprehension_raw.md), [implementation readiness](reviewer_implementation_readiness_raw.md) and [structural](reviewer_structural_raw.md). Their manifests distinguish full reads from targeted excerpts and searches. Optional `plugins/contexts.rs` and `plugins/lifecycle.rs` were discovered to be absent and were not used as evidence.

Only after all six reports arrived did the synthesizer read root's [source follow-up](source_followup.md) and [engineering dispositions](implementation_decisions.md). Those are separate evidence and proposed resolutions, not independent reviewer findings. They were not fed back into later reviews. Reviewers changed no files and ran no builds, hardware operations, UI automation or private database queries.

## Preserved product constraints

Persistent notes, images and live widgets survive page, folder and profile changes. Restart restores content, position and settings. Profile changes cannot freeze a live widget. Closing stays closed; restart cannot replay an old sound. Plugin loss preserves static content and makes live data visibly unavailable until a fresh compatible owner returns. These constraints are settled and must not be traded away to simplify implementation.

## Prioritized findings

### M1. One timer authority, reused from the existing planner — Severity: Major

* **Reviewer lenses:** design, zoom-out, readiness, structural; codebase-details supports delivery observability.
* **Affected scope:** current-code evidence, lines 21–25; sound, lines 29–33; registry, line 47; staging, line 81.
* **Gap and evidence:** The target recognizes render-driven completion as unsafe but omits the existing host `TimerPlanner` in `apps/desktop/src-tauri/src/key_lighting.rs:49–128`. Its accepted-settings hook is in `plugins/mod.rs:1274–1287`; its current-profile and lighting ownership checks at `key_lighting.rs:356–439` cannot be inherited unchanged by durable desktop jobs. Adding unrelated lighting, sound and panel planners would create competing completion decisions. Legacy timer serialization at `plugins/dev.deckweave.timers.sdPlugin/timer.mjs:47–52` can also discard newly added run markers.
* **Mitigation:** Extract the bounded deadline/completion logic into one output-independent authority. Give every run a stable identity, revision, authoritative state/deadline, explicit pause/reset/cancel and bounded completion retention. Device views, widgets, sound and lighting consume one accepted completion through separate adapters. Fence legacy writers during adoption; make adoption and job creation idempotent; never import an overdue saved timer as a new audible event.
* **Alternatives:** Extract the current planner, or replace it and explicitly migrate lighting to the new authority. A narrowly typed deadline job is preferable to generic arbitrary background callbacks. Legacy runs can remain legacy until their next explicit start if their temporary limitations are stated.
* **Disposition and gate:** Root proposes extraction and one run authority, adopting legacy runs on a deliberate subsequent start or explicit service opt-in with durable fencing outside the old serializer. Detailed crash/persistence boundaries, clock/sleep behavior, delivery retention and adoption still need design and tests. No exactly-once transaction with a physical audio device is claimed.

### M2. Process admission must include durable service demand — Severity: Major

* **Reviewer lenses:** structural; design and codebase-details support resource and lifecycle verification.
* **Affected scope:** lines 45–51.
* **Gap and evidence:** Current package admission starts from visible slots at `plugins/mod.rs:795–798`, starts packages at `863–874` and retires sessions without remaining assignment demand at `847–861`. A second context map alone cannot restore a hidden widget if its package never starts.
* **Mitigation:** Use the existing single package supervisor, combining visible-action and active durable-service demand. Static notes/images create no plugin demand. Explicit disable/removal, upgrade pause and shutdown override demand and retire callbacks. A restored compatible widget must be able to admit its package before registration, within quotas.
* **Alternatives:** Reviewers allowed extending the existing supervisor or coordinating another service supervisor with it. Root selects extending the existing single supervisor. Do not introduce another independent launcher for the same package or retain every hidden action.
* **Gate:** Demonstrate restart without a visible source action, removal of the last device assignment, plugin loss/reconnect and cleanup after the last live service closes. This is distinct from keeping a panel window visible.

### M3. Stable identity and editing ownership within a plugin — Severity: Major

* **Reviewer lenses:** comprehension, structural; readiness supports preserved identities.
* **Affected scope:** lines 9, 37 and 41–49.
* **Gap:** A plugin namespace prevents cross-plugin replacement but does not distinguish two timer actions both using `localId: "timer"`. The visible SDK context is not a durable identity. Action defaults, saved panel configuration and inventory edits also need an explicit relationship.
* **Mitigation:** Give each service instance and panel a host-issued durable identity; scope local names within the service instance. Keep the saved panel's configuration independent of the source action's current assignment. The inspector must clearly select either an existing panel or defaults for future panels. The source action is provenance and an editing entry point, not lifetime ownership. Host placement edits remain authoritative.
* **Alternatives:** Durable action/service scope for local IDs, or host-issued persistent handles. Snapshot configuration with an explicit update command, or an inspector selecting an existing saved instance, are both coherent.
* **Disposition and gate:** Root selects host-issued service/panel IDs and explicit panel/default editing scopes. Include an example with two same-plugin instances, hidden source pages, inventory editing and reconnect. Copy/repeat defaults are recorded separately in M11.

### M4. Updates must not reopen closed panels or overwrite user placement — Severity: Major

* **Reviewer lenses:** comprehension, readiness, structural; codebase-details supports save failure visibility.
* **Affected scope:** lines 37, 41–49 and 89.
* **Gap:** A conventional `upsert` could recreate a panel after user close. A valid current connection can also send old geometry while the user drags the panel. Connection generations alone do not settle these conflicts.
* **Mitigation:** Separate deliberate open/create, update-only and durable close semantics. Preserve a closed record or creation epoch to reject delayed updates. Distinguish content revisions from host-owned presentation state or define a conflict rule that cannot lose user intent. Never replay queued button events during restoration or reconnect.
* **Alternatives:** Explicit commands, or an opening epoch required to change visibility. Whole-record revisions are simpler but create conflicts with frequent telemetry; field/domain revisions can isolate those updates.
* **Disposition and gate:** Root proposes separate open/update/close operations and a pending close that hides only after commit; failure leaves the panel visible with Retry. Test close versus update/reconnect, stale generations, reinstall of a closed instance and telemetry versus drag/resize. These are proposed mitigations, not verified behavior.

### M5. A measurable resource envelope must select the renderer — Severity: Major

* **Reviewer lenses:** design, codebase-details, zoom-out.
* **Affected scope:** lines 39, 47, 55–59, 71 and 82.
* **Gap:** “Bounded” and “modest” do not set an acceptance rule. One renderer service can still retain many plugin processes. A functioning one-plugin demo does not establish low resource use.
* **Mitigation:** Engineering must set provisional global/per-owner limits for open panels, live owners, saved records, timer jobs, update bytes/rates, decoded images and restore time. Measure host plus browser/plugin children against the same baseline: incremental private memory, idle CPU/wakeups, update age, input latency and resources retained after close/failure. Include several distinct plugin owners, not only many panels from one owner.
* **Alternatives:** Prototype one Tauri candidate first. Compare a shared surface or native renderer only if a measured failure justifies another implementation; record native rendering/accessibility maintenance cost. No backend is selected by the hypothesis alone.
* **Disposition and gate:** Root proposes one/four/eight static panels, a CPU widget, timer output, oversized images, noisy-owner load and close/reopen cycles. These are workloads, not promised supported caps or measured results. Engineering owns thresholds; no owner questionnaire about RAM budgets is required.

### M6. Bound ingress and separate commands from replaceable values — Severity: Major

* **Reviewer lenses:** design, codebase-details; structural supports independent state authority.
* **Affected scope:** lines 29, 37–39 and 57–59.
* **Gap and evidence:** Coalescing at the renderer does not bound earlier queues, parsing or image decode. The current shared runtime drains plugin work before later input delivery (`plugins/mod.rs:985–1109`) and already has a 32-read socket bound at `1446–1453`; that count does not bound proposed payload cost.
* **Mitigation:** Enforce host-side per-owner and global byte/rate/queue/decode limits before expensive processing. Use latest-value mailboxes for replaceable samples. Keep close, lifecycle, configuration, timer control and button commands ordered and acknowledged; never coalesce two presses or erase cancellation. No render/audio/storage work may block the input loop or hold Core's mutex while waiting for windows.
* **Alternatives:** SDK throttling is helpful but cooperative; host enforcement is required. Independent bounded workers/queues avoid one helper process per plugin.
* **Disposition and gate:** Root adopts command/update separation. Overload acceptance must prove bounded queue memory, continued device input and timely close/cancel with actionable quota responses.

### M7. Versioned inventory needs upgrade, downgrade and partial-recovery rules — Severity: Major

* **Reviewer lenses:** readiness, structural, codebase-details; zoom-out supports storage reuse.
* **Affected scope:** lines 43–51 and 81–82.
* **Gap and evidence:** Widget-kind versions alone do not version host envelope, assets or rollback behavior. Existing `crates/deckweave-core/src/store.rs:103–119` rejects unsupported database/config formats; `143–229` provides guarded transactions and unknown-field preservation. Neither a format bump nor additive fields automatically guarantees safe downgrade.
* **Mitigation:** Specify an independently versioned host-service storage boundary, transactional revision handling and a rollback outcome that preserves profiles/assignments/preferences. Keep plugin payloads opaque until a compatible owner registers. Preserve malformed/unsupported records as per-panel unavailable entries; do not erase or block unrelated valid records. Make pending/failed edits and the committed revision observable.
* **Alternatives:** A bounded application-level configuration section may reuse existing conventions; a separate inventory isolates writes and old writers but adds recovery work. A breaking migration requires a verified backup and explicit gate. Avoid a generic storage framework or best-effort JSON file by default.
* **Disposition and gate:** Root chooses a separately versioned host-service database, leaving the existing configuration format and profiles untouched; older applications leave it alone. This preserves records on rollback without claiming an old application displays new panels. Exact layout/transactions remain design work. Test old configuration → upgrade → create/edit/close → older app unrelated edit → upgrade, plus malformed records and failed commits, using isolated databases rather than the live workspace.

### M8. Managed content needs total budgets and explicit reclamation — Severity: Major

* **Reviewer lenses:** design, structural; codebase-details supports missing-asset health.
* **Affected scope:** lines 43–47, 51, 53 and 57.
* **Gap:** A live-window cap and individual image limit do not bound retained closed/unavailable records, total image storage or write amplification. Close, removal, replacement and plugin removal have different retention meanings.
* **Mitigation:** Define separate live/saved/content/decoded-image/total-asset limits. Store managed image references rather than repeated image payloads inside configuration saves. Specify shared reference ownership, interrupted replacement, orphan reclamation and missing-asset recovery. Close preserves saved content; explicit removal must define when unreferenced assets are reclaimed. Keep high-frequency samples ephemeral.
* **Alternatives:** Managed copies best support restart restoration; external references require explicitly weaker failure semantics. A separate asset directory can work with either inventory layout if commit and cleanup are recoverable.
* **Gate:** Test asset replacement/removal/interrupted save and ensure one bad asset cannot prevent unrelated panels restoring. Capture remains a later visible command after this lifecycle is established.

### M9. Optional capabilities need mixed-version rollout, including bundled SDKs — Severity: Major

* **Reviewer lenses:** readiness, comprehension.
* **Affected scope:** lines 23, 29, 37, 47–51 and 81–82.
* **Gap and evidence:** The timer imports its bundled `./lib/sdk.mjs`; changing only `G:/APPS/DW/plugin-sdk` does not update that consumer. Transport send success is not capability discovery. Old hosts/plugins must retain their existing visibility behavior.
* **Mitigation:** Version sound, panels and background-job capabilities separately. Check support before exposing related configuration or registering jobs; unsupported calls must fail within a bound. Validate host/SDK/types/protocol/template/bundled consumer together. New-host/old-plugin and old-host/new-plugin combinations must keep ordinary timer controls usable and must not claim a hidden-page alarm works without service support.
* **Alternatives:** Host-first rollout followed by consumer enablement, or atomic host-plus-bundled-plugin delivery. Both need runtime negotiation and compatible saved-widget handling after rollback.
* **Gate:** Include mixed-version release tests and no invented Elgato visibility/service events. Do not emulate a durable service by retaining a retired visible context.

### M10. Typed outcomes and local diagnostics must explain actual delivery — Severity: Major

* **Reviewer lenses:** codebase-details, comprehension; design supports bounded retention.
* **Affected scope:** lines 29–33, 47–59 and 89.
* **Gap and evidence:** Current SDK booleans describe socket submission, not execution. Existing session errors are surfaced through visible slots; default SDK errors go to console while normal process stdout/stderr are discarded (`plugins/mod.rs:666–668`, `967–1034`; `sdk.mjs:54–74`). Durable services cannot depend on that diagnostic route.
* **Mitigation:** Expose typed asynchronous outcomes or request handles distinguishing unsupported, submitted, accepted, duplicate, rejected, cancelled, playback-started and failed. State retry/deduplication scope and retention. Keep preview distinct from timer output. Correlate a bounded host-owned diagnostic buffer and inventory status with package, service/run/panel, request, generation and revision IDs. Record registration/recovery, stale rejection, compatibility, quota, persistence and renderer outcomes without text/image payloads, tokens or sensitive paths. Measure schedule lateness separately from audio-start latency.
* **Alternatives:** Acknowledgment plus later status events, or a bounded asynchronous result. A local ring buffer/status snapshot is sufficient; distributed tracing and arbitrary console capture are unnecessary.
* **Disposition and gate:** Root proposes typed dispositions and a bounded local buffer. Playback started does not prove audible output. Tests must distinguish historical suppression from a broken path that never delivers new completions.

### M11. Make user-visible policies explicit — Severity: Major

* **Reviewer lenses:** comprehension, readiness, structural; design/codebase-details flag timer restart/sleep boundaries.
* **Affected scope:** lines 33, 37, 43–51, 63 and 89.
* **Questions in the independent reports:** What does repeat/copy of a creating action mean for durable instances? Does closing a timer panel only dismiss its presentation or also stop the job? Does an already-authorized host job sound after producer-process crash? Do existing timers acquire new notifications on upgrade? What is shown for expiry during app downtime versus sleep/resume?
* **Root-selected implementation defaults:** Repeated Show/Open targets the same saved instance, without toggling or duplication. Copying an action gives it a fresh creation identity; it neither opens/clones a panel nor claims another saved instance automatically. Explicit existing-instance linking can follow later. Editing action defaults never silently changes another saved panel. Close dismisses the view; Stop/Reset explicitly cancels the timer. A fully accepted, persisted host job survives producer-process crash and can deliver its prevalidated native sound/panel outputs; dead plugin callbacks never execute. Explicit owner disable/removal cancels its jobs. Upgrade adds no new audible or overlay side effects to existing timers. Restore historical elapsed timers as done silently; distinguish a late completion in the same running process after sleep. No wake-from-sleep or closed-app alarm promise is implied.
* **Alternatives and mitigation:** Toggle/share semantics, close-and-stop or cancel-on-crash were possible alternatives, but root selects the defaults above. These are reviewable implementation directions, not quotations or claims of separate owner approval. Periodic updates and reconnect never infer new open/callback intent. The raw reports preserve the original questions; this disposition avoids sending settled persistence or internal engineering choices back to the owner.

### N1. Reuse the existing CPU sampler for the proof — Severity: Nit

* **Reviewer lens:** zoom-out.
* **Affected scope:** lines 39, 47, 53, 82 and 89.
* **Reason and mitigation:** `plugins/dev.deckweave.monitor.sdPlugin/plugin.mjs` already shares a two-second sampler among visible contexts. Adapt its demand to visible actions or live service instances. Do not add a host telemetry framework or one sampler per panel merely for the example. The alternative is a new sampler only if a specific missing requirement justifies it.

### N2. Make recovery discoverable and stage optional customization — Severity: Nit

* **Reviewer lenses:** comprehension, zoom-out.
* **Affected scope:** lines 41, 49, 51 and 82–89.
* **Reason and mitigation:** Provide a plainly named, keyboard-reachable Panels entry with generic locate/reset-position/close controls even when a plugin is unavailable. Show the recovery route when enabling click-through. Preserve durable size, position and settings, while staging optional appearance/preset UI after core lifetime, focus and mouse behavior pass. A deliberate arrangement mode is an alternative to per-panel locate controls.

### N3. Add a rough effort range and intermediate exams — Severity: Nit

* **Reviewer lens:** design; zoom-out supports narrowing the first candidate.
* **Affected scope:** lines 76–89.
* **Reason and mitigation:** Before implementation authorization, estimate sound, timer authority, SDK/service lifetime, persistence, renderer and physical acceptance separately. Use timer authority and resource prototype as stop/reshape checkpoints, followed by combined lifetime/overload acceptance. The brainstorm need not invent a completed schedule or unsupported performance numbers.

## Disposition and next gate

No reviewer identified a concept-invalidating Blocker. Continue with a bounded design pass, carrying M1–M10 as explicit engineering requirements and M11's root-selected defaults. No further owner answer is required to make that design pass concrete. The [updated concepts brief](concepts_brief.md) preserves confirmed persistence and records the proposed safeguards and staged proof. Root's separate engineering dispositions are proposed resolutions; none of the findings is marked implemented, benchmarked or physically accepted.

Before broader feature work, prove one timer authority and one bounded renderer/persistence slice. Then validate SDK compatibility, owner lifecycle, saved close/restore, stale events and overload. Physical Windows acceptance must still cover audible output, focus, mouse pass-through, monitor removal and DPI. Source review or serialization tests cannot establish those outcomes.
