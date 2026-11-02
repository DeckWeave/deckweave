Review result: revise the concept before accepting an implementation design. The brief provides enough context, reuses the existing host/provider boundary, and correctly distinguishes action removal, device removal, hardware settings, and whole-keyboard lighting. I found five Major architecture decisions to resolve; none requires reopening owner readiness or the chosen stack.

1. **Major — Recovery must outlive the provider package being removed.**

   **Affected scope:** `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md:50–64, 103–104`; IO input ownership and extension lifecycle.

   **Evidence:** The brief promises to stop an instance and preserve configuration, and says provider-owned devices must stop before changing package files. It also proposes reusing IO ownership/restore rules. Those rules have an important limitation: `G:/APPS/io_type_84/modules/exclusive-depth.md:70–88` records temporary stock-assignment changes durably, retains the recovery journal until restoration succeeds, and explicitly states that stock capture has no hardware lease. Forced process termination or unplugging can leave keys captured until a later recovery operation.

   **Risk:** Stopping or uninstalling the provider while restoration is pending can remove the executable or its journal needed to recover the physical keyboard. Retaining profiles alone does not preserve recovery capability. This also affects provider update and crash recovery.

   **Mitigation requirement:** Establish a host-owned recovery obligation that survives disable, uninstall, failed update, and provider crash. Specify that ordinary removal first requests restoration and checks its result. For disconnected hardware or failed restoration, retain an explicit pending-recovery state and the recovery data outside the removable package directory, with a defined way to restore using a compatible provider. Do not present removal as complete restoration. This is a concept-stage responsibility and failure-policy decision; journal schema and exact process messages can wait.

2. **Major — Persistent exclusion and package publication need one crash-consistent lifecycle policy.**

   **Affected scope:** `concepts_brief.md:42–46, 64, 72–74, 100`; extension source selection and lifecycle persistence.

   **Evidence:** The brief correctly requires removed packages not to return through lower-priority roots, but leaves the source/ownership record open. Today `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/catalog.rs:41–58` collapses duplicate package IDs by retaining only the first scan-root occurrence. `plugins/mod.rs:47–54, 76–87` rescans managed, repository, and Elgato roots. `plugin_manager.rs:231–281` pauses, renames package directories, and reloads the host; this existing publication sequence does not persist removal intent or coordinate it with configuration changes.

   **Risk:** A crash between deleting/renaming a managed copy and recording its exclusion can activate another source on restart. The inverse ordering can leave an installed package unexpectedly blocked after a failed operation. Concurrent update, uninstall, and explicit reinstall can also disagree about which source and state should win. A correct confirmation screen does not resolve this durable-state problem.

   **Mitigation requirement:** Define lifecycle intent as a durable host record and state the restart reconciliation rule before extending the installer. Decide whether exclusion applies to a package identity across all sources, how explicit reinstall/reconnect clears it, and what failed removal means to the user. Preserve candidate-source inventory rather than relying on the already-collapsed runtime catalogue to explain ownership. A transactional state machine or small operation journal is appropriate; exact tables and implementation mechanics belong in the design.

3. **Major — The promised independent-instance removal behavior needs an identity fallback policy.**

   **Affected scope:** `concepts_brief.md:46, 50, 59, 72, 103`; discovery, exclusion, and restoration.

   **Evidence:** The brief recognizes that USB paths are not permanent identity and says IO identity must be checked. `G:/APPS/io_type_84/crates/io-platform/src/device.rs:47–71` selects one matching HID path and rejects multiple matching keyboards. The constructed identity at lines 84–90 contains model, VID/PID, firmware and protocol attributes; it does not establish a unique physical-instance identifier. The current DeckWeave device model stores only an optional opaque `hardware_key` (`G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs:245–258`).

   **Risk:** Refactoring to “open by identity” alone does not prove that identical keyboards can be distinguished across reconnects. If the tested hardware exposes no dependable unique identity, exclusion can suppress both devices or restored assignments can attach to the wrong one. The same ambiguity is especially serious for the recovery journal in finding 1.

   **Mitigation requirement:** Make identity verification a gate before claiming automatic independent restoration. Define behavior for ambiguous devices: explicit user pairing/rebinding, a bounded port-affinity fallback with its limitations disclosed, or restricted automatic reattachment. Never silently infer unique identity from model information. The exact identity encoding can wait; the product behavior when uniqueness cannot be established cannot.

4. **Major — Whole-keyboard compositions need a bounded first deliverable and an ownership rule.**

   **Affected scope:** `concepts_brief.md:54, 62, 80, 103`; lighting workspace, preset model, and action integration.

   **Evidence:** The brief includes whole-keyboard compositions and reusable presets in scope, while also correctly gating host rendering on verified volatile output. `G:/APPS/io_type_84/docs/PROTOCOL.md:112–126` says current RGB readback is ambiguous, the stock RAM RGB path is not enabled pending output/restoration verification, and the proposed panel protocol is not an existing manufacturer API. The brief explicitly leaves manual/action/automation precedence for reviewers to resolve.

   **Risk:** Without resolving those choices, “compositions” can mean a static per-key color arrangement, a stock firmware effect preset, or a continuously rendered host animation. These have different persistence, recovery, scheduling, and UI requirements. A workspace designed around the third interpretation could become the speculative layer/timeline engine that the brief warns against.

   **Mitigation requirement:** Define the first accepted lighting model: for example, named device-local presets for stock effects and static key groups, with host-rendered compositions exposed only after a separate capability gate. Choose one active lighting owner per output and an explicit transition rule when manual selection, an action-triggered preset, or automation takes control. State whether switching action profiles changes lighting automatically or only through an explicit association. Preview should distinguish desired composition from measured output. Detailed animation controls can wait; preset ownership and the meaning of “composition” are concept-stage decisions.

5. **Major — Shared provider isolation needs per-instance service guarantees, not only bounded queues.**

   **Affected scope:** `concepts_brief.md:59, 62, 102`; provider supervisor, transport scheduling, and resource economics.

   **Evidence:** The brief proposes one provider process serving multiple instances, bounded queues, and coalesced volatile slider updates. It does not yet distinguish control/recovery traffic from bulk display or animation traffic. The existing provider contract already requires ordered digital edges and latest-frame replacement (`G:/APPS/DW/deckweave/docs/PROVIDER-CONTRACT.md:12–18`). IO uses a serialized HID command path; its temporary RGB command accepts at most 56 payload bytes, four bytes per position (`G:/APPS/io_type_84/docs/PROTOCOL.md:54`). Consequently, a complete 84-key color update requires at least six such payloads; an illustrative 30 updates per second would already require at least 180 output payloads per second per keyboard, before readback and input traffic.

   **Risk:** A slow or noisy instance can monopolize shared IPC/encoding/USB scheduling, delay another instance’s input, or delay the close/restore request needed for removal. A bounded FIFO limits memory while still allowing long latency and loss of the wrong message class. One process also means process failure affects all its sessions, even if ordinary close operations preserve siblings.

   **Mitigation requirement:** Require per-instance queue ownership and fairness, cancellation/close/recovery priority, explicit overflow behavior by message class, and bounded supervisor restart/backoff. Digital edges must not be silently coalesced; overload should fail or resynchronize the affected session. Establish measured throughput and input-latency budgets before selecting an animation rate, and degrade or stop rendering when they cannot be met. Exact queue sizes and scheduler code can wait for implementation design. Cloud QPS, database scaling, and LLM token budgets are not meaningful cost drivers for this local feature.

Implementation details that can wait include the wire encoding, concrete persistence tables, generator directory names, custom editor API shape, numeric queue limits, and final packet timing. The five decisions above affect ownership, user-visible guarantees, recovery, and the feasible first deliverable, so they should be settled in the concepts brief or carried into the PRD as explicit gates.

Read manifest:

- **Fully read:** `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/design_critic.md`.
- **Fully read:** `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md`, lines 1–110.
- **Fully read:** `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/zoom-out-standard.md`, lines 1–60.
- **Fully read:** `G:/APPS/DW/deckweave/docs/PROVIDER-CONTRACT.md`, lines 1–20.
- **Fully read:** `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugin_manager.rs`, lines 1–994; the initially truncated extraction section was reread separately.
- **Fully read:** `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs`, lines 1–861, in complete chunks.
- **Additional fully read:** `G:/APPS/io_type_84/modules/exclusive-depth.md`, lines 1–143.
- **Additional targeted reads/searches:** IO `docs/PROTOCOL.md`, `docs/STATUS.md`, and `crates/io-platform/src/device.rs`; DeckWeave `plugins/mod.rs` and `plugins/catalog.rs`. These additional files were not all read in full.

All required reads were completed. Review was read-only; no files, builds, Git state, hardware, applications, history, or memory were changed or operated.
