# Personal daily workspace

* **Author:** DeckWeave maintainers with the owner
* **Date:** 2026-11-03

## Overview

Create a usable Mirabox workspace for reports, development, communication and
system monitoring. The owner delegated initial layout and settings choices.
Keep the existing profiles and keyboard firmware untouched. Implement the missing
capabilities as reusable plugins and small host services.

## Analysis

Six fresh EGM reviewers completed static reviews. Their main findings were a
missing shared Pomodoro identity, unsafe untargeted hotkeys, GPU sampling gaps,
and setup recovery. Additional findings concerned document eligibility, strip
space, default-profile routing and compatibility with old timers.

Use one durable host Pomodoro job for all page projections. Add separate versioned
records rather than extending old panel content or reinterpreting countdown IDs.
Persist transitions, not display ticks. A transition attempts sound once; show
completion even if sound fails. Do not replay historical sounds after restart.

Keep the existing shared CPU/RAM producer and add one persistent native GPU helper.
Poll at two-second intervals while needed. Report the selected adapter and valid,
warming-up, stale and unavailable samples separately. No process per sample.

Commands must verify their target. A local VS Code companion executes an explicit
allowlist and checks the active Typst document before insertion/format/export.
Use existing Typewriter/Tinymist behavior and normal undo; export uses their own
configured destination. Browser search means address-bar focus or in-page Find;
the user types and submits. Codex desktop navigation and VS Code Codex commands
are separate targets. No prompt submission or command approval is automated.

Use a local setup manifest, preflight and SQLite backup. Stage profile changes
through Core commands, then commit the staged configuration with one revision
check. Preserve unrelated settings. Repeated setup recognizes its own profile;
never silently overwrites a profile changed by the owner. Keep a restore set for
configuration, host-service data, installed companion and compatible executable.

Mirabox has no existing application associations. Make the personal profile its
default, recording the former default for recovery. Keep global Smart Profiles.
Generate each page's strip from one layout helper, with distinct binding IDs.

## Goals vs. Non-Goals

* **In-Scope Goals:** app launch/focus; installed report commands; safe Codex and
  browser navigation; CPU/RAM/GPU/VRAM; date/time; explicit-location weather;
  work/break Pomodoro with host sound; repeatable personal setup.
* **Out-of-Scope Non-Goals:** Telegram/email content, note previews, keyboard
  firmware changes, automatic prompts, cloud monitoring and unsolicited overlays.

## Alternatives Weighed

* Global hotkeys alone are smaller but can affect an unrelated application.
  Use a targeted helper and an editor command bridge.
* A timer in each page binding is simpler but creates competing runs. Use one
  host job and independent visible projections.
* Opening a desktop timer panel would reuse existing UI but consumes another
  webview and does not meet the hardware-first request. Keep overlay optional.
* Replacing CPU/RAM monitoring with a general telemetry framework adds work
  without user value. Extend the existing plugin with bounded GPU sampling.
* A general page-template inheritance feature would reduce duplication but adds
  unnecessary model complexity. Generate this setup from a shared strip helper.

## Key Decisions & Staging for Next Steps

Home opens VS Code, Codex desktop, browser, Telegram, Telemost and Zotero, plus
four task-page links. Each task page reserves key 10 for Home. D4 cycles pages.
D1 controls volume. D2 controls Pomodoro. Secondary timer operations use labelled
System-page keys. The strip allocates 300×128 to weather, 220×128 to date/time
and 280×128 to Pomodoro. Timer tap toggles running/paused; reset and skip have
separate labelled controls. Hardware widgets use large text and neutral dark
backgrounds. Action failures use the action's temporary error feedback; metric
and weather failures remain explicit in their own images.

Defaults are 25/5/15 minutes, four work rounds, manual next-phase start and a
moderate notification volume. Preserve the existing Mirabox JPEG transport.
Inspect rendered images at actual dimensions, test state/IPC/recovery boundaries,
then apply and launch the release. Only hardware legibility and physical comfort
need owner feedback after automated checks.
