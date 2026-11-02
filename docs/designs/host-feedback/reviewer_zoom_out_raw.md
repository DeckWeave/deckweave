BRAINSTORM report — Reuse & YAGNI Brainstormer

The feature addresses a clear need, and the confirmed panel lifetime is sufficient justification for introducing durable panel instances. Keeping live widgets independent of device visibility is necessary for the stated workflow; it is not speculative infrastructure. The proposed structured content, limited sound bundle, explicit ownership, and deferred capture command are sensible scope boundaries.

I found no Blocker against continuing concept development. The main implementation risk is duplicating an existing host timer planner while adding sound and panels. The other findings concern containing the initial scope and establishing a useful backend selection criterion. I evaluated the document as a hypothesis: proposed API absence is not implementation drift.

**Read manifest**

These required files were successfully read in full:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/zoom_out.md`
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md`
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/zoom-out-standard.md`
- `G:/APPS/DW/deckweave/docs/designs/host-feedback/working_hypothesis.md`

The target SHA256 was verified as `14B2854797631DF75411411AAE2B32E3E50117C2D09502B812611DB6762023ED`.

These supporting files were successfully read, either in full or through the indicated targeted excerpts/searches:

- `G:/APPS/DW/deckweave/README.md` — full read.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/key_lighting.rs` — targeted reads of timer planning, worker dispatch, ownership validation and accepted-settings integration; symbol and test searches.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs` — targeted reads/searches of session/context retirement, assigned package discovery and accepted-settings integration.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/network.rs` — targeted registration, routing and nonce searches.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/shell.rs` — targeted window and lifecycle searches.
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugin_lifecycle.rs` — targeted persistence and removal-intent searches.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/actions.rs` — targeted action and revision searches.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs` — configuration/preferences excerpts and targeted searches.
- `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs` — save transaction excerpt and targeted persistence searches.
- `G:/APPS/DW/deckweave/crates/deckweave-platform/src/audio.rs` — targeted audio boundary search.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/plugin.mjs` — targeted reads including state persistence, draw completion and visibility handlers.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/timer.mjs` — source read and targeted state/serialization evidence.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.monitor.sdPlugin/plugin.mjs` — full read.
- `G:/APPS/DW/plugin-sdk/sdk.mjs` — targeted context construction, disposal and messaging excerpts/searches.
- `G:/APPS/DW/plugin-sdk/sdk.d.ts` — targeted context and event declaration searches.
- `G:/APPS/DW/plugin-sdk/PROTOCOL.md` — full read.

Discovery confirmed that these suggested paths do not exist; they were not treated as evidence:

- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/contexts.rs`
- `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/lifecycle.rs`

`G:/APPS/DW/deckweave/AGENTS.md` and `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/protocol.rs` were also absent at the searched paths. I did not read `concepts_brief.md`, prior reviews, other reviewer reports, private databases or conversation/memory sources. No files were changed, no builds were run, and no hardware or UI automation was used.

**Implementation findings**

1. **Major — reuse the existing host countdown planner before introducing another completion scheduler.**

   Dimension: Contrarian / YAGNI & Code Minimization.

   Target: “Evidence from the current code,” lines 21–25; “Shared sound service,” line 33; background registry, line 47; staging, line 81.

   The hypothesis correctly identifies the need for host scheduling while pages are hidden, but it omits an important existing component. `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/key_lighting.rs:58` already defines `TimerPlanner`. Its update path keeps a monotonic deadline, bounds armed jobs, avoids arming output from an already-running saved state, and removes due jobs before dispatch. The accepted-settings hook is wired into `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/plugins/mod.rs:1280`. This is materially closer to the desired solution than starting with the timer plugin’s draw loop.

   This planner cannot simply become the new service unchanged. Its job contains a lighting-specific `Alert`; acceptance requires lighting configuration to be enabled, and `timer_current` at `G:/APPS/DW/deckweave/apps/desktop/src-tauri/src/key_lighting.rs:356` validates the current profile and assignment. Those conditions do not provide the independent lifetime required for a saved desktop timer widget.

   The risk is creating separate lighting, sound and panel schedulers that disagree about completion identity, pause/reset, reconnect and replay. One user timer could then have multiple competing completion decisions.

   Alternative and mitigation: explicitly evaluate extracting the existing bounded completion planning into a small output-independent component, with lighting, sound and panel delivery as separate consumers of one accepted completion. Keep ownership policy separate because a device lighting target and a persistent desktop timer instance have different lifetimes. Reuse the existing deadline/no-replay behavior where applicable, without broadening the legacy visible-context protocol. Update the hypothesis’s evidence paragraph to acknowledge this component and state that extraction versus adaptation is the design question.

   This is an implementation design risk, not a reason to reject the feature or a claim that its proposed API should already exist.

2. **Nit — use the existing monitor sampler to prove the live-widget lifetime.**

   Dimension: Contrarian / YAGNI & Code Minimization.

   Target: structured content, line 39; background registry, line 47; first examples, lines 53 and 82; acceptance, line 89.

   A CPU example is useful because it demonstrates that a widget remains live after its source page disappears. It does not require a new host CPU telemetry subsystem. `G:/APPS/DW/deckweave/plugins/dev.deckweave.monitor.sdPlugin/plugin.mjs:10` already collects CPU/memory values, shares one sampling interval, and samples every two seconds. Its current interval starts and stops with visible contexts at lines 39–56.

   Alternative and mitigation: adapt this plugin’s sampling ownership to the advertised service registration, with one sampler active when either a visible monitor action or a live monitor panel needs it. Feed structured values to the panel service while retaining the existing device rendering path. This demonstrates the new lifetime with little unrelated code and prevents a sampler or plugin process per CPU panel.

   The present visibility behavior is expected current behavior, not drift. The opportunity is to name this reuse explicitly before an example becomes an independent implementation.

3. **Nit — reuse persistence conventions without turning panel state into action settings or a second generic storage framework.**

   Dimension: Contrarian / YAGNI & Code Minimization.

   Target: persistent inventory and saved state, lines 43–49; resource limits, line 57.

   Durable panel inventory is necessary. A separate profile-independent domain does not automatically require a separate database, a general persistence service, or duplicated settings serialization. `G:/APPS/DW/deckweave/crates/deckweave-core/src/store.rs:143` already has transactional, expected-revision configuration writes and preserves unknown JSON fields. `G:/APPS/DW/deckweave/crates/deckweave-core/src/model.rs:364` provides an application configuration envelope outside individual profiles.

   Conversely, putting all saved panel content back into the creating action’s settings would preserve the coupling the feature is trying to remove. The hypothesis already points away from that mistake.

   Alternative and mitigation: during design, evaluate a bounded application-level panel inventory using the existing storage and migration conventions. If a separate table or store is preferable, document the concrete reason, such as independent write behavior or asset ownership, rather than introducing a generic storage abstraction. Retain the hypothesis’s separation between durable configuration and sampled widget values. No schema commitment is needed at the brainstorm stage.

**Product and scope questions**

4. **Major — define the resource criterion that can actually select a panel backend.**

   Dimension: Contrarian / YAGNI & Code Minimization.

   Target: rendering backend, line 39; resource limits, line 57; alternatives, line 71; measured implementation, line 82.

   Deferring the renderer until measurement is appropriate. However, “actual resource requirement,” “bounded panel counts” and “modest bounded rate” do not yet say what would make a Tauri solution acceptable. Without a representative workload and decision criterion, the spike could grow into two renderers or a broad native rendering abstraction because any measured cost can be called too large.

   This is not a demand for a finished performance plan in the hypothesis. It is a missing constraint that materially affects the proposed architecture and effort.

   Alternative and mitigation: establish one bounded initial workload and a provisional resource envelope before committing to backend implementation. For example, specify the intended first-release maximum number of simultaneously open panels, a representative mix of static notes/images and live values, and what idle CPU/memory growth would make the first candidate unacceptable. Measure application and child-process cost together. Start with one candidate backend and require a demonstrated failure against that envelope before funding another implementation.

   The product question is the expected simultaneous panel workload and acceptable cost. The engineering question is how to measure it consistently. Neither changes the confirmed requirement that persistent live panels survive profile changes and restore after restart.

5. **Nit — distinguish essential recovery controls from the full customization surface.**

   Dimension: Contrarian / YAGNI & Code Minimization.

   Target: placement and appearance settings, line 41; global inventory, lines 43–49; staging, line 82.

   The first delivery already includes persistence, authenticated ownership, reconnect, stale-state behavior, focus rules, DPI, monitor changes, two mouse modes and button callbacks. Adding the complete placement/appearance matrix at the same time increases the acceptance surface before the core lifetime is proven. Some settings are useful, but their immediate necessity is not explained by the note/timer/CPU proof.

   Alternative and mitigation: keep durable position/size/settings, reachable close/move controls and safe monitor fallback as essential behavior. Consider staging additional corner/edge presets and appearance customization after the first backend proves its lifetime and input behavior. Use conservative host defaults initially where no confirmed product setting requires an exposed choice. This reduces UI and QA work without weakening persistence or profile independence.

**Assessment**

The shared host boundary is justified, and the alternatives section already rejects several real architectural traps. Durable widget registration, fresh callback ownership and unavailable states are necessary consequences of the confirmed restart/profile behavior; I would retain them. I would not recommend replacing the panel feature with OS notifications or freezing hidden widgets.

Before implementation, resolve the timer-planner reuse decision and give backend measurement a concrete resource criterion. The remaining reuse and staging suggestions can be incorporated during the next design pass without blocking readiness to discuss the hypothesis.
