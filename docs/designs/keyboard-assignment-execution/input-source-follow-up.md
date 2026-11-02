# Input source follow-up

Date: 2026-11-02. This records the read-only inventory before implementation, not a seventh independent Goldfish review. No input collector was enabled during that inventory. The subsequent isolated-provider implementation and its separate acceptance boundary are described in the [current guide](../../KEYBOARD-ASSIGNMENTS.md).

## Current-device inventory

The read-only Windows probe found two IO84 Raw Input keyboard collections. Both resolve through the interface's device instance to the same nonempty ContainerId as the current MI_02 vendor-control devnode. See the [sanitized result](../../evidence/2026-11-02/io84-input-identity.json) and [probe source](../../evidence/2026-11-02/io84-input-identity-probe.ps1).

The probe only enumerates devices and reads properties. It does not register for input, capture keys, open HID handles or write settings. It emits aggregate counts, without raw interface paths, instance IDs, container IDs or serials.

This supports current-session collection pairing. It does not establish which collection emits a physical F13–F24 event, whether both can report the same press, release behavior, identity after a port change, or differentiation between two identical IO84 units. A missing, changed or ambiguous pairing must remain unavailable until reconciled. VID/PID alone is not sufficient evidence of a physical source.

Microsoft documents the [ContainerId grouping property](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-containerid) and the device-interface name returned by [GetRawInputDeviceInfo](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getrawinputdeviceinfoa).

## Registration ownership in the desktop process

The pinned [Cargo.lock](../../../Cargo.lock) includes Tao 0.37.1. Its installed Windows source calls `register_all_mice_and_keyboards_for_raw_input` during event-loop initialization and when changing the device-event filter (`event_loop.rs`, lines 200 and 330). `raw_input.rs`, lines 125–160, implements this by registering both mouse and keyboard collections.

Microsoft states that [only one window per raw-input device class can receive registration within a process](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices); the last registration supplies the target. Therefore a second independent keyboard registration inside the desktop process could replace Tao's registration or be replaced by it.

This is an integration constraint, not evidence that existing DeckWeave input has failed. The proposed route must share the existing process registration owner or use an isolated input/provider process. It must not rely on two unrelated windows repeatedly taking registration ownership.

## Requirements for the next design

- Keep one authoritative input owner per process and class. Preserve ordinary typing and mouse input.
- Match current collection handles to the confirmed vendor session, including a set of collections where necessary; do not assume exactly one.
- Forward only owned aliases. Do not retain unrelated keyboard events or payloads.
- Define repeat and duplicate-collection handling, plus Down/Up/Cancel ownership across profile changes, source loss and overflow. [RAWKEYBOARD](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawkeyboard) defines make/break events; that API definition does not replace physical IO84 verification.
- Keep session identity, persistent logical device identity and historical alias ownership distinct.
- Do not silently activate a global RegisterHotKey fallback when device pairing fails.
- Treat F13–F24 as ordinary OS-visible keyboard output. This inventory does not prove or implement suppression in other programs.

The remaining physical test should target emitted configured aliases and lifecycle behavior. Repeating accepted ordinary typing or Mirabox tests does not answer these new questions.
