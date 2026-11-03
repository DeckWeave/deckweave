// Illustrative display states, passed through the actual Mirabox JPEG renderer.
import { writeFile, mkdir } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { ROOT } from './personal-workspace.mjs';
import { render as clock } from '../plugins/dev.deckweave.clock.sdPlugin/clock.mjs';
import { render as weather } from '../plugins/dev.deckweave.weather.sdPlugin/weather.mjs';
import { render as monitor } from '../plugins/dev.deckweave.monitor.sdPlugin/monitor.mjs';
import { render as focus } from '../plugins/dev.deckweave.focus.sdPlugin/focus.mjs';
import {
  render as command,
  settings,
} from '../plugins/dev.deckweave.workspace.sdPlugin/operations.mjs';
const uri = (svg) => 'data:image/svg+xml;base64,' + Buffer.from(svg).toString('base64');
const date = new Date('2026-11-03T07:34:00Z'),
  now = +date;
const job = {
  phase: 'work',
  status: 'running',
  round: 2,
  config: { rounds: 4 },
  live_remaining_ms: 1245000,
};
const options = { language: 'ru', timezone: 'Asia/Yekaterinburg' };
const metrics = {
  cpu: 18,
  memory: 68,
  usedGiB: 43.5,
  totalGiB: 64,
  uptime: 90061,
  gpu: {
    sampled_at_ms: now,
    selected_adapter_id: 'fixture',
    adapters: [
      {
        id: 'fixture',
        index: 0,
        name: 'Illustrative RTX',
        dedicated_used_bytes: 3.2 * 2 ** 30,
        dedicated_total_bytes: 24 * 2 ** 30,
        busiest_engine_percent: 32,
        gpu_status: 'ready',
        memory_status: 'ready',
      },
    ],
  },
};
const data = {
  observed: now / 1000,
  temperature: -2,
  feels: -6,
  code: 3,
  day: true,
  timezone: 'Asia/Yekaterinburg',
  sunsets: [],
  hours: [],
};
const images = {
  strip: [
    weather(data, { ...options, name: 'Екатеринбург' }, 'current', [300, 128], now / 1000),
    clock(date, options, [220, 128]),
    focus(job, options, [280, 128]),
  ].map(uri),
  keys: [
    ...['cpu', 'memory', 'gpu', 'gpu-memory'].map((metric) =>
      monitor(metric, metrics, options, [112, 112], now),
    ),
    focus(job, options, [112, 112]),
    clock(date, options, [112, 112]),
    ...[
      'report.preview',
      'report.pdf',
      'report.formula',
      'report.reference',
      'report.image',
      'report.table',
    ].map((operation) => command(settings({ operation, language: 'ru' }), [112, 112])),
  ].map(uri),
};
const target = path.join(ROOT, '.local/personal-workspace-artwork');
await mkdir(target, { recursive: true });
await writeFile(path.join(target, 'input.json'), JSON.stringify(images));
const destination = path.join(ROOT, 'docs/evidence/2026-11-03/personal-workspace-widgets.png');
const run = spawnSync(
  'cargo',
  [
    'run',
    '-p',
    'deckweave-n4',
    '--example',
    'render-widgets',
    '--',
    path.join(target, 'input.json'),
    destination,
  ],
  { cwd: ROOT, encoding: 'utf8' },
);
if (run.status !== 0) throw Error(run.stderr + run.stdout);
console.log(destination);
