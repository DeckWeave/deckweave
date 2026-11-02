What this proposes: Main and Fn become views of the keyboard’s actual stored assignments. Users choose an action in one place; firmware-supported actions execute on the keyboard, while host actions use an internally managed F13–F24 alias. Aliases belong to a physical control and onboard layer, so switching DeckWeave profiles changes the host action without rewriting the keyboard.

How the existing system works, per the document and inspected UI: The current Fn view displays profile host bindings, leaving native assignments absent from that view. Creating a host binding and programming its Fn output are separate operations. `FnBindingSetup.svelte` exposes F-code selection, hardware assignment, and global-shortcut limitations. The unified workflow and proposed ownership safeguards are not established as implemented.

All findings below have dimension **UX & Developer Ergonomics**. They assess a brainstorming hypothesis, not a final implementation plan.

### Findings

**1. Major — Profile scope can remain misleading despite execution-location labels.**
Affected target: “Candidate interaction,” lines 17–21; “Updated hypothesis,” lines 44–46 and 58.

“Keyboard” versus “DeckWeave” explains where an action executes, but does not fully explain where an edit applies. The same view sits below an existing profile selector. A user editing an onboard assignment while viewing “Work” could reasonably expect that assignment to belong to Work. Under this hypothesis, it also changes the physical behavior used with Streaming and with DeckWeave stopped. The existing editor reinforces profile context through its toolbar and profile-wide Fn explanation (`DeviceEditor.svelte`, lines 657–670 and 922–926).

Ergonomic requirement: Show assignment scope at the point of editing: onboard edits affect the keyboard across DeckWeave profiles; host action edits affect the selected profile. Explain what an empty binding in another profile does. Location and scope must both be understandable without knowing alias internals.

**2. Major — Removal reserves ownership correctly but leaves the user-visible result undecided.**
Affected target: lines 48–50 and 68.

Keeping an alias after a host action is removed prevents unsafe reuse. It does not answer what “Remove” means to the user. Possible results include a key still emitting an F-code with no action in this profile, a key running actions in other profiles, or restoration of its previous native behavior after a separate operation. The current interface uses generic Remove/Delete controls (`DeviceEditor.svelte`, lines 1195–1198 and 1303–1309); its setup component explicitly warns that removal does not restore the previous onboard assignment (`FnBindingSetup.svelte`, lines 174–178).

Ergonomic requirement: Distinguish removing the current profile’s action from restoring keyboard behavior. A key with a retained trigger and no current action must remain visible as a meaningful state, rather than appearing simply unassigned. Restoration must explain effects on other profiles.

**3. Major — “One user operation” lacks a clear outcome when only part succeeds.**
Affected target: lines 21 and 48–50.

The hypothesis rightly separates saved configuration from hardware readiness. It does not describe what users see when configuration saves but hardware writing fails, hardware changes but verification times out, or navigation/restart interrupts the operation. “Recovery state” is an internal concept unless it tells users what is known, what may still happen on a physical press, and what they can do next. Automatically restoring or retrying would also conflict with the stated protection against external edits and settings replay.

Ergonomic requirement: Define a small set of understandable outcomes covering saved-only, writing/verifying, confirmed mapping, uncertain mapping, disconnected, and recovery required. Each non-ready outcome needs a next action and an explicit statement about the keyboard’s known or uncertain behavior. Preserve that explanation after reopening the editor.

**4. Major — “Ready” must not imply physically accepted functionality.**
Affected target: lines 21, 38, 48, and 52.

Readback can establish that the requested stored mapping is present. It cannot establish that every codec form executes correctly, that the input interface was paired correctly, or that the full host action works on physical input. The hypothesis acknowledges those limits, but “confirmed hardware readiness” can still collapse several different claims. The current Test action deliberately runs software and explicitly says that it does not verify physical input (`DeviceEditor.svelte`, lines 550–560 and 1188–1192).

Ergonomic requirement: Separate “stored mapping verified,” “host route available,” and “physical behavior checked” in the conceptual model. Do not require a complex diagnostic dashboard; simply avoid awarding broader readiness than the available evidence supports. A successful software Test must never clear a hardware-input problem.

**5. Major — The replacement gesture needs an understandable boundary.**
Affected target: lines 17, 21, and 48.

Dragging Launch onto Fn+T will now change the keyboard, rather than merely save application configuration. The hypothesis says the UI “must show” replacement, but leaves timing and user control unclear. This matters especially for existing lighting functions, transport/OS mode controls, unknown assignments, and ordinary Main-layer keys. A label displayed after the write would not help users understand the consequence before it occurs. The established editor otherwise saves assignments immediately without Apply (`modules/editor.md`, line 5).

Ergonomic requirement: Make the native behavior being replaced, cross-profile scope, and closed-app consequence legible before the hardware change commits. Choose an intentional lightweight interaction for this boundary, consistent with immediate editing. Unknown native records need an explicit replacement policy, not just preservation during edits elsewhere.

**6. Major — Alias scarcity becomes a hidden prerequisite unless surfaced through the workflow.**
Affected target: lines 50 and 56.

Automatic allocation removes unnecessary F-code setup, but twelve aliases cover twelve independent control/layer assignments, not twelve actions per profile. Reservations can persist after action removal, and unrelated preexisting mappings consume or obstruct capacity. Users may therefore hit exhaustion with few visible current actions. “Conflicts and limitations translated into actionable errors” is the correct direction but not yet a user model.

Ergonomic requirement: Explain exhaustion using controls and retained keyboard assignments. Offer a discoverable list of reserved controls, reasons for retention, and safe restoration/release options. Do not direct users to choose a different F-code manually. State whether the initial twelve-code limit applies across connected devices, all known devices, or another explicit allocation scope.

**7. Major — Unsupported, unknown, and unavailable actions need different treatment.**
Affected target: lines 17–19 and 29–38.

The proposed list places native action categories beside host actions, while several categories are currently read-only and physical execution remains unaccepted. A user must be able to tell “this existing assignment is recognized but cannot yet be written” from “this record is unknown” and “this action requires DeckWeave.” Otherwise the unified list can recreate the original false-readiness problem.

Ergonomic requirement: Distinguish readable, writable, unavailable, and unknown states using ordinary language. Preserve readable native behavior even when its editor is unavailable. Explain device capability restrictions at selection time. Do not silently substitute host execution for a native request: that changes its closed-app behavior.

**8. Major — Closed windows, stopped runtime, and reconnect are different user states.**
Affected target: lines 13, 52–56, and 65.

“With DeckWeave closed” is ambiguous in this product: the README says closing all windows leaves the runtime in the tray (`README.md`, line 29). The hypothesis later uses the clearer “DeckWeave stopped.” Reconnect also raises a workflow question: no settings replay is permitted, but the user needs to know whether an existing host route has been re-established or a mapping discrepancy was detected.

Ergonomic requirement: Consistently distinguish closing the editor, exiting DeckWeave, and disconnecting the keyboard. Explain that host-bound keys continue emitting ordinary F-codes when the runtime stops. On reconnect, make observed mismatch and route availability visible without implying an automatic rewrite.

**9. Nit — Input pairing is a prerequisite with no user-facing resolution path yet.**
Affected target: lines 52–54.

Raw Input evaluation is appropriately tentative. If pairing the vendor interface to the keyboard input interface cannot be proven automatically, the user could see a connected device with working onboard edits but no usable host actions. That is not adequately explained by a generic connected/disconnected indicator.

Ergonomic requirement: Keep hardware connection and host-input identification separate. An unresolved pairing must produce an actionable explanation and must not enable misleading ready status. Exact pairing mechanics can remain a later design question.

### Proposed mitigations

These are proposals, not implemented safeguards:

- Add two concise assignment attributes: execution location and edit scope.
- Specify a user journey from observed native assignment through replacement, write/readback, host-route availability, and recovery.
- Give “Remove current profile action” and “Restore keyboard assignment” distinct meanings and consequences.
- Keep retained aliases visible through their physical controls; expose capacity management only when needed.
- Define capability and verification vocabulary before drafting detailed editor layouts.
- Compare viable alternatives explicitly: profile-specific onboard rewriting, stable aliases, and host-only assignment without automatic hardware programming. The supplied feature-spec standard requires documented alternatives before a specification passes; the current hypothesis already gives reasons against parts of these paths but does not present a complete comparison.

### Unresolved user decisions

1. What should the default removal operation do, especially when other profiles use the same control?
2. What should a host-bound key do in a profile without an action: remain inert apart from its F-code, inherit an action, or another explicit policy?
3. What interaction should precede replacement of a native assignment, including unknown or mode-changing functions?
4. Are shared onboard mappings across all DeckWeave profiles the accepted default?
5. How should the editor present action forms that can roundtrip stored bytes but lack accepted physical execution evidence?

### Read-file manifest

Required and fully read:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/comprehension.md`
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md`
- `G:/APPS/DW/deckweave/docs/designs/keyboard-assignment-execution/working_hypothesis.md`, lines 1–80

Additional fully read:

- `G:/APPS/DW/deckweave/README.md`
- `G:/APPS/DW/deckweave/modules/editor.md`
- `G:/APPS/DW/deckweave/apps/desktop/src/FnBindingSetup.svelte`
- `G:/APPS/DW/deckweave/apps/desktop/src/keyboard-action-errors.ts`

Targeted read:

- `G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte`: keyword search plus lines 1–110, 210–275, 458–580, 635–710, 788–863, 915–1005, 1185–1217, and 1260–1312.

Instruction discovery: checked ancestor locations and searched the relevant docs/apps/modules subtrees for `AGENTS.md`; none found in those locations. No prior chats or memory retrieved. No files changed, builds run, physical devices accessed, hotkeys registered, or user data mutated.

**Status: Completed — read-only UX and ergonomics brainstorm review.**
