Structural-Brainstormer report — BRAINSTORM mode

The concept has sufficient context for review and a sound architectural direction. It separates action packages, device support and physical instances; preserves the action protocol; centralizes hardware communication in providers; and limits reuse of IO code to transport and codecs. No Blocker found. The following are concept-stage maintainability risks, not demands for a completed protocol specification or PRD.

1. **Major — Long-Term Tech Debt & Maintainability: shared registry needs a clear package-type boundary.**

   Scope: extension identity, catalogue validation and SDK/template maintenance.

   Evidence: `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md:58` requires distinct action/device protocols; lines 90 and 106 retain shared SDK/template/registry repositories with separate interfaces or generator modes. However, `G:/APPS/DW/plugin-registry/schema/registry.schema.json:3` identifies the current registry as an action registry; lines 55–68 require `controllers` on every entry, lines 87–92 restrict controllers to Keypad/Encoder/Panel, and lines 27–36 allow workspace sources only under action `.sdPlugin` directories.

   Rationale: sharing repositories is sensible, but extending this schema by adding optional device fields would couple unrelated package contracts and create entries whose valid interpretation depends on combinations of absent fields. The current action-only source path also cannot simply serve as a generic device source. Over time this makes validators, generators, installers and catalogue presentation diverge.

   Maintainability requirement: explicitly commit to typed action/device registry entries with shared package metadata and separately validated payload/source requirements. Keep registry format version, action compatibility and device protocol compatibility distinct. The brief needs this boundary and compatibility policy; detailed field definitions can wait for design. Preserve the package ownership record as an installed-host concern rather than inferring deletion rights from catalogue source metadata.

2. **Major — Long-Term Tech Debt & Maintainability: lighting needs one host-owned authority model before multiple execution paths are implemented.**

   Scope: lighting workspace, presets, automation and provider communication.

   Evidence: the brief’s lines 52 and 82 keep action execution in the host and exclude a second automation engine. Line 54 introduces firmware modes, per-key edits, host-rendered compositions, reusable presets and manual/action/automation precedence, explicitly leaving ownership unresolved. Lines 59 and 62 give providers independent instance sessions and serialized writes.

   Rationale: serialized writes prevent simultaneous transport access, but do not decide which source is entitled to control a device or zone. If the advanced editor, action-triggered preset and composition renderer independently issue writes, the eventual behavior becomes a collection of last-writer rules. A firmware mode can overwrite a composition, or stopping an action can restore an obsolete manual snapshot. That couples UI lifetime, profile state and provider recovery.

   Maintainability requirement: establish one host-owned per-device lighting state and authority mechanism. Store reusable lighting presets independently from action profile contents; profiles/actions may reference them. Manual selection and automation submit requests through that common owner; providers report supported output paths and apply the selected state. Define the conceptual behavior on composition stop, disconnect and provider restart, including which desired state survives. Resolve the precedence rule in the next design step before building competing output paths. This does not require a layer/timeline engine.

3. **Nit — Long-Term Tech Debt & Maintainability: current handoff documents disagree about readiness and SDK sequencing.**

   Scope: documentation ownership and staged implementation.

   Evidence: the brief’s lines 5, 98 and 110 record confirmed readiness for independent review, whereas `G:/APPS/DW/deckweave/docs/NEXT.md:11` still states that readiness has not been confirmed. `NEXT.md:21` puts stabilization against the second physical adapter before release of external SDK/templates; the brief’s lines 102–104 put SDK/generator work before IO and packaging afterward.

   Rationale: the staging difference is compatible if the early SDK is provisional, but that qualification is implicit. These competing handoff statements can cause later work to repeat the readiness checkpoint or treat a mock-validated contract as settled before IO exercises it.

   Mitigation: update NEXT’s status and explicitly call the stage-three contract/generator provisional until the IO adapter validates the boundary. Keep publication/freeze after that validation. No additional owner approval is needed.

Read manifest — all mandatory reads completed:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/structural.md` — full read.
- `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md` — full read, lines 1–110.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/zoom-out-standard.md` — full read, lines 1–60.
- `G:/APPS/DW/deckweave/docs/DECISIONS.md` — full read, lines 1–37.
- `G:/APPS/DW/deckweave/docs/NEXT.md` — full read, lines 1–25.
- `G:/APPS/DW/plugin-registry/schema/registry.schema.json` — full read, lines 1–111.

Additional inspection was limited to a targeted read-only search for cancellation/release behavior in Core and plugin-host source. No edits, builds, Git operations, hardware operations or application interactions were performed.
