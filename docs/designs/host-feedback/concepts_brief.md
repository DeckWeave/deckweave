# Shared sound and desktop panels

* **Author:** DeckWeave working hypothesis
* **Date:** 2026-11-03
* **Status:** The owner clarified persistence on 2026-11-03. The working hypothesis is ready to present for the EGM readiness gate. No Goldfish reviews have run. Proposed APIs and behavior below are not implemented.

## Overview

Plugins need host services for audible feedback and information above other desktop windows. A timer should request a sound and a notification; a note, pinned image or system monitor should use the same panel service without starting another application. Configuration belongs beside the plugin action that owns it.

Profiles remain full sets of pages, folders and actions for one device. They are not part of the panel renderer. Hardware Main/Fn assignments remain common to the device while the active profile resolves host actions.

The owner confirmed that changing profiles must not change or close desktop panels. Profiles are switched frequently like sets of folders and pages. Persistent panels restore after DeckWeave restarts, with their previous content, position and settings. This applies to plugin widgets as well as pinned notes and images; profile changes must not silently turn a working widget into a frozen picture.

## Analysis

### Evidence from the current code

The core already stores a selected profile and additional profiles on each device, each with pages, folders, Fn bindings and an application association. AddProfile, SelectProfile and ProfileApplication are implemented. The editor exposed profile selection and creation behind a small settings icon, with no rename, copy or delete. The accompanying implementation extends that existing model rather than replacing it.

The Audio plugin controls master output through deckweave-platform. That is volume control, not a shared notification sound service. Timers currently render their state and preserve timer handoffs between visible contexts. Their draw path detects completion, but no shared sound or desktop panel API exists. A drawing tick must not become an alarm source: redraw, reconnect or opening a page must not replay an old completion.

Plugin processes, property inspectors, host context identifiers and lifecycle handling already exist. Host extensions must be optional and advertised, so ordinary Elgato plugins keep their existing protocol behavior. The plugin SDK and template are sibling repositories under G:/APPS/DW; the contract and example must ship there alongside host support.

The current plugin host retires a visible action slot with `willDisappear` when it leaves the active set (apps/desktop/src-tauri/src/plugins/mod.rs). The SDK then disposes its timers and deletes that context (plugin-sdk/sdk.mjs). Keeping a panel window open while using this retired context would stop updates or retain unusable buttons. Durable panel ownership must therefore be independent of visible device slots. This is an implementation change, not a capability of the current runtime.

### Shared sound service

Proposed calls are `ctx.playSound({ sound, volume, eventId })` and a host-provided list of bundled sounds. Names are provisional. The host validates the live plugin/context, deduplicates event IDs, bounds request rates and simultaneous playback, and returns a useful unsupported or playback error. Volume is local to this playback; it must never change master system volume.

Start with a small licensed, offline bundle: completion, short notice, and warning. Provide preview and volume in the plugin inspector. Custom local files can follow with a file picker and bounded decoded size; accepting arbitrary URLs or arbitrary paths from plugin messages is unnecessary for the first delivery. Do not launch a player process per sound. The service should sleep when idle, load short samples once as needed, and release audio resources on shutdown or output loss.

Timer integration must emit one completion event from a state transition with an identity separate from render/context lifetime. A sound started by an explicit Preview is separate from timer delivery. Decide deliberately whether an expired timer restored after restart announces itself; initial direction is to show its expired state without unexpectedly sounding an old alarm. Timers that must alarm while their page is hidden need a host-owned schedule, not a visible-action interval. This is a required functional boundary, not solved by adding playSound to draw().

### Desktop panel service

Proposed calls are `ctx.panels.upsert(localId, spec)`, `ctx.panels.close(localId)` and events delivered to an authenticated panel service context. A visible action can create or configure a panel, but that action's later disappearance does not dispose the panel. Names are provisional. The host assigns the owner namespace; plugin-supplied IDs cannot replace another plugin's panel. Update an existing panel in place rather than creating a window for every tick.

Use structured content first: text/note, image, progress/value, and explicit buttons bound to the owner's registered callbacks. Accept neither arbitrary HTML nor remote script in the first API. The service owns placement, theme, typography, monitor bounds, update coalescing and disposal. This keeps appearance consistent and prevents each plugin from creating its own browser or helper process. The rendering backend is still an engineering decision; a shared service does not by itself prove low memory use. Measure a Tauri implementation against the actual resource requirement before committing to many webviews or a native renderer.

Plugin settings contain monitor, corner/edge/free placement, offset, size, color/opacity, duration or pinned mode, and mouse mode. Separate `interactive` and `click-through`. A click-through panel still needs a recoverable way to move or dismiss it from DeckWeave settings. Showing or updating a panel must not steal keyboard focus. Clamp restored geometry to a connected monitor's work area and handle DPI and monitor removal.

Transient notifications should stack with bounds, expiry and deduplication; an expired transient notification does not reappear after restart. Persistent panels have stable identities, a saved inventory and an explicit close command. Switching a page, folder or profile does not close, reset, duplicate, or re-create them. Closing a persistent panel records that intent so restart does not resurrect it. Device disconnection does not delete desktop content.

Save content or a managed local image reference, plugin owner, panel instance ID, widget configuration, logical monitor-relative bounds, size, colors, mouse mode and open/closed state. Persist user changes and drag/resize completion, not every CPU sample or animation tick. Restore windows without stealing focus, then reconnect live widgets. Static notes and images render from saved content without starting a plugin solely to redraw them. A missing monitor uses a visible fallback on a connected monitor; retain the preferred placement so temporary monitor removal does not destroy it.

The host owns a background service registry for panels and active timer jobs, separate from device visibility. Only plugins with active services need those background contexts; this is not permission to keep every hidden action running. The plugin registers a versioned widget kind and receives saved instances on reconnect. Use a plugin connection generation and a panel revision for updates and button events. Never persist an opaque WebSocket context, callback closure, or old connection token. Profile changes alone do not retire this service registration.

Clicking a widget must target that widget's saved owner and configuration. It must not synthesize a press on the same key in whichever device profile happens to be current. Changes to the source action must be explicit updates of the panel instance; a different profile's assignment does not replace it. A missing or changed action reference produces an actionable unavailable state rather than executing a different action. Settings remain reachable from the owning plugin inspector and from the global panel inventory even when the original page is hidden.

Plugin disable/removal or a crash retires its live callbacks without erasing saved panels. Keep pinned text and images visible. Mark live widget data unavailable or stale and disable its buttons until the owner reconnects; do not show a cached CPU reading as current. The panel inventory always allows close or removal. Plugin reinstallation may reclaim only its own compatible saved widget instances after registration; unsupported versions retain saved content without executing old callbacks. Old Elgato plugins receive no invented visibility or service events: this is an advertised DeckWeave extension.

Screenshot capture and image pinning are distinct from displaying an existing image. A later capture command needs a visible area-selection flow, local storage ownership and clear deletion behavior; no background capture. First prove the panel service with a note, timer completion and a CPU value, then add the user-facing screenshot command to that service.

### Resource and failure limits

Use one service per application, bounded panel counts and image sizes, and merge repeated updates to the same panel. No continuous animation or full-screen redraw for static notes. A CPU widget sends values at a modest bounded rate. An idle note causes no plugin polling merely to remain visible. Expose local diagnostics for panel count, dropped/coalesced updates, audio playback failures and timing, without logging note text or image content.

Sound output loss and panel rendering failures must not stop timers or device input. A plugin process crash marks its owned live panels unavailable and cancels outstanding callbacks, retaining saved content. Multi-monitor placement, click-through, native focus behavior and actual audio quality need real Windows acceptance. Tests of serialization alone cannot establish those outcomes.

## Goals vs. Non-Goals

* **In scope:** shared host sound and panel API, documented SDK support, bundled sound preview, optional timer notification settings, persistent and transient panel primitives, lifecycle and resource bounds, Windows 11 first with platform boundaries for macOS/Linux.
* **Separate existing work:** repair saved IO84 input routing and complete profile management without changing onboard assignments.
* **Not initial scope:** arbitrary plugin web pages inside overlays, a second widget marketplace, streaming video, network image loading, global keyboard capture, or a separate overlay runtime for every plugin.

## Alternatives Weighed

* A player/notification process in each plugin duplicates work and makes performance and cleanup inconsistent. Use a shared host boundary.
* OS notifications alone do not support persistent images and action widgets. They can remain an optional platform delivery method for ordinary notices.
* One full-screen transparent webview makes per-panel hit testing and click-through difficult. One webview per panel may multiply memory. Neither is selected without measurement.
* Closing panels on profile changes conflicts with the owner's confirmed workflow. Keeping only their last screenshot would also break live widgets. Preserve independent panel instances and give them a background service lifetime, with explicit close and unavailable states.
* Replaying saved device-key input for widget clicks can execute a different profile's action. Route by the saved panel owner and fresh service registration instead.
* F13 alone is not proof that a saved keyboard source is ready. Preserve device-specific pairing/readback; do not solve reconnect by falling back to a global hotkey.

## Key Decisions & Staging for Next Steps

1. Finish the independently authorized IO84 diagnosis and existing profile management. Keep all user assignments and onboard settings intact.
2. The owner resolved panel lifetime: persistent notes, images and live widgets survive profile changes and restore after restart. Present the updated anchored hypothesis for explicit EGM review readiness.
3. Run fresh-context Goldfish reviews of the API, lifecycle, resource cost, UX and maintenance risks. This brief has not passed that review yet.
4. Implement a bounded shared sound service, SDK contract and example; integrate timer transitions and hidden-page scheduling rather than render ticks. Verify no replay on reconnect.
5. Implement and measure one panel backend with note, CPU and timer examples; verify focus, mouse modes, multi-monitor/DPI and crash cleanup.
6. Add capture-and-pin on top of the proven service, with a visible selection flow and explicit ownership of saved images.

## Confirmed product behavior and acceptance boundaries

On 2026-11-03 the owner confirmed: restart restores the previous state; changing profiles has no effect on the desktop panels. Profiles are independent sets of pages that the user will switch frequently, as in Elgato.

The implementation must demonstrate a persistent note and a live CPU or timer widget while switching pages, folders and profiles; neither may disappear, duplicate or stop merely because its creating action is no longer visible. After a normal application restart, the content, position and settings return without focus theft or repeating an old sound. Closing a panel stays closed after restart. Plugin loss produces an unavailable live widget, and reconnection cannot replay a queued button event. A missing monitor keeps the panel reachable. Automated tests cover ownership and saved state; physical Windows checks cover focus, mouse pass-through, DPI/monitor placement and audible output.
