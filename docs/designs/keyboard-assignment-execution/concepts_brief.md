# Keyboard assignments: onboard first, host when needed

* **Author:** DeckWeave EGM synthesis
* **Date:** 2026-11-02

## Overview

The current Fn editor shows DeckWeave actions without showing the keyboard assignments that actually produce input. A saved Launch action can therefore look usable while Fn+T still performs its original keyboard function. The proposed editor shows the observed Main and Fn assignments, then uses keyboard execution for supported native actions and DeckWeave execution for launch, plugins, conditions and other host work.

This brief preserves the proposal reviewed by six independent reviewers. After that review the owner approved the common onboard baseline, per-profile host actions, retained aliases on action removal, and explicit conditional restoration. The first implementation now includes the ownership/migration/recovery/input-route boundaries described in the [current implementation guide](../../KEYBOARD-ASSIGNMENTS.md). The proposal and reviewer reports below are historical reasoning, not physical acceptance. The user's existing Fn+T Launch has been preserved.

## Analysis

### What is established

The adapter already reads complete Main/Fn blocks. A read-only label projection has been added in source but is not deployed. Ordinary keyboard usages have a driver write/readback path. Mouse, Consumer/media/browser, modifier combinations and a narrow set of device functions have prototype codecs; they are not integrated writable DeckWeave capabilities. A stored-byte roundtrip does not prove physical execution or persistence after power loss for those forms. The current running release still exposes manual F-code setup.

The shared runtime already executes host actions and has a source-based input lifecycle. The existing Fn shortcut route is global and one-shot; it rejects Key Logic. The driver owns codecs, validation and transport. These are reuse boundaries, rather than reasons to introduce another action runtime or import the prototype editor.

The source review found three gates before automatic programming: physical ownership is currently stored inside profile bindings; older format-2 writers can discard newly added typed fields; and saving host state, driver recovery files and keyboard mappings is not one atomic transaction. The [review report](goldfish_review_report.md) gives source references and preserves the distinction between findings and proposed changes.

### Physical ownership and conservative allocation

Proposed engineering direction: create one authoritative ownership record outside profiles. It identifies a logical keyboard, physical control, onboard layer, reserved alias, exact original and installed records, provenance, and operation outcome. Profile bindings refer to that source. Current UI and core allocators must not compete. A profile switch changes host resolution without a keyboard write.

Root-selected engineering direction after review: each verified physical keyboard has twelve aliases shared across its Main and Fn physical sources. The same code can be used by another positively identified keyboard because the new route is device-specific; missing pairing must never be replaced by app-wide allocation or global activation. Count each keyboard/control/layer once even if many profiles use it. Include inactive profiles, observed mappings, requested mappings, unresolved writes and disconnected ownership in that keyboard's conflict checks. Unrelated preexisting F-codes are occupied or unresolved; they are not automatically adopted. Exhaustion stops the assignment and explains which retained controls consume capacity. It never reuses F13 as a fallback. This is the root's design direction, not unanimous reviewer agreement, proven source capability or implemented allocation.

Migration preserves all existing host bindings, including Fn+T Launch, without programming hardware. Distinguish a legacy saved trigger reference, a currently observed mapping, proven ownership, and a conflict. A current read cannot reconstruct an original native record that was never saved. Conflicting legacy profiles remain visible and unresolved. Legacy global shortcut references must be reconciled and their global routes retired before a source is handed to the new device-specific path; no parallel routes or global fallback are allowed. Introduce an enforceable storage writer-version boundary with a durable pre-migration backup before saving new ownership data. An executable or database rollback does not restore keyboard state.

### Device identity and the input route

The current adapter declares session-only identity and no persistent ID. Durable historical ownership must remain separate from the vendor connection generation and Windows input collection identity. Do not attach old originals to the first matching VID/PID. If continuity is ambiguous, retain reservations and require reconciliation before routing, programming or restoration.

Root follow-up evidence was read only after all six reviews. A read-only inventory found that both current IO84 Raw Input keyboard collections resolve through Windows device properties to the current vendor interface's nonempty ContainerId. This supports pairing the present collection set. It does not show which collection emits F13–F24, whether both report one press, whether release events work, whether identity survives a port change, or how two identical IO84 units differ. No event collector or mapping write was enabled. See [the separate input-source follow-up](input-source-follow-up.md) and [inventory result](../../evidence/2026-11-02/io84-input-identity.json).

Raw Input is the preferred candidate for device-specific input, subject to those exams. The pinned Tao 0.37.1 event loop already registers keyboard and mouse Raw Input. Windows permits one target window per raw-input class in a process, with the last registration taking effect. The next design must compare sharing that existing owner against an isolated input/provider process; an unrelated second desktop registration is excluded. This is an integration constraint, not evidence that existing input is broken. [Microsoft registration contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices).

Only configured aliases from the confirmed collection set may enter the host runtime. Discard unrelated events without retaining their keys or payloads. Keep bounded diagnostics about source confidence, operation state, conflicts and dropped events. Do not silently enable the global shortcut path when device pairing fails, and never run both backends for one binding. Ordinary F-codes remain visible to other applications; this proposal does not implement suppression or establish standalone Fn events.

Keep the first new host slice one-shot. Before exposing hold/release or Key Logic, prove emitted Down/Up events and define repeat, duplicate-collection handling, overflow, disconnect and source handover. Releases and cancellation must address the source and binding captured on press, including across profile changes. Reuse shared runtime rules rather than forwarding Down/Up into the current one-shot `activate_fn` path.

### Write and recovery boundaries

“One assignment” means one intended logical edit. The established transport writes a complete changed layer block. Preserve every unrelated raw record and compare the complete intended block diff; do not imply single-record transport atomicity. Keep SOCD/RS paired assignments and unknown/reference-dependent selected records read-only in the first slice. The prototype can clear linked records when replacing one member, while the current adapter rejects that overwrite; blindly porting the prototype changes the safety boundary.

Proposed recovery direction: persist an operation identity, original bytes, intended bytes, reservation and pending hardware obligation before a write. Correlate host save, driver backup/pending intent, write outcome and readback with that operation. Reopening or reconnecting reads and reconciles; it does not replay an uncertain SET. An uncertain operation keeps its reservation and cannot claim an active host route.

Alias release is a distinct explicit operation. Restore the exact saved original only when the currently observed record still equals the owned installed record and identity is confirmed. If the record differs, preserve the external edit and keep a conflict for reconciliation. Existing generic recovery preserves neighbouring records and protects changes after a recovery preview; it does not establish this stronger same-record history rule. Free the alias only after a read confirms that the old physical mapping no longer emits it. Unknown historical originals cannot be invented.

### What the user must be able to understand

Show two separate facts: where an action executes and where an edit applies. “Keyboard” means common hardware behavior across DeckWeave profiles; “DeckWeave” means the action in the selected profile. Display observed onboard behavior even when it is unknown or cannot yet be edited. Keep recognized-but-read-only, unsupported, unknown and host-only actions distinct. Never silently substitute a host action for a requested native action.

Proposed interaction default: removing an action removes only this profile's host action. It leaves the physical alias reserved and visible as “Keyboard trigger retained; no action in this profile.” No implicit inheritance or keyboard restoration occurs. A separate “Restore keyboard assignment” flow explains affected profiles and verifies the hardware change before release. These user-visible semantics still need owner alignment.

Proposed lightweight interaction: on the first transition from a native assignment to a DeckWeave-owned trigger, show a compact replacement preview with the observed native behavior, the all-profile keyboard effect and app-stopped consequence. “Use DeckWeave” commits that physical transition; cancelling leaves the observed assignment intact. This preview is part of the assignment gesture, not a separate F-code setup screen. Subsequent host-action edits for an already-owned source save immediately without rewriting the keyboard. Unknown, paired and mode-changing records are unavailable for replacement in the first slice. This UX is a concrete proposal for refinement, not an additional approval already obtained.

Use understandable outcomes rather than a single ready flag:

| Outcome | What it establishes | Next step when incomplete |
| --- | --- | --- |
| Action saved | Host configuration exists | Establish the mapping and input route |
| Writing / verifying | A hardware operation is in progress | Keep its owner and operation visible |
| Stored mapping verified | Readback matches intended bytes | Establish source-route availability |
| Host route available | A confirmed source and supported runtime route are enabled | This does not certify every physical action form |
| Mapping uncertain / recovery required | Hardware result is unresolved | Read/reconcile; do not silently retry or release |
| Identity or mapping conflict | Current device/state does not match ownership evidence | Preserve hardware and offer reconciliation |
| Physical behavior accepted | The supported device/firmware/transport envelope passed controlled end-to-end exams | Do not infer this from software Test or readback alone |

The editor projection should use fresh observed mappings as hardware truth, profile data as requested host behavior, and a durable operation as pending/uncertain state. It must show disagreements instead of letting one source overwrite the others' meaning. Hardware connection alone is not input-route readiness. Closing windows leaves the tray runtime running; “DeckWeave stopped” means the runtime has exited. Native assignments continue then; a host-bound key still emits its F-code but cannot run its DeckWeave action.

## Goals vs. Non-Goals

* **In-Scope Goals:** Show observed Main/Fn assignments; keep unknown bytes intact; expose execution location and edit scope; preserve existing host bindings; build a narrow automatic host-assignment path with explicit ownership, versioned migration, conditional restoration and truthful outcomes; deliver required rich Key Logic after physical-source and lifecycle proof; reuse the existing driver and shared runtime; add native action forms only behind declared adapter capabilities and category-specific acceptance.
* **Out-of-Scope Non-Goals:** Firmware replacement, calibration, reset, transport/OS-mode changes as ordinary assignments, automatic settings replay on open/reconnect, profile-specific onboard rewrites, arbitrary alias modifier schemes, aliases beyond the first twelve per verified physical keyboard, F-code suppression, standalone Fn detection, first-slice hold/Key Logic, macros/DKS/paired-action editing, and treating temporary RAM lighting as a stored assignment. Temporary lighting still lacks accepted visible hue/blink and hardware-timeout evidence.

## Alternatives Weighed

| Alternative | Benefit / strongest argument for it | Disposition and reason |
| --- | --- | --- |
| Truthful read-only Main/Fn view first | Fixes missing onboard information with no new hardware-write lifecycle | Selected as the first independent stage; insufficient as the final Launch workflow because manual programming remains |
| Continue saving only host bindings | Reuses today's simple editor operation | Rejected as the final interaction because it can present an unusable physical assignment as complete |
| Keep manual F13–F24 setup as the normal workflow | Exposes global-shortcut limits without new allocation machinery | Rejected as the new default because it makes users manage the internal bridge; preserve existing data and diagnostic visibility |
| Automatic aliases with global RegisterHotKey | Reuses the current one-shot route | Rejected as the new device-specific default: other keyboards can activate it, registration can conflict, and it cannot provide release semantics; no silent fallback |
| Stable aliases with paired Raw Input | Keeps common onboard mappings stable and can attribute emitted input to a source | Preferred candidate, conditional on event, identity, duplicate and integration proof; inventory alone is insufficient |
| Share Tao's input owner | Avoids a competing in-process registration and may reduce process plumbing | Retained for the next design; verify supported access to source identity and lifecycle without framework fragility |
| Isolated input/provider process | Avoids taking Tao's registration and fits external-provider boundaries | Retained for comparison; adds process transport, shutdown and source-generation handling; not selected without evidence |
| Twelve aliases per verified physical keyboard | Supports multiple keyboards without an app-wide twelve-source ceiling | Selected by the root after review for the new device-specific path; pairing, per-keyboard reservations and retirement of legacy global routes are mandatory; no silent fallback |
| Installation-wide twelve-code uniqueness | Simplifies global-shortcut collision bookkeeping | Rejected by the root as the new allocation policy because it imposes an app-wide ceiling and cannot substitute for missing source identity; retained as an evaluated conservative alternative |
| Rewrite hardware on every profile switch | Can make native behavior profile-specific | Deferred as a separate explicit feature; increases writes, recovery states and closed-app ambiguity and is unnecessary for switching host actions |
| Port the full prototype editor and writer | Brings many existing encodings quickly | Rejected as an integration strategy: advanced tables, pair-clearing behavior and host validation differ; reuse narrow codecs and tests in the adapter |

## Key Decisions & Staging for Next Steps

The established constraints remain: onboard execution where supported; host execution for arbitrary actions; no automatic replay; driver-owned transport/codecs; shared host runtime; exact preservation and no misleading ready state. The synthesis selected one ownership ledger, one allocator, twelve aliases per verified physical keyboard across Main+Fn, enforceable writer-version migration, read-only reconciliation after uncertainty, conditional explicit restoration, no silent global fallback, and one-shot first delivery followed by required rich Key Logic. These were proposals at review time; the current guide distinguishes the subsequently implemented boundaries from pending physical acceptance and rich input. They are not claims of unanimous reviewer agreement or accepted hardware behavior.

The owner subsequently approved one common onboard baseline across all profiles while host actions vary by profile. Removing this profile's action retains the owned trigger; a profile without an action does not rewrite hardware; a separate Return to keyboard assignment previews affected profile references and restores conditionally. First replacement previews, storage fields, collision bookkeeping and codec validation remain engineering/design responsibilities. Unknown, paired and mode-changing replacement stays unavailable initially. See the implementation guide for current behavior; the alternatives and staged exams here retain their original review context.

| Stage | Deliverable | Exit exam / stop condition |
| --- | --- | --- |
| 1. Truthful view | Observed Main/Fn assignments with host overlays, scope and current limitations | Prefilled lighting and unknown records display without writes; old adapters without label metadata remain usable; live Fn+T Launch is preserved |
| 2. Bounded input feasibility | Compare Tao-owner integration with isolated input/provider routing; prove current collection attribution | Configured alias press and release attribution, duplicates, repeat and background behavior examined in a controlled authorized test; ambiguity or registration competition stops this stage |
| 3. Ownership and migration design | Versioned physical-source ownership, preserved legacy bindings, conflict states and durable operation lifecycle | Isolated-data exams cover inactive-profile conflicts, unknown originals, disconnected owners, capacity exhaustion, old-writer rejection, downgrade and every crash boundary; no migration SET |
| 4. Narrow automatic host assignment | One-shot host actions through verified aliases, with logical single-edit block preservation | One physical press gives one action; unrelated records survive byte-for-byte; failure/restart keeps recovery visible; external same-record edits survive restoration attempts; profile switches cause no keyboard writes |
| 5. Category expansion | Separately declared native mouse, Consumer, combination and allowlisted functions | Encoding and readback plus category-specific physical behavior and app-stopped/power-cycle exams pass within an explicit device/firmware/transport envelope |
| 6. Required rich input | Hold/release and shared Key Logic integration after physical-source proof | Original-source release/cancel, held profile switch, disconnect, overflow and backend change exams pass; no standalone Fn claim |

Physical exams in this table are future acceptance gates, not authorization already exercised by this review. No builds, registrations, physical device operations or user-data mutations were performed by the review swarm.

Resource pressure is local: twelve initial aliases per verified physical keyboard, recovery records, bounded event queues, and engineering effort across the adapter, core persistence, Windows input and editor. There is no demonstrated cloud, quota or LLM-token operating cost for this feature. A delivery date or person-day budget is not established; event feasibility and the ownership migration must be bounded before estimating write-enabled delivery. Stage 1 can be accepted separately and should not wait for all native action categories. Final design work must add explicit file ownership, APIs, migration ordering and test assertions after these gates and interaction policies are settled.
