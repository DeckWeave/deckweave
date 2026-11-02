Read-only BRAINSTORM review completed. No Blocker found. The brief explains the objective and preserves the important distinctions between package uninstallation, assignment removal and device removal. Two concept-stage UX decisions need resolution before an implementation design is accepted.

1. **Major — UX & Developer Ergonomics: lighting ownership and workspace behavior.**

   **Scope/evidence:** `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md:54` proposes a Lighting workspace, reusable presets and separate stock/host-rendered modes, but explicitly leaves precedence between manual selections, action-triggered presets and automation unresolved. Line 48 also places lighting among device-setting groups.

   **Usability friction:** A user cannot yet predict whether changing an action profile changes lighting, whether an action permanently replaces their manual selection, or what returns after automation stops. This affects the visible product model, rather than merely internal implementation.

   **Ergonomic requirement/mitigation:** Define one concrete model in the revised brief. A workable starting point is: device-owned named lighting presets; a Lighting tab containing the keyboard preview, selection tools and mode-specific controls; Device settings links into that tab; action profiles reference presets rather than owning copies. State a precedence rule and the restoration behavior when an override ends, a profile changes or the provider disconnects. Show the currently active source and provide an explicit return-to-manual operation. Keep unavailable host-rendered modes out of the selectable options until their output path is verified. No timeline/layer engine is needed to settle these interactions.

2. **Major — UX & Developer Ergonomics: preserved device configurations have no defined restoration entry point.**

   **Scope/evidence:** Brief line 50 promises to preserve a removed device’s configuration “for explicit restoration.” Line 46 defines Add device as discovered, not-yet-added hardware plus Create virtual device. Those flows do not identify where a removed virtual instance or currently disconnected physical device can be found for restoration. Existing `G:/APPS/DW/deckweave/apps/desktop/src/Registry.svelte:8–13` derives its list from configured devices, and lines 18–20/45–46 expose only the existing add operation.

   **Usability friction:** Preservation is not discoverable if users can only create a new virtual instance or wait for physical hardware to be discovered. They may create duplicate instances and conclude that their old profiles were lost.

   **Ergonomic requirement/mitigation:** Give Add device a visible Saved/Removed devices section, or specify an equivalent recovery surface. Distinguish Restore saved device from Create new virtual device and identify the retained profiles/settings before restoration. Define what restoration does while hardware is offline and when the required extension is absent. The database schema and exact identity-matching algorithm can remain later design work.

3. **Nit — UX & Developer Ergonomics: map repository-supplied packages explicitly to a management operation.**

   **Scope/evidence:** Brief line 24 identifies managed copies, repository bundles and the Elgato installation as separate discovery sources. Lines 42–44 assign operations to managed, Elgato and built-in packages, but do not explicitly say whether a repository bundle is considered built-in or another external source. The actual roots are separately listed in `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs:47–52`.

   **Usability friction:** A developer testing a checkout cannot confidently infer which operation appears or how to reconnect a deliberately disconnected repository package.

   **Ergonomic requirement/mitigation:** Add a small source-to-operation mapping, including the reverse operation: managed → Uninstall/Reinstall; Elgato → Disconnect/Reconnect; repository bundle → explicitly chosen Disconnect/Reconnect or Disable/Enable; built-in → Disable/Enable. Preserve the already stated rule against deleting source checkout files.

The existing-removal statement is appropriately qualified. `PlanEditor.svelte:52–58` and `270–275` remove optional branches; lines 165–169 and 207–215 remove nested items while disabling removal of the final required item. These are discoverability and empty-container semantics to review, not evidence that nested removal is wholly missing. Likewise, the brief already specifies three separate Mirabox controls and applied-state honesty; neither needs to be raised again as an unanswered requirement.

**Required read manifest — all complete**

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/comprehension.md` — entire file read.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/zoom-out-standard.md` — entire file read.
- `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md` — entire file, lines 1–110.
- `G:/APPS/DW/deckweave/apps/desktop/src/Registry.svelte` — entire file, lines 1–121.
- `G:/APPS/DW/deckweave/apps/desktop/src/PluginManager.svelte` — entire file, lines 1–431.
- `G:/APPS/DW/deckweave/apps/desktop/src/PlanEditor.svelte` — entire file, lines 1–289.

Additional evidence was limited to targeted source searches and the cited plugin-root excerpt. No edits, builds, Git operations, hardware access or application execution were performed.
