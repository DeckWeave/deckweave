BRAINSTORM review completed. The hypothesis is viable as a direction, but automatic hardware programming needs explicit migration and rollback safeguards before release. These are concept-stage requirements, not a demand that the working hypothesis already satisfy the implementation standard’s zero-decision rule.

Paths below use `DW = G:/APPS/DW/deckweave`, `IO = G:/APPS/DW/device-io-type-84`, and `Prototype = G:/APPS/io_type_84`.

1. **Blocker — Migration & Backward Compatibility: legacy profile bindings cannot directly become physical alias ownership.**

   **Affected scope:** `DW/docs/designs/keyboard-assignment-execution/working_hypothesis.md:44–50,76`.

   The new ownership rule attaches an alias to a physical control and onboard layer. The existing persistent representation attaches `trigger` to `Profile.fn_bindings`; `Device` contains both an active profile and inactive profiles (`DW/crates/deckweave-core/src/model.rs:240–288`). The allocator examines only active profiles, treats removal as freeing capacity, and offers “remove an unused Fn action” as exhaustion advice (`fn_actions.rs:3–20`). This is incompatible with the proposed retained ownership rule.

   Migration cannot assume that matching profile entries describe a successfully programmed device. Nor can it resolve two inactive profiles assigning different aliases to the same physical control by silently choosing one. Existing Fn+T Launch must remain intact even when its hardware mapping has never been established.

   **Migration requirement:** the Concepts Brief must distinguish preserved host bindings, inferred legacy alias references, verified physical ownership, and unresolved conflicts. Loading legacy data must perform no SET. A successful fresh read can establish what currently emits an alias, but cannot recreate an original native record that was never backed up.

2. **Blocker — Migration & Backward Compatibility: format-2 rollback can discard ownership and restoration information.**

   **Affected scope:** hypothesis lines 48–50 and 78.

   `Store::save` serializes a typed `Configuration` back into a single JSON record (`DW/crates/deckweave-core/src/store.rs:131–151`). The current SQL guard checks that `format_version` remains 2; it does not distinguish different format-2 writer capabilities (`store.rs:155–156`). New ownership fields added under the same version could therefore be ignored and dropped when an older format-2 build saves configuration. The existing initialization code preserves unknown JSON while adding a missing version field, but ordinary typed saves do not provide equivalent preservation (`store.rs:90–99,135`).

   Losing original mapping records after a successful hardware change is a rollback hazard: the keyboard still emits aliases while the downgraded app no longer knows ownership or how to restore them.

   **Migration requirement:** select an enforceable writer-compatibility boundary before durable ownership is introduced. A version bump with old-writer rejection is one viable choice. Keep a durable pre-migration database backup, and document that restoring that database does not restore keyboard state. A rollback procedure must reconcile both stores rather than equating executable downgrade with safe recovery.

3. **Blocker — Migration & Backward Compatibility: one user operation spans independent durable systems.**

   **Affected scope:** hypothesis lines 21 and 48–50.

   The proposal correctly separates saved configuration from hardware readiness, but the operation crosses SQLite configuration, driver recovery files, and onboard blocks. SQLite revision checks cover host configuration only (`store.rs:140–151`). The adapter separately persists a pending write intent before SET (`IO/src/recovery.rs:249–264`). Neither mechanism proves that the host binding, reserved alias, original raw record and hardware result have advanced together.

   Crashes can occur after reservation but before SET, after SET but before readback, or after verified readback but before host activation is saved. Reopening must also respect “no automatic settings replay” at hypothesis line 65.

   **Migration requirement:** specify a durable operation identity and recovery states connecting these facts. Uncertain hardware outcomes must reserve the alias and prevent misleading activation or reuse. Startup should inspect and reconcile using reads; it must not retry an uncertain SET. The protocol explicitly rejects automatic retries and multi-block atomicity assumptions (`Prototype/docs/PROTOCOL.md:90–96`). Resolving a hardware obligation must also resolve or retain the corresponding host ownership obligation.

4. **Major — Migration & Backward Compatibility: current mapping restoration does not implement the proposed external-edit rule.**

   **Affected scope:** hypothesis line 50, “Never restore over a later external edit.”

   Existing recovery merges original records into a fresh current snapshot for slots changed by the interrupted operation. For those slots it copies the old four-byte record without first checking whether the current slot still equals the requested target (`IO/src/recovery.rs:391–410`). A fresh confirmation token rejects edits made after preview (`recovery.rs:638–643`), but an external edit made before preview can already be included in a restoration target that overwrites it.

   This is especially relevant when deleting the last host action or rolling back an upgrade: “restore original assignment” is attractive, but cannot safely mean unconditional restoration of an old backup.

   **Migration requirement:** define a three-way rule using original, installed/requested, and currently observed records. Automatic cleanup should proceed only when the ownership evidence still matches. Divergence should preserve current hardware and retain an unresolved record for explicit resolution. The exact original record must survive even when its page type is unknown. Current preservation of neighbouring raw records is useful groundwork, not proof that this stronger ownership-aware restoration exists.

5. **Major — Migration & Backward Compatibility: durable ownership cannot rely on the current session identity.**

   **Affected scope:** hypothesis lines 44, 50, 52 and 68.

   The driver advertises `identity.confidence = "session"` and `persistentId = null` (`IO/src/model.rs:99`). The protocol states that empty USB serial prevents persistent identity from being established by serial alone and forbids selecting the first candidate among multiple devices (`Prototype/docs/PROTOCOL.md:5–7`). Existing recovery deliberately treats vanished opaque identities conservatively (`IO/src/recovery.rs:17–53,156–200`).

   After restart, port changes, or connection of a second identical keyboard, the application cannot safely attach old ownership and original-record backups to whichever matching model appears. Raw Input pairing introduces another identity relationship; it does not solve durable vendor-interface identity automatically.

   **Migration requirement:** define the evidence sufficient to reattach ownership after reconnect. Until that evidence exists, retain reservations, show identity uncertainty, and require explicit physical selection for restoration or new programming. Do not rewrite mappings on reconnection. Decide what happens when a user replaces a keyboard with an identical unit.

6. **Major — Migration & Backward Compatibility: input-source rollout can change existing behavior before ownership migration is settled.**

   **Affected scope:** hypothesis lines 52–56 and 74–78.

   The current shortcut route is explicitly global, registers eligible aliases, rejects duplicate active-device aliases and retires queued messages when routes change (`DW/apps/desktop/src-tauri/src/keyboard_shortcuts.rs:75–107,227–285`). Core validation currently rejects Key Logic for Fn bindings because only one-shot activation is available (`fn_actions.rs:76–88`). Moving to Raw Input changes device attribution and potentially press/release semantics; it is not merely a backend substitution.

   Allowing new held-action configurations before the new source is verified creates a downgrade problem: the fallback cannot execute them correctly. Conversely, concurrently running both sources risks duplicate launch/plugin execution.

   **Migration requirement:** gate source selection and supported interaction semantics independently. Preserve legacy one-shot bindings without automatically upgrading them to hold behavior. Require successful interface pairing before device-specific readiness. Source switching must retire queued events and any held state. A fallback must disclose its reduced capabilities and cannot silently reinterpret a configuration that requires release events.

Proposed mitigations, separate from findings:

- Start with read-only unified Main/Fn presentation and additive capability metadata. Keep missing metadata compatible with older adapters.
- Migrate host data without programming hardware. Preserve all profiles and the live Fn+T Launch binding.
- Establish writer compatibility and an ownership journal before enabling automatic programming.
- Enable a narrow, verified keyboard-usage alias path first; integrate mouse, Consumer and device-function writers in separate capability-gated stages.
- Validate crash and downgrade boundaries using isolated data and transport mocks before authorised physical acceptance. Byte readback verifies stored records; it does not establish physical execution or power-cycle persistence.

Unresolved interaction decisions for the next clarification:

- Does removing the last action retain the reservation indefinitely, offer restoration, or require a separate “return to keyboard assignment” operation?
- How are conflicting legacy profile aliases presented without silently changing existing bindings?
- What does an inactive profile do when the physical key remains an alias but that profile has no action?
- How does the user confirm an identical keyboard after identity continuity is lost?
- Is the initial release one-shot only, with press/release interactions deferred until Raw Input acceptance?
- Is alias exhaustion scoped globally across known keyboards, or can verified device-specific input eventually permit reuse? The hypothesis currently takes the conservative global approach.

Read-file manifest:

**Read in full**

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/implementation_readiness.md`
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/implementation-readiness-standard.md`
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md`
- `DW/docs/designs/keyboard-assignment-execution/working_hypothesis.md`
- `DW/README.md`
- `DW/modules/storage.md`
- `DW/modules/runtime.md`
- `DW/crates/deckweave-core/src/store.rs`
- `DW/crates/deckweave-core/src/fn_actions.rs`
- `Prototype/docs/PROTOCOL.md`

**Targeted reading**

- `DW/crates/deckweave-core/src/model.rs`: profile, binding, route, device, configuration/defaults and Fn command representations.
- `DW/apps/desktop/src-tauri/src/keyboard_shortcuts.rs`: lines 1–289, runtime lifecycle, registration, conflict rejection and readiness.
- `IO/src/model.rs`: lines 75–210 and 290–350, capabilities, raw mapping projection and editing.
- `IO/src/recovery.rs`: lines 1–60,156–245,249–272,391–445,536–705, identity scope, retained records, intent persistence, restoration merge and confirmation/readback.

Status: **completed, read-only**. No edits, builds, device operations, shortcut registration or user-data mutations were performed.
