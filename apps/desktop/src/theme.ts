import type { Preferences } from './types';

export function luminance(hex: string): number {
  const channels = hex
    .slice(1)
    .match(/../g)!
    .map((v) => {
      const n = parseInt(v, 16) / 255;
      return n <= 0.04045 ? n / 12.92 : ((n + 0.055) / 1.055) ** 2.4;
    });
  return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
}

export function themeColors(theme: 'light' | 'dark', color: string) {
  const accent = /^#[\da-f]{6}$/i.test(color) ? color : '#e8b466';
  const contrast = (a: number, b: number) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  const l = luminance(accent);
  // Arbitrary accents can be white, black or low contrast. Keep selected text
  // readable and derive a separate focus/link color against the panel surface.
  const ink = l > 0.179 ? '#000000' : '#ffffff';
  const panel = theme === 'light' ? '#ffffff' : '#252525';
  let focus = accent;
  const target = theme === 'light' ? 0 : 255;
  for (let step = 1; contrast(luminance(focus), luminance(panel)) < 4.5 && step <= 20; step++) {
    focus =
      '#' +
      accent
        .slice(1)
        .match(/../g)!
        .map((v) =>
          Math.round(parseInt(v, 16) * (1 - step / 20) + target * (step / 20))
            .toString(16)
            .padStart(2, '0'),
        )
        .join('');
  }
  return { accent, ink, focus };
}

export function applyTheme(
  root: HTMLElement,
  preferences?: Pick<Preferences, 'theme' | 'accent_color'>,
) {
  const theme = preferences?.theme ?? 'dark';
  const { accent, ink, focus } = themeColors(theme, preferences?.accent_color ?? '#e8b466');
  root.dataset.theme = theme;
  root.style.setProperty('--accent', accent);
  root.style.setProperty('--accent-ink', ink);
  root.style.setProperty('--focus', focus);
  root.style.setProperty('--preview-focus', themeColors('dark', accent).focus);
}
