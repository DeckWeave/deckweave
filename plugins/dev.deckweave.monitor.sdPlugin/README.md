# System Monitor

Optional DeckWeave action plugin, GPL-3.0-or-later. Node 22+, no dependencies,
network access or elevated privileges. SDK 0.1.0 is vendored in `lib/`.

Drag CPU, Memory, GPU, Dedicated GPU memory or Uptime onto a key, dial area or Panel
region. Settings change the label, language and GPU adapter. CPU is the whole
system average over a two-second sample; the first sample is a warm-up. Memory
means used physical RAM (OS caches may be included), uptime is since OS boot.
Pressing widgets does nothing unless the optional desktop-panel setting is enabled.

GPU uses a single persistent native `deckweave-system.exe` helper while GPU widgets
or open GPU metric panels need it. The same two-second sampler serves all consumers;
no helper process is launched per sample. Sampling and the helper stop when the last
consumer disappears. CPU/RAM alone never launch the helper. No network requests.

Windows WDDM 2+ GPU Engine PDH counters are grouped by adapter LUID, physical GPU and
engine; process values are summed per engine, then the busiest engine is shown.
This is not an average or a sum across different engines. Adapter memory comes from
`GPU Adapter Memory / Dedicated Usage`, not per-process allocation counters. DXGI
provides adapter names/LUIDs and dedicated-video-memory capacity. Values are GiB.
The default selects the adapter with the busiest engine; GPU and VRAM widgets share
that choice. Pin a specific adapter in settings when desired. A removed pinned
adapter reports unavailable rather than silently changing identity.

Warm-up, unavailable and samples older than 6.5 seconds have distinct states. Zero
is displayed only for valid sampled zero. An adapter with no dedicated video-memory
capacity reports VRAM unavailable. The inspector shows the full adapter identity,
sample age and reason. Helper failures are logged locally with a bounded message;
timeouts kill the helper and the next two-second tick can recover. One request may
be in flight; repeated consumers share it. Native PDH arrays are bounded to 4 MiB,
helper responses to 64 KiB, and native sampling to once per second at most.

Build and install from the source checkout:

```powershell
cargo build -p deckweave-platform --bin deckweave-system --release --locked
New-Item -ItemType Directory -Force plugins/dev.deckweave.monitor.sdPlugin/bin
Copy-Item target/release/deckweave-system.exe plugins/dev.deckweave.monitor.sdPlugin/bin/
node plugins/dev.deckweave.monitor.sdPlugin/tools/package.mjs pack
```

Distribute the complete plugin folder including `bin/deckweave-system.exe`. A
source-tree fallback finds `target/release` then `target/debug` beside Cargo.toml.
The helper preserves the Audio plugin protocol; `{id:1,command:'gpu_sample'}` is an
additional read-only newline JSON command. It returns status, sample timestamp,
selected adapter ID and adapter values with separate GPU/VRAM status and reasons.

The same SVG adapts to key, Encoder `$A0` feedback and actual Panel canvas sizes.
Tests: `node --test plugins/dev.deckweave.monitor.sdPlugin/monitor.test.mjs
scripts/tests/starter-plugins.test.mjs` and `cargo test -p deckweave-platform --lib
--bin deckweave-system --locked` from the app repository.

Native API references: [PDH formatted arrays](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhgetformattedcounterarrayw),
[language-neutral counters](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhaddenglishcounterw),
[DXGI adapter identity/capacity](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ns-dxgi-dxgi_adapter_desc1),
[busiest-engine meaning](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/).
