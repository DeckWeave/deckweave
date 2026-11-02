Read manifest

Successfully read:

- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/agents/comprehension.md` — complete persona, including the BRAINSTORM instructions.
- `C:/Users/Wismut/.codex/plugins/cache/egm/egm/0.1.0/resources/document_standards/feature-spec-standard.md` — complete supplied standard.
- `G:/APPS/DW/deckweave/docs/designs/host-feedback/working_hypothesis.md` — complete target, lines 1–89. Its SHA256 matched `14B2854797631DF75411AAE2B32E3E50117C2D09502B812611DB6762023ED`.
- `G:/APPS/DW/plugin-sdk/sdk.mjs` — targeted searches and lines 55–215.
- `G:/APPS/DW/plugin-sdk/sdk.d.ts` — complete file, lines 1–82.
- `G:/APPS/DW/plugin-sdk/PROTOCOL.md` — complete file, lines 1–48.
- `G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/plugin.mjs` — complete file, lines 1–120.

`G:/APPS/DW/deckweave/plugins/dev.deckweave.timers.sdPlugin/timer.mjs` was discovered and searched for matching symbols, but not read in full. No prior reviews, concepts brief, private data, or conversation history were retrieved. No files were changed.

Overall assessment

The hypothesis gives a coherent user model: panels belong to durable instances, continue across page and profile changes, restore after restart, and retain useful content when their plugin becomes unavailable. These are confirmed product requirements and should remain fixed. The document also correctly separates transient notifications, persistent panels, visible action contexts, and background service ownership.

No implementation Blocker was identified by this ergonomics review. The proposed API’s absence from the current SDK is expected and is not implementation drift. The Major findings below identify product semantics and developer contracts that should be resolved before implementation, rather than requiring a completed wire protocol at this hypothesis stage.

1. Major — UX & Developer Ergonomics: panel identity within one plugin is underspecified

Affected scope: “Desktop panel service,” lines 37, 43–49.

The host-assigned plugin namespace protects one plugin from replacing another plugin’s panel. It does not explain how two assignments of the same plugin avoid replacing each other. A timer author could naturally use `ctx.panels.upsert("timer", spec)` in every action. Two timer actions would then either share one panel unintentionally or require authors to invent persistent IDs outside the visible context model.

This matters because the existing SDK’s `ctx.id` is a visible context identity, and its protocol explicitly says that identity can change on navigation. The hypothesis correctly forbids using that lifetime for durable ownership, but a new author still lacks a safe identity to use instead.

Ergonomic requirement: expose an SDK-supported stable instance identity or host-issued panel handle, and make the scope of `localId` explicit. Include an example with two instances of the same widget and a later reconnect.

Alternatives and mitigation: either scope local IDs to a host-owned durable action/service instance, or create panels through the service registry and return persistent handles. Avoid making each plugin generate and persist its own identity scheme. Decide whether copying an action creates an independent widget instance or deliberately shares an existing one.

2. Major — UX & Developer Ergonomics: closing a panel and updating it need distinct semantics

Affected scope: lines 37, 43–47 and acceptance boundary at line 89.

The requirement that a closed panel stays closed is clear, but `upsert` conventionally creates an absent object. A live CPU widget may continue submitting updates immediately after the user closes its window. Without an explicit distinction, the next update could reopen it. Reconnection or restart restoration could produce the same problem even if the saved open/closed field is correct.

Ergonomic requirement: ordinary content updates and reconnect handling must preserve the user’s closed state. Only a deliberate show/open operation should reopen a panel. A close event should tell the owner whether to stop streaming updates or retain a background job.

Alternatives and mitigation: separate create/show from update; alternatively, define `upsert` as updating the saved instance while preserving visibility unless an explicit visibility operation is supplied. Include acceptance cases for closing during a stream, reconnecting after close, and pressing the creating action after close.

The remaining product question is what a subsequent explicit action press does: reopen the saved instance, toggle its visibility, or create another instance. None of these choices requires changing the confirmed cross-profile lifetime.

3. Major — UX & Developer Ergonomics: two settings entry points need one understandable editing model

Affected scope: lines 9, 41, 45 and 49–51.

Configuration is placed beside the creating action, while the durable panel also has its own saved configuration and global inventory editor. The document correctly requires explicit source-action updates, but does not say which fields an action inspector edits once several panel instances exist, or what a user sees when the action’s configuration differs from an existing panel.

A user could change the action’s color or timer duration and reasonably expect the visible panel to change. A different implementation might treat the edit as a default for future panels. Both interpretations fit the current wording.

Ergonomic requirement: label whether an editor changes the selected panel, a creation default, or an explicitly selected group of panels. The inventory must identify the saved instance and its owning plugin clearly enough to distinguish two similar timers. Generic host controls must remain usable when the owner plugin is unavailable.

Alternatives and mitigation: use a snapshot model with a clearly labelled “Update this panel” operation, or let the inspector select an existing instance and edit that instance directly. Keep geometry and other user placement edits authoritative unless the user deliberately resets them. Demonstrate editing from the inventory while the original action is hidden, followed by opening the action inspector again.

4. Major — UX & Developer Ergonomics: successful submission must not look like successful delivery

Affected scope: lines 23, 29, 37, 47 and 57–59.

“Useful unsupported or playback error” is the right intention, but the developer interaction needs a clearer conceptual contract. Current SDK methods return a boolean for socket submission; `PROTOCOL.md`, lines 25–28, explicitly distinguishes that from hardware acknowledgment. Carrying that pattern into `playSound` would make `true` easy to misread as audible playback. Panel quota rejection, incompatible widget registration, stale ownership and rendering failure create the same ambiguity.

Ergonomic requirement: the SDK must distinguish unsupported capability, local submission, host acceptance, deliberate deduplication and failed delivery. Authors need a documented way to surface an actionable user state without inventing raw event listeners and correlation IDs for every call.

Alternatives and mitigation: use typed asynchronous results, or return a request handle with typed status events. Either approach can preserve the existing SDK’s boolean methods for old messages. Advertise capabilities in one documented SDK entry point and provide examples of an unsupported-host fallback and an exhausted panel limit.

For sound, the result should also explain whether retrying with the same `eventId` is safe. The public interface should hide connection-generation and request-correlation bookkeeping wherever possible.

5. Major — UX & Developer Ergonomics: the timer needs a user-visible background-operation promise

Affected scope: lines 33, 47, 63, 81 and 89.

The hypothesis correctly requires host-owned scheduling for hidden-page alarms and rejects render-driven completion. The remaining usability issue is what the user believes enabling a timer notification guarantees. “Sound enabled” does not communicate whether the timer continues when its panel closes, its source action disappears, its device disconnects, its plugin fails, or DeckWeave exits.

The document gives an initial direction for expired timers restored after restart, but does not yet connect that direction to the settings experience. A user should not discover the running-application boundary by missing an alarm.

Ergonomic requirement: define the timer job’s lifetime separately from panel visibility and present that promise in the timer UI. Failure to register a background job must produce an observable unavailable/error state instead of silently degrading to visible-page operation. Keep the confirmed no-old-sound-on-restart behavior explicit.

Alternatives and mitigation: make host scheduling the default whenever timer notifications are enabled, or expose a clearly labelled background setting. If closing a panel also offers to stop a timer, use an explicit separate control rather than making window closure silently cancel it. Include a walkthrough covering hidden page, closed panel, disconnected device and application restart.

6. Nit — UX & Developer Ergonomics: recovery controls should be discoverable before a panel becomes hard to reach

Affected scope: lines 41, 49, 51 and 89.

A global inventory and recoverable click-through mode are already required. The concept could make the recovery path more concrete: a user who enables click-through or chooses very low opacity needs to know where that panel can be selected, moved, made interactive and closed later.

Ergonomic requirement: provide a plainly named Panels entry point with keyboard-reachable generic controls, including locating or resetting the selected panel’s placement. Show the recovery location when click-through is enabled.

Alternatives and mitigation: provide per-panel “Locate” and “Reset position” controls, or a temporary host-controlled arrangement mode entered deliberately from the inventory. The latter must not make routine updates steal focus.

Product questions to resolve

These are decisions about user expectations, not implementation blockers:

- Does repeating the creating action reopen, toggle, or create another saved panel?
- Does copying an action produce a new durable widget instance or share an existing one?
- Does editing an action change creation defaults or require an explicit update of a selected existing panel?
- Does closing a timer panel leave the timer running, and how is stopping the timer presented separately?

The confirmed requirements—profile independence, restart restoration, live widgets remaining live across navigation, and closed panels staying closed—do not need reopening.

---

Read-only verification output:

```text
Algorithm : SHA256
Hash      : 14B2854797631DF75411411AAE2B32E3E50117C2D09502B812611DB6762023ED
Path      : G:\APPS\DW\deckweave\docs\designs\host-feedback\working_hypothesis.md
```

The report’s manifest contained a transcription typo only. The verified target hash matches the supplied hash; no review conclusions change. No files were modified.
