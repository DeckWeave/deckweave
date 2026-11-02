Review outcome: the confirmed panel lifetime is coherent, but four migration and rollout risks should be carried into the Concepts Brief before implementation planning. No Blocker was identified at this hypothesis stage. The missing proposed APIs are expected future work, not implementation drift. The Implementation Readiness Standard’s zero-decision rule applies to a later implementation plan; I have not applied it as a completeness requirement to this concept.

Read manifest — successful reads:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/implementation_readiness.md` — entire persona, including BRAINSTORM instructions.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md` — entire standard.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/implementation-readiness-standard.md` — entire standard.
- `G:/APPS/DW/deckweave/docs/designs/host-feedback/working_hypothesis.md` — entire target, lines 1–89. SHA256 verified as `14B2854797631DF75411411AAE2B32E3E50117C2D09502B812611DB6762023ED`.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs` — lines 1–254.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs` — targeted searches and lines 360–378, 457–486.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs` — targeted searches and lines 770–875, 1188–1221.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/shell.rs` — targeted source search.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/plugin.mjs` — lines 1–118.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/timer.mjs` — lines 1–58.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/lib/sdk.mjs` — lines 1–16.
- `G:/APPS/DW/plugin-sdk/sdk.mjs` — targeted searches and lines 1–48, 173–212.
- `G:/APPS/DW/plugin-sdk/sdk.d.ts` — targeted source search.
- `G:/APPS/DW/plugin-sdk/PROTOCOL.md` — lines 1–40 and targeted source search.

Optional paths `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/contexts.rs` and `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/lifecycle.rs` were checked and do not exist at those locations. No alternative location was assumed. No private database, conversation, memory, prior review, or `concepts_brief.md` was read. No writes, builds, hardware actions, or UI automation were performed.

Implementation and rollout risks:

1. **Major — Migration & Backward Compatibility: persistent panel storage needs an explicit downgrade contract.**

   Affected target: “Desktop panel service,” lines 43–51; “Key Decisions & Staging,” lines 81–82.

   The hypothesis specifies what must survive restart, including closed state and versioned widget instances, but does not yet establish what happens when a user runs an older DeckWeave build after creating those records. This matters independently of plugin widget schema versions.

   The current store has explicit database and configuration format gates: database versions beyond 3 are rejected in `store.rs:103–107`, and configuration formats other than 3 are rejected at `store.rs:118–119` and `143–145`. It also preserves unknown JSON fields during compatible saves (`store.rs:176–229`). Thus neither “bump the format” nor “add fields without a bump” automatically yields the desired rollback behavior. The former can prevent the old application opening the configuration; the latter can retain panel records that the old build cannot display or manage.

   Migration requirement: state that installing the feature must preserve existing profiles, assignments and preferences, and specify a supported rollback outcome. Consider either an independently versioned panel store that older applications leave untouched, or an additive, versioned configuration section with a documented old-writer preservation contract. A format-breaking migration is also viable if it includes a verified backup and an explicit compatibility gate, but its effect on opening the rest of DeckWeave must be acknowledged.

   Concrete mitigation: test old configuration → upgraded application → create/edit/close panels → older application performs an unrelated profile edit → upgraded application. Verify retained content, closed intent, IDs and preferences. If rollback intentionally requires restoring a backup, make that limitation explicit rather than promising transparent downgrade.

2. **Major — Migration & Backward Compatibility: moving existing timers to host scheduling requires a single-authority handover.**

   Affected target: “Shared sound service,” lines 29–33; background registry at line 47; staging at line 81.

   The target correctly separates completion from drawing and requires host scheduling for hidden-page alarms. The migration hazard is the interval during which an existing timer has both its old plugin runtime and a new host job, or neither.

   Today, `plugin.mjs:20–24` saves timer runtime into action settings. Completion is detected in `draw()` at lines 26–30, and a visible context runs a 250 ms interval at line 59. Navigation persists runtime and creates a handoff at lines 61–65. `timer.mjs:47–52` serializes only status, value and startedAt; restoration converts an elapsed running countdown to done at line 25. Introducing a stable run ID or delivery marker solely into that runtime object would need care: the present serializer would omit it on its next write.

   Migration requirement: establish one authoritative owner for a running timer during upgrade, restart, navigation and rollback. Migration must preserve running/paused/done state and remaining duration, must not treat an imported expired timer as a new completion, and must not create two audible deliveries.

   Two viable alternatives are to migrate compatible running timers transactionally into host jobs, or to leave legacy runs on their previous path and enable host ownership only for a subsequent start or explicit opt-in. The latter simplifies handover but needs its temporary hidden-page limitation stated.

   Concrete mitigation: give each adopted run a stable identity and ownership/version marker outside replaceable legacy runtime where necessary; fence old writers after adoption; make job creation idempotent; commit adoption before relying on the new scheduler. Exercise upgrade while running, paused and overdue; navigation during adoption; plugin death during adoption; and rollback with a saved host job. The confirmed “no old sound after restart” requirement remains binding.

3. **Major — Migration & Backward Compatibility: advertised extensions need a mixed-version release sequence, including bundled SDK copies.**

   Affected target: current-code evidence at line 23; service registration at lines 47–51; staging at lines 81–82.

   Advertising an optional extension is the right compatibility boundary. The unresolved rollout issue is how independently updated host, SDK, template and plugin packages behave when only some have the extension.

   The timer imports `./lib/sdk.mjs` (`plugin.mjs:2`), and that local SDK copy exists with version `0.1.0`. Updating the sibling SDK repository alone therefore does not update this installed consumer. The current protocol explicitly says socket submission does not establish extension support (`PROTOCOL.md:19`). The current SDK also disposes visible contexts on `willDisappear` (`sdk.mjs:208–211`). A new consumer must not silently fall back to retaining those contexts when the background service extension is absent.

   Migration requirement: define supported combinations at concept level: new host with old plugins, new plugins with an old host, a restarted plugin with incompatible saved widget configuration, and SDK/template consumers that update later. Preserve legacy visibility semantics throughout.

   Alternatives include shipping host support first and enabling the bundled consumer later, or an atomic host-plus-bundled-plugin release with explicit minimum host requirements for separately distributed plugins. Both still need runtime capability negotiation because installed packages can outlive a host upgrade or rollback.

   Concrete mitigation: version capabilities separately for sound, panels and background timer jobs; check capabilities before registering services or showing related configuration; bound unsupported-call failure; keep ordinary timer controls functional when an optional service is unavailable. Include bundled SDK copies and templates in release validation, and retain saved widgets without callback execution when the connecting owner cannot support their version. A missing service capability must not be reported as a successful hidden-page alarm.

4. **Major — Migration & Backward Compatibility: restored inventory and returning plugins need a close/update conflict rule.**

   Affected target: lines 37, 43–51; acceptance at line 89.

   Stable identities, panel revisions and recorded close intent already address much of this risk. The remaining issue is the ordering between persistent close, asynchronous upsert and service reconnection. A delayed update from a previous connection, or a plugin that re-enumerates its saved instances after reconnecting, could otherwise reopen a panel the user closed. This would violate the explicit acceptance boundary even if every individual record deserializes correctly.

   Migration requirement: make durable user close authoritative over stale updates and restore/reconnect enumeration. State what an older or downgraded compatible plugin is allowed to do when it encounters a panel with newer revisions or closed state.

   Alternatives are a retained closed-state record/tombstone with revision checks, or explicit creation epochs so subsequent reopening creates a new authorized incarnation. Deleting the inventory record immediately is simpler but requires another mechanism to reject delayed upserts using the old identity.

   Concrete mitigation: persist close intent before acknowledging durable closure; reject updates from obsolete connection generations and revisions; distinguish an ordinary content refresh from an explicit reopen command; never replay button events during reconciliation. Test close concurrent with reconnect, close followed by application restart, delayed updates after close, and reinstall reclaiming a compatible but closed instance. A storage failure should leave the user with a truthful recoverable state rather than silently claiming that closure will survive restart.

Product questions, separate from implementation blockers:

- **Major — migration default for existing timers; target lines 33 and 63.** “Optional timer notification settings” does not explicitly say whether existing timer assignments acquire sound, persistent panels or hidden-page scheduling on upgrade. Recommend preserving current notification behavior unless the user opts in, while retaining existing running state. The question is the default for legacy assignments, not whether persistent panels survive profile changes; that behavior is already confirmed.

- **Nit — reopening intent; target lines 37 and 43.** Clarify which user action may intentionally reopen a closed persistent panel. An explicit reopen from the inventory or owning inspector is straightforward; ordinary periodic `upsert` should not imply the same consent. This product decision supplies the intent boundary needed by finding 4.

The concept already makes strong compatibility choices: profile-independent ownership, preserved static content when a plugin is absent, unavailable live widgets, fresh service registration, and no synthetic key replay. Those decisions should remain intact. The required additions concern safe transitions between versions and between old and new timer ownership, rather than reconsidering the owner’s confirmed panel behavior.
