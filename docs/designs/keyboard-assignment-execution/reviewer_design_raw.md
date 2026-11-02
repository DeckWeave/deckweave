Completed: read-only BRAINSTORM architecture review. The hypothesis is suitable for continued clarification, but should not yet authorize automatic hardware assignment. Its separation of keyboard execution from host execution is coherent. The main unresolved risks are durable alias ownership, dependent mappings, input lifecycle, and the scope of the first release.

All safeguards below are proposals. This review implemented nothing, changed no files or user data, ran no builds, registered no hotkeys, and touched no devices.

## Findings

1. **Major — Architecture / smallest useful scope: two product outcomes are bundled without explicit stage gates.**

   **Target:** `working_hypothesis.md:17–21, 29–35, 44–56`.

   The concept combines truthful read-only Main/Fn presentation, new onboard writers for several action classes, automatic host-trigger programming, persistent allocation, profile-independent ownership, and a new Raw Input route. These have different evidence and recovery requirements. The existing editor confirms that the presentation problem is independently solvable: `DeviceEditor.svelte:237–245` displays profile host bindings, while `FnBindingSetup.svelte:31–35` separately retrieves an observed Fn mapping.

   **Proposed mitigation:** Define a smallest useful first outcome and make later outcomes conditional. A reasonable sequence is truthful observed assignments; one supported automatic host assignment with recovery; broader onboard action categories after separate physical acceptance. Do not treat all categories in the action list as one delivery commitment.

2. **Major — Architecture / recovery: “writes only that assignment” does not cover dependent records or the physical write unit.**

   **Target:** `working_hypothesis.md:17, 21, 35, 48`.

   There are two separate limitations. The prototype writes a complete changed layer block, rather than an isolated four-byte record (`changes.rs:507–540`; `PROTOCOL.md:92–94`). It also clears every matching SOCD/RS partner when replacing one member (`changes.rs:197–207`; `model.ts:32–36`). Therefore, preserving all other keys is not automatically true merely because macros and advanced pairs are outside the new writer’s scope.

   **Proposed mitigation:** Distinguish the logical assignment change from the transport write unit. For the first slice, reject replacement of a mapping with known dependencies unless the UI can explain and recover the complete affected set. Preserve unknown bytes in the block, and verify the intended block diff. The concepts brief needs this boundary; it does not yet need the final transport implementation.

3. **Major — Architecture / ownership: the proposed durable owner lacks an identity and recovery boundary.**

   **Target:** `working_hypothesis.md:44, 48–50, 65, 68`.

   Owning an alias by physical keyboard, control and layer is preferable to owning it by profile, but the document does not say how that physical keyboard remains identifiable after restart, USB port changes, database loss, reinstall, or import. The prototype explicitly reports that a blank USB serial cannot provide persistent identity alone (`PROTOCOL.md:7`). The proposed rule against restoring over external edits also needs a baseline: which exact installed record must still match before restoration is allowed?

   Existing prototype backups durably store complete before/target snapshots (`changes.rs:432–474`), but that is evidence of a backup mechanism, not evidence that DeckWeave has a durable alias-ownership journal or cross-session recovery policy.

   **Proposed mitigation:** Make ambiguous identity a stop condition for automatic writes and restoration. Preserve an operation’s owner, exact original record, intended replacement and outcome across crashes. After an uncertain operation, reconcile observed state before enabling the binding or offering restoration. Explain what a fresh installation can safely recognize and what requires user adoption. These are required policies before automatic programming.

4. **Major — Quota / ownership: twelve-code capacity depends on the event-source contract.**

   **Target:** `working_hypothesis.md:50, 52–56`.

   The document says the first pool has twelve codes, but does not define whether that is twelve across the installation, twelve per keyboard, or twelve within another collision domain. Device-filtered Raw Input could potentially distinguish equal codes from different keyboards; the global fallback cannot. Reserving codes for disconnected known devices also trades available capacity for reconnection safety.

   Current allocation examines active profiles and falls back to F13 when no free entry exists (`DeviceEditor.svelte:261–267`). Current runtime conflict rejection likewise counts active eligible global routes (`keyboard_shortcuts.rs:227–247`). These are insufficient foundations for the proposed durable ownership model.

   **Proposed mitigation:** State the initial collision domain explicitly. Choose either conservative installation-wide uniqueness or device-local allocation contingent on verified pairing and no ambiguous global fallback. Exhaustion must stop allocation with an actionable result. Decide how disconnected owners consume capacity and how ownership can be explicitly retired.

5. **Major — Architecture / runtime lifecycle: press/release support requires more than changing the Windows event source.**

   **Target:** `working_hypothesis.md:44–46, 52–54`.

   Raw Input is presented appropriately as an evaluation, but the proposed profile-switch behavior does not resolve a key held while the profile changes, a missing release after disconnect, repeated key-down events, or editing pause. The shared runtime already states that release belongs to the original binding/source (`modules/runtime.md:5`), while the existing route is one-shot `activate_fn` and is recreated as eligibility changes (`keyboard_shortcuts.rs:102–105, 169–171`).

   **Proposed mitigation:** Establish the conceptual invariant that a press owns its corresponding release/cancellation, even when profiles change. Require cancellation on device/session loss and a repeat policy before enabling hold-dependent actions. A smaller first scope can remain one-shot if that limitation is explicit.

6. **Major — Product semantics / profile ownership: retaining aliases after action removal leaves an unresolved user-visible state.**

   **Target:** `working_hypothesis.md:44–50, 56, 58`.

   Retaining ownership after deleting a host binding prevents unsafe reuse, but it creates a keyboard key that continues emitting an F-code without a host action. The document does not settle what the editor displays in a profile with no binding, whether creating a native assignment changes the keyboard for every profile, or how users deliberately restore the original record. Those choices are central to the unified assignment view.

   **Evidence:** The current UI explicitly warns that removing the host action does not restore the onboard mapping (`FnBindingSetup.svelte:174–177`), and current host assignments are profile-specific (`DeviceEditor.svelte:240, 251`).

   **Proposed mitigation:** Describe the unbound-but-reserved state and separate “remove this profile’s action” from “restore the keyboard assignment and release ownership.” Explain the global effect of editing common onboard mappings while viewing one profile. This is a product decision, not just allocation bookkeeping.

7. **Major — Evidence / acceptance: readback readiness and physical readiness remain too close in the wording.**

   **Target:** `working_hypothesis.md:21, 38, 48`.

   The hypothesis correctly disclaims physical proof at line 38, but “report readiness only after … read back” can still be interpreted as a sufficient condition. Readback establishes stored-byte agreement; it does not establish that Fn+T produces the expected input, that the correct keyboard is paired, that exactly one host action fires, or that onboard assignments work after DeckWeave exits and power cycles. The protocol explicitly retains these limits (`PROTOCOL.md:94, 100`).

   **Proposed mitigation:** Define separate evidence for configuration saved, stored mapping verified, input route available, and physical acceptance. Do not promise ordinary users a fully ready assignment solely from stored-byte agreement. Define the first supported device/firmware/transport envelope and a controlled physical acceptance gate.

8. **Major — Standards / decision quality: alternatives, effort and success exams are not explicit enough for promotion to a feature specification.**

   **Target:** `working_hypothesis.md:15–58, 80`.

   The alternatives rule requires documented comparison, while the hypothesis primarily describes its preferred path. The strongest alternatives are distinct: read-only truthfulness first; manual global aliases as an explicitly limited interim mode; automatic aliases with global one-shot input; automatic aliases with paired Raw Input; and profile-specific onboard rewriting. The document mentions several of these but does not compare their trade-offs together.

   Heilmeier cost, timeline and measurable exams are also missing. This is acceptable unfinished brainstorm material, but it should remain visible as unfinished.

   **Proposed mitigation:** Add a compact alternatives comparison and provisional effort/uncertainty ranges. Set decision gates around pairing proof, recovery policy, first-slice acceptance and broader action categories. Relevant exams include zero unrelated-byte changes, recovery after interrupted writes, rejection of ambiguous owners, correct profile switching, no duplicate activation, and app-closed onboard behavior. No cloud, database-query or LLM-token scaling pressure is established by this concept; local alias capacity and hardware recovery dominate its economics.

## Unresolved user decisions

- Is the first useful release truthful Main/Fn display plus one automatic host-action path, or the broader onboard action palette?
- Is one-shot activation acceptable initially, or are hold/release actions required?
- Is global F-code behavior acceptable as an interim mode, including activation from another keyboard and visibility to other applications?
- Should twelve aliases be shared conservatively across the installation, or should capacity depend on successful device-specific pairing?
- What should an empty profile do with an already programmed host alias?
- Should removal preserve the alias by default, with restoration as a separate explicit operation?
- Is common onboard configuration across all profiles the intended product rule?
- What user-assisted recovery/adoption is acceptable when keyboard identity or ownership history is unavailable?

## Suggested stage gates

1. Agree on scope, profile semantics, deletion/restoration behavior and collision domain.
2. Prove the selected keyboard-input pairing and required one-shot or press/release semantics.
3. Review durable ownership, uncertain-write and external-edit recovery policies before enabling automatic programming.
4. Accept one controlled assignment end to end, including interruption recovery and preservation of unrelated mappings.
5. Broaden supported onboard categories only with category-specific physical evidence.

These are proposed review gates, not an implementation plan or completed acceptance.

## Read-file manifest

Fully read:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/design_critic.md`
- `G:/APPS/DW/deckweave/docs/designs/keyboard-assignment-execution/working_hypothesis.md`
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md`
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/heilmeier-standard.md`
- `G:/APPS/DW/deckweave/README.md`
- `G:/APPS/DW/deckweave/modules/editor.md`
- `G:/APPS/DW/deckweave/modules/runtime.md`
- `G:/APPS/DW/deckweave/apps/desktop/src/FnBindingSetup.svelte`
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/keyboard_shortcuts.rs`
- `G:/APPS/io_type_84/docs/PROTOCOL.md`

Targeted source reads:

- `G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte`: keyword search and lines 195–280, 455–525, 790–865, 980–1005.
- `G:/APPS/io_type_84/src/features/editor/model.ts`: keyword search and lines 1–260, 380–434.
- `G:/APPS/io_type_84/crates/io-platform/src/changes.rs`: keyword search and lines 1–80, 160–225, 370–550.

No prior chats, memory, or other files retrieved. Review status: **completed**.
