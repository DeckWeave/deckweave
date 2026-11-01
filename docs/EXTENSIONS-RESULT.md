# Library, output and starter extensions — 2026-11-02

## Delivered

- Display editor next to action search, modelled on the installed Stream Deck's
  `startEditButton`: show/hide groups and individual actions, reorder with arrows,
  restore defaults. Global SQLite preferences apply to all windows. Hiding does
  not disable assignments. Native UI checked order, hiding and restore.
- Honour `VisibleInActionsList`; hidden tutorial actions stay out of the ordinary
  list. IconEditor-only packages are resource libraries. Unsupported packages
  appear in Settings with their reason. Development demos are opt-in. Existing
  vendor installations were not deleted or modified; Elgato Weather stays off.
- N4 scheduling/cache/notification changes: [implementation and limits](N4-OUTPUT-PERFORMANCE.md).
  The tested JPEG encoding is unchanged.
- Independent action SDK, generator and validated optional registry in the three
  companion repositories. Windows/Linux CI definitions for SDK/template/registry;
  Windows main CI. No public releases, hosted CI execution or installer claimed.
- Three useful source-distributed plugins: Windows master Audio, CPU/RAM/uptime
  Monitor, countdown/focus and stopwatch Timers. Keypad, Encoder feedback and
  native Panel support. Standalone ZIP packaging includes licenses and SHA-256;
  Audio refuses a missing helper. [Author workflow and boundaries](EXTENSIONS.md).
- Host accepts bounded, validated final settings from recently retired contexts,
  protecting later PI edits. Timers bridge immediate return with bounded local
  handoffs. [Independent findings and fixes](EXTENSIONS-REVIEW.md).

## Evidence

`npm run check` passed: Svelte 0 errors/warnings, formatting, production web,
Rust fmt/clippy with denied warnings, **87 Rust tests** (one manual CPU benchmark
excluded from the routine suite), **19 application JS tests**, documentation.
The CPU benchmark was run separately and passed. Companion suites: SDK 8,
template 2 integration tests (including 5 generated-plugin tests), registry 8.
Registry workspace and strict distribution-file preflights passed.

Release built with `npm run build`; executable SHA-256:
`60c163fa045be87343f974cff8c6683b7e3a7f8090cf8a75cfdc4a197e503658`, 19,412,992 bytes.
Native helper read-only status matched Windows' current 66% / unmuted; automatic
checks never changed actual master volume. Fake-helper tests cover ordered mixed
touch/dial input and cancellation during startup/disappearance.

Native UI: drag CPU/RAM/uptime/countdown/stopwatch onto keys, Audio and RAM into
Panel regions, timer onto a dial, page navigation onto D4 on both pages.
Configured the dial timer to 3 seconds through its real en/ru PI, started it and
observed green completion. The key countdown remained 25 minutes independently.
After restart Tools, assignments, library defaults and artwork returned.
Audio initially revealed a missing font in the rasterizer; switching its SVG to
the bundled IBM Plex Sans fixed the actual image. [Native screenshot](evidence/2026-11-02/extensions-tools.png).

Real N4 successful-write batches: startup eleven frames **6.309 ms**, Default page
with assignments eleven frames **11.034 ms**, two Tools transitions with six
changed frames **4.204 / 5.947 ms**. Unchanged keys reused their pixels. These
exclude later plugin artwork and are not LCD acknowledgement or click-to-pixel
measurements. [Sanitized USB data](evidence/2026-11-02/n4-output-usb.json).
The removed 275 ms minimum was a source-proven eleven × 25 ms idle-read delay;
there is no fabricated before/after physical LCD measurement.

## Resource observations

Ten-second samples of the complete process tree, including Node/helper/WebView:

| Scenario | Private commit | CPU seconds / elapsed | Processes |
| --- | --- | --- | --- |
| Settings + Tools editor | 545.9 MiB | 1.3906 / 10.173 s | 17 |
| Tools editor only | 509.1 MiB | 1.0469 / 10.159 s | 16 |
| All windows closed, physical Tools continues | 231.1 MiB | 0.0938 / 10.163 s | 10 |

The detailed editor sample attributes about 238 MiB to WebView2, 179 MiB to four
Node backends, 83 MiB to the Rust host and 2 MiB to the audio helper. Closing all
windows removed all WebView2 descendants in the observed run. Existing configured
virtual-device plugins were preserved and are included; these are not minimal
one-plugin samples. Private commit is not working set; summed working sets can
count shared pages repeatedly. CPU seconds are aggregate across processes, not
normalized machine CPU percentages. No process churn during these samples.

[Two windows](evidence/2026-11-02/extensions-idle.json),
[one editor](evidence/2026-11-02/extensions-editor-idle.json),
[process breakdown](evidence/2026-11-02/extensions-processes.json),
[background](evidence/2026-11-02/extensions-background-idle.json).
These are short observations, not an agreed resource budget or endurance test.
Further memory reduction belongs to a measured profiling pass; it is not solved
by the output-latency fix.

## Next

Only [two physical observations](EXTENSIONS-ACCEPTANCE.md) remain for this slice.
Next implementation work: full profile/page/folder management and clearer nested
action editing; measured memory profiling; application distribution/runtime
packaging. The separate external hardware-provider SDK remains future work.
