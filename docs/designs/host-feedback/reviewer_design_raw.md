The proposed direction is viable. Independent panel instances, saved widget ownership, explicit unavailable states, and host scheduling address the central lifetime problem. I found no Blocker that invalidates the concept. Four Major architecture risks need explicit safeguards in the next brief; none requires reversing the confirmed behavior that panels survive profile changes and restart.

This is a BRAINSTORM assessment of a hypothesis, not an implementation-drift review. Proposed APIs being absent is expected.

Read manifest, all successful:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/design_critic.md` — full read, lines 1–63.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md` — full read, lines 1–36.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/heilmeier-standard.md` — full read, lines 1–30.
- `G:/APPS/DW/deckweave/docs/designs/host-feedback/working_hypothesis.md` — full read, lines 1–89. SHA-256 verified as `14B2854797631DF75411411AAE2B32E3E50117C2D09502B812611DB6762023ED`.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs` — targeted text searches and reads of lines 45–80, 690–855, 930–1075, 1080–1125, and 1440–1468.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs` — targeted text search and read of lines 128–168.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/plugin.mjs` — targeted text search and full read of lines 1–120.
- `G:/APPS/DW/plugin-sdk/sdk.mjs` — targeted text search and read of lines 145–217.

No files were written. No builds, hardware operations, UI automation, private databases, conversation retrieval, or prior reviews were used. No external technical assertions required browsing.

Implementation risks and safeguards:

1. **Major — An unquantified resource envelope cannot select or validate the renderer.**

   **Dimension: Cost, Quota & Token Economics — resident memory, CPU, process count, update rate and startup load.**

   **Target:** `G:/APPS/DW/deckweave/docs/designs/host-feedback/working_hypothesis.md`, “Desktop panel service,” lines 39 and 47; “Resource and failure limits,” lines 55–59; staging, line 82.

   The hypothesis correctly avoids declaring either webview arrangement cheap, but “bounded,” “modest” and “actual resource requirement” currently have no measurable meanings. Persistent live panels also retain plugin work beyond device visibility. One shared panel service does not eliminate the resident cost of the plugins that provide those widgets.

   The relevant capacity variables are panel count, distinct live plugin owners, timer-job count, update frequency, payload size, image decode size, and renderer surfaces. A useful conceptual model is baseline host cost plus renderer cost plus the resident cost of each required plugin process. The next brief needs to measure those components separately; otherwise a backend can appear economical in a one-plugin demonstration while becoming expensive with several different widget owners.

   **Mitigation requirement:** Establish an initial supported envelope before backend commitment: global and per-plugin live-panel limits; saved-panel and timer-job limits; update and payload limits; image decode limits; incremental idle and loaded memory/CPU targets; and acceptable restoration time. Benchmark at least an idle note, multiple live widgets from one owner, live widgets from several owners, and restart at the supported maximum. Include the transient-notification burst alongside persistent panels.

   Limits should reject additional work with an actionable response, while preserving existing panels. They must not silently close panels during profile switching.

   **Alternatives:** Compare per-panel windows, a shared rendering surface, and any native candidate under the same workload. A single implementation prototype is reasonable initially, but passing its budget—not merely functioning—should authorize commitment. Measure actual resource use; the current document does not justify choosing one backend yet.

2. **Major — Timer scheduling and plugin state need one authority and a bounded delivery contract.**

   **Dimension: Cost, Quota & Token Economics — scheduler state, background execution, deduplication storage and recovery work.**

   **Target:** Same hypothesis, lines 29, 33, 47, 59 and 81.

   The document makes the right distinction between drawing and completion, and explicitly requires host scheduling for hidden pages. It does not yet establish whether the host schedule is the authoritative timer or a wakeup that asks the plugin to perform a transition. That distinction determines whether timer completion survives a plugin crash, how reset cancels an old deadline, and how a visible action and background service avoid completing the same run twice.

   Existing timer completion occurs inside `draw()` at `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/plugin.mjs:26–30`; visible contexts draw every 250 ms at lines 53–59. The SDK disposes context timers on disappearance at `G:/APPS/DW/plugin-sdk/sdk.mjs:178–183` and `:209–211`. The hypothesis recognizes this limitation; the remaining issue is the new authority boundary, not unexpected drift.

   A host job and a plugin timer that each own completion create two sources of truth. Keeping every timer plugin alive instead avoids that split but changes the feature into a generic background-execution facility, with larger lifecycle and resource costs.

   **Mitigation requirement:** Prefer a narrowly typed host job contract for the first timer integration. Give each timer run a stable job identity and generation, an authoritative deadline/state, explicit replacement/cancellation, and a defined completion acknowledgement. A reset must invalidate the previous generation before its deadline can deliver. Visible displays and panel widgets should observe that job rather than independently decide delivery.

   Bound active jobs and retained completion/deduplication records. Define deduplication scope, retention and eviction together with the guarantee being offered. Specify whether plugin loss prevents plugin-specific rendering only, or also prevents completion delivery. Preserve the documented restart behavior: restoring expired state must not replay an old sound.

   **Alternatives:** A host-owned deadline plus completion record is smaller and more predictable than arbitrary plugin background callbacks. A plugin-owned scheduler is possible, but then the brief must explicitly accept continued plugin residency and explain crash recovery and cancellation authority. This is an engineering decision required before timer implementation; it does not reopen the owner’s panel-lifetime decision.

3. **Major — Coalescing must protect the shared host before expensive processing.**

   **Dimension: Cost, Quota & Token Economics — ingress throughput, queue memory and scheduling fairness.**

   **Target:** Same hypothesis, lines 29, 37, 39 and 57–59.

   Merging repeated panel updates is necessary, but it only bounds renderer work if messages are first allowed to accumulate, parse or decode. A fast widget—or several individually compliant widgets—can still consume the shared host loop or inflate queues before the renderer sees the coalesced state.

   The current runtime processes plugin sessions in one loop and handles incoming messages before later device-input delivery: `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs:985–1029` and `:1037–1109`. Its socket drain already has a 32-read bound at lines 1446–1453; this is an existing safeguard worth preserving. A count of messages alone, however, does not define the processing budget for the proposed panel payloads, images or persistence operations.

   **Mitigation requirement:** Define global and per-owner limits for message bytes, accepted update rate, queued work and expensive decode operations. Maintain a latest-value mailbox for replaceable widget updates before rendering or persistence. Keep commands such as close, cancellation and lifecycle retirement separate from replaceable data updates, so coalescing does not erase them. Renderer and audio work must not hold up the plugin/input loop or core-state locks.

   Add overload acceptance that drives panel updates above the supported rate and verifies bounded queue memory, continued device input, timely close/cancellation, and useful quota feedback. The document already requires dropped/coalesced-update diagnostics; the limits should make those counters explainable.

   **Alternatives:** Enforcing SDK-side throttling alone is cooperative and insufficient for third-party plugins. Host enforcement plus optional SDK throttling provides predictable economics without requiring a separate helper process for each plugin.

4. **Major — Persistent inventory and managed images need a total storage budget and reclamation policy.**

   **Dimension: Cost, Quota & Token Economics — disk growth, configuration write amplification and restore cost.**

   **Target:** Same hypothesis, lines 43–47, 51, 53 and 57.

   The text bounds panels and image sizes and avoids saving every CPU sample. It does not distinguish a live-window cap from the number of retained closed or unavailable instances, nor set a total managed-image budget. Individual limits do not prevent indefinite growth through repeated creation, replacement, close, removal and reinstallation. Persistent unavailable instances can legitimately remain after a plugin disappears, so that storage cannot rely on plugin cleanup.

   The present core store serializes the complete typed configuration, preserves unknown fields, and updates a single configuration record on save: `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs:143–165`. Putting substantial panel content or frequent panel revisions directly into that path could amplify writes and configuration-copy work. The hypothesis does not yet choose that storage layout; this is a design risk to resolve rather than a claim that the implementation already does it.

   **Mitigation requirement:** Define separate caps for live panels, retained saved instances, text/configuration bytes, decoded image dimensions, and total managed-image storage. Specify how replacement and explicit removal reclaim unreferenced assets, how shared references are counted, and how interrupted updates leave either the old or new valid state. Preserve close intent durably before acknowledging that it will survive restart.

   Keep high-frequency widget values ephemeral unless there is a stated restoration reason. Store images as managed assets with references rather than placing image payloads inside every configuration save. Decide whether panel inventory has a separate persistence boundary or shares the existing configuration store, using expected write size/frequency and recovery complexity as the comparison.

   **Alternatives:** A separate inventory plus managed asset directory offers isolated write volume but adds recovery and garbage-collection work. A bounded inventory in the current store may be adequate for small text/configuration records if measurement confirms it. Neither option requires a new database service or cloud dependency.

Additional planning observation:

5. **Nit — The next brief needs an effort estimate and staged resource exams.**

   **Dimension: Cost, Quota & Token Economics — engineering effort and verification cost.**

   **Target:** Same hypothesis, lines 76–89.

   The staging and functional acceptance are useful, but the Heilmeier cost/timeline questions remain open. This is acceptable at the hypothesis stage and should not be turned into a demand for a completed implementation plan. Before implementation authorization, provide a rough effort range split among sound, authoritative timer scheduling, SDK/service lifetime, persistence, renderer prototype, and physical Windows acceptance. Identify which result can stop or reshape later work.

   A useful intermediate exam is a renderer/resource prototype plus timer authority proof before broader widget support. The final exam should combine the existing lifetime acceptance with the chosen maximum workload and overload behavior. No external QPS, cloud-storage or LLM-token budget is implicated by the proposed offline scope; the relevant economics are local resources and engineering time.

Product questions, separate from implementation blockers:

- The only product input needed for resource sizing is the expected initial workload and tolerance for resource use: how many simultaneous persistent panels/live widgets should feel routine, and what visible update delay is acceptable. Engineering can propose conservative defaults and show measured trade-offs.
- The confirmed persistence, profile independence, live-widget continuity, explicit close behavior, and no old-sound replay are settled constraints. They should not be reopened to make resource management easier.
- Renderer selection, timer authority, admission control, deduplication retention and asset reclamation are engineering design decisions. They do not need another product decision unless a measured limitation forces a visible change to the promised behavior.
