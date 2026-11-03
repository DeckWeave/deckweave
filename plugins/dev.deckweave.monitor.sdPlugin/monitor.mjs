// SPDX-License-Identifier: GPL-3.0-or-later
import { escapeXml } from './lib/sdk.mjs';
export function cpuSample(cpus) {
  return cpus.reduce(
    (sum, cpu) => ({
      idle: sum.idle + cpu.times.idle,
      total: sum.total + Object.values(cpu.times).reduce((a, b) => a + b, 0),
    }),
    { idle: 0, total: 0 },
  );
}
export function utilization(previous, current) {
  const total = current.total - previous.total,
    idle = current.idle - previous.idle;
  return total <= 0 || idle < 0 ? null : Math.max(0, Math.min(100, (1 - idle / total) * 100));
}
export function uptime(seconds) {
  const minutes = Math.max(0, Math.floor(seconds / 60));
  return minutes >= 1440
    ? `${Math.floor(minutes / 1440)}d ${Math.floor(minutes / 60) % 24}h`
    : `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}
export const gpuMetric = (action) => ['gpu', 'gpu-memory'].includes(action.split('.').at(-1));
const finite = (value) => Number.isFinite(value);
const gib = (value) => value / 2 ** 30;

/** Visible values and diagnostics share the same freshness/selection decisions. */
export function metricView(metric, data, settings = {}, now = Date.now()) {
  const ru = settings.language === 'ru',
    type = metric.split('.').at(-1);
  const label = String(
    settings.label ||
      {
        cpu: 'CPU',
        memory: 'RAM',
        uptime: ru ? 'Работа ОС' : 'Uptime',
        gpu: 'GPU',
        'gpu-memory': 'VRAM',
      }[type] ||
      'GPU',
  ).slice(0, 24);
  const unavailable = {
    label,
    value: '—',
    detail: ru ? 'Нет данных' : 'Unavailable',
    status: 'unavailable',
    percent: null,
  };
  if (gpuMetric(metric)) {
    const sample = data.gpu;
    const adapter = sample?.adapters.find(
      (a) => a.id === (settings.adapterId || sample.selected_adapter_id),
    );
    if (!sample && !data.gpuError)
      return {
        ...unavailable,
        status: 'warming_up',
        value: '…',
        detail: ru ? 'Первый замер' : 'Warming up',
        reason: 'Starting GPU sampler',
      };
    if (!sample || !adapter)
      return {
        ...unavailable,
        reason:
          data.gpuError ||
          sample?.reason ||
          (settings.adapterId
            ? 'Selected adapter is not connected'
            : 'GPU helper has no adapter data'),
      };
    const shortName = `GPU ${adapter.index}`;
    const age = now - sample.sampled_at_ms;
    if (age > 6500 || age < -1000)
      return {
        ...unavailable,
        status: 'stale',
        detail: ru ? 'Устарело' : 'Stale sample',
        adapter,
        reason: data.gpuError || `Last sample ${Math.max(0, Math.round(age / 1000))} seconds ago`,
      };
    const memory = type === 'gpu-memory',
      status = memory ? adapter.memory_status : adapter.gpu_status;
    if (status !== 'ready')
      return {
        ...unavailable,
        status,
        value: status === 'warming_up' ? '…' : '—',
        detail:
          status === 'warming_up'
            ? ru
              ? 'Первый замер'
              : 'Warming up'
            : ru
              ? 'Нет данных'
              : 'Unavailable',
        adapter,
        reason: memory ? adapter.memory_reason : adapter.gpu_reason,
      };
    if (memory)
      return {
        label: settings.label ? label : `VRAM · ${adapter.index}`,
        value: `${gib(adapter.dedicated_used_bytes).toFixed(1)}`,
        unit: 'GiB',
        detail: `/ ${gib(adapter.dedicated_total_bytes).toFixed(1)} GiB`,
        percent: Math.min(
          100,
          (adapter.dedicated_used_bytes / adapter.dedicated_total_bytes) * 100,
        ),
        status,
        adapter,
        reason: null,
      };
    return {
      label,
      value: `${Math.round(adapter.busiest_engine_percent)}%`,
      detail: shortName,
      percent: adapter.busiest_engine_percent,
      status,
      adapter,
      reason: null,
    };
  }
  if (type === 'uptime')
    return {
      label,
      value: uptime(data.uptime),
      detail: ru ? 'С запуска ОС' : 'Since OS boot',
      percent: null,
      status: 'ready',
    };
  const percent = type === 'cpu' ? data.cpu : data.memory;
  if (!finite(percent))
    return type === 'cpu'
      ? {
          ...unavailable,
          value: '…',
          status: 'warming_up',
          detail: ru ? 'Первый замер' : 'Warming up',
        }
      : unavailable;
  return {
    label,
    value: `${Math.round(percent)}%`,
    percent,
    status: 'ready',
    detail:
      type === 'cpu'
        ? ru
          ? 'Вся система'
          : 'Whole system'
        : `${data.usedGiB.toFixed(1)}/${data.totalGiB.toFixed(0)} GiB`,
  };
}

export function render(metric, data, settings, canvas, now) {
  const w = Math.max(32, Math.min(4096, Number(canvas[0]) || 112));
  const h = Math.max(24, Math.min(1024, Number(canvas[1]) || 112));
  const view = metricView(metric, data, settings, now);
  const wide = w / h > 2.6;
  // A warm neutral surface lets the large number carry the hierarchy. Minimum
  // 13px detail at the native 112px key; no microscopic instructions or decoration.
  const scale = Math.min(w / 112, h / 112),
    inset = 9 * scale;
  const labelSize = Math.min(20, 15 * scale),
    detailSize = Math.min(20, 13 * scale);
  const valueSize = Math.min(h * (wide ? 0.65 : 0.38), w / Math.max(2.8, view.value.length * 0.64));
  const valueColor =
    view.status === 'ready' ? (view.percent >= 85 ? '#edbd7e' : '#f6f2e9') : '#c9bca6';
  const text = (x, y, size, value, color, anchor = 'start') =>
    `<text x="${x}" y="${y}" text-anchor="${anchor}" font-family="IBM Plex Sans, Segoe UI, sans-serif" font-size="${size}" fill="${color}">${escapeXml(value)}</text>`;
  const label =
    view.label.length > (wide ? 24 : 12) ? view.label.slice(0, wide ? 23 : 11) + '…' : view.label;
  const detail =
    view.detail.length > (wide ? 34 : 21)
      ? view.detail.slice(0, wide ? 33 : 20) + '…'
      : view.detail;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}"><rect width="100%" height="100%" fill="#242321"/>${
    wide
      ? text(inset, h * 0.33, labelSize, label, '#e1d7c7') +
        text(inset, h * 0.76, detailSize, detail, '#c9bca6') +
        text(w - inset, h * 0.7, valueSize, view.value, valueColor, 'end')
      : text(inset, h * 0.22, labelSize, label, '#e1d7c7') +
        `<g font-weight="600">${text(w / 2, h * 0.66, valueSize, view.value, valueColor, 'middle')}</g>` +
        text(w / 2, h * 0.89, detailSize, detail, '#c9bca6', 'middle')
  }</svg>`;
}
