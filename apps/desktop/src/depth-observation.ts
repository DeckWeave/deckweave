export interface DepthFrame {
  enabled: boolean;
  state: 'off' | 'starting' | 'observing' | 'stopping' | 'error';
  sequence: number;
  samples: { controlId: string; travelUm: number; maxTravelUm?: number; ageMs: number }[];
  error?: string | null;
}
export const emptyDepthFrame = (): DepthFrame => ({
  enabled: false,
  state: 'off',
  sequence: 0,
  samples: [],
});
/** Missing or expired telemetry is unknown; only an actual zero sample is released. */
export function keyDepth(
  frame: DepthFrame,
  controlId: string,
  receivedAt: number,
  now: number,
  nominalMaxUm: number,
) {
  if (
    !frame.enabled ||
    frame.state !== 'observing' ||
    !Number.isFinite(nominalMaxUm) ||
    nominalMaxUm <= 0
  )
    return null;
  const sample = frame.samples.find((s) => s.controlId === controlId);
  if (
    !sample ||
    !Number.isInteger(sample.travelUm) ||
    sample.travelUm < 0 ||
    sample.travelUm > 10000 ||
    !Number.isFinite(sample.ageMs) ||
    sample.ageMs < 0 ||
    !Number.isFinite(receivedAt) ||
    !Number.isFinite(now) ||
    sample.ageMs + Math.max(0, now - receivedAt) >= 600
  )
    return null;
  return { um: sample.travelUm, fraction: Math.min(1, sample.travelUm / nominalMaxUm) };
}
