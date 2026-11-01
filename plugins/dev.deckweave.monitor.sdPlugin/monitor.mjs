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
export function render(metric, data, settings, canvas) {
  const w = Math.max(32, Math.min(4096, Number(canvas[0]) || 112)),
    h = Math.max(24, Math.min(1024, Number(canvas[1]) || 112));
  const ru = settings.language === 'ru',
    type = metric.split('.').at(-1);
  const percent = type === 'cpu' ? data.cpu : type === 'memory' ? data.memory : null;
  const label = String(
    settings.label ||
      (type === 'cpu'
        ? 'CPU'
        : type === 'memory'
          ? ru
            ? 'Память'
            : 'Memory'
          : ru
            ? 'Время работы'
            : 'Uptime'),
  ).slice(0, 24);
  const value =
    type === 'uptime' ? uptime(data.uptime) : percent === null ? '…' : `${Math.round(percent)}%`;
  const detail =
    type === 'memory'
      ? `${data.usedGiB.toFixed(1)} / ${data.totalGiB.toFixed(1)} GiB`
      : type === 'cpu'
        ? ru
          ? 'Весь процессор'
          : 'Whole system'
        : ru
          ? 'С запуска ОС'
          : 'Since OS boot';
  const scale = Math.min(w / 112, h / 112),
    wide = w / h > 2;
  const font = wide
    ? Math.min(h * 0.46, w / 9)
    : Math.min(h * 0.3, (w / Math.max(3, value.length)) * 1.4);
  const accent = percent !== null && percent >= 85 ? '#ffb468' : '#8bd5fb';
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}"><rect width="100%" height="100%" fill="#152334"/><text x="${8 * scale}" y="${19 * scale}" font-family="IBM Plex Sans" font-size="${14 * scale}" fill="#d8e6f5">${escapeXml(label)}</text><text x="${w / 2}" y="${wide ? h * 0.64 : h * 0.6}" text-anchor="middle" font-family="IBM Plex Sans" font-size="${font}" font-weight="600" fill="${accent}">${escapeXml(value)}</text><text x="${w / 2}" y="${h - 12 * scale}" text-anchor="middle" font-family="IBM Plex Sans" font-size="${10 * scale}" fill="#d8e6f5">${escapeXml(detail)}</text>${percent === null ? '' : `<rect x="${6 * scale}" y="${h - 5 * scale}" width="${((w - 12 * scale) * percent) / 100}" height="${3 * scale}" fill="${accent}"/>`}</svg>`;
}
