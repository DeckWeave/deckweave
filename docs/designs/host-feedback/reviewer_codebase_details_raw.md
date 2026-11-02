Review mode: BRAINSTORM. Lens: Observability & Operational SRE Debuggability, with source grounding.

The hypothesis identifies the central lifecycle hazard correctly and already includes useful diagnostic requirements at line 57. Its strongest decisions are stable panel ownership, plugin connection generations, panel revisions, explicit stale states, and separating timer completion from rendering. I found no implementation blocker that makes the proposed direction infeasible. I found four Major observability requirements to settle before implementation acceptance. These concern how to prove and diagnose the proposed behavior; absence of the proposed APIs is not implementation drift.

Read manifest

All paths below are absolute. Reads succeeded as described.

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/codebase_details.md` — complete persona read.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md` — complete standard read.
- `G:/APPS/DW/deckweave/docs/designs/host-feedback/working_hypothesis.md` — complete target read, including numbered lines 1–89. SHA256 verified as `14B2854797631DF75411411AAE2B32E3E50117C2D09502B812611DB6762023ED`.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs` — successful targeted searches and numbered reads of lifecycle/error handling at lines 900–1080, process setup at lines 650–676, protocol handling at lines 1180–1248 and 1370–1442.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/shell.rs` — successful targeted search and read of lines 1–145.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/key_lighting.rs` — successful targeted searches and read of lines 30–48.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs` — successful targeted search and read of lines 1–220.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/actions.rs` — successful targeted search for profile commands.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs` — successful targeted search for profile commands.
- `G:/APPS/DW/deckweave/crates/deckweave-platform/src/audio.rs` — complete source read.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/plugin.mjs` — complete source read.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/timer.mjs` — successful reads of timer restore, transition, serialization and render logic.
- `G:/APPS/DW/plugin-sdk/sdk.mjs` — successful targeted searches and numbered read of lines 40–240.
- `G:/APPS/DW/plugin-sdk/sdk.d.ts` — complete source read.
- `G:/APPS/DW/plugin-sdk/PROTOCOL.md` — complete source read.

Discovery checks established that the optional `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/contexts.rs` and `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/lifecycle.rs` do not exist. The plugin directory inventory contains `mod.rs`, `network.rs`, `catalog.rs` and `artwork.rs`. These missing optional paths are not findings.

No concepts brief, previous reviews, private databases, conversation history or memory were read. No files were changed; no builds, hardware operations or UI automation were performed. No external technical assertions required browsing.

Implementation-readiness findings

1. Major — Correlated lifecycle diagnostics need a defined collection path.

Dimension: Observability & Operational SRE Debuggability.

Affected target: “Desktop panel service,” lines 47–51; “Resource and failure limits,” lines 57–59; acceptance boundaries, line 89.

Operational risk: The proposed identities prevent certain unsafe operations, but the brief does not yet require a diagnosable record of those decisions. After a plugin reconnect, an operator must distinguish an intentionally rejected old-generation button event from an absent callback, unsupported widget version, failed registration or renderer failure. Aggregate panel counts and audio errors cannot answer that question.

Source grounding: The existing host stores a session error on process exit or socket failure in `plugins/mod.rs:967–991`, then publishes it through the session’s visible slots at `1031–1034`. The future service deliberately outlives those slots. The SDK defaults callback errors to `console.error` in `sdk.mjs:54–62`, while the host starts the normal plugin process with stdout and stderr discarded at `plugins/mod.rs:666–668`. The protocol handler also silently ignores unmatched events at `1387`. Therefore the new background service cannot assume that existing visible-action feedback or plugin console output provides an operational diagnostic trail.

Mitigation requirement: Require a bounded local structured diagnostic stream and a service status snapshot independent of visible device contexts. Correlate relevant operations using host-generated request/operation IDs, plugin package identity, non-secret connection generation, stable panel/job identity and revision. Record lifecycle transitions and stable reason codes for unsupported capability, missing owner, incompatible widget version, stale generation/revision, validation rejection, callback cancellation, render failure and successful recovery. Make the latest status reachable from the global inventory when no device action is visible.

Alternative: A full distributed tracing backend is unnecessary for this local feature. A bounded host-owned ring buffer plus status snapshot can satisfy correlation and triage. If plugin-side reporting is added, prefer a structured, rate-limited diagnostic extension rather than capturing arbitrary console output by default. Exclude content, payload dumps, connection tokens and sensitive file paths.

2. Major — Timer and sound diagnostics must distinguish delivery decisions from playback outcomes.

Dimension: Observability & Operational SRE Debuggability.

Affected target: “Shared sound service,” lines 29–33; “Resource and failure limits,” line 57; staging, line 81; acceptance boundaries, line 89.

Operational risk: “Audio playback failures and timing” does not distinguish a timer that never completed, a deliberately suppressed old completion, a deduplicated request, a rate-limited request, successful queuing, output-device loss or successful playback initiation. “The timer did not sound” would remain difficult to triage, and “no replay” could pass because delivery was accidentally disabled.

Source grounding: `plugin.mjs` currently transitions a running countdown to `done` inside `draw()` and persists there, while `willAppear` starts a 250 ms context interval. `timer.mjs:11–28` can also restore an elapsed running countdown directly into `done`; serialization at `47–52` records runtime status/value/start time, not a distinct completion delivery record. The hypothesis correctly proposes replacing rendering as the alarm trigger; that new delivery path also needs explicit diagnostic outcomes. `sdk.mjs:64–74` currently returns transport acceptance only, consistent with the warning in `PROTOCOL.md` that a true send result is not an acknowledgment of actual execution.

Mitigation requirement: Require one traceable completion identity and explicit dispositions such as newly completed, suppressed on restore, duplicate, cancelled, rate-limited, unsupported, playback started and playback failed. Distinguish preview from timer delivery. Measure schedule lateness separately from request-to-playback-start latency. Specify what a successful API response acknowledges; neither socket acceptance nor playback initiation proves that the user heard a sound.

Alternative: Use a request acknowledgment and a later completion/failure event, or a bounded asynchronous result if the selected backend supports that cleanly. Either is viable; an unqualified boolean “success” is insufficient. Do not retry a failure blindly without retaining the same logical completion identity.

3. Major — Persistence and restoration need observable health, including failed close persistence.

Dimension: Observability & Operational SRE Debuggability.

Affected target: “Desktop panel service,” lines 43–51; acceptance boundaries, line 89.

Operational risk: The confirmed product promise depends on saving content/settings and durable close intent. If persistence fails, a panel could appear closed but return after restart, or restored windows could silently omit incompatible records or missing images. “Unavailable” covers plugin loss, but the brief does not require distinct storage, restore and compatibility statuses.

Source grounding: The existing `Store::save` in `store.rs:143–167` uses a transaction, revision comparison and explicit errors; initialization also rejects unsupported or inconsistent stored state rather than silently resetting it. That is a useful existing durability boundary to evaluate for reuse. It does not yet establish where the proposed independent panel inventory will live or how its health will be reported.

Mitigation requirement: Require visible persistence failure status for user edits and explicit close/removal operations. Diagnostics should identify the last successfully committed inventory revision, pending/failed changes, restore attempts and dispositions: restored static content, waiting for owner, incompatible widget kind/version, missing asset, invalid geometry and monitor fallback. A failed record must not cause silent deletion or an unexplained reset of the entire inventory. Define a recoverable outcome when close intent cannot be committed.

Alternative: Reuse the existing transactional store with an independent panel collection, or introduce a separately versioned panel store. The brief need not select tables or serialization now, but it should require the same externally observable durability and failure behavior for either choice. State whether “closed” is shown immediately with a pending/error indicator or acknowledged only after saving succeeds.

4. Major — Resource measurements need acceptance workloads and limits that reveal shared-service interference.

Dimension: Observability & Operational SRE Debuggability.

Affected target: backend evaluation, line 39; “Resource and failure limits,” lines 57–59; alternatives, line 71; staging, line 82.

Operational risk: “Bounded,” “modest,” and “measure” identify the right concerns but provide no decision rule for accepting the backend. Counts of dropped/coalesced updates alone cannot reveal an oversized queue, a single noisy owner starving another widget, retained background contexts after closure, or degraded device input while rendering/audio work is active.

Source grounding: The current runtime and SDK explicitly manage per-context cleanup: `plugins/mod.rs:941–949` retires contexts that leave the active set; `sdk.mjs:178–183` aborts work and runs cleanup, and `209–211` disposes disappeared contexts. The proposed background lifetime intentionally bypasses that visibility-based cleanup boundary. Resource measurements must therefore count actual active services and reclamation, not infer cleanup from a page change.

Mitigation requirement: Before committing to the backend, define representative workloads and acceptance thresholds for idle notes, an updating CPU widget, timer delivery and the maximum supported panel/image load. Measure incremental process-tree memory, idle CPU/wakeups, update queue depth/age, update/render latency, active background contexts/jobs and retained resources after close or plugin failure. Include a noisy-owner scenario and verify that device input and unrelated timers remain responsive. Report rejection versus intentional coalescing separately.

Alternative: Use a small fixed initial panel cap and a documented Windows baseline if broad scalability work is premature. Compare candidate renderers on the same workload and hardware. A synthetic panel-count total without resource and responsiveness measurements is insufficient.

Product questions, separate from implementation blockers

- Target line 33 already identifies a real policy decision: what happens when a timer expires during restart or downtime? The initial direction—restore expired state silently—is coherent with line 89. Adopt it explicitly for the first delivery, and define how sleep/resume relates to that policy so a late live completion is not confused with a restored historical one. This does not reopen the confirmed requirement that panels survive profile changes.
- The supported first-delivery panel count, image-size budget and acceptable timing/resource thresholds remain product/engineering budget decisions. They should be chosen for the first benchmark and acceptance run rather than inferred from the phrase “low memory use.”
- Decide how the user sees a failed save or failed close-intent commit. This is a recovery interaction required by the persistence promise, not a request to weaken that promise.

No Blocker finding is supported by the inspected sources. These Major findings are additions to the proposed observability and acceptance contract, not claims that proposed host services should already exist.
