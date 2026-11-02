Codebase Details Goldfish — BRAINSTORM

Review complete. No Blocker found. The concept is sufficiently anchored and already addresses actionable errors, session generations, bounded queues, cancellation, settings state and readback. Its remaining observability gaps should become explicit requirements before detailed protocol design. A desktop-local diagnostic trail is sufficient; a remote telemetry service or distributed tracing backend is unnecessary.

1. Major — Observability & Operational SRE Debuggability: lifecycle operations need correlated, retained outcomes.

Affected scope: `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md:40–50,59,64,74,100`.

The brief requires source-aware uninstall/disconnect and persistent device exclusion, but does not require an inspectable record of an operation’s stages and result. A failed removal can leave several materially different states: runtime stopped but files retained, package removed but exclusion not persisted, or another scan-root copy discovered. A card’s current error cannot explain which step completed before restart.

Concrete evidence: `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs:68–87` exposes pause/reload timeouts; `:617–629` acknowledges pause/resume requests; `:47–53` establishes multiple scan roots. Current hardware diagnostics serialize a snapshot and recent inputs (`G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/hardware.rs:195–228`), rather than a lifecycle-operation history.

Mitigation requirement: retain a bounded local lifecycle history with an operation correlation ID, package/provider identity and version, source/ownership category, affected instance IDs, stage, outcome and actionable failure reason. Discovery should explain “excluded by user,” “support disabled,” “shadowed by another source,” and “open failed.” Removal/re-add and reinstall recovery must remain diagnosable after restart. Exact storage and event schema belong to detailed design.

2. Major — Observability & Operational SRE Debuggability: external provider failures need a host-owned diagnostic channel.

Affected scope: brief `:58–59,70,102`.

“One supervised provider process” and “actionable errors” do not establish how the host receives useful failure evidence when a process fails before registration, exits unexpectedly, emits malformed messages, or loses only one of several instance sessions.

Concrete evidence: existing action processes discard stdout/stderr in `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs:519–521,563–565`. Startup timeout and exit status become a session error (`:795–804`), but `logMessage` has no handling arm (`:1005–1161`) and invalid JSON is silently discarded (`:1219–1235`). This is a limitation of the current host, not a claim that the proposed device protocol already exists. Hardware discovery also reports one aggregate provider error through `G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs:691–697`.

Mitigation requirement: the device contract must offer bounded, rate-limited structured diagnostics that the host retains, including failures before registration. Correlate host/provider version, provider process session, instance ID and generation. Expose registration timeout, unexpected exit, malformed-message rejection, stale-generation rejection, queue overflow/drop and cancellation failure without allowing log floods. Require mock-provider scenarios to verify diagnostic outcomes as well as functional outcomes. Wire format and restart policy remain later-design decisions.

3. Major — Observability & Operational SRE Debuggability: settings and lighting require evidence of the last attempted hardware operation and its owner.

Affected scope: brief `:48,54,62,101,103`.

The current/desired/pending/error requirement is sound, but a support engineer must also distinguish transport success, readback verification, unavailable readback and physical verification. For lighting, the same symptom can result from a failed write or from a valid change immediately overridden by an action or automation. Resolving precedence without exposing the current owner leaves that behavior difficult to diagnose.

Concrete evidence: existing N4 diagnostics explicitly warn that transfer timings are successful HID transfers rather than display acknowledgements (`G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/hardware.rs:211–215`). The worker records transfer and render timing (`:431–453,804–840`), while Core counts completed output frames (`G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs:766–775`). The current editor’s device counters are N4-specific (`G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte:912–920`).

Mitigation requirement: provide an inspectable last-operation result for each setting/output group: request correlation, generation, volatile versus persistent write, requested state, transport result, readback/verification status and recovery outcome. Lighting diagnostics must identify the active execution mode and controlling source—manual selection, preset action or automation—and explain an override. Preserve the existing distinction between transfer completion and actual hardware acknowledgement. Metric names and UI layout can wait for detailed design.

4. Major — Observability & Operational SRE Debuggability: keyboard diagnostics need an explicit data-minimization boundary.

Affected scope: brief `:52,59,63,102–103`.

The brief proposes reusing N4 lifecycle/transport patterns and introducing keyboard input/depth observation, but does not constrain what input evidence enters diagnostics. Blindly generalizing the current trace could retain ordinary typing or sensitive action configuration.

Concrete evidence: N4 traces retain raw packet prefixes and decoded input in a 256-entry ring (`G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/hardware.rs:33–40,723–740`) and write those traces to disk (`:211–223`). Existing diagnostics already redact fields named `settings` and omit image blobs (`:195–217,252–270`), which is a useful starting point but not a keyboard-specific capture policy.

Mitigation requirement: default diagnostics should contain operational metadata and aggregate counts, excluding raw ordinary keystrokes, typed text, device serials/paths and action/provider secrets. Any raw keyboard/depth capture needed for a hardware investigation should be an explicit, bounded diagnostic mode with a visible capture state and controlled export. Require redaction tests in the mock-provider stage. The detailed redaction schema is a later-design task.

Read manifest — every required file was read in full:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/codebase_details.md` — fully read.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/zoom-out-standard.md` — fully read.
- `G:/APPS/DW/deckweave/docs/designs/device-extension-lifecycle/concepts_brief.md` — fully read, lines 1–110.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs` — fully read, lines 1–1482.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/hardware.rs` — fully read, lines 1–1203.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/lib.rs` — fully read, lines 1–1585.
- `G:/APPS/DW/deckweave/apps/desktop/src/DeviceEditor.svelte` — fully read, lines 1–991.

Additional evidence was limited to targeted diagnostic/logging searches in DeckWeave’s desktop Rust and Core sources. No files were changed; no builds, Git operations, hardware access, app execution, history or memory access occurred.
