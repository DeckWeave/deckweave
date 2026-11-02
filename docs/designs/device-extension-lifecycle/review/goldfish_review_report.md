# Goldfish Review Report

**Mode:** BRAINSTORM, independent concept review. **Date:** 2026-11-02.

**Target:** [original concepts brief](G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md), lines 1–110. The exact reviewed bytes are preserved in [reviewed_target.txt](reviewed_target.txt), SHA-256 `46F53C2B85164233DC5C6A54181BED18671969518DF9FE655B8968733B00C574`. All target line references below refer to that snapshot, even if the main brief is revised later.

## Reviewer status and isolation

All six requested lenses returned actual final reports. Each was a general-purpose native subagent created with `fork_turns: "none"`, given its installed absolute persona path, the target, the common [zoom-out standard](C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/zoom-out-standard.md), and its selected sources. The agents directory filenames/frontmatter were read before dispatch. Every reviewer reports that all mandatory files were read fully; individual manifests are retained verbatim below. The host exposes the final reviewer reports, not nested tool transcripts, so read status is reviewer-attested rather than independently replayed by the synthesizer. No missing read or failed reviewer was counted as clean.

Only four concurrent slots were available, including root and synthesizer. Consequently no more than two Goldfish ran at once: design/codebase-details first, zoom-out/comprehension next, implementation-readiness/structural last. A finished slot could be reused without waiting for its companion. No reviewer received another reviewer's findings. No retries were needed. The synthesizer wrote only this review directory; reviewers were read-only and did not run builds, Git operations, hardware operations, apps, history or memory retrieval.

| Lens | Actual native agent ID | Completion | Required reads | Verbatim report |
| --- | --- | --- | --- | --- |
| egm-design | `/root/egm_device_review/design` | completed | all complete, attested | [design.md](design.md) |
| egm-codebase-details | `/root/egm_device_review/codebase_details` | completed | all complete, attested | [codebase_details.md](codebase_details.md) |
| egm-zoom-out | `/root/egm_device_review/zoom_out` | completed | all complete, attested | [zoom_out.md](zoom_out.md) |
| egm-comprehension | `/root/egm_device_review/comprehension` | completed | all complete, attested | [comprehension.md](comprehension.md) |
| egm-implementation-readiness | `/root/egm_device_review/implementation_readiness` | completed | all complete, attested | [implementation_readiness.md](implementation_readiness.md) |
| egm-structural | `/root/egm_device_review/structural` | completed | all complete, attested | [structural.md](structural.md) |

## Prioritized findings

**Deduplicated result: 0 Blockers, 12 Majors, 4 Nits.** All reviewers found sufficient objective/context and supported the broad direction. Their objections concern concept decisions and acceptance gates; they do not require wire formats, packet recipes, exact database tables or code-level task lists at this stage. The proposed correction artifact [concepts_brief.md](concepts_brief.md) incorporates the defaults below. It has not itself received an independent review and is not an accepted implementation design.

### M01 — Physical recovery must survive removal — Major

- **Provenance:** design finding 1; implementation-readiness finding 3.
- **Scope:** target lines 50–64, 103–104; IO exclusive-input lifecycle, uninstall, disable, update and crash recovery.
- **Evidence:** [IO ownership rules](G:/APPS/io_type_84/modules/exclusive-depth.md), lines 70–88, retain a durable journal until successful restore and explicitly lack a hardware lease. [IO device](G:/APPS/io_type_84/crates/io-platform/src/device.rs), lines 332–335, best-effort monitor drop is not assignment restoration.
- **Discrepancy:** preserving profiles and stopping the provider does not preserve a working physical recovery route. A removed package may contain the only recovery executable or journal.
- **Correction:** the host owns recovery obligations and data outside removable package directories. Stop action execution immediately, request restoration before ordinary teardown, and retain a compatible recovery implementation until obligations are resolved. Offline/failed restoration remains visibly pending and accessible despite normal discovery exclusion. Never label logical removal as physical restoration. Recovery identity must be explicitly verified before sensitive writes.

### M02 — Durable lifecycle intent must govern crash/restart reconciliation — Major

- **Provenance:** design finding 2; implementation-readiness finding 2; codebase-details finding 1.
- **Scope:** target lines 42–50, 64, 74, 100.
- **Evidence:** [catalogue](G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/catalog.rs), lines 41–59, collapses IDs to first source; [installer](G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugin_manager.rs), lines 231–281, publishes/rolls back directories without a transaction spanning suppression and retained configuration.
- **Discrepancy:** a successful file operation plus later suppression persistence can reactivate a lower-priority copy after a crash. Current errors do not explain interrupted stages.
- **Correction:** retain all candidate sources plus host-owned operation intent. Once removal/disconnect is durably accepted, gate the package ID across every root before teardown/file mutation; restart reconciliation preserves that gate. Clear only on explicit reinstall/reconnect (or Enable for a built-in disabled component). Serialize competing operations per package. Stop failure leaves blocked/pending recovery and prevents destructive file cleanup; file failure leaves blocked/cleanup pending. Keep bounded local stage/outcome history in diagnostics. Storage machinery belongs to design.

### M03 — Ambiguous physical identity needs explicit rebind behavior — Major

- **Provenance:** design finding 3.
- **Scope:** target lines 46, 50, 59, 72, 103.
- **Evidence:** [IO open](G:/APPS/io_type_84/crates/io-platform/src/device.rs), lines 47–90, rejects multiple matches and constructs model/protocol information without establishing per-unit uniqueness; [device model](G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs), lines 245–258, stores an opaque optional hardware key.
- **Discrepancy:** open-by-identity does not prove two identical devices can restore profiles/exclusions correctly across reconnects.
- **Correction:** verify the actual identity capability before automatic independent reattachment. Where ambiguous, require explicit identify/rebind; never silently attach saved profiles, recovery records or onboard writes by model or USB path. Current-session handles may distinguish simultaneous devices without promising persistent identity. Restrict automatic recovery until identity is proven.

### M04 — Lighting needs one host authority and a concrete workspace — Major

- **Provenance:** design finding 4; zoom-out finding 1; comprehension finding 1; structural finding 2; codebase-details finding 3.
- **Scope:** target lines 48, 54, 59, 62, 80, 103.
- **Evidence:** target explicitly defers precedence. [Existing notification output](G:/APPS/io_type_84/modules/notification-sources.md), lines 36–48, already uses a worker, token rejection and restoration rather than composition layers.
- **Discrepancy:** serialized transport alone cannot choose among firmware mode, manual edits, preset action and automation or decide restoration after Stop.
- **Correction/default:** device-owned named presets; action profiles reference IDs rather than copy preset content. Device editor tabs are Actions, Lighting and Device settings. Lighting owns its full keyboard canvas, stock-mode controls, key-group selection and preset list; Device settings links there. Opening reads without writes. Active manual editing takes an explicit host edit lock; competing action/automation requests are rejected with a visible reason and never replayed later. Outside the lock, the most recent explicit eligible request owns that output group; a temporary request has a bounded token/lifetime and returns to the latest valid base when stopped. A completed manual edit/preset selection updates the base immediately; persistent hardware fields commit at a clear interaction boundary, with pending/error state and no generic Apply step. Closing ends the edit lock while keeping the chosen state until a new explicit request. Profile switches affect lighting only through an explicit preset association and cancel temporary requests owned by the departing profile. Show active source and Return to manual. Provider loss cancels temporary ownership; reconnect does not blindly replay it. All writes go through this authority. Restoration recomputes the current valid base rather than replaying a stale snapshot.

### M05 — Stock lighting and unproven host compositions need separate acceptance outcomes — Major

- **Provenance:** zoom-out finding 2; design finding 4.
- **Scope:** target lines 54, 80, 103–104.
- **Evidence:** [INTEGRATION](G:/APPS/io_type_84/docs/INTEGRATION.md), lines 53, 88, 119–120, and [current status](G:/APPS/io_type_84/docs/STATUS.md), lines 10–18, 37–39, distinguish stock settings, candidate RAM output and unavailable panel API. [IO device](G:/APPS/io_type_84/crates/io-platform/src/device.rs), lines 179–199, limits the candidate to 1–14 keys; accepted whole-keyboard atomicity is not established.
- **Discrepancy:** unconditional completion language can imply that unverified streaming is already a feasible whole-keyboard composition engine.
- **Correction:** first deliver stock whole-keyboard modes, supported static multi-key arrangements and named presets. Retain host compositions as an explicit requested outcome with a separate physical capability gate: visible color, address coverage, supported backgrounds, sustained transport, release and restoration. Do not claim whole-keyboard atomicity from six 14-key batches. Keep the supported workspace useful if this gate fails; report the conditional outcome as unresolved rather than silently declaring all compositions complete. No layer/timeline engine or assumed panel output.

### M06 — Shared processes need per-instance service guarantees — Major

- **Provenance:** design finding 5.
- **Scope:** target lines 59, 62, 102.
- **Evidence:** [provider contract](G:/APPS/DW/deckweave/docs/PROVIDER-CONTRACT.md), lines 12–18, requires ordered edges and latest-frame replacement; serialized small IO payloads amplify animation traffic.
- **Discrepancy:** a bounded FIFO can delay another instance or the close/recovery operation required for safe removal. A process crash affects all its sessions.
- **Correction:** per-instance queue ownership/fairness, cancellation/close/recovery priority, class-specific overflow behavior, bounded restart/backoff. Preserve digital edges; overflow fails/resynchronizes an affected session instead of silently coalescing edges. Establish throughput/latency measurements before picking animation rates and degrade output first. Document common process-failure scope while preserving sibling isolation for ordinary close/removal.

### M07 — Provider failures need bounded host-owned diagnostics — Major

- **Provenance:** codebase-details finding 2; settings-result portion of finding 3.
- **Scope:** target lines 48, 58–63, 102–103.
- **Evidence:** [action host](G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs), lines 519–521 and 563–565, discards subprocess output; lines 1219–1235 silently discard invalid JSON. [hardware diagnostics](G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/hardware.rs), lines 211–215, correctly distinguish HID transfer from display acknowledgement.
- **Correction:** bounded/rate-limited local diagnostics covering pre-registration failure, process exit, malformed/stale traffic, queue overflow, cancellation and recovery. Correlate operation/provider/instance/generation internally; retain last-setting transport/readback/recovery outcomes. Keep implementation IDs and counters behind existing diagnostics, with concise actionable state in ordinary cards/forms. A successful transfer is not physical acceptance. No remote telemetry service is required.

### M08 — Keyboard diagnostics need a capture boundary — Major

- **Provenance:** codebase-details finding 4.
- **Scope:** target lines 52, 59, 63, 102–103.
- **Evidence:** [hardware worker](G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/hardware.rs), lines 33–40, 211–223, 723–740, retains/exports raw packet and input traces; its settings redaction is a useful but incomplete keyboard policy.
- **Correction:** default diagnostics contain operational metadata/aggregate counts, not ordinary key streams, typed text, raw identity/path or secrets. Any raw depth/input capture is explicit, bounded, visibly active and controlled on export. Verify redaction in mock-provider tests. This is an extension of the existing diagnostic facility, not a new monitoring product.

### M09 — Preserved configurations need a visible restoration surface — Major

- **Provenance:** comprehension finding 2.
- **Scope:** target lines 46, 50.
- **Evidence:** [Registry](G:/APPS/DW/deckweave/apps/desktop/src/Registry.svelte), lines 8–20, 45–46, currently lists configured devices and the existing create operation.
- **Correction:** Add device includes Discovered, Saved/Removed, and Create virtual device. Restore saved instance preserves its IDs/profiles; physical restoration may remain offline or support-missing until the correct provider and identity are available. Removed virtual devices restore without hardware. Distinguish Restore from creating another instance and expose Forget saved settings separately.

### M10 — Catalogue must connect to the existing distribution flow — Major

- **Provenance:** zoom-out finding 3.
- **Scope:** target lines 23, 40, 46, 90, 104–106.
- **Evidence:** [registry README](G:/APPS/DW/plugin-registry/README.md), lines 3–5, 35–41, describes metadata-only development entries and no published archives; DeckWeave already has OpenDeck/ZIP installation.
- **Correction:** use the DeckWeave registry for reviewed typed entries, label existing OpenDeck action sources, and route compatible entries through the current install preview/managed lifecycle. Development workspace entries are not public install buttons. A generated self-contained provider must install from file before registry publication. Validate release metadata/archive/checksum locally; public publication remains a separately authorized operation, not implied by this concept review.

### M11 — Shared repository needs discriminated package contracts — Major

- **Provenance:** structural finding 1.
- **Scope:** target lines 58, 90, 106.
- **Evidence:** [registry schema](G:/APPS/DW/plugin-registry/schema/registry.schema.json), lines 3, 27–36, 55–68, 87–92, is action-only with mandatory controllers and `.sdPlugin` workspace paths.
- **Correction:** shared metadata plus explicitly typed action/device entries, separately validated package/source requirements. Keep registry format version, action compatibility and device process protocol version distinct. Host installation ownership controls deletion rights; catalogue source metadata does not. Reuse SDK/template repository tooling without reusing the action context as a hardware contract.

### M12 — Saved-state migration and downgrade policy are required — Major

- **Provenance:** implementation-readiness finding 1.
- **Scope:** target lines 42–64, 74, 100–104.
- **Evidence:** [store](G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs), lines 21–50, 64–80, version-gates and rewrites typed JSON; [Core validation](G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs), lines 1437–1449, currently admits only virtual/N4 providers.
- **Correction:** preserve N4 instance/profile/binding IDs and disabled-plugin intent. Allow unavailable providers as retained inert records without preventing startup. Before incompatible writes create a durable pre-migration backup and advance a format gate that old builds already reject; old writers must not rewrite upgraded state. Document rollback using the backup. Acceptance covers upgrade, missing-provider restart, identity preservation, interrupted migration and downgrade recovery.

### N01 — Repository source operation is implicit — Nit

- **Provenance:** comprehension finding 3; target lines 24, 42–44.
- **Correction:** managed → Uninstall/Reinstall; Elgato → Disconnect/Reconnect; repository checkout → Disconnect/Reconnect; host built-in → Disable/Enable. Never delete source checkout/Elgato files. This mapping is included in the proposed revision.

### N02 — Reuse execution gating and cancellation — Nit

- **Provenance:** zoom-out finding 4; target lines 42–50, 59, 64, 100.
- **Evidence:** existing `disabled_plugins` and [Core cancellation](G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs), lines 201–214, 1176–1194.
- **Correction:** extend those primitives with source/exclusion/dependency semantics rather than building parallel execution gates or synthesizing release events during removal.

### N03 — Keep settings metadata narrow — Nit

- **Provenance:** zoom-out finding 5; target lines 60–61, 68.
- **Correction:** first contract handles common scalar/choice fields and reusable control/group selection; concrete host Lighting workspace handles keyboard composition. Defer advanced editor hooks until IO proves an unmet requirement. No permanently running per-device WebView or universal UI language.

### N04 — Handoff status and SDK stabilization need alignment — Nit

- **Provenance:** structural finding 3; target lines 5, 98, 102–110; [NEXT](G:/APPS/DW/deckweave/docs/NEXT.md), lines 11, 21.
- **Correction:** main handoff must stop saying owner readiness is unconfirmed. Stage-three SDK/generator is provisional and mock-tested; freeze/publication follows the real IO adapter. Root reports updating NEXT and DECISIONS after all six reviews completed; those changes were not part of the reviewed target. The synthesizer did not edit them because its ownership is restricted to this review directory.

## Recommended disposition

Keep the architecture direction and owner requirements. Resolve the listed technical defaults in the concept, then write the implementation design with migration, interrupted-operation, recovery and capability acceptance gates. Do not reopen the confirmed readiness or stack/firmware questions. The owner need only align on genuinely new product tradeoffs if later evidence forces a scope change; hardware observations and provenance/licence checks remain evidence gates. The revised synthesis is a proposal informed by these six reviews, not a claim of review approval for its own text.
