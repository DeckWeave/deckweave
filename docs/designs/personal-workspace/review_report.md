# Personal workspace: review disposition

Date: 2026-11-03. Six fresh EGM reviewers completed; no blockers were reported.
The owner delegated defaults and implementation, superseding further clarification
and handoff gates for this run. Scope remains the working hypothesis.

| Reviewer | Main finding | Resolution |
| --- | --- | --- |
| Design | Panel timers cannot represent a shared Pomodoro; sound cannot be guaranteed exactly once | Separate durable versioned jobs, guarded commands, at-most-once sound attempts with outcomes; silent restart |
| Codebase | GPU absent; targeting and setup recovery lack evidence | Persistent PDH/DXGI helper; targeted native helper and VS Code bridge; staged Core commands, backup, single revision-guarded commit |
| Zoom out | Avoid a monitoring rewrite; allocate controls and strip space | Retained CPU/RAM producer; bounded GPU source; 300/220/280 strip; Home and D4 page cycle |
| Comprehension | Search, document context and errors ambiguous | Address focus/Find only; Typst/trust/focus/stale-selection guards; readable errors and native-size rendering |
| Readiness | Rollback must include host services; old timer identity must survive | Both databases backed up; additive focus records/new action IDs; old countdown/stopwatch preserved |
| Structural | Smart Profiles may leave the new profile; copied strips can drift | Mirabox has no app associations; selected device default, preserved global preference; common strip generator |

Implementation review also corrected native navigation keys' small text-only
fallback. Vector icons and captions retain plugin artwork and error priority.

Verified: fake-time state/restart/conflict behavior; real SDK hidden-profile
completion and one silent Windows sound outcome; real GPU samples; real isolated
VS Code focus/pipe/Typewriter/undo; preserved configuration diff; reapplication;
package validation; native JPEG images; live device editor. Owner acceptance is
physical readability, sound comfort and personal-project PDF/preview behavior.
See [usage and evidence](../../PERSONAL-WORKSPACE.md).
