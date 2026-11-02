import type { Binding, KeyboardAlias, Locale } from './types';
import type { ProviderSessionState } from './provider-ui';

export type KeyboardLayer = 'main' | 'fn';
// Selection and assignment can happen in one drag/drop gesture, before Svelte
// flushes its effects. Keep the proposal made for the new context; discard only
// a proposal captured for a different selection/profile/layer/workspace.
export function keyboardPreviewForContext<T extends { context: string }>(
  preview: T | null,
  context: string,
): T | null {
  return preview?.context === context ? preview : null;
}
export function keyboardOwner(
  owners: KeyboardAlias[] | undefined,
  controlId: string,
  layer: KeyboardLayer,
) {
  const matches = (owners ?? []).filter(
    (owner) => owner.control_id === controlId && owner.layer === layer,
  );
  return (
    matches
      .filter((owner) => owner.provenance === 'deckweave' && owner.phase !== 'released')
      .at(-1) ?? matches.at(-1)
  );
}

export function keyboardPreviewCurrent(
  captured: OnboardAssignment | null,
  generation: number | null,
  current: OnboardAssignment,
  currentGeneration: number | undefined,
) {
  return (
    generation !== null &&
    generation === currentGeneration &&
    !!captured?.record &&
    captured.record.length === 4 &&
    captured.record.every((n, i) => n === current.record?.[i])
  );
}
export interface OnboardAssignment {
  label: string;
  kind: string;
  known: boolean;
  observed: boolean;
  record: number[] | null;
  usage: number | null;
  replaceable: boolean;
}
const object = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;

// This fallback deliberately decodes keyboard usages only. Device functions come
// from the adapter's catalogue, never from the physical key's position or name.
function keyboardUsage(usage: number): string | null {
  if (usage >= 4 && usage <= 29) return String.fromCharCode(65 + usage - 4);
  if (usage >= 30 && usage <= 38) return String(usage - 29);
  if (usage === 39) return '0';
  if (usage >= 58 && usage <= 69) return 'F' + (usage - 57);
  if (usage >= 104 && usage <= 115) return 'F' + (usage - 91);
  return (
    (
      {
        40: 'Enter',
        41: 'Esc',
        42: 'Backspace',
        43: 'Tab',
        44: 'Space',
        45: '-',
        46: '=',
        47: '[',
        48: ']',
        49: '\\',
        51: ';',
        52: "'",
        53: '`',
        54: ',',
        55: '.',
        56: '/',
        57: 'Caps Lock',
        70: 'Print Screen',
        71: 'Scroll Lock',
        72: 'Pause',
        73: 'Insert',
        74: 'Home',
        75: 'Page Up',
        76: 'Delete',
        77: 'End',
        78: 'Page Down',
        79: 'Right',
        80: 'Left',
        81: 'Down',
        82: 'Up',
        224: 'Left Ctrl',
        225: 'Left Shift',
        226: 'Left Alt',
        227: 'Left Win',
        228: 'Right Ctrl',
        229: 'Right Shift',
        230: 'Right Alt',
        231: 'Right Win',
      } as Record<number, string>
    )[usage] ?? null
  );
}

export function onboardAssignment(
  session: ProviderSessionState | null,
  settingId: string | undefined,
  controlId: string,
  layer: KeyboardLayer,
  locale: Locale,
): OnboardAssignment {
  const ru = locale === 'ru';
  const raw = object(object(object(session?.observed?.[settingId ?? ''])?.[controlId])?.[layer]);
  const metadata = object(
    object(object(session?.observed?.onboardKeyLabels)?.[controlId])?.[layer],
  );
  const page = raw?.page;
  const parameters = raw?.parameters;
  const record =
    typeof page === 'number' &&
    Number.isInteger(page) &&
    page >= 0 &&
    page <= 255 &&
    Array.isArray(parameters) &&
    parameters.length === 3 &&
    parameters.every((n) => Number.isInteger(n) && n >= 0 && n <= 255)
      ? ([page, ...parameters] as number[])
      : null;
  // A keyboard usage field on an unknown page must not turn it into a writable key.
  const usage = record?.[0] === 2 && record[1] === 0 && record[3] === 0 ? record[2] : null;
  const labels = object(metadata?.label);
  const label =
    typeof labels?.[locale] === 'string'
      ? (labels[locale] as string)
      : typeof labels?.en === 'string'
        ? labels.en
        : null;
  const fallback = usage !== null ? keyboardUsage(usage) : null;
  const known = typeof metadata?.known === 'boolean' ? metadata.known : !!fallback;
  return {
    label:
      label ??
      fallback ??
      (record
        ? ru
          ? 'Неизвестное назначение'
          : 'Unknown assignment'
        : ru
          ? 'Назначение не прочитано'
          : 'Assignment not read'),
    kind: typeof metadata?.kind === 'string' ? metadata.kind : fallback ? 'keyboard' : 'unknown',
    known,
    observed: record !== null,
    record,
    usage,
    replaceable: usage !== null && known && usage !== 0xaf,
  };
}

export function keyboardOverlay(
  native: OnboardAssignment,
  host: Binding | undefined,
  expectedUsage: number | null,
) {
  return {
    label: host?.label || native.label,
    host: !!host,
    // Saved host intent is distinct from hardware agreement. Neither proves routing.
    mapping:
      expectedUsage === null
        ? ('unconfigured' as const)
        : !native.observed
          ? ('unread' as const)
          : native.usage === expectedUsage
            ? ('verified' as const)
            : ('mismatch' as const),
  };
}
