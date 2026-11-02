# Goldfish Review Report

**Target:** `G:/APPS/DW/deckweave/docs/designs/keyboard-assignment-execution/working_hypothesis.md` (lines 1–80)

**Mode:** BRAINSTORM. **Date:** 2026-11-02. **Disposition:** continue design clarification; automatic hardware programming is not ready for release. The findings assess a concept, not compliance with a finished implementation plan. Blockers below are gates before write-enabled implementation/release, not reasons to stop read-only presentation work.

## Reviewer Status

| Reviewer | Status | Actual persona | Supplied standards | Raw report |
| --- | --- | --- | --- | --- |
| `egm-design` | completed | `agents/design_critic.md` | Feature specification; Heilmeier | [Design](reviewer_design_raw.md) |
| `egm-codebase-details` | completed | `agents/codebase_details.md` | Feature specification | [Codebase](reviewer_codebase_raw.md) |
| `egm-zoom-out` | completed | `agents/zoom_out.md` | Zoom out | [Zoom out](reviewer_zoomout_raw.md) |
| `egm-comprehension` | completed | `agents/comprehension.md` | Feature specification | [Comprehension](reviewer_comprehension_raw.md) |
| `egm-implementation-readiness` | completed | `agents/implementation_readiness.md` | Implementation readiness; Feature specification | [Migration/readiness](reviewer_readiness_raw.md) |
| `egm-structural` | completed | `agents/structural.md` | Feature specification | [Maintainability](reviewer_structural_raw.md) |

Personas and standards resolve to the installed bundle `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/`. Every reviewer returned its own required-read manifest and explicit completed status; source reads are marked full, targeted or search-only in the raw reports. All six actual completions were received before synthesis. No reviewer failed or needed retry; none is counted as a clean result without a report.

Concurrency was capacity-aware: three batches of two independent reviewers, ordered design/codebase, zoom-out/comprehension, readiness/structural. Each batch partner was launched without waiting for the other review. Four global slots include the root and synthesizer, leaving two reviewer slots. This was not six-way simultaneous execution. Every reviewer used `fork_turns: "none"`, read its actual bundled persona, and received only the target, standards, explicit relevant source paths and task constraints. Later reviewers did not receive earlier findings or root follow-up evidence. No prior chats or memory were supplied.

The target was not modified during review. Reviewers were read-only: no builds, physical device operations, input registration or user-data mutation. The synthesizer wrote only this report, the concept brief and raw reviewer artifacts. The root's separate evidence files are distinguished below. Source references capture reviewer reads in a shared workspace; they do not imply that concurrent source drafts are deployed or accepted.

## Prioritized Findings (Blockers -> Major -> Nits)

Deduplicated independent-lens findings: **3 Blockers, 8 Majors, 0 standalone Nits**. Overlapping findings retain the highest reported severity. Raw reports remain unchanged, including their lower-severity observations and unresolved question lists. The separate root follow-up below adds an integration constraint; it is not counted as a seventh review or silently folded into raw findings.

### 1. Physical ownership and legacy profile bindings are different domains - Severity: Blocker

* **Reviewer Lens:** `egm-structural`, `egm-implementation-readiness`, with design/codebase/zoom-out support.
* **Affected Scope:** target lines 44–50, 68, 76.
* **Discrepancy Description:** The desired alias survives profile changes and action deletion, but current `Profile.fn_bindings` contains both trigger and action. Core allocation scans active profiles and the editor separately allocates with an F13 fallback on exhaustion. This is not an inventory of actual physical sources. Legacy profiles may disagree; a saved Fn+T Launch is not evidence that hardware was programmed. A pre-alias original might never have been recorded.
* **Evidence:** `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs:240–288`; `fn_actions.rs:3–21,108–128`; `G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte:248–268`; `G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs:328–334`.
* **Rationale & Recommendation:** Propose an independent, versioned physical keyboard/control/layer ownership model with one allocator and profile references. Migrate host configuration without SET; preserve all profiles and live Fn+T Launch. Distinguish saved references, observed output, proven ownership and conflict. Do not invent historical originals or silently adopt unrelated F13 mappings. This model is not implemented by this review.

### 2. Older format-2 writers can discard new ownership data - Severity: Blocker

* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`.
* **Affected Scope:** target lines 48–50, 78.
* **Discrepancy Description:** Current typed saves serialize configuration back into one JSON value. The old-writer guard recognizes format 2, not a new ownership capability within format 2. Adding fields without an enforceable writer boundary allows older code to save away alias provenance and original records while the keyboard continues emitting aliases.
* **Evidence:** `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs:90–99,131–156`.
* **Rationale & Recommendation:** Require a writer-version migration and pre-migration backup before ownership becomes durable. Test old-writer rejection with isolated data. Document that restoring a database or executable does not undo keyboard changes; rollback must reconcile both host and hardware state.

### 3. One user gesture spans independent durable systems - Severity: Blocker

* **Reviewer Lens:** `egm-implementation-readiness`, `egm-structural`, with design/codebase support.
* **Affected Scope:** target lines 21, 48–50, 65.
* **Discrepancy Description:** Host configuration, ownership reservation, driver backup/pending files and onboard writes cannot be one atomic transaction. Existing host updates and driver readback do not connect every crash boundary. A failure after SET but before confirmation is different from a failure before SET. Deriving these outcomes from current selection or a few UI flags creates misleading readiness and unsafe reuse.
* **Evidence:** `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs:140–151`; `G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs:294–339`; `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/device_providers.rs:2331–2336`; `G:/APPS/DW/device-io-type-84/src/main.rs:188–263`; `G:/APPS/DW/device-io-type-84/src/recovery.rs:249–264`; `G:/APPS/io_type_84/docs/PROTOCOL.md:90–96`.
* **Rationale & Recommendation:** Propose a durable operation identity and explicit stages linking reservation, host save, original bytes, driver intent, write/readback and activation. Retain reservations and recovery obligations after uncertainty. Startup/reconnect reads and reconciles without replaying an uncertain SET. Test every interruption boundary before automatic programming.

### 4. Historical ownership outlives currently trustworthy device identity - Severity: Major

* **Reviewer Lens:** all architecture, codebase, migration and maintainability lenses; comprehension identifies the user-facing failure.
* **Affected Scope:** target lines 44, 50, 52–54, 68.
* **Discrepancy Description:** The current descriptor exposes session confidence and no persistent ID. Model/VID/PID or a vanished opaque instance cannot establish that an old original record belongs to a newly connected identical keyboard. Vendor identity, Windows input collection pairing and historical ownership are separate relationships.
* **Evidence:** `G:/APPS/DW/device-io-type-84/src/model.rs:99`; `recovery.rs:17–53,156–200`; `G:/APPS/io_type_84/docs/PROTOCOL.md:5–7`.
* **Rationale & Recommendation:** Preserve separate logical identity, current connection generation, input collection set and confidence. Ambiguity retains reservations and stops routing, programming and restoration pending reconciliation. The post-review inventory narrows current-session collection uncertainty, as described below, but does not close durable identity or event acceptance.

### 5. Generic recovery does not establish conditional alias restoration - Severity: Major

* **Reviewer Lens:** design, codebase, zoom-out, migration/readiness and maintainability.
* **Affected Scope:** target lines 48–50, 68.
* **Discrepancy Description:** `merge_restore` preserves unrelated records but copies originals into historically changed slots without checking that their current values still equal the recorded targets. A preview token prevents changes after preview; it does not protect an external same-record edit made before preview. Deleting an action proves neither ownership nor physical release of an alias.
* **Evidence:** `G:/APPS/DW/device-io-type-84/src/recovery.rs:391–410,638–643`.
* **Rationale & Recommendation:** Define a separate alias-release rule comparing exact original, installed and currently observed bytes under confirmed identity. Restore only the still-owned record; preserve divergence as conflict. Read back removal before freeing capacity. Reuse backup/readback infrastructure without claiming that its existing explicit recovery workflow already implements this policy.

### 6. Broad action categories cross capability and dependency boundaries - Severity: Major

* **Reviewer Lens:** design, codebase, zoom-out, comprehension and maintainability.
* **Affected Scope:** target lines 17–21, 27–38, 69.
* **Discrepancy Description:** The driver and host currently accept a one-field keyboard-usage write form. Adding mouse, Consumer, combinations and functions needs coordinated capabilities and validation. The transport writes full changed blocks, so “only that assignment” is a logical preservation guarantee. Prototype replacement can clear paired SOCD/RS records, while the current driver rejects those replacements. Decode, encode, byte verification and physical acceptance are distinct claims.
* **Evidence:** `G:/APPS/DW/device-io-type-84/src/model.rs:127–194`; `main.rs:221–227`; `G:/APPS/DW/deckweave/crates/deckweave-core/src/provider_settings.rs:521–565`; `G:/APPS/DW/deckweave/apps/desktop/src/types.ts:143–151`; `G:/APPS/io_type_84/crates/io-platform/src/changes.rs:41–79,197–207,507–547`; `G:/APPS/io_type_84/docs/PROTOCOL.md:90–100`.
* **Rationale & Recommendation:** Propose adapter-owned typed capabilities with readable, assignable, dependency and evidence distinctions. Reuse narrow codecs and tests, not the whole prototype editor. Keep selected unknown, paired and mode-changing records unavailable for replacement initially. Verify complete block diffs preserve unrelated bytes; do not promise single-record atomic writes or physical behavior from readback.

### 7. Device input and held-action semantics need a lifecycle contract - Severity: Major

* **Reviewer Lens:** design, codebase, zoom-out, migration/readiness and maintainability.
* **Affected Scope:** target lines 52–56.
* **Discrepancy Description:** The current source supplies one-shot alias numbers and core explicitly rejects Key Logic. A different collector does not make `activate_fn` a Down/Up route. Repeat, held profile switches, lost release, overflow, disconnect, backend handover and possible duplicate collections need defined ownership. Running two backends risks duplicate launches.
* **Evidence:** `G:/APPS/DW/deckweave/crates/deckweave-platform/src/shortcuts.rs`; `G:/APPS/DW/deckweave/crates/deckweave-core/src/fn_actions.rs:73–90,108–193`; `model.rs:681–697`; `G:/APPS/DW/deckweave/modules/runtime.md`; `modules/providers.md`; `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/keyboard_shortcuts.rs:102–105,163–171`.
* **Rationale & Recommendation:** Keep the first new slice one-shot and device-paired. Rich Key Logic remains a required final outcome after physical-source proof; it is not removed from scope. Reuse shared source/binding release and cancellation rules before enabling it. Filter configured aliases before host dispatch, retain no unrelated typing, and do not infer standalone Fn events. No silent global fallback or concurrent routes for one binding. The separate root follow-up adds the process registration-owner constraint below.

### 8. Saved configuration, readback, route availability and physical acceptance are different outcomes - Severity: Major

* **Reviewer Lens:** design, codebase and comprehension; all lenses retain the physical-evidence limit.
* **Affected Scope:** target lines 21, 38, 48–54.
* **Discrepancy Description:** Readback confirms stored representation, not source pairing, physical dispatch, one-action-per-press, power-cycle persistence or all action forms. Software Test does not test physical input. Current provider and shortcut statuses are separate and can collapse recovery into disconnected, making partial failure hard to explain after restart.
* **Evidence:** `G:/APPS/DW/deckweave/apps/desktop/src/FnBindingSetup.svelte:69–140`; `DeviceEditor.svelte:550–560,1188–1192`; `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/keyboard_shortcuts.rs:17–23,137–155,254–256`; `G:/APPS/DW/device-io-type-84/src/main.rs:247–260`.
* **Rationale & Recommendation:** Define explicit saved-only, writing/verifying, stored-mapping-verified, host-route-available, uncertain/recovery and identity/conflict outcomes with next actions. Correlate bounded diagnostics with an operation and verified generation. Preserve visible uncertainty after reopening. Keep category-level physical acceptance separate from per-operation byte verification and software tests.

### 9. Execution labels do not explain profile scope or removal effects - Severity: Major

* **Reviewer Lens:** design, comprehension, zoom-out, migration/readiness and maintainability.
* **Affected Scope:** target lines 13, 17–21, 44–50, 56–58.
* **Discrepancy Description:** A hardware assignment viewed under a Work profile can be mistaken for Work-only behavior. Retaining an alias after deletion creates a meaningful unbound physical state that must not look empty. A profile with no host action cannot restore native behavior without breaking the stable mapping proposal. Immediate drag-and-drop will now have a physical replacement consequence. “Closed” is also ambiguous because closing editor windows leaves runtime in the tray.
* **Evidence:** `G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte:657–670,922–926,1195–1198,1303–1309`; `FnBindingSetup.svelte:174–178`; `G:/APPS/DW/deckweave/modules/editor.md`; `G:/APPS/DW/deckweave/README.md`.
* **Rationale & Recommendation:** Show execution location and edit scope. Propose removal of only the selected profile action, a visible retained-trigger state, and separate conditional Return to keyboard assignment with affected profile references previewed. A profile without an action does not secretly inherit, restore or rewrite hardware. Make the first native replacement intentional and explain app-stopped behavior before writing. The central remaining interaction-policy choice is a common onboard baseline across profiles versus profile-specific physical behavior; the former is proposed, not user-approved.

### 10. Twelve codes require an explicit collision domain and ownership-based exhaustion UX - Severity: Major

* **Reviewer Lens:** design, codebase, comprehension, migration/readiness and maintainability; overlaps a zoom-out Nit.
* **Affected Scope:** target lines 50, 56, 68.
* **Discrepancy Description:** The hypothesis does not explicitly resolve installation-wide versus device-local uniqueness. Global shortcut conflict detection covers active routes only. Hidden retained/disconnected owners can exhaust capacity while the current profile shows few actions. Counting each inactive-profile reference separately would overcount one physical source. Raw Input attribution does not make a global fallback device-specific.
* **Evidence:** `G:/APPS/DW/deckweave/crates/deckweave-core/src/fn_actions.rs:3–21`; `G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte:261–267`; `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/keyboard_shortcuts.rs:227–247`.
* **Rationale & Recommendation:** The root selected twelve aliases per verified physical keyboard across Main+Fn after the independent reviews. Count each keyboard/control/layer once, preserving disconnected and unresolved reservations plus observed/requested mappings and inactive-profile references within that keyboard. Reuse across positively identified keyboards requires the new device-specific route. Missing pairing stops activation/allocation; installation-wide uniqueness cannot replace identity. Legacy global references remain unresolved until reconciled, and global routes must be retired before source handover. Reject exhaustion without a fallback code. Installation-wide uniqueness was considered and rejected for imposing an app-wide ceiling. This is a root design direction, not unanimous lens agreement or implemented behavior.

### 11. Broad destination lacks independent stage gates, explicit alternatives and delivery uncertainty - Severity: Major

* **Reviewer Lens:** design, zoom-out and comprehension, supported by readiness/maintainability staging proposals.
* **Affected Scope:** target lines 15–58, 74–80.
* **Discrepancy Description:** Read-only presentation, new onboard codecs, ownership migration, automatic programming and a new Windows source have different evidence requirements. The hypothesis primarily describes a preferred direction without a consolidated alternatives comparison. Resource effort, dates and measurable exams remain unestablished. This is normal unfinished brainstorm work, but cannot be presented as an implementation-ready specification.
* **Evidence:** Current label projection offers an independent presentation slice (`G:/APPS/DW/device-io-type-84/src/model.rs:127–147`); target's own integration/evidence table and limitations; supplied Feature Specification, Heilmeier, Zoom-Out and Implementation Readiness standards interpreted for BRAINSTORM.
* **Rationale & Recommendation:** The concept brief proposes truthful display first, bounded input feasibility, ownership/migration, a narrow automatic host slice, category-specific expansion and required rich Key Logic after lifecycle proof. It compares manual/global/stable/per-profile approaches and identifies acceptance exams. It records that no calendar/effort commitment is established until feasibility is bounded. No need for a generic allocator framework, second action runtime or full prototype import is established.

## Root Follow-up After Independent Reviews

These observations were not included in reviewer prompts and do not alter the preserved raw reports.

**Current-session collection grouping observed.** The root's read-only Windows inventory found two current IO84 keyboard collections, both resolved by interface InstanceId -> devnode -> nonempty ContainerId to the present vendor MI_02 container. The synthesizer read the [sanitized result](../../evidence/2026-11-02/io84-input-identity.json), [probe source](../../evidence/2026-11-02/io84-input-identity-probe.ps1) and [root analysis](input-source-follow-up.md) only after all six completed. No input registration, event capture, HID open or write occurred. This reduces current-session pairing uncertainty. It does not establish collection emission/deduplication, Up/Down behavior, stable identity, identical-device differentiation, suppression, latency or physical host execution.

**Raw Input registration ownership is an additional integration constraint.** `G:/APPS/DW/deckweave/Cargo.lock:4065–4066` pins Tao 0.37.1. Installed `C:/Users/Wismut/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tao-0.37.1/src/platform_impl/windows/event_loop.rs:200,330` calls its registration helper; sibling `raw_input.rs:125–160` registers mouse and keyboard. The synthesizer inspected those source sections and verified Microsoft's primary [RegisterRawInputDevices contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices): only one target window per raw-input class in a process is registered; a later call supersedes it. A second independent desktop collector can therefore compete with Tao. This is not evidence that current DeckWeave input has failed.

**Proposed design requirement:** compare supported integration through the existing process registration owner with an isolated input/provider process. Preserve ordinary mouse/keyboard behavior and one authoritative owner per process/class. Do not choose between these alternatives solely from inventory, and do not silently add a second desktop registration. [RAWKEYBOARD](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawkeyboard) defines make/break flags, but its API contract is not physical IO84 acceptance.

## Synthesis and Handoff

The [concept brief](concepts_brief.md) follows the brainstorm skill's exact five-section structure and separates established evidence from proposed safeguards. The working hypothesis remains unchanged. Raw reviewer question lists remain available, but technical allocation, migration/versioning, validation and recovery choices are engineering work rather than an omnibus owner questionnaire.

The one central remaining interaction-policy choice is **adopting one common onboard baseline across all profiles while host actions vary by profile**. Proposed conservative behavior under that model: removing the current profile action retains the owned trigger; a profile with no action does not rewrite hardware; Return to keyboard assignment previews affected references and restores conditionally. Unknown, paired and mode-changing replacements remain unavailable initially. These are proposals for owner alignment, not additional approval already received.

No implementation, deployment or physical acceptance is established by this report. The automatic Fn programming draft remains gated on the ownership and execution model.
