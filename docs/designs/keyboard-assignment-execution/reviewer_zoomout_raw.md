Completed: read-only BRAINSTORM review against the Zoom-Out Standard and Skeptical Architect persona. The hypothesis has sufficient context and correctly identifies the root problem: the editor projects host bindings as if they described the keyboard’s stored behavior. “Onboard first, host when needed” fits DeckWeave’s shared runtime/device-adapter boundary. The proposal merits continued clarification, with the following scope and reuse risks.

1. **[Major] Dimension: Contrarian / YAGNI & Code Minimization — separate the immediate presentation correction from the complete execution catalogue.**

   Affected target: `working_hypothesis.md:17–21, 29–35, 78`.

   The central complaint can be materially improved before integrating mouse, Consumer, combinations and function writers. Current `DeviceEditor.svelte:237–245` derives its visible Fn assignments solely from `profile.fn_bindings`; `:849–855` consequently displays host labels and assignment indicators. The native adapter already projects observed raw records through `device-io-type-84/src/model.rs:127–141` and supplies onboard labels through `:143–147`.

   Alternative/reuse: make the smallest useful slice an observed Main/Fn view with explicit host overlays and truthful readiness. Retain unsupported forms as readable existing assignments. Treat expansion of the writable catalogue as separate, evidence-gated work. The hypothesis currently describes a broad destination without distinguishing this independently useful first result.

2. **[Major] Dimension: Contrarian / YAGNI & Code Minimization — stable physical ownership requires changing the existing ownership model, not merely hiding the F-code selector.**

   Affected target: `working_hypothesis.md:44–50, 68`.

   The existing representation puts `trigger` inside `FnBinding`, which belongs to `Profile` (`deckweave-core/src/model.rs:240–257`). `fn_actions.rs:3–23` allocates against current device profiles, while `DeviceEditor.svelte:261–267` separately chooses from those same active-profile bindings. Neither is a durable inventory of physical key/layer ownership. Deleting a host binding currently removes the object containing its alias.

   Alternative/reuse: retain existing profile action bindings and runtime execution, but establish one authoritative physical-source ownership representation outside profiles. Eliminate competing UI allocation logic. Reserve once per physical source, even when many profiles reference it. This is necessary complexity for the proposed semantics; a general-purpose resource allocator or second action runtime is unnecessary.

3. **[Major] Dimension: Contrarian / YAGNI & Code Minimization — durable ownership is constrained by session-only device identity.**

   Affected target: `working_hypothesis.md:44, 50, 65, 68`.

   The native descriptor explicitly reports `identity.confidence = "session"` and `persistentId = null` (`device-io-type-84/src/model.rs:99`). `io_type_84/docs/PROTOCOL.md`, “Подключение,” also warns that an empty serial cannot establish persistent identity. A ledger keyed only by an opaque previous instance could incorrectly attribute stored aliases or original records to another identical keyboard.

   Alternative/reuse: reuse existing session generation and recovery confirmation concepts. Bound the initial experience to a positively paired session and require fresh identity confirmation when persistent continuity is unavailable. Do not solve this by automatic settings replay on reconnect. The hypothesis should state what remains reserved and what becomes unavailable when the physical source cannot be confidently identified.

4. **[Major] Dimension: Contrarian / YAGNI & Code Minimization — evaluate Raw Input as a bounded feasibility dependency before expanding interaction semantics.**

   Affected target: `working_hypothesis.md:52–56`.

   The proposal appropriately marks interface pairing and press/release support as unproved. Actual platform code is `deckweave-platform/src/shortcuts.rs`, using `RegisterHotKey`; no Raw Input implementation surfaced in the searched core/platform sources. `keyboard_shortcuts.rs` currently consumes alias numbers and dispatches `activate_fn`; `fn_actions.rs:76–90` explicitly rejects Key Logic, hold and release.

   Alternative/reuse: first determine whether the vendor interface can be paired reliably with the intended keyboard input source. Reuse the existing runtime after that boundary, preserving its source/release semantics. Keep one-shot host actions as the smallest execution target. Hold/release and richer Key Logic should follow demonstrated source events rather than become implicit requirements of the first assignment editor. Supporting two event backends as user-selectable infrastructure would add maintenance without yet establishing a need.

5. **[Major] Dimension: Contrarian / YAGNI & Code Minimization — existing recovery is reusable infrastructure, but does not establish the stronger same-key external-edit guarantee.**

   Affected target: `working_hypothesis.md:48–50`.

   The target promises “Never restore over a later external edit.” Native `device-io-type-84/src/recovery.rs:391–408` clones current state, then copies original records for slots changed by the recorded operation. In this inspected merge function, copying is not conditioned on that current record still equalling the recorded target. Preserving unrelated records therefore must not be confused with preserving a later edit to the same record.

   Alternative/reuse: build on the existing durable snapshots, preview, uncertain-write and recovery machinery. Define restoration as a conditional operation against freshly observed state, with a conflict outcome when the owned record changed externally. This includes relinquishing an alias: deleting the app action is not proof that the physical mapping stopped emitting it. The report does not assert that such conditional ownership restoration is implemented.

6. **[Major] Dimension: Contrarian / YAGNI & Code Minimization — distinguish logical single-assignment edits from physical block writes.**

   Affected target: `working_hypothesis.md:21`.

   “Writes only that assignment” can imply a transport capability that the source does not establish. `io_type_84/docs/PROTOCOL.md:92–94` describes complete changed-block SET, backup and byte readback, and explicitly leaves single-position writes for investigation. `io-platform/src/changes.rs:516–547` chunks complete changed blocks and compares their readback.

   Alternative/reuse: say the operation changes only the selected assignment’s intended contents while preserving other raw records. Reuse the established full-block change path. Do not introduce partial-write optimization to satisfy wording, and do not present byte equality as evidence that every action executes physically or survives power loss.

7. **[Major] Dimension: Contrarian / YAGNI & Code Minimization — catalogue integration crosses an existing host validation contract.**

   Affected target: `working_hypothesis.md:19, 31–35, 69`.

   Keeping codecs in the device driver is sound, but it is not sufficient merely to expose additional native codecs. `deckweave-core/src/provider_settings.rs:521–565` recognizes `bindingFormat = "keyboard-usage"` and validates a single `usage` field. The native writer likewise accepts only bounded ordinary keyboard usages and refuses paired mappings (`device-io-type-84/src/model.rs:159–194`). The prototype’s `changes.rs:41–79` contains a broader typed allowlist.

   Alternative/reuse: extend the existing declared keyboard capability contract narrowly for supported forms. Reuse validated prototype encodings rather than importing its entire editor or advanced-action model. Keep macros, DKS and paired actions outside this first catalogue: they alter separate tables or multiple records and would invalidate the apparent simplicity of a single-key operation.

8. **[Nit] Dimension: Contrarian / YAGNI & Code Minimization — clarify capacity and inactive-profile semantics without inflating allocation.**

   Affected target: `working_hypothesis.md:50, 56`.

   Counting all inactive profile bindings independently would unnecessarily consume the twelve aliases, contradicting ownership by physical source. Conversely, a profile with no host binding cannot restore a previous onboard action automatically without violating the stable common mapping proposal.

   Alternative/reuse: describe capacity as twelve reserved physical key/layer sources within the chosen event-source conflict domain. State the behavior of a reserved source in profiles where it has no action. Continue deferring profile-specific hardware rewriting; it would substantially enlarge recovery and lifecycle complexity.

Proposed mitigations, distinct from findings: prioritize observed onboard presentation; make physical alias ownership authoritative outside profiles; reuse provider sessions, snapshots, readback and runtime dispatch; gate event routing on proven device pairing; add conditional restoration conflict handling; extend action capabilities narrowly. None of these safeguards was implemented during this review.

Unresolved user decisions: acceptable behavior when a host-owned source has no action in the selected profile; whether “remove action” retains the physical alias by default and exposes restoration separately; acceptable session re-identification UX; whether the first delivered host slice must identify the originating keyboard or may explicitly retain the existing global one-shot limitation; which additional onboard action categories deserve first physical acceptance.

Read-file manifest:

- **Full:** `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/zoom_out.md`; corresponding `resources/document_standards/zoom-out-standard.md`; `G:/APPS/DW/deckweave/docs/designs/keyboard-assignment-execution/working_hypothesis.md`; `G:/APPS/DW/deckweave/README.md`; `modules/editor.md`; `modules/runtime.md`; `apps/desktop/src/FnBindingSetup.svelte`; `apps/desktop/src-tauri/src/keyboard_shortcuts.rs`; `crates/deckweave-core/src/fn_actions.rs`; `crates/deckweave-platform/src/shortcuts.rs`.
- **Targeted:** `DeviceEditor.svelte` assignment selection, allocation, rendering and inspector sections; `deckweave-core/src/model.rs:240–290`; `provider_settings.rs:521–590`; `G:/APPS/DW/device-io-type-84/src/model.rs` descriptor, observed mappings/labels and write validation; its `recovery.rs` search results and `:391–414`; `G:/APPS/io_type_84/docs/PROTOCOL.md` connection, assignment format, write requirements, knowledge limits and codec sections; `src/features/editor/model.ts:176–222`; `crates/io-platform/src/changes.rs` validation and apply/readback sections.
- **Search-only:** actual core/platform sources for Raw Input and Key Logic; native provider recovery/identity references; IO/DeckWeave sources for onboard capabilities and labels. Missing guessed `input.rs`/`key_logic.rs` paths were not read.

Status: completed. No edits, builds, device operations, shortcut registrations, prior-chat retrieval or user-data mutations performed.
