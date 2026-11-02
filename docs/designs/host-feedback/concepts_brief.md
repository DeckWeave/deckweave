# Shared sound and desktop panels

* **Author:** DeckWeave working hypothesis
* **Date:** 2026-11-03
* **Status:** Before the EGM readiness gate. No Goldfish reviews have run. Proposed APIs and behavior below are not implemented.

## Overview

Plugins need host services for audible feedback and information above other desktop windows. A timer should request a sound and a notification; a note, pinned image or system monitor should use the same panel service without starting another application. Configuration belongs beside the plugin action that owns it.

Profiles remain full sets of pages, folders and actions for one device. They are not part of the panel renderer. Hardware Main/Fn assignments remain common to the device while the active profile resolves host actions.

## Analysis

### Evidence from the current code

The core already stores a selected profile and additional profiles on each device, each with pages, folders, Fn bindings and an application association. AddProfile, SelectProfile and ProfileApplication are implemented. The editor exposed profile selection and creation behind a small settings icon, with no rename, copy or delete. The accompanying implementation extends that existing model rather than replacing it.

The Audio plugin controls master output through deckweave-platform. That is volume control, not a shared notification sound service. Timers currently render their state and preserve timer handoffs between visible contexts. Their draw path detects completion, but no shared sound or desktop panel API exists. A drawing tick must not become an alarm source: redraw, reconnect or opening a page must not replay an old completion.

Plugin processes, property inspectors, host context identifiers and lifecycle handling already exist. Host extensions must be optional and advertised, so ordinary Elgato plugins keep their existing protocol behavior. The plugin SDK and template are sibling repositories under G:/APPS/DW; the contract and example must ship there alongside host support.

### Shared sound service

Proposed calls are `ctx.playSound({ sound, volume, eventId })` and a host-provided list of bundled sounds. Names are provisional. The host validates the live plugin/context, deduplicates event IDs, bounds request rates and simultaneous playback, and returns a useful unsupported or playback error. Volume is local to this playback; it must never change master system volume.

Start with a small licensed, offline bundle: completion, short notice, and warning. Provide preview and volume in the plugin inspector. Custom local files can follow with a file picker and bounded decoded size; accepting arbitrary URLs or arbitrary paths from plugin messages is unnecessary for the first delivery. Do not launch a player process per sound. The service should sleep when idle, load short samples once as needed, and release audio resources on shutdown or output loss.

Timer integration must emit one completion event from a state transition with an identity separate from render/context lifetime. A sound started by an explicit Preview is separate from timer delivery. Decide deliberately whether an expired timer restored after restart announces itself; initial direction is to show its expired state without unexpectedly sounding an old alarm. Timers that must alarm while their page is hidden need a host-owned schedule, not a visible-action interval. This is a required functional boundary, not solved by adding playSound to draw().

### Desktop panel service

Proposed calls are `ctx.panels.upsert(localId, spec)`, `ctx.panels.close(localId)` and click events routed back to the owning live context. Names are provisional. The host assigns the owner namespace; plugin-supplied IDs cannot replace another plugin's panel. Update an existing panel in place rather than creating a window for every tick.

Use structured content first: text/note, image, progress/value, and explicit buttons bound to the owner's registered callbacks. Accept neither arbitrary HTML nor remote script in the first API. The service owns placement, theme, typography, monitor bounds, update coalescing and disposal. This keeps appearance consistent and prevents each plugin from creating its own browser or helper process. The rendering backend is still an engineering decision; a shared service does not by itself prove low memory use. Measure a Tauri implementation against the actual resource requirement before committing to many webviews or a native renderer.

Plugin settings contain monitor, corner/edge/free placement, offset, size, color/opacity, duration or pinned mode, and mouse mode. Separate `interactive` and `click-through`. A click-through panel still needs a recoverable way to move or dismiss it from DeckWeave settings. Showing or updating a panel must not steal keyboard focus. Clamp restored geometry to a connected monitor's work area and handle DPI and monitor removal.

Transient notifications should stack with bounds, expiry and deduplication. Persistent panels need stable identities, an inventory and an explicit close command. Plugin disable/removal closes its live interactive widgets and retires callbacks. No stale callback may execute after restart, action reassignment or profile change. A retained image or note can be independent of a live action, but that is a separate ownership policy requiring an explicit product decision.

Screenshot capture and image pinning are distinct from displaying an existing image. A later capture command needs a visible area-selection flow, local storage ownership and clear deletion behavior; no background capture. First prove the panel service with a note, timer completion and a CPU value, then add the user-facing screenshot command to that service.

### Resource and failure limits

Use one service per application, bounded panel counts and image sizes, and merge repeated updates to the same panel. No continuous animation or full-screen redraw for static notes. A CPU widget sends values at a modest bounded rate. An idle note causes no plugin polling merely to remain visible. Expose local diagnostics for panel count, dropped/coalesced updates, audio playback failures and timing, without logging note text or image content.

Sound output loss and panel rendering failures must not stop timers or device input. A plugin process crash closes or marks its owned live panels and cancels outstanding callbacks. Multi-monitor placement, click-through, native focus behavior and actual audio quality need real Windows acceptance. Tests of serialization alone cannot establish those outcomes.

## Goals vs. Non-Goals

* **In scope:** shared host sound and panel API, documented SDK support, bundled sound preview, optional timer notification settings, persistent and transient panel primitives, lifecycle and resource bounds, Windows 11 first with platform boundaries for macOS/Linux.
* **Separate existing work:** repair saved IO84 input routing and complete profile management without changing onboard assignments.
* **Not initial scope:** arbitrary plugin web pages inside overlays, a second widget marketplace, streaming video, network image loading, global keyboard capture, or a separate overlay runtime for every plugin.

## Alternatives Weighed

* A player/notification process in each plugin duplicates work and makes performance and cleanup inconsistent. Use a shared host boundary.
* OS notifications alone do not support persistent images and action widgets. They can remain an optional platform delivery method for ordinary notices.
* One full-screen transparent webview makes per-panel hit testing and click-through difficult. One webview per panel may multiply memory. Neither is selected without measurement.
* Automatically preserving all action panels across profile changes risks stale controls and visual clutter. Separate live action widgets from user-pinned content.
* F13 alone is not proof that a saved keyboard source is ready. Preserve device-specific pairing/readback; do not solve reconnect by falling back to a global hotkey.

## Key Decisions & Staging for Next Steps

1. Finish the independently authorized IO84 diagnosis and existing profile management. Keep all user assignments and onboard settings intact.
2. Resolve the user-facing lifetime of pinned content versus action widgets. Then present the anchored hypothesis for explicit EGM review readiness.
3. Run fresh-context Goldfish reviews of the API, lifecycle, resource cost, UX and maintenance risks. This brief has not passed that review yet.
4. Implement a bounded shared sound service, SDK contract and example; integrate timer transitions and hidden-page scheduling rather than render ticks. Verify no replay on reconnect.
5. Implement and measure one panel backend with note, CPU and timer examples; verify focus, mouse modes, multi-monitor/DPI and crash cleanup.
6. Add capture-and-pin on top of the proven service, with a visible selection flow and explicit ownership of saved images.

## Product question still open

What should happen to user-pinned notes and images, and to plugin action widgets, when their profile changes or DeckWeave restarts? This affects persistence, cleanup, settings location and whether an on-screen control can still perform an action. No final policy is assumed here.
