Review completed, read-only. No files edited, builds/tests run, device operations performed, hotkeys registered, or user data changed. No prior chats or memory retrieved. Proposed mitigations below are requirements for the concept; none is claimed implemented.

The source supports the hypothesis’s central distinction: onboard assignments and host bindings are separate models. No Blocker-level false statement was found. Several Major decisions remain before this hypothesis can become an implementable specification.

Findings

1. Major — Durable physical ownership lacks an identity contract.

Target: `G:/APPS/DW/deckweave/docs/designs/keyboard-assignment-execution/working_hypothesis.md:44–50`, especially “belongs to a physical keyboard control.”

Evidence:
- `G:/APPS/DW/device-io-type-84/src/model.rs:99` advertises `identity.confidence:"session"` and `persistentId:null`.
- `G:/APPS/DW/device-io-type-84/src/recovery.rs:18–19` explicitly says a vanished opaque identity is never trusted again.
- `G:/APPS/io_type_84/docs/PROTOCOL.md:7` says an empty serial cannot establish permanent identity and multiple candidates must not default to the first.
- Current recovery requires a preview token, current instance confirmation and an unchanged scoped revision: `G:/APPS/DW/device-io-type-84/src/recovery.rs:585–643`.

Risk: durable ownership across restart/reconnect, plus Raw Input interface pairing, cannot simply key ownership by the current provider instance. Two identical keyboards or a changed HID path can otherwise lead to a false ready state or restoration on the wrong device.

Proposed mitigation: distinguish persistent logical keyboard identity, current vendor session and current Raw Input source. Specify confidence and reattachment rules, fail closed on ambiguity, and require explicit reassociation where evidence cannot identify the keyboard. Diagnostics should expose pairing confidence and its reason without logging ordinary input.

2. Major — Existing persistence is profile-owned; changing ownership requires migration and crash semantics.

Target: hypothesis `:44–50`, `:76`.

Evidence:
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs:240–267`: `Profile.fn_bindings` owns `{trigger,binding}`; `FnRoute` includes profile and binding identity. It has no independent device/control/layer ownership record or original record.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/fn_actions.rs:3–21`: allocator scans current `d.profile.fn_bindings`, excluding the same control. It does not consider observed mappings or inactive profiles.
- `G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte:261–267` independently allocates from active bindings and falls back to F13 when exhausted.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs:328–334` deletes the stored Fn binding immediately when removing the action.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs:41–103,131–156` has format-2 transactional JSON storage and old-writer guard, not an alias ownership migration.

Risk: existing Fn+T Launch must migrate without inventing its pre-alias record. Existing profiles can disagree about triggers for the same physical control. Host persistence and hardware programming cannot share an atomic transaction; a crash between them leaves a reservation or programmed alias with incomplete host state.

Proposed mitigation: specify migration of existing bindings into separate ownership and profile action references, explicit conflict/unknown-original states, durable operation identity, and recovery after every persistence/write boundary. Decide whether the new schema needs a format bump or another effective old-writer gate; merely adding a serde field does not protect it from an older format-2 writer.

3. Major — “Never restore over a later external edit” needs a new alias-restoration contract.

Target: hypothesis `:48–50`, `:68`.

Evidence:
- `G:/APPS/DW/device-io-type-84/src/recovery.rs:391–409`: current `merge_restore` copies original four-byte records into every slot changed by the historical operation. It does not first compare the current slot against the operation’s target.
- `G:/APPS/DW/device-io-type-84/src/recovery.rs:638–643` protects against changes *after preview*, which is a different guarantee.
- `G:/APPS/DW/device-io-type-84/src/main.rs:200–254` already provides fresh reads, durable backup, pending marker and final readback for ordinary settings.

Risk: reusing ordinary explicit recovery as automatic alias release could overwrite a later external edit made before recovery preview. The hypothesis correctly states the desired safeguard but does not identify that existing recovery has different semantics.

Proposed mitigation: preserve explicit ordinary recovery as its own workflow. Define conditional alias release: verify current owned record against the exact expected installed record, detect divergence, preserve ownership/recovery evidence until resolved, and verify removal before freeing the code. Record installed/original raw bytes and relevant operation identity. Reuse existing durable backup/readback machinery, not its unconditional historical-slot restoration semantics.

4. Major — Unified assignment types need coordinated provider and host contracts.

Target: hypothesis `:19`, `:29–35`, `:69`.

Evidence:
- `G:/APPS/DW/device-io-type-84/src/model.rs:127–147` already exposes raw page/parameters and optional keyboard usage, plus separate decoded labels.
- Driver writes accept only one-field `{usage}` values: same file `:159–194`.
- Host validates `bindingFormat == "keyboard-usage"` and the same one-field shape: `G:/APPS/DW/deckweave/crates/deckweave-core/src/provider_settings.rs:521–565`.
- Frontend descriptor exposes only a generic `bindingFormat:string`: `G:/APPS/DW/deckweave/apps/desktop/src/types.ts:143–151`.
- Prototype validator contains reusable mouse, Consumer, combination and narrow FUNC semantics: `G:/APPS/io_type_84/crates/io-platform/src/changes.rs:41–79`.
- Current driver rejects overwriting SOCD/RS records: `G:/APPS/DW/device-io-type-84/src/model.rs:188–192`; prototype instead clears both matching pair records: `G:/APPS/io_type_84/crates/io-platform/src/changes.rs:197–207`.

Risk: “adapter declares encodable actions” requires a richer contract, not just adding action titles. Blindly porting the prototype writer changes paired-assignment behavior and violates the simple expectation that only the selected assignment changes.

Proposed mitigation: define a versioned assignment vocabulary, capabilities by form/layer/parameter range, read-only unknown/reference-dependent states, and independent physical-verification status. Keep hardware codec/validation in the driver; shared runtime owns launch/plugins/conditions. Preserve the current paired-record rejection for this slice unless a separate, visible pair-edit policy is accepted. Reuse the prototype’s typed semantics and tests as reference, with explicit integration in driver, host validator and frontend.

5. Major — Raw Input requires a new lifecycle route, not just a new input collector.

Target: hypothesis `:52–56`.

Evidence:
- `G:/APPS/DW/deckweave/crates/deckweave-platform/src/shortcuts.rs`: bounded 32-event queue, explicit F13–F24 registrations with `MOD_NOREPEAT`, and events containing only `u8`.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/fn_actions.rs:73–88` explicitly rejects Key Logic recursively for Fn bindings.
- Same file `:108–184` routes only the active profile’s Fn bindings and calls one-shot `run_binding`; `activate_fn` intentionally creates no physical held state.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs:681–697` already has Down/Up/Cancel input vocabulary.
- `G:/APPS/DW/deckweave/modules/runtime.md:5` documents release addressed to its original binding/source.
- Existing desktop route validation includes profile/binding/lifecycle/generation protection: `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/keyboard_shortcuts.rs:102–105,163–171,249–285`.

Risk: forwarding a Raw Input Down/Up into `activate_fn` loses release semantics. Re-resolving Up against the newly active profile can release the wrong action. Disconnect, overflow or backend changes can strand held actions or deliver duplicate activations.

Proposed mitigation: retain the shared runtime’s source and original binding release semantics; define alias input routes with layer and backend identity, paired device generation, repeat handling, Down/Up/Cancel behavior, and cleanup on disconnect/profile switch/backend handover. Keep global fallback one-shot capabilities explicit. No claim of physical Fn events follows from receiving the emitted F-code.

6. Major — Readiness and operational evidence need an end-to-end state model.

Dimension: Observability & Operational SRE Debuggability.

Target: hypothesis `:21`, `:48–54`.

Evidence:
- Current UI polls provider and route status separately: `G:/APPS/DW/deckweave/apps/desktop/src/FnBindingSetup.svelte:69–91`; programming confirmation watches generation, `settings_edit` and observed usage: `:97–140`.
- Desktop route status contains `{state,trigger,error,source}`: `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/keyboard_shortcuts.rs:17–23`.
- `ready()` maps error/recovery-pending states to `disconnected`: same file `:254–256`.
- There is useful existing overflow/worker-exit reporting: same file `:137–155`.
- Driver returns `snapshotId` after successful readback: `G:/APPS/DW/device-io-type-84/src/main.rs:247–260`.

Risk: “not ready” can mean host saved, alias reserved, hardware write uncertain, mapping externally changed, input interface unpaired or backend failed. Current status cannot correlate one drag operation with all these stages. Triage is especially difficult after a restart or partial failure.

Proposed mitigation: carry one operation/correlation identifier through host save, reservation, driver backup/pending marker, readback and route activation. Expose distinct diagnostic reasons and last verified generation/read sequence. Add bounded diagnostic events/counters for allocation conflict/exhaustion, mapping mismatch, unpaired source, stale-generation drops, duplicate path prevention, overflow and cancellation. Retain only configured alias metadata; do not log ordinary input, executable secrets, or unrelated Raw Input payloads.

7. Major clarification — “writes only that assignment” is a logical preservation guarantee.

Target: hypothesis `:21`.

Evidence:
- Current driver calls `transport::write_block` for the full changed Main/Fn block: `G:/APPS/DW/device-io-type-84/src/main.rs:221–227`.
- Prototype docs explicitly describe full changed-block SET and say single-position writing needs separate investigation: `G:/APPS/io_type_84/docs/PROTOCOL.md:90–96`.

Proposed mitigation: say that only the selected logical assignment changes and all unrelated raw records survive; state that transport currently writes the full changed block. This avoids implying established single-record physical writes or atomicity.

Unresolved user/product decisions

- Whether unprovable reconnect identity requires explicit reassociation and what the user sees for two identical keyboards.
- Whether alias codes are globally unique across all remembered keyboards or scoped to confidently paired Raw Input devices; compatibility with global fallback constrains that choice.
- What happens when a profile has no action for an owned physical alias. The hardware still emits that F-code.
- Whether removing an action merely retains ownership, and which separate user action releases/restores the key.
- How existing profiles that assign different aliases to the same physical control migrate; unknown historical originals cannot be reconstructed.
- Whether the first release exposes Key Logic at all or retains one-shot capability until Raw Input pairing/lifecycle is physically accepted.
- Whether replacement of a paired SOCD/RS assignment remains blocked.
- Whether OS propagation of F-codes is an accepted behavior, since Raw Input does not establish suppression.

Alternative evaluation needed before feature-spec approval

The hypothesis considers RegisterHotKey fallback and postpones profile-specific onboard rewriting. Before claiming compliance with the supplied feature-spec standard, compare explicit designs: Raw Input as required backend versus global one-shot fallback; global versus per-paired-device allocation; retaining unbound aliases versus explicit restoration/release; integrating prototype codecs versus a narrower keyboard-only first slice. Reasons should include identity confidence, collision behavior, lifecycle complexity and hardware acceptance. This is a brainstorm gap, not a demand for a finished implementation plan now.

Read-file manifest

Required files read:
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/codebase_details.md` — full.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md` — full.
- `G:/APPS/DW/deckweave/docs/designs/keyboard-assignment-execution/working_hypothesis.md` — full, lines 1–80.
- `G:/APPS/DW/deckweave/README.md` — full.
- `G:/APPS/DW/deckweave/modules/editor.md` — full.
- `G:/APPS/DW/deckweave/modules/runtime.md` — full.
- `G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte` — targeted source lines 190–275 and complete keyword search across assignment/layer/render paths; not full-file reading.
- `G:/APPS/DW/deckweave/apps/desktop/src/FnBindingSetup.svelte` — full.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/keyboard_shortcuts.rs` — full.
- `G:/APPS/io_type_84/src/features/editor/model.ts` — full.
- `G:/APPS/io_type_84/crates/io-platform/src/changes.rs` — targeted lines 1–245, 328–565 and keyword search; not full-file reading.
- `G:/APPS/io_type_84/docs/PROTOCOL.md` — full.
- `G:/APPS/io_type_84/docs/EDITOR_IMPLEMENTATION.md` — full.

Additional files read:
- `G:/APPS/DW/deckweave/modules/storage.md` — full.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/fn_actions.rs` — full.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs` — targeted lines 130–175, 220–280, 675–713 and searches.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs` — targeted lines 290–342 and searches.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/provider_settings.rs` — targeted lines 510–580 and searches.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs` — lines 15–158 and searches.
- `G:/APPS/DW/deckweave/crates/deckweave-platform/src/shortcuts.rs` — full.
- `G:/APPS/DW/deckweave/apps/desktop/src/types.ts` — targeted lines 130–157 and searches.
- `G:/APPS/DW/device-io-type-84/src/model.rs` — descriptor line 99 via search; lines 100–215, 245–375 and searches.
- `G:/APPS/DW/device-io-type-84/src/main.rs` — lines 188–260 and searches.
- `G:/APPS/DW/device-io-type-84/src/recovery.rs` — lines 1–55, 391–414, 535–680 and searches.
- `G:/APPS/io_type_84/modules/notification-sources.md` — full.

Repository-wide searches also returned snippets from core tests, plugin/main/action integration and prototype service/input ownership files. Those snippets were discovery evidence only; they were not treated as fully audited source.

Completed status: BRAINSTORM codebase-boundary and operational-debuggability review complete. Stored-byte readback is evidence of the stored representation only; it does not establish physical execution or power-cycle persistence of each action class.
