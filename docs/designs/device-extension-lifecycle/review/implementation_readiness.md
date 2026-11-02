Implementation Readiness — BRAINSTORM. Review complete. Three Major migration/rollout risks; no Blocker. These are concept acceptance gates, not requests for packet formats or implementation task lists.

1. **Major — Migration & Backward Compatibility: define the saved-state and downgrade boundary.**

   **Scope:** `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md:42–64, 74, 100–104`.

   The brief promises preserved assignments/configuration, recoverable device-support removal and incremental N4 migration, but does not establish how existing saved state crosses the new descriptor/source/identity boundary, or what happens when an older DeckWeave build opens the upgraded database.

   **Evidence:** `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs:21–50` supports database version 1 and rejects other versions; `:64, 71–80` loads and rewrites the typed configuration JSON. `.../model.rs:245–259, 293–345` contains the current device and preference structures, including existing disabled plugin IDs. `.../lib.rs:80–82` validates the complete configuration on startup; `:1437–1449` rejects provider IDs beyond virtual/N4 and other hardware identity formats. An older build therefore cannot open saved IO devices, and additive JSON fields alone do not ensure downgrade safety because an older writer can discard unknown fields.

   **Migration requirement:** Before implementation acceptance, commit to preserving existing N4 instance/profile/binding identities and disabled-package intent, and to retaining installed-but-unavailable provider data without preventing application startup. Choose a documented rollback policy: for example, a durable pre-migration backup plus a format/version gate preventing older builds from rewriting upgraded state. Require upgrade, missing-provider restart and downgrade/recovery acceptance cases. The exact schema belongs to detailed design.

2. **Major — Migration & Backward Compatibility: removal needs a durable interrupted-operation policy.**

   **Scope:** brief `:42–50, 64, 74, 100`.

   “Stop before changing files” and persisted exclusions describe the successful path, but not the outcome if stopping fails, file deletion fails, or the application terminates between file changes and saving the suppression record. Because lower-priority scan roots remain available, an interrupted removal can reactivate the package the user removed or leave its retained state inconsistent with the UI.

   **Evidence:** `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/catalog.rs:41–59` selects the first discovered package for an ID. `.../plugin_manager.rs:232–241` currently pauses, publishes and reloads; `:265–279` restores the old directory when the replacement rename fails. That existing file rollback does not provide a durable transaction spanning the proposed uninstall/source suppression and configuration state.

   **Migration requirement:** State the invariant that once removal/disconnection is durably accepted, no scan/restart may execute the affected package until explicit re-enable/reinstall. Require recoverable operation state and a visible pending/failed outcome for interrupted removal; establish whether a stop failure aborts removal or enters an explicit recovery state. Include crash/failure cases around suppression persistence, file changes and startup rescanning. Storage ordering and journal implementation can remain downstream decisions.

3. **Major — Migration & Backward Compatibility: recovery must survive IO removal and provider uninstallation.**

   **Scope:** brief `:50, 52, 62, 64, 103`.

   Reusing IO ownership/restore rules is appropriate, but the new device/provider removal lifecycle needs an explicit exception for unresolved temporary input takeover. Stopping a process or excluding a device does not itself restore the physical keyboard. An uninstall can remove the executable capable of recovery, while device exclusion can prevent the reconnect path from offering it.

   **Evidence:** `G:/APPS/io_type_84/modules/exclusive-depth.md:70–87` persists a synchronized recovery journal before changing stock assignments, deletes it only after successful restoration, and documents that crash/USB loss has no hardware lease timeout. Recovery after reconnect blocks ordinary writes/new takeover. `G:/APPS/io_type_84/AGENTS.md:54–57` likewise warns that crash/USB loss has no hardware TTL. `G:/APPS/io_type_84/crates/io-platform/src/device.rs:332–335` performs best-effort monitor shutdown on drop; that is not assignment restoration.

   **Migration requirement:** Gate exclusive input support on a recovery path that remains available after device removal, disabled support, update and interrupted uninstall. Define the user-visible result when restoration cannot complete: preserve the recovery record outside package-owned files, expose pending recovery despite the normal discovery exclusion, and retain or provide the compatible recovery implementation. Removal may still stop action execution immediately. This requirement does not apply to ordinary persistent keyboard settings, which the brief correctly says removal must preserve.

The proposed capability gating for stock lighting versus host compositions, the separate LED panel, preserved Elgato action protocol, and reuse of Mirabox command builders are appropriately bounded at concept level. I found no need to demand their detailed protocol implementations before concept acceptance.

Read manifest — every required file fully read:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/implementation_readiness.md` — full, lines 1–62.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/zoom-out-standard.md` — full, lines 1–60.
- `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md` — full, lines 1–110.
- `G:/APPS/DW/deckweave/docs/PROVIDER-CONTRACT.md` — full, lines 1–20.
- `G:/APPS/io_type_84/AGENTS.md` — full, lines 1–82.
- `G:/APPS/io_type_84/crates/io-platform/src/device.rs` — full, lines 1–538.
- `G:/APPS/io_type_84/docs/INTEGRATION.md` — full, lines 1–180.
- `G:/CODEX/companion-surface-mirabox-stream-dock/src/streamdock.ts` — full, lines 1–304.
- `G:/CODEX/companion-surface-mirabox-stream-dock/src/protocol.ts` — full, lines 1–118.

Additional evidence: full reads of IO `docs/STATUS.md` and DeckWeave `crates/deckweave-core/src/store.rs`; targeted reads/searches of the cited Core model/validation, plugin installer/catalogue/host, and IO ownership/service/module files. Strictly read-only; no builds, edits, hardware or application operations.
