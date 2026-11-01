# N4 output latency

The hardware output worker previously called a blocking `read_timeout(..., 25)` before **every** image. An idle input queue therefore added 275 ms of intentional waiting to an eleven-frame update (ten keys and the strip), before rendering and USB transfers. It also emitted a full application-change notification after each successful frame, making the editor repeatedly fetch the complete snapshot and preview images.

## Scheduling and prepared images

- Pending output or a new invalidation uses a zero-timeout input poll. An idle worker still waits up to 25 ms, so it does not spin.
- The worker reads input between every complete BAT/BGPIC–chunks–STP transaction. A continuously changing first key cannot starve other keys; the existing round-robin selection remains.
- A per-device LRU cache retains at most 64 prepared JPEG/data-URL pairs and 8 MiB of counted source/image bytes. It caches pixels, excluding page/binding identity, and does no rendering, file access or USB work under the Core mutex.
- Identical pixels on another page update preview metadata without another HID transfer. Blank keys and pinned controls can therefore stay in place. Cache contents are local to the connection session; reconnect starts with a fresh transfer of all controls.
- Preview-change notifications are coalesced to approximately 30 Hz during output and always emitted when output settles. Successful-frame counters remain exact.
- The worker retains current geometry for routing input instead of cloning the complete configuration and every plugin image for each physical input report.

The accepted JPEG mode is unchanged: baseline 4:2:0, standard Huffman tables, quality attempts 85/65/45, 180-degree rotation and the existing payload limits. No USB report size, transaction format or device firmware assumption is changed.

## Measurements

`%APPDATA%\app.deckweave.studio\n4-diagnostics.json` now includes `output_performance` per device. Its bounded counters report:

| Field | What is measured |
| --- | --- |
| `pending_input_reads`, `idle_input_reads` | Calls, total and maximum microseconds spent in HID input reads |
| `snapshots` | Core-lock acquisition and snapshot cloning time |
| `render_and_encode` | Cache lookup or rendering/JPEG preparation time |
| `hid_transfers` | Complete successful image transactions |
| `cache_hits`, `cache_entries`, `cache_bytes` | Prepared-image reuse and cache bounds |
| `preview_notifications` | Coalesced output-change events |
| `page_batches` | Most recent 32 page changes: elapsed time, output frames, render/HID/input-wait breakdown and whether a newer page superseded the update |

Page timings start when the worker obtains the snapshot containing the changed page, including that snapshot cost, and finish after the last required HID transaction. They are **not** physical display acknowledgements, nor complete mouse-click-to-pixel timings. Dynamic plugin images arriving later are separate output updates. A superseded batch is retained rather than presented as a completed page.

Performance publications have a revision independent of Core events. A completed batch published just after the last frame's Core event therefore still reaches the file without another input. Metrics-only changes, including idle input-read totals, refresh at most once every five seconds; actual Core changes retain the existing one-second supervisor cadence. Failed file writes do not consume the pending revision.

For a repeatable CPU-only sample, run:

```powershell
cargo test -p deckweave-desktop hardware::tests::output_render_benchmark --release -- --ignored --nocapture
```

This sample renders twenty default eleven-frame pages both without reuse and with a warm cache; it never opens USB. Its numbers must not be reported as physical device latency. The old 275 ms scheduling floor is derived directly from eleven empty 25 ms reads. The new pending-read floor is zero; rendering, USB and firmware presentation still take time.

Release sample on the development PC, 2026-11-02: twenty cold eleven-frame pages took **34,038 µs total (1.702 ms/page)**; the same twenty pages with a warm prepared-image cache took **16 µs total**, retaining 54,551 counted cache bytes. These are default builtin visuals, not an SVG-heavy plugin workload. [Raw CPU sample](evidence/2026-11-02/n4-output-cpu.json) records the scope and command. The large avoidable cost in this workload was the 275 ms input-wait floor, rather than CPU rendering.

Regression tests cover polling between every frame, idle blocking, fair selection, reuse across changed page/binding identity, actual changed-pixel detection, cross-page cached JPEG reuse, and both cache limits. Existing renderer tests continue to enforce the accepted baseline JPEG sampling and packet budgets.

## Remaining hardware observation

An accelerated release should be observed on Mirabox while switching several real pages and operating a dial/key. Diagnostics can establish successful transfer timings and input delivery automatically. Only the device owner can confirm physical presentation quality or a firmware display lag that is absent from HID write timings. Existing user assignments must be preserved when measuring.
