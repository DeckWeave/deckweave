# Device and extension management

* **Author:** Codex, incorporating the owner's requirements, source audit and six independent EGM reviews.
* **Date:** 2026-11-02.
* **Status:** Proposed revision for owner alignment. The owner confirmed readiness for the independent review. All six reviews of the [frozen original](review/reviewed_target.txt) completed; their [consolidated report](review/goldfish_review_report.md) is separate. **This revised text has not itself been independently reviewed and is not an accepted implementation design.** No production code, hardware settings or running applications were changed by this review.

## Overview

DeckWeave needs a complete way to install, identify, configure, disable and remove extensions, with **action-plugin uninstallation as the primary removal scenario**, and to add, configure, remove and restore real or virtual devices. Removing an assignment from a control, deleting a nested scenario step, uninstalling an action package, removing device support and removing one device instance are separate operations with separate consequences.

The proposed user model has two extension types: **Actions** and **Device support**. They share installation and extension management. **Devices** lists independent physical or virtual instances, each with its own profiles, settings and editor. Installing one support package may discover several instances. The host owns the user interface, persistence, action execution and lighting arbitration; the provider owns discovery, hardware communication and verified hardware capabilities. A user should not have to understand process protocols to distinguish an installed support package from a connected keyboard.

IO Type 84 is the next physical adapter to validate an independently installable device contract alongside the existing Mirabox N4. It must preserve stock firmware and ordinary typing on discovery or editor opening. Rust/Tauri/Svelte, Windows first, independent instances, the host action engine, the existing Elgato action protocol and a common editor model remain owner requirements.

## Analysis

### Existing foundations and gaps

The [Registry](../../../apps/desktop/src/Registry.svelte) creates another Virtual Studio through its current Add operation. Device and plugin removal need new lifecycle operations; the editor's assignment Remove and [nested plan removal](../../../apps/desktop/src/PlanEditor.svelte) already exist and need discoverability/cancellation review rather than an assertion that they are absent.

The [model](../../../crates/deckweave-core/src/model.rs), [Core](../../../crates/deckweave-core/src/lib.rs) and [hardware worker](../../../apps/desktop/src-tauri/src/hardware.rs) retain built-in provider/geometry branches. Replace those incrementally with a descriptor registry, preserving N4 IDs, accepted input numbering, JPEG/page rendering and existing profiles. Reuse the existing package execution gate, `disabled_plugins` and cancellation primitives; add the missing ownership, dependency, exclusion and recovery semantics to them.

The [action host](../../../apps/desktop/src-tauri/src/plugins/mod.rs) scans managed copies, repository bundles and Elgato installations in priority order. Its runtime catalogue collapses duplicate IDs. Safe management therefore needs a host inventory retaining **all candidate sources**, the active selection and the reason another candidate is inactive. Deleting the active managed directory alone can expose another copy.

The [provider contract](../../PROVIDER-CONTRACT.md) already places hardware communication outside the Core lock and separates provider sessions from action execution. The [SDK](G:/APPS/DW/plugin-sdk/README.md), [template](G:/APPS/DW/plugin-template/README.md) and [registry](G:/APPS/DW/plugin-registry/README.md) are currently action-oriented. Their repositories and tooling should be extended with distinct device contracts rather than advertised as an already working device SDK.

The earlier [Mirabox transport](G:/CODEX/companion-surface-mirabox-stream-dock/src/streamdock.ts) and [protocol builders](G:/CODEX/companion-surface-mirabox-stream-dock/src/protocol.ts) contain vibration and separate screen/LED brightness commands. Port their existing builders and brightness gamma behavior; do not invent packets. [IO transport](G:/APPS/io_type_84/crates/io-platform/src/device.rs) and [integration evidence](G:/APPS/io_type_84/docs/INTEGRATION.md) supply narrow reusable codecs/settings/physical mapping and recovery rules. The current single-device opener and model information do not prove persistent per-unit identity. IO's source documents also distinguish implemented codec, SET/GET readback, visible behavior and power-cycle acceptance; those distinctions remain visible in DeckWeave.

### Remove actions and manage extensions

Settings → Extensions has Installed and Catalogue views, with Actions and Device support filters. Cards show name, version, author, type, source, enabled/available state and a concise actionable failure. Installation and update continue through the existing modal/preview regardless of list position. Action Property Inspectors remain action settings.

| Source/ownership | Operation | Reverse operation | Files affected |
| --- | --- | --- | --- |
| DeckWeave-managed installation | Uninstall… | Explicit Reinstall | Only the managed package, after safe stop |
| Elgato installation | Disconnect from DeckWeave | Explicit Reconnect | No deletion of Elgato files |
| Repository checkout bundle | Disconnect from DeckWeave | Explicit Reconnect | No deletion of source files |
| Host built-in component | Disable | Enable | No package deletion |

Action-plugin uninstall confirmation lists dependencies across active **and inactive** profiles, folders, pinned controls, Multi Action, Switch, Key Logic, Dial Stack and Action Wheel. Device-support removal lists every affected instance. Stop affected execution before file cleanup. Preserve profiles, nested action settings, missing action names and device configuration by default; unavailable assignments must remain intelligible and offer reinstall. Removed actions cannot execute. Forgetting saved data is a separate explicit destructive operation.

Assignment Remove affects only that assignment, never its package or other controls. Audit its keyboard/context-menu access and discoverability. For nested scenarios, preserve required-container invariants and use cancellation rather than release-action execution; do not create an invalid empty plan through an undiscoverable shortcut.

The host records lifecycle intent durably. Once uninstall/disconnect is accepted durably, the package ID is blocked across **every scan root**, even after a crash, before stopping and changing files. Only explicit reinstall/reconnect clears that removal intent; ordinary rescanning, discovery, updates or the presence of another copy cannot clear it. Existing Disable/Enable intent remains distinct from source ownership. Per-package lifecycle operations are serialized.

An interrupted or failed operation remains truthful: stop failure keeps execution gated and reports Stop/Recovery pending without deleting the package; file failure reports Cleanup pending while keeping execution blocked. Startup reconciles durable intent rather than guessing from directory existence. A bounded local operation history captures completed stages and errors in the existing diagnostics facility. Exact journal/table mechanics and persistence ordering belong to implementation design.

### Add, remove and restore independent devices

Settings → Devices → Add device has three clear entry points: **Discovered hardware**, **Saved/Removed devices**, and **Create virtual device**. Missing support links to the Device support catalogue or Install from file. First discovery may add supported new hardware automatically as Mirabox does now; an explicit saved removal takes precedence over auto-add across restart and USB reconnect.

Remove from DeckWeave stops that instance, closes its editor, cancels held/queued actions without executing release actions, and retains its configuration. Sibling instances keep working during ordinary single-instance close/removal. The operation does not reset, flash or erase physical hardware. **Forget saved settings** is separate and clearly destructive.

Saved/Removed devices lists the retained instance name, profiles, settings and required support. **Restore saved device** preserves its original instance/profile/binding IDs; it does not silently create another virtual instance. A virtual device restores immediately. A physical instance can restore as Offline or Support missing, retaining its data until the right hardware/support is available. Explicit restore/re-add clears that instance's exclusion; reinstalling a package alone does not silently restore deliberately removed device instances.

Providers expose opaque identity and current-session handles. Verify persistent physical uniqueness before automatically reattaching profiles or exclusions to identical hardware. USB paths and model/firmware attributes are not permanent per-unit identity. If uniqueness is ambiguous, require an explicit identify/rebind step; do not silently restore profiles, temporary input-recovery records or sensitive onboard writes to a guessed device. Session-local independence may be supported before stable cross-reconnect identity is proven, with that limitation stated. The implementation design must define this conservative fallback rather than manufacture a serial number.

### Device settings workspace and applied-state honesty

The gear beside a device opens **that exact instance** in the common editor, with **Actions**, **Lighting** when supported, and **Device settings** tabs. Actions uses the actual control geometry and existing host execution. Keyboard controls do not pretend to contain LCDs. Device settings contains connection/status and supported hardware groups; IO groups include onboard assignments and actuation, with further groups only when implemented and verified. Its Lighting entry navigates to the full Lighting workspace rather than duplicating a small lighting form. Shared keyboard/control-group selection can be reused across the tabs.

Mirabox Device settings must contain three independent real controls: **Press vibration on/off**, **Screen brightness**, and **Dial backlight brightness**. Each goes through the existing owning worker to the established hardware command. Verify the two output paths separately and verify vibration separately; one successful slider must not stand in for all three.

Common scalar/choice settings are host-rendered from provider metadata. The concrete host Lighting workspace and reusable control selection cover the first keyboard implementation. A custom advanced editor boundary is reserved for a demonstrated IO need; it is not a prerequisite for the first provider, a universal UI language or a permanently open WebView per device.

Controls distinguish current observed, last requested, pending and error state. Opening a device or Lighting tab reads state and does not write it. A stored desktop value is not labelled applied when offline or after failure. Distinguish successful transport, verified readback, unavailable readback and physically accepted behavior. Completed edits take effect immediately without a generic draft/Apply step. Persistent hardware fields commit at a clear interaction boundary, such as finishing a slider gesture or selecting an option, with pending/error state; coalesced volatile previews are allowed only on verified paths. Do not write flash per slider frame or animation frame, auto-reset on discovery, or discard unknown/reserved bytes. Keep a recovery snapshot when an operation requires one.

### Whole-keyboard lighting, modes and compositions

Lighting is a full workspace with a keyboard preview, preset list, selected key/group tools and mode-specific controls. A stock firmware mode acts on the whole keyboard and presents effect, brightness, speed, direction and supported colors together. Supported static per-key editing allows multi-key groups and a reusable static color arrangement. These are the first whole-keyboard modes/composition deliverables; a selected-key inspector alone does not meet the requirement.

Lighting presets belong to the **device instance**, independently of action profile contents. An action or profile references a preset ID. Renaming/editing a preset has one source of truth; missing references are visible. Changing an action profile changes lighting only when that profile has an explicit preset association. Presets are named/reusable and distinguish stock effects, static key arrangements and any later verified host-rendered composition.

The host has one authority per output group; the provider only applies the selected state. The recommended default is:

1. Opening Lighting only reads. Starting an edit takes a visible manual edit lock for that output; manual changes do not race automation. Competing action/automation requests during the lock report that manual editing owns the output and are **not queued for surprise replay**.
2. A completed manual edit/preset selection updates the base immediately and cancels obsolete temporary ownership. Persistent fields commit at their completed interaction boundary, with pending/error state and no generic Apply step. Closing/releasing editing ends the lock, keeping the chosen preset until a new explicit eligible request. **Return to manual** cancels temporary output and selects the current valid manual base.
3. Outside manual editing, the most recent explicit accepted preset request owns the output. A temporary action/automation request has a scoped token and declared lifetime/Stop; ending it restores the latest valid base, not a stale captured snapshot. Superseded tokens cannot restore over a newer request. This is bounded arbitration, not composition layers.
4. A profile switch cancels temporary requests belonging to the departing profile; only an explicit association on the new profile requests a preset. A request rejected by a manual edit lock is not replayed after the lock ends. Device removal or provider loss cancels temporary ownership. Reconnect reads actual state, resolves any recovery obligation first, and does not replay stale actions or lighting overrides; host-rendered output requires a fresh explicit request. A configured profile association may submit a fresh request after state readback and recovery; it cannot revive an expired override.

The existing global pause of Smart Profiles while any editor/settings window is open remains in force; this lighting edit lock does not add another Smart Profiles switch. Show the active source/mode and a simple reason an override was refused. Internal tokens, operation IDs and generation counters stay in diagnostics. Preview represents desired configuration unless readback establishes more; it is not evidence of physical color output.

Host-rendered compositions remain a requested direction with a **separate conditional acceptance outcome**, not an assumed capability. Before enabling them, verify volatile output, visible colors, required address coverage, supported stock backgrounds, sustained throughput/input latency, cancellation and release/restoration. The existing candidate path handles 1–14 keys and restricted static backgrounds; six batches do not prove atomic whole-keyboard output or interaction with stock animation. If the gate fails, supported stock modes/static groups/presets remain useful and the host-composition outcome is explicitly unresolved. Do not silently claim the whole request complete. Do not build arbitrary layers/blending/timelines first.

The keyboard LED panel is a separate output. Its proposed host API is not a verified stock setter and is not offered as working. Firmware extension, global input suppression and a second automation engine remain outside this change.

### Provider sessions, scheduling and recovery

Keep the existing action WebSocket protocol and Elgato compatibility. Define a distinct versioned device process protocol extending the [provider boundary](../../PROVIDER-CONTRACT.md). One supervised provider process may own many independent instance sessions. Descriptors describe stable model/control IDs, geometry, actual input events, display targets, lighting zones and settings; keyboard keys, LCD keys, touch surfaces and LED zones are distinct capabilities. Unsupported settings are absent or explained, never successful no-ops.

Sessions use opaque identity, generations, cancellation and bounded queues. No USB, encoding, filesystem or process wait runs under the Core lock. Input edges remain ordered and are not coalesced; stale generations cannot release new contacts or acknowledge new output. Bulk frames can replace obsolete frames. Use per-instance fairness and queue ownership; give close/cancellation/recovery priority over bulk rendering. Overflow fails/resynchronizes the affected session instead of silently losing digital edges. Establish measured budgets before choosing animation rates and degrade/stop output when needed. Process restart/backoff is bounded; a process crash can affect all its sessions, whereas ordinary one-instance removal must preserve siblings.

The provider serializes hardware writes. Host recovery obligations survive provider crash, disable, failed update, device removal and uninstall. Store their data **outside removable package directories** and retain/provide a compatible recovery implementation until resolved. IO temporary exclusive capture has no hardware lease: a crash/unplug may leave assignments captured. Thus logical removal immediately prevents actions, requests restoration before normal teardown, and displays Pending physical recovery if hardware is absent or restore fails. A recovery entry remains accessible despite discovery exclusion. If recovery code must be retained or restoration has not completed, show removal as pending physical recovery; do not claim a completed uninstall while executable cleanup remains pending. Never claim physical restoration merely because the host stopped a process. Recovery blocks new takeover/ordinary writes until settled; confirm identity before restoring anything.

Ordinary persistent onboard settings are preserved on removal and are not automatically reset. Depth observation, normal digital input and temporary takeover are separate opt-in capabilities. Do not generalize measured PgUp/PgDn behavior to every key or imply Raw Input globally suppresses typing. Discovery/editor opening keeps normal typing intact.

### Diagnostics, persistence and compatibility

Extend the existing diagnostic UI with bounded/rate-limited local lifecycle/provider evidence, including failure before registration, malformed/stale traffic, process exit, queue overflow and recovery. Retain last-operation transport/readback/recovery status and controlling lighting source. Ordinary cards/forms show concise state and next steps; internal IDs, generations, counters and stage history remain in diagnostics. No remote telemetry backend is needed.

Default keyboard diagnostics retain operational metadata and counts, not ordinary keystrokes, typed text, raw serial/path identity or settings/secrets. Raw keyboard/depth capture, when needed for hardware investigation, is explicit, bounded, visibly active and controlled on export. Verify redaction as well as failure observability with the mock provider.

Before incompatible persistence changes, make a durable pre-migration backup and advance a format gate that older builds already reject. Preserve existing N4 instance/profile/binding IDs, package disabled intent, unknown unavailable-provider records and recovery data. A missing provider must leave data inert and the application usable rather than failing complete startup validation. Old writers must not rewrite upgraded data and silently drop fields. Document rollback by restoring the pre-migration backup, with upgrade/missing-provider/interrupted-migration/downgrade acceptance cases. Exact tables and protocol framing belong to implementation design.

### SDK, template, catalogue and registry

Retain **plugin-sdk**, **plugin-template** and **plugin-registry**. Add separate action/device interfaces and generator modes or subdirectories, with independent compatibility boundaries. Do not create three duplicate infrastructure repositories. Keep the action context/Property Inspector contract separate from hardware registration and do not require every provider to embed an application.

Registry entries are explicitly typed Actions or Device support, sharing package metadata but validating their payload/source requirements independently. Registry format version, action compatibility and device process protocol version are distinct. The present action-only `controllers`/`.sdPlugin` schema cannot become a device schema merely through optional fields. Installed ownership is a host record, not a deletion right inferred from registry metadata.

The DeckWeave registry supplies reviewed typed catalogue metadata; existing OpenDeck action sources remain labelled. Compatible entries feed the current installation preview and managed-install lifecycle, rather than a second downloader. Development-only workspace entries are never shown as public installable releases. Install from file is a first-class path before a public registry entry exists. Validate self-contained provider archives and checksum/release metadata locally; no public repository/release publication is authorized by this research step.

The generator must create an independent device project with no absolute dependency on a `G:` checkout, plus a mock/example and documentation/tests authors can run without real hardware. Reuse template packaging/lifecycle conventions. Early device host/SDK/generator work is **provisional** until IO, the second physical adapter, validates the boundary; compatibility freeze/public release follows that test. Keep en/ru across lifecycle and device settings.

Use narrow IO transport/codec/mapping/recovery modules with provenance, not its complete Vue UI, persistence or automation runtime and not an absolute path dependency into its checkout. The IO README has not established a project licence for redistribution; verify provenance/notices and permission before public packaging. Proposed physical adapter location after direction acceptance is **G:/APPS/DW/device-io-type-84**; an optional GitHub repository can be created later. Extract Mirabox into a separate provider repository only when the boundary is actually ready.

## Goals vs. Non-Goals

**In scope:** action-plugin uninstall as the primary removal case; discoverable assignment/nested-step removal; source-aware install/update/disable/uninstall/disconnect/reconnect; dependency accounting; device add/remove/re-add/saved restoration; independent instances; capability settings; real Mirabox vibration plus separate screen and dial backlight brightness; whole-keyboard stock modes/static group compositions and reusable lighting presets; conditional host compositions with explicit evidence gates; provisional device host/SDK/generator/typed registry; IO stock-firmware integration; en/ru; standalone examples, failure tests and documentation.

**Outside this change:** new keyboard firmware, reset/calibration/OTA on connection, unverified LED-panel output, claims of global input suppression, replacing Elgato action protocol, Android or Logitech adapter implementation, synchronized device groups, importing old application profiles, a second automation engine, arbitrary lighting timelines/layers, or public release publication without a separate instruction.

## Alternatives Weighed

| Option | Assessment |
| --- | --- |
| Add another built-in Rust provider branch permanently | Useful as a transition, but fails independently installable third-party support. |
| Treat devices as ordinary Elgato actions | Does not supply hardware registration, instance lifetime, descriptor or device settings boundaries. Preserve actions, add a distinct device protocol. |
| Add three hardware infrastructure repositories | Duplicates release/docs/validation. Extend the current repositories with typed contracts. |
| Import the entire IO application or permanently open per-device apps | Duplicates persistence/automation and inconsistent Apply/draft behavior. Reuse narrow modules and the host workspace. |
| Delete every discovered copy or only delete the active managed directory | The first modifies external installations/source; the second can reveal another copy. Host ownership plus durable cross-root intent is required. |
| Remove a device database row and let discovery continue | Undoes user intent and hides retained configuration. Use saved/removed records and explicit restoration. |
| Build a general layer/timeline engine before hardware acceptance | Unproven output cannot justify a broad runtime. Start with supported stock modes/static arrangements and bounded arbitration. |

## Key Decisions & Staging for Next Steps

The owner's readiness, stack, action/device extensibility, independent instances, IO integration and three Mirabox controls are established requirements. The proposed defaults above resolve technical choices without repeating readiness or firmware permission questions. They remain proposed until the owner aligns on this revised concept.

1. **Current lifecycle/UI first.** Add source inventory and dependency information, action-package uninstall/disconnect, package-wide execution intent, interrupted-operation reconciliation and saved device restoration. Preserve existing cancellation/execution primitives. Make the action-removal and one-device flows reviewable in today's UI before broadening hardware. Define migration/rollback before incompatible saved-state writes.
2. **Mirabox settings.** Add capability-based settings to its existing worker, wiring vibration, screen brightness and dial backlight brightness separately through proven builders. Verify software invariants; request only remaining physical observations that software cannot establish.
3. **Provisional device boundary.** Implement mock provider, versioned host/SDK, schema validation and device generation. Exercise multi-instance close, process loss, malformed/stale input, overload, diagnostics/redaction, exclusion/restart and interrupted lifecycle without hardware. Generate/install a standalone project outside the local checkout assumptions. Do not freeze the external contract here.
4. **IO as second physical adapter.** Verify identity/explicit rebind, read-only snapshot and actual layout, then supported onboard settings and the full stock-lighting/static-group/preset workspace with deliberate write/readback/recovery. Preserve normal assignments on startup. Keep host-composition physical acceptance separate. Add measured depth behavior only after host-owned recovery survives removal/update/crash; do not infer untested every-key semantics.
5. **Package and stabilize together.** Complete SDK/template/registry metadata, documentation and migration regressions. Validate install/update/uninstall and retained configuration/recovery with a real self-contained provider archive. Freeze the contract only after N4 and IO exercise it. Physical tests establish visible behavior/readback/power-cycle claims separately. Public repository/archive publication is a later explicit operation.

### Remaining evidence gates and unresolved decisions

* **Hardware identity:** actual IO per-unit identity across duplicate devices/reconnect remains unproven. Default is explicit rebind; do not weaken it to automatic matching without evidence.
* **Lighting acceptance:** exact supported settings, visible color behavior, volatile coverage/atomicity, backgrounds, throughput and recovery remain measured gates. Whole-keyboard host composition is conditional; stock modes/static arrangements do not imply this gate has passed. If hardware cannot meet the requested host-composition outcome, report that concrete limitation for owner alignment.
* **Exclusive capture recovery:** the compatible recovery path, identity verification and host-retained obligation must be demonstrated before enabling temporary takeover. No physical restore guarantee follows from shutdown alone.
* **Redistribution:** IO module provenance/licence/notices must be resolved before public packaging. Do not claim permissions that source documentation has not established.
* **Implementation design:** protocol framing/version negotiation, persistence tables/journal mechanics, dependency traversal, exact queue limits/timeouts, archive schema and concrete migration steps remain downstream design work. These are technical execution choices, not new product approval gates.

### EGM handoff

Six independent read-only reviews are recorded with actual native IDs and manifests in [goldfish_review_report.md](review/goldfish_review_report.md). This canonical proposal incorporates the review recommendations. NEXT and DECISIONS were updated after all reviews completed; those updates and this revision were outside the independently reviewed snapshot. Provisional SDK work remains distinct from release/freeze after IO validation. No further independent review round is claimed for this synthesis.
