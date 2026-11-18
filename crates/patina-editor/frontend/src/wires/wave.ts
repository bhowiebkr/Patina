// The wave of an audio value on its wire (spec 15.9): the samples of `Viewer::wave` (5.3) drawn
// along the path of the wire. Each frame gives a new block, so the wave moves.

import { MAX_POINTS, STRIDE, wirePoints, type Shape } from './wires.ts';

/** The largest distance of a sample from the wire, in graph pixels. */
export const WAVE_AMPLITUDE = 6;
/** A wire shorter than this, in graph pixels, has no room for a wave. */
const MIN_LENGTH = 24;
/** The part of the length at each end where the amplitude goes to 0, so the wave starts and stops at the pins. */
const TAPER = 1 / 8;

const points = new Float32Array(MAX_POINTS * STRIDE);
const lengths = new Float64Array(MAX_POINTS);

/** The samples of a wave text: integers from -100 to 100 with a space between them, as -1 to 1. Empty for a bad text. */
export function parseWave(text: string): number[] {
  const parts = text.split(' ');
  const valid = parts.every((p) => /^-?\d{1,3}$/.test(p) && Math.abs(Number(p)) <= 100);
  return valid ? parts.map((p) => Number(p) / 100) : [];
}

const round = (v: number) => String(Math.round(v * 10) / 10);

/**
 * The SVG path of `samples` along the first `n` points of `line` (STRIDE numbers for each point).
 * The samples spread evenly over the length. A positive sample is on the left of the direction
 * of the wire: above a wire that goes to the right. Empty for a short wire or no samples.
 */
export function wavePath(line: Float32Array, n: number, samples: readonly number[], amplitude = WAVE_AMPLITUDE): string {
  const m = samples.length;
  if (m === 0 || n < 2) return '';
  lengths[0] = 0;
  for (let k = 1; k < n; k++) {
    lengths[k] = lengths[k - 1] + Math.hypot(line[k * STRIDE] - line[(k - 1) * STRIDE], line[k * STRIDE + 1] - line[(k - 1) * STRIDE + 1]);
  }
  const total = lengths[n - 1];
  if (total < MIN_LENGTH) return '';
  let d = `M${round(line[0])} ${round(line[1])}`;
  let k = 1;
  for (let i = 0; i < m; i++) {
    const s = ((i + 0.5) / m) * total;
    while (k < n - 1 && lengths[k] < s) k++;
    const [ax, ay] = [line[(k - 1) * STRIDE], line[(k - 1) * STRIDE + 1]];
    const [dx, dy] = [line[k * STRIDE] - ax, line[k * STRIDE + 1] - ay];
    const len = lengths[k] - lengths[k - 1] || 1;
    const f = (s - lengths[k - 1]) / len;
    const t = s / total;
    const off = samples[i] * amplitude * Math.min(1, t / TAPER, (1 - t) / TAPER);
    d += `L${round(ax + f * dx + (dy / len) * off)} ${round(ay + f * dy - (dx / len) * off)}`;
  }
  return `${d}L${round(line[(n - 1) * STRIDE])} ${round(line[(n - 1) * STRIDE + 1])}`;
}

/** The SVG path of the wave text on the wire from (sx, sy) to (tx, ty) with this shape. */
export function wireWave(sx: number, sy: number, tx: number, ty: number, shape: Shape, wave: string): string {
  const n = wirePoints(sx, sy, tx, ty, shape, points, 0);
  return wavePath(points, n, parseWave(wave));
}
