Zoom Out / BRAINSTORM review of `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md`

Assessment: the root problem and motivation are sufficiently clear. Action-package removal, physical-device removal and device-support removal are correctly separated. A distinct external provider contract is justified by the stated requirement for independently installable third-party hardware support. Extending the existing SDK/template/registry repositories, retaining the working Mirabox transport and extracting narrow IO modules are sound simplifications. No Blocker found. Three Major concept decisions remain before an implementation design can be accepted.

1. **Major — Contrarian / YAGNI & Code Minimization: lighting has a workspace location but no authoritative ownership model.**

   Scope: concepts brief lines 54, 80 and 103.

   Evidence: line 54 explicitly leaves precedence among manual selection, an action-triggered preset and automation unresolved. `G:/APPS/io_type_84/modules/notification-sources.md:36–48` already has a narrower output model: one worker, selected slots, known static backgrounds, token rejection, Stop restoration and last-active-indicator precedence on a slot. It explicitly lacks composition layers.

   Rationale: named presets alone do not establish whether they belong to a device, an action profile or a temporary runtime session. Implementing the UI before settling this will spread competing state across settings, profiles, actions and the provider, potentially creating the second automation engine that the brief excludes. This is a concept-level decision, not a wire-protocol detail.

   Concrete mitigation: define device-local named lighting presets independently from action profiles. Let an action reference a preset rather than copy its contents. Describe one minimal temporary-override rule, its visible active owner and what happens on Stop, profile switch, removal, provider loss and reconnect. Keep arbitration in DeckWeave; the provider executes the selected hardware operation. A suitable initial workspace is one keyboard canvas with a selected group, a stock-mode panel and a preset list; host composition is a separate capability-gated mode. Defer arbitrary layers, blending and timelines until a real scenario requires them.

2. **Major — Contrarian / YAGNI & Code Minimization: explicitly separate the deliverable stock-lighting workspace from the conditional host-composition implementation.**

   Scope: concepts brief lines 54, 80 and stages 4–5 at lines 103–104.

   Evidence: `G:/APPS/io_type_84/docs/INTEGRATION.md:53` supports the stock settings direction; lines 88 and 119–120 distinguish the candidate RAM output path and unavailable panel setter. More current evidence in `G:/APPS/io_type_84/docs/STATUS.md:10–18,37–39` records successful transport, but leaves visible color confirmation and composition with stock animations open. `G:/APPS/io_type_84/modules/notification-sources.md:36–54` restricts current host output to 1–14 keys and static modes 1/20, rejects stock-animation backgrounds, and states that visible RGB remains unaccepted. `G:/APPS/io_type_84/crates/io-platform/src/device.rs:179–199` enforces the same narrow frame limit and calls the operation a candidate.

   Rationale: the brief correctly acknowledges the verification gate, but its unconditional in-scope phrase “whole-keyboard modes/compositions” can still make completion of the entire feature depend on an unproven host output mechanism. A working stock-mode editor and an editor for hypothetical whole-keyboard compositions have substantially different runtime requirements. The latter cannot be obtained merely by exposing more descriptor fields.

   Concrete mitigation: retain the requested composition direction, but state separate acceptance outcomes. Deliver whole-keyboard stock modes, supported static group editing and reusable presets first. Make physical host compositions conditional on an explicit capability-acceptance result covering visible output, address coverage, background behavior and release/recovery. An unavailable capability should leave the supported workspace useful. Do not imply that streaming six batches of fourteen keys proves atomic whole-keyboard output. Packet sizes and scheduling belong to the later design; this review needs only the honest product boundary and staging.

3. **Major — Contrarian / YAGNI & Code Minimization: decide how the existing optional registry actually feeds the proposed Catalogue.**

   Scope: concepts brief lines 23, 40, 46, 90 and 104–106.

   Evidence: `G:/APPS/DW/plugin-registry/README.md:3–5` says the repository is metadata-only and changing it neither installs nor enables an extension. Lines 35 and 41 say current workspace entries are development-only and there are no published archive entries. Lines 39–41 define the existing release-archive/checksum contract. `G:/APPS/DW/deckweave/README.md:55` already directs users to ZIP installation and the OpenDeck catalogue. The brief proposes Device support catalogue links and registry completion without identifying the authoritative catalogue connection.

   Rationale: retaining one repository avoids duplicate infrastructure, but does not by itself create the advertised end-to-end distribution flow. Leaving this connection implicit risks implementing a second catalogue/downloader alongside the existing installation UI or exposing development-only paths as installable products.

   Concrete mitigation: choose a minimal catalogue policy in the brief: the DeckWeave registry supplies reviewed action/device entries; existing OpenDeck action sources remain identifiable; both feed the existing installation preview and managed-install lifecycle where compatible. Permit install-from-file for a generated provider before any public registry release exists. Define stage 5 completion as validating and installing a real self-contained provider archive; require a published entry only if publication is in the accepted release scope. Registry schema fields, fetching and archive validation details belong to the subsequent design.

4. **Nit — Reuse opportunity: make the existing disable and cancellation paths explicit starting points.**

   Scope: concepts brief lines 42–50, 59, 64 and stage 1 at line 100.

   Evidence: persistent `disabled_plugins` already exists in `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs:305,318–355`. `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs:650–663,974–987` detects enable-state changes and gates disabled packages. `G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs:201–214` cancels actions by editor source or device; lines 1176–1194 emit cancellation rather than ordinary release events.

   Rationale: there is already a useful core for “stop executing this package/device without deleting configuration.” A parallel lifecycle manager with a separate disabled-state store and cancellation mechanism would duplicate established behavior.

   Concrete mitigation: add a short reuse constraint: extend the existing package execution gate and action cancellation primitives. Source ownership, explicit removal/exclusion and unavailable-assignment metadata supply the missing lifecycle semantics. Existing enable state alone is insufficient for safe uninstall, so preserve the distinction between logical execution state and managed-file ownership. Package-specific nested dependency traversal and stop ordering remain design work.

5. **Nit — Reuse opportunity: keep settings metadata narrower than a universal editor schema.**

   Scope: concepts brief lines 60–61 and 68.

   Evidence: `G:/APPS/DW/plugin-sdk/README.md:25–37` already contains action-context lifecycle and disposal conventions while explicitly excluding hardware registration. `G:/APPS/DW/plugin-template/README.md:13–17` already establishes standalone generation, deliberate SDK snapshots and packaging conventions. The brief appropriately rejects both a universal UI framework and importing the whole IO application.

   Rationale: the strongest simplification is to reuse repository tooling and lifecycle conventions while separating contracts. Reusing the action-context object or growing metadata into a complete keyboard application language would blur ownership.

   Concrete mitigation: make the first device settings contract cover common scalar/choice fields and reusable control-group selection; implement the concrete Lighting workspace in the host. Add an advanced extension editor boundary only when the IO adapter demonstrates a requirement that these host surfaces cannot express. Do not require that hook as a prerequisite for the first external provider.

Concept decisions to settle now: lighting preset ownership and override behavior; the conditional acceptance boundary for host compositions; the authoritative catalogue/distribution path.

Appropriate details for the later implementation design: protocol framing and version negotiation; identity persistence and ambiguity recovery; dependency traversal; shutdown timeouts and process supervision; exclusion storage; descriptor and settings schemas; packaging formats and validation tests. The brief already identifies these risks at an appropriate altitude and does not need code-level helper functions or packet recipes.

Read manifest — every caller-required file was read completely:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/zoom_out.md` — full read, lines 1–74.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/zoom-out-standard.md` — full read, lines 1–60.
- `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md` — full read, lines 1–110.
- `G:/APPS/DW/plugin-sdk/README.md` — full read, lines 1–37.
- `G:/APPS/DW/plugin-template/README.md` — full read, lines 1–17.
- `G:/APPS/DW/plugin-registry/README.md` — full read, lines 1–51.
- `G:/APPS/io_type_84/AGENTS.md` — full read, lines 1–82.
- `G:/APPS/io_type_84/docs/INTEGRATION.md` — full read, lines 1–180.

Additional evidence: full reads of `G:/APPS/DW/deckweave/README.md` and `G:/APPS/io_type_84/modules/notification-sources.md`; targeted reads/searches of current IO status, IO transport/core declarations, Mirabox command builders, DeckWeave catalogue/installer/plugin runtime and Core cancellation/model code.

Review was read-only. No files, builds, applications, hardware or repository history were changed or executed.
