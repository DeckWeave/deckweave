Maintenance and lifecycle coupling review — BRAINSTORM

The hypothesis establishes the right primary boundary: desktop panels and live widgets survive page, folder and profile changes because their owners have a lifetime separate from visible device slots. This confirmed behavior should remain fixed. I found no Blocker to continuing concept development. The Major findings below are architectural decisions to settle before implementation; they are not claims that the proposed APIs should already exist.

Read manifest

Successful reads:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/structural.md` — full persona.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md` — full standard.
- `G:/APPS/DW/deckweave/docs/designs/host-feedback/working_hypothesis.md` — full hypothesis, lines 1–89. SHA256 verified as `14B2854797631DF75411411AAE2B32E3E50117C2D09502B812611DB6762023ED`.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs` — targeted searches and line reads covering package admission, session retirement, settings persistence and the existing timer integration.
- `G:/APPS/DW/plugin-sdk/sdk.mjs` — targeted search and lines 90–220 covering action context state, timers, disposal and event dispatch.
- `G:/APPS/DW/plugin-sdk/sdk.d.ts` — targeted search of the current notification extension.
- `G:/APPS/DW/plugin-sdk/PROTOCOL.md` — targeted search of process ownership, context identity and extension behavior.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/plugin.mjs` — full file, lines 1–120.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/timer.mjs` — lines 1–65.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/key_lighting.rs` — targeted search and lines 30–135, 175–220 and 350–450.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs` — lines 1–180 and targeted persistence search.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs` — targeted search of stored identities and configuration types.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs` — targeted search and lines 1770–1830.

The suggested `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/contexts.rs` and `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/lifecycle.rs` do not exist at those paths. I did not treat their absence as a defect. I did not read the concepts brief, prior reviews, other reviewer reports, private databases or conversation history. No files were changed, builds run or hardware accessed.

Implementation and architecture findings

1. Major — Long-Term Tech Debt & Maintainability: establish one authoritative countdown lifecycle across device rendering, host scheduling and panel widgets.

Affected target: “Shared sound service,” lines 21 and 33; “Desktop panel service,” line 47; staging, line 81.

The hypothesis correctly requires a host-owned schedule and completion identity independent of rendering. However, the current host already contains a related scheduler: `key_lighting.rs` defines `TimerPlanner` at lines 49–128, arms it from accepted timer settings at lines 380–439, and consumes due jobs at lines 197–207. The plugin separately holds timer state in its visible context and detects completion in `draw()` at plugin lines 26–30.

The existing lighting planner is deliberately coupled to the timer package, countdown action, lighting configuration and active profile. Its validation calls `current_plugin_profile()` at key-lighting lines 363–378; the core implementation at `lib.rs` lines 1799–1813 checks the current device profile. That makes this planner unsuitable for direct reuse as the independent panel lifetime without changing its ownership model.

Adding a second scheduler for sound and a third completion transition for a panel would create several authorities for pause, reset, expiry, reconnection and replay suppression. A timer could then appear done in one surface while another still owns a pending notification.

Maintainability requirement: identify one authoritative timer run and completion event. Device views, persistent widgets, sound and lighting should consume that state or event through explicit adapters.

Alternatives and mitigation: either extract a general host scheduler from the lighting planner, preserving lighting as a delivery adapter, or introduce a general scheduler and explicitly migrate the lighting integration. Keeping separate planners is possible only with one owning the run state and the others acting as consumers. The brief should record which direction is intended and require a shared completion identity across enabled delivery channels.

2. Major — Long-Term Tech Debt & Maintainability: make durable services a separate reason to start and retain a plugin process.

Affected target: “Desktop panel service,” lines 45–51.

A background context registry alone is insufficient if package admission still follows action assignments. Current code derives packages needed for startup from active slots at `plugins/mod.rs` lines 795–798 and starts them at lines 863–874. It closes sessions with no remaining assigned package at lines 847–861. Saved widget instances therefore need to participate in package startup and retention independently of device assignments.

The target already states this architectural direction. The remaining coupling risk is treating service contexts as a second collection inside the existing slot loop while leaving process admission and shutdown decisions unchanged. After a restart with the source profile hidden, the host must know that a compatible live widget needs its plugin before that plugin can reconnect and register its widget kind.

Maintainability requirement: explicitly separate process demand from visible action demand. Active live services and timer jobs should be durable, bounded reasons for package admission; saved static text and images should not create that demand. Disable, removal, upgrade pause and shutdown must override admission and retire service callbacks consistently.

Alternatives and mitigation: use a unified package supervisor that receives demand from action slots and services, or a service supervisor coordinated with the existing process owner. Avoid independent process launchers for the same package. Require acceptance coverage for restarting with no visible source action and for removing the last device assignment while retaining a saved live widget.

3. Major — Long-Term Tech Debt & Maintainability: distinguish content updates from authority to reopen or overwrite user configuration.

Affected target: “Desktop panel service,” lines 37 and 41–49; acceptance boundary, line 89.

The proposed `upsert` combines creation and update. The hypothesis also gives users control over close, position, size and settings while plugins continue sending updates. A late CPU sample or reconnect snapshot could therefore recreate a panel after the user closes it, or restore a plugin’s old geometry after a drag.

Connection generation and panel revision are useful protections, but their meaning is not yet established. A generation rejects an old connection; it does not resolve competing updates from a valid plugin connection and the global panel inventory.

Maintainability requirement: user close state and host-owned presentation settings must have defined authority independent of producer content updates. Closing must invalidate pending updates that could reopen the instance, and ordinary live-data updates must not carry permission to reopen it.

Alternatives and mitigation: separate explicit create/open commands from update-only commands, or require an opening epoch issued by an explicit open operation. Define whether revision checks apply to the whole record or independently to content and settings. A whole-record revision is simpler but can cause unnecessary conflicts between frequent telemetry and a user moving the panel. Whatever design is chosen, require tests for close racing with an update, reconnect after close, and telemetry racing with drag/resize completion.

4. Major — Long-Term Tech Debt & Maintainability: extend compatibility rules from registered widget kinds to the full durable panel record.

Affected target: “Desktop panel service,” lines 43–51.

Versioned widget kinds and retaining unsupported content are good starting points. The saved record will also contain host-owned fields, plugin-owned configuration and managed image references. These have different migration and deletion lifetimes. Without an explicit boundary, a plugin upgrade can require host-specific interpretation of old configuration, while removing a panel can accidentally delete a shared image or leave permanent orphan files.

Current configuration storage already uses version checks, guarded writes and atomic migrations in `store.rs`, including lines 19–107 and 143–168. The concept should choose how the new inventory relates to that established persistence boundary rather than accidentally creating an unrelated best-effort JSON store.

Maintainability requirement: give the host panel envelope its own compatibility policy, keep plugin payloads opaque until a compatible owner registers, and define a recoverable commit boundary for open/closed intent and managed assets. A bad or unsupported instance must not prevent restoration of unrelated panels.

Alternatives and mitigation: store the inventory within the existing versioned configuration, or use a dedicated versioned inventory with a documented transaction and recovery boundary. For images, choose managed copies or explicitly weaker external references; managed copies best match the promised restoration behavior. Define reference ownership and reclamation for close versus removal. Detailed schemas can wait for design, but these ownership and recovery principles should enter the brief now.

Product questions, separate from implementation blockers

- Major clarification — lines 43 and 47: does closing a timer panel dismiss only its presentation, or also cancel its timer job? Independent panels and timer jobs suggest separate lifetimes, but users and plugin authors need an explicit rule. A reasonable default is that dismissal closes the view while pause/reset/cancel are separate controls.
- Major clarification — lines 33, 47, 51 and 59: when a plugin crashes while a host-owned countdown is armed, should the host still deliver the already-authorized completion sound? Marking the widget unavailable is clear; the surviving job’s delivery policy is not. Choose explicitly between host continuation of an accepted job and cancellation on owner loss. Profile changes must remain irrelevant to this decision.

These questions do not reopen the confirmed requirement that profiles have no effect on desktop panels. They define the remaining relationships among a panel, its producer process and an independently scheduled job.

The alternatives section already rejects several costly coupling patterns. Its next useful addition is a short evaluation of timer authority, package admission and persistence boundaries. Those are the areas most likely to become maintenance burdens as additional widget kinds and notification channels arrive.
