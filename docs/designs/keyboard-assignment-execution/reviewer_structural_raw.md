BRAINSTORM review completed. I applied the supplied persona’s long-term coupling and maintenance lens. I read the feature-specification standard as background for risk and alternative analysis; I did not audit this working hypothesis against a final specification template. This was read-only: no files changed, builds, device operations, hotkeys or user-data mutations.

The hypothesis’s strongest architectural decision is separating a persistent physical trigger from the action selected by a profile. The main maintainability risk is that the existing implementation stores those concepts together. Adding automatic hardware programming to that model would make a temporary shortcut feature responsible for a permanent device configuration lifecycle.

1. **Blocker — Long-Term Tech Debt & Maintainability: persistent alias ownership needs an independent domain model.**

   **Observed facts:** The hypothesis explicitly makes an alias belong to a physical keyboard control and onboard layer, surviving action removal and profile changes (`working_hypothesis.md:44–50,68`). The current `Profile` owns `fn_bindings`, and `FnBinding` contains both trigger and action (`deckweave-core/src/model.rs:240–258`). `allocate_trigger` consults the active `device.profile.fn_bindings`; `fn_routes` likewise derives routes from current profiles (`fn_actions.rs:3–21,108–128`). The editor independently scans those same bindings and returns F13 if its search finds no free code (`DeviceEditor.svelte:248–268`).

   **Risk:** Profile state is not a durable inventory of physical outputs. Reusing it for ownership would require every profile operation, action deletion, import, device removal and future alias extension to remember hardware allocation rules. Divergent UI and core allocators would further increase maintenance cost.

   **Maintainability requirement / proposed mitigation:** Introduce a versioned device/control/layer ownership record independent of actions, with one authoritative allocator. Profile bindings should reference the physical source. Migration must preserve the existing Fn+T Launch and must distinguish existing host configuration from proven ownership of an observed F-code. Do not adopt a preexisting F13 mapping merely because it matches a saved trigger.

2. **Blocker — Long-Term Tech Debt & Maintainability: “one user operation” crosses two persistence systems without an explicit operation lifecycle.**

   **Observed facts:** The candidate interaction joins saving a host action, replacing an onboard record and confirming readiness (`working_hypothesis.md:21,48`). The draft `SetFnBinding` branch can update profile bindings and provider settings together in core configuration (`lib.rs:294–339`), while provider hardware writes execute later (`device_providers.rs:2331–2336`). The driver separately persists a snapshot, rechecks current bytes, records pending intent, writes and reads back (`device-io-type-84/src/main.rs:188–263`). These are useful existing safeguards, but they do not constitute a combined alias/action transaction.

   **Risk:** Restart or failure between host save, alias reservation, SET, readback and final ownership confirmation leaves several independently meaningful truths. A collection of UI booleans and inferred status strings would become a brittle distributed state machine.

   **Maintainability requirement / proposed mitigation:** Specify a durable operation identity and a small explicit lifecycle covering reservation, saved configuration, hardware confirmation, uncertainty and restoration. Associate driver outcomes with that operation rather than the currently selected key. Recovery must reconcile the observed record with the intended record after restart without replaying an uncertain SET. A profile switch during programming must not change which physical source the operation belongs to.

3. **Major — Long-Term Tech Debt & Maintainability: alias identity outlives the currently trustworthy device identity.**

   **Observed facts:** The hypothesis requires ownership through profile changes and action removal, and evaluation of Raw Input pairing (`working_hypothesis.md:44–54`). The provider descriptor currently declares session confidence and no persistent ID (`model.rs:99`). The protocol documents an empty USB serial and warns against selecting the first matching device (`PROTOCOL.md`, “Подключение”).

   **Risk:** An ownership key based on an ephemeral device path or session identity can leak reservations, attach a previous keyboard’s original mapping to a replacement unit, or confuse identical devices after reconnection. Raw Input adds another interface identity that must be correlated with the vendor interface.

   **Maintainability requirement / proposed mitigation:** Separate durable logical device identity, present connection generation and paired Windows input identity. Record confidence and require explicit reconciliation where identity is ambiguous. Define what survives unplugging, restarting, moving USB ports, removing a device from DeckWeave and connecting two identical keyboards. Fail closed for routing and restoration when pairing cannot be established.

4. **Major — Long-Term Tech Debt & Maintainability: the generalized assignment vocabulary needs a capability contract richer than “writable.”**

   **Observed facts:** The hypothesis includes keyboard, mouse, consumer, combinations and device functions, while acknowledging different integration and verification levels (`working_hypothesis.md:19,27–38`). The current descriptor advertises `bindingFormat: "keyboard-usage"`; `edit_mappings` accepts only standard keyboard usages and refuses overwriting SOCD/RS pairs (`model.rs:99,159–196`). Raw mapping records and labels are separate observations (`model.rs:127–148`). The protocol describes coupled advanced records and explicitly limits physical execution evidence.

   **Risk:** A generic menu of action names can pressure the editor to infer device support, or produce many model-specific branches. Decoding an existing function, encoding its bytes, verifying those bytes and accepting its physical behavior are different capabilities. Device functions also contain actions that should never enter an ordinary automatic assignment path.

   **Maintainability requirement / proposed mitigation:** Define a versioned assignment representation with opaque preservation and adapter-owned validation/encoding. Capability declarations should distinguish readable/displayable, assignable, dependency-sensitive and evidence status. Keep the narrow function allowlist in the adapter. Unsupported and paired records need clear, non-destructive interaction rules; their presence must not imply the unified editor can safely replace them.

5. **Major — Long-Term Tech Debt & Maintainability: replacing a one-shot source with down/up input affects runtime semantics beyond Windows plumbing.**

   **Observed facts:** The hypothesis proposes evaluating Raw Input and possibly enabling Key Logic (`working_hypothesis.md:52–54`). The current shortcut source supplies F-code activations with `MOD_NOREPEAT`, not release events (`deckweave-platform/src/shortcuts.rs`). Core explicitly rejects Key Logic recursively and implements immediate logical activation (`fn_actions.rs:75–90,129–193`). Existing runtime documentation says releases address the original binding/source and disconnect clears holds without executing release actions (`modules/runtime.md`, `modules/providers.md`).

   **Risk:** Merely forwarding Raw Input down/up to existing Fn activation would leave repeat handling, held aliases during profile switches, reconnect cancellation and fallback transitions underspecified. Generalizing the existing Fn-specific APIs incrementally could create two competing input lifecycles.

   **Maintainability requirement / proposed mitigation:** Introduce an input-source contract that declares one-shot versus down/up behavior and routes normalized events into the shared runtime. Pin release to the source captured on press. Specify repeat, disconnect, focus/profile changes and source failover. Do not enable Key Logic until pairing and lifecycle behavior are established; do not infer standalone Fn events.

6. **Major — Long-Term Tech Debt & Maintainability: automatic alias restoration needs stricter conflict policy than generic explicit recovery.**

   **Observed facts:** The hypothesis says to save the exact original record and never restore over a later external edit (`working_hypothesis.md:48–50`). Existing recovery merges original records for slots the recorded operation changed (`recovery.rs:391–410`). It uses a fresh preview and rejects drift occurring after that preview (`recovery.rs:549–704`). That explicit recovery workflow does not, by itself, establish the proposed automatic alias-release policy.

   **Risk:** Reusing “restore backup” as “remove host assignment” could overwrite an external edit that predates the recovery preview. Conversely, retaining every alias forever would exhaust the small pool and make maintenance dependent on manual cleanup.

   **Maintainability requirement / proposed mitigation:** Define conditional restoration: restore the saved original only when the current record still matches the owned alias record. Otherwise retain a conflict requiring reconciliation. Store provenance and exact records with ownership, rather than using a generic snapshot filename as the ownership contract.

7. **Major — Long-Term Tech Debt & Maintainability: finite capacity and global fallback need one explicit allocation policy.**

   **Observed facts:** Twelve initial codes and the prohibition on silent reuse are explicit (`working_hypothesis.md:50,56`). Current global routing rejects duplicate eligible triggers (`keyboard_shortcuts.rs:240–267`), but that is runtime conflict detection, not full ownership allocation. The prototype allocator’s error suggests removing an unused action, which conflicts with the new rule that removal alone does not free hardware ownership (`fn_actions.rs:21`).

   **Maintainability requirement / proposed mitigation:** Decide whether the first pool is globally unique across known keyboards or scoped per paired device. Raw Input may permit device scoping; global fallback cannot transparently inherit that policy. Expose capacity exhaustion as an actionable ownership problem and make fallback availability explicit. Preserve the distinction between stored-byte confirmation and physical execution acceptance.

Unresolved interaction decisions are: what deleting a host action does to its physical alias; what an inactive profile with no binding displays and emits; whether Main-layer host assignments enter the first delivery; how ambiguous reconnect pairing is resolved; whether users can explicitly retain an alias without an action; and whether fallback may run when device-specific routing is unavailable. The unified editor also needs a deterministic precedence rule when an observed onboard record, a saved host action and an unfinished operation disagree.

The concepts brief should compare a persistent shared alias layer, profile-specific hardware rewriting, and explicit manual trigger setup. The hypothesis already gives substantive reasons to prefer stable aliases, but the other approaches should remain documented alternatives rather than quietly reappearing as exception paths.

Read-file manifest:

- **Full:** `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/structural.md`
- **Full:** `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md`
- **Full:** `G:/APPS/DW/deckweave/docs/designs/keyboard-assignment-execution/working_hypothesis.md`
- **Full:** `G:/APPS/DW/deckweave/README.md`
- **Full:** `G:/APPS/DW/deckweave/modules/editor.md`
- **Full:** `G:/APPS/DW/deckweave/modules/runtime.md`
- **Full:** `G:/APPS/DW/deckweave/modules/providers.md`
- **Full:** `G:/APPS/DW/deckweave/apps/desktop/src/FnBindingSetup.svelte`
- **Full:** `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/keyboard_shortcuts.rs`
- **Full:** `G:/APPS/io_type_84/docs/PROTOCOL.md`
- **Full:** `G:/APPS/DW/deckweave/crates/deckweave-core/src/fn_actions.rs`
- **Full:** `G:/APPS/DW/deckweave/crates/deckweave-platform/src/shortcuts.rs`
- **Targeted:** `G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte`, lines 1–290, 791–867, 981–1007; assignment/Fn symbol searches across the file.
- **Targeted:** `G:/APPS/DW/device-io-type-84/src/model.rs`, descriptor and mapping implementation through line 198; mapping/capability symbol searches across the file.
- **Targeted:** `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs`, lines 236–275; Fn-related symbol search.
- **Targeted:** `G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs`, lines 286–385.
- **Targeted:** `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/device_providers.rs`, lines 2311–2350; session/readiness/recovery symbol searches.
- **Targeted:** `G:/APPS/DW/device-io-type-84/src/main.rs`, lines 188–263; write/backup/readback symbol searches.
- **Targeted:** `G:/APPS/DW/device-io-type-84/src/recovery.rs`, lines 391–448 and 549–704; recovery/identity symbol searches.

Missing-path probes for `deckweave-core/src/core.rs` and `storage.rs` returned no file and supplied no evidence. No prior chats or memory were retrieved. All required review work is complete; implementation and hardware acceptance remain outside this review.
