# Keyboard assignments: onboard first, host when needed

Date: 2026-11-02. Status: working hypothesis for EGM clarification, not an approved design or completed Goldfish review.

## Problem

The editor currently confuses two things: the keyboard's stored Main/Fn assignments and DeckWeave action bindings. It renders only the latter in its Fn view. The native adapter already reads the complete Main and Fn blocks; this is a presentation gap, not evidence that a different Fn layer was read.

Dragging Launch onto Fn+T currently saves a host action without programming the internal trigger. The visible assignment therefore looks ready while physical Fn+T still performs its existing onboard action. Exposing a separate F13–F24 setup step does not match the user's expected workflow.

## User intent

Use the keyboard's own execution wherever its firmware can represent the requested behavior. Those assignments must keep working with DeckWeave closed. Use DeckWeave for arbitrary program launch, plugins, external conditions, timers and other behavior the device cannot store or execute. Preserve the actual prefilled Fn layer, including device lighting functions.

## Candidate interaction

One Main/Fn assignment view shows the current onboard behavior immediately after a successful read. Selecting a key opens its existing assignment. Unknown records remain visible as unknown and survive edits to other keys.

The action list exposes keyboard keys, combinations, mouse buttons/wheel, media/browser keys and supported device functions alongside DeckWeave actions. Each assignment shows its execution location in ordinary language: "Keyboard" or "DeckWeave". The device adapter declares which action forms it can encode; the app does not guess from a key's position or action title.

Selecting a supported onboard action writes only that assignment with backup and readback. Selecting a host action configures its trigger as part of the same user operation. Internal F-codes are implementation details, with conflicts and limitations translated into actionable errors. The UI must distinguish saving the host configuration from confirmed hardware readiness. A failed or uncertain hardware write must never leave a misleading ready state.

Standard Calculator/Mail/media usages are onboard outputs even though the operating system ultimately handles them. An arbitrary executable path is a host action. A firmware lighting-function assignment is different from a conditional host lighting notification.

## What the source actually supports

| Area | Existing prototype | Current DeckWeave adapter |
|---|---|---|
| Main/Fn stored blocks | Complete 128 by 4-byte records per layer | Already read and preserved |
| Ordinary keyboard usages | Writer and readback path | Writable now |
| Mouse buttons and wheel | Typed codec and block writer | Read-only pending integration |
| Consumer/media/browser keys | 16-bit Consumer codec | Read-only pending integration |
| Modifier combinations | Up to two modifiers plus keyboard usage | Read-only pending integration |
| Device lighting/Win lock functions | Narrow allowlist | Read-only pending integration |
| Macros/DKS/advanced pairs | Separate tables and constraints | Not part of the current mapping writer |
| Arbitrary program/plugins/conditions | Host execution | General action runtime exists |

The prototype's stored-byte roundtrip evidence does not establish physical execution for every mouse, media or function assignment. Factory reset and transport/OS-mode changes are not ordinary automatically writable actions. The factory-reset function may be decoded for display without enabling its execution or assignment.

Source references: `G:/APPS/io_type_84/src/features/editor/model.ts`, `G:/APPS/io_type_84/crates/io-platform/src/changes.rs`, `G:/APPS/io_type_84/docs/PROTOCOL.md`, `G:/APPS/io_type_84/docs/EDITOR_IMPLEMENTATION.md`, `G:/APPS/io_type_84/modules/notification-sources.md`.

## Updated hypothesis after the user's F13–F24 proposal

Use F13–F24 as stored trigger aliases for actions that need the host. An alias belongs to a physical keyboard control and its onboard layer, not to a DeckWeave profile or action. Changing the profile changes the action resolved by that trigger, without rewriting the keyboard. The common onboard Main/Fn configuration remains stable. This is the proposed default, not a claim of user approval for all profile behavior.

Example: IO84 Fn+T stores F13 once. The Work profile maps that source to an editor launch; the Streaming profile maps the same source to an OBS action. A native Fn+Up brightness assignment remains an onboard function and does not need an alias or the running app.

The editor should choose and manage aliases internally. Dragging a host action onto a key must not require a separate technical F-code setup step. It must show when replacing an existing native assignment, save its exact original record, and report readiness only after the hardware mapping has been read back. An uncertain write keeps its recovery state rather than claiming success.

Keep alias ownership across profile switches and removal of app actions. Release an alias for reuse only after the old physical mapping is verified to no longer emit it. Include observed mappings, saved/requested mappings, inactive profiles and known other connected devices in allocation. Do not treat an unrelated preexisting F13 mapping as a free DeckWeave trigger. Never restore over a later external edit.

For Windows, prefer evaluating Raw Input as the event source for the assigned F-codes, rather than treating a global hotkey as the final device input contract. Microsoft documents that Raw Input can identify the source device. This may also provide output-key press/release events needed by the existing Key Logic runtime. It still requires proving how the selected IO vendor interface pairs with the keyboard input interface. It is not evidence that the physical Fn button itself emits events, and it does not by itself suppress the F-code in other applications.

The existing RegisterHotKey path is a limited fallback/prototype: it is global and gives one-shot activation. Do not register both paths for the same binding or count one physical activation twice. Neither path should record ordinary typing. A Raw Input implementation must filter to the paired keyboard and configured aliases before forwarding input, and must not retain unrelated input data.

The first alias pool has twelve codes. More than twelve independent host-controlled key/layer assignments needs a separately verified extension. Do not silently reuse codes or construct arbitrary modifier combinations that could also affect ordinary input. With DeckWeave stopped, native assignments continue normally; a host-bound key emits its assigned F-code but its DeckWeave action does not run. F13–F24 are ordinary keyboard outputs and can be used by other programs; they are not an invisible private device protocol.

Profile-specific rewriting of onboard mappings is a later explicit feature with its own change/recovery policy. It is not needed for switching host actions in this proposal.

Primary references: [Microsoft Raw Input overview](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input), [RAWINPUT source device information](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinput), [RegisterHotKey semantics](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey).

## Constraints already established

- No firmware replacement, calibration or factory reset.
- No automatic settings replay on device open or reconnect.
- Host-only actions must not also emit the original onboard action unintentionally.
- Fn itself has no established standalone press/release event. The current Windows F13–F24 route is global and does not identify the originating keyboard.
- A stored trigger alias cannot be reassigned merely because its host binding disappeared or a profile changed. Another physical key may still emit it. Reserve and release ownership from actual observed/stored mappings, with a verified restoration path.
- The keyboard driver owns codecs and hardware transport. General user actions and conditions stay in the shared DeckWeave runtime.
- Temporary key lighting stays a separate RAM output. Its transport smoke passed with exact stored-state restoration; visible hue/blink and hardware timeout remain unaccepted.

## Current work state

The responsiveness fix is committed and measured. The initial Fn host-action and temporary-lighting slice is present in the main working tree; it is not a completed unified assignment editor. The running release still has the earlier manual F-code setup and must not be described as completing the new interaction.

The user created Fn+T Launch in the live app; preserve it. Do not delete it as test data. No Fn mapping was changed during this session's controlled RAM probe.

Read-only onboard labels have been added and tested in the IO source as `observed.onboardKeyLabels[controlId].main/fn`, without changing raw mappings or writers. That metadata is not deployed yet. A draft automatic Fn programming path remains paused and must not be released before alias ownership and the unified execution model are resolved.

EGM stage: the user confirmed the readiness checkpoint on 2026-11-02 with "Давай". Independent fresh-context multi-lens Goldfish review is now authorized. This authorizes review of the hypothesis; it does not establish that the proposed design is implemented or accepted. Existing implementation workers are not Goldfish reviewers.
