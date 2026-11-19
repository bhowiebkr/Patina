// The timeline widget line (spec 4.3), as `patina_sdk::Ui::timeline` writes it, and the math of
// its drags. The drags use the clamps of `Timeline::apply`, so the preview shows the result.

/** A part of a source at a time of a timeline. `columns` is the waveform between the two trims. */
export interface Clip {
  start: number;
  length: number;
  offset: number;
  /** The length of the source. 0 is a source with no end. */
  source: number;
  color: string;
  columns: [number, number][];
  name: string;
}

export type Lane =
  | { kind: 'clips'; mute: boolean; solo: boolean; name: string; clips: Clip[] }
  | { kind: 'auto'; mute: boolean; solo: boolean; name: string; low: number; high: number; unit: string; points: [number, number][] };

export interface TimelineData {
  beatsPerBar?: number;
  length: number;
  playhead: number;
  playing: boolean;
  looping: boolean;
  range: [number, number];
  snap: number;
  /** The selected clip or point: the lane and the index in the lane. */
  selected: [number, number] | null;
  lanes: Lane[];
}

/** The part of a clip that a drag holds. */
export type ClipPart = 'move' | 'start' | 'end';

// The limits of `patina_sdk::Timeline`, so a bad line cannot make a huge element.
const MAX_LANES = 16;
const MAX_CLIPS = 64;
const MAX_POINTS = 256;
const MAX_COLUMNS = 64;
/** The shortest clip in seconds. */
export const MIN_CLIP = 0.01;
const COLOR = /^#[0-9a-f]{6}$/;

const finite = (n: number[]) => n.every(Number.isFinite);
const fit = (v: number, low: number, high: number) => Math.min(Math.max(v, low), Math.max(low, high));

/** Reads the fields of a timeline line after its text, or null when they are bad. */
export function parseTimeline(rest: string[]): TimelineData | null {
  const head = rest[0]?.split(' ').map(Number) ?? [];
  const [length, playhead, playing, looping, a, b, snap] = head;
  if ((head.length !== 7 && head.length !== 8) || !finite(head) || !(length > 0)) return null;
  const beatsPerBar = head[7] ?? 0;
  if (!Number.isInteger(beatsPerBar) || beatsPerBar < 0 || beatsPerBar > 32) return null;
  const sel = rest[1] ? rest[1].split(' ').map(Number) : [];
  if (rest[1] && !(sel.length === 2 && sel.every((v) => Number.isInteger(v) && v >= 0))) return null;
  const lanes: Lane[] = [];
  let at = 2;
  while (at < rest.length && lanes.length < MAX_LANES) {
    const [kind, mute, solo, ...words] = rest[at].split(' ');
    const flags = { mute: mute === '1', solo: solo === '1' };
    if (kind === 'clips') {
      const count = Number(words[0]);
      if (!Number.isInteger(count) || count < 0 || count > MAX_CLIPS) return null;
      const clips = rest.slice(at + 1, at + 1 + count).map(clip);
      if (clips.length !== count || !clips.every((c): c is Clip => c !== null)) return null;
      lanes.push({ kind, ...flags, name: words.slice(1).join(' '), clips });
      at += 1 + count;
    } else if (kind === 'auto') {
      const [low, high] = words.slice(0, 2).map(Number);
      const n = rest[at + 2] ? rest[at + 2].split(' ', 2 * MAX_POINTS).map(Number) : [];
      if (words.length < 2 || !finite([low, high]) || low > high || n.length % 2 !== 0 || !finite(n) || at + 2 >= rest.length) return null;
      const points = Array.from({ length: n.length / 2 }, (_, i): [number, number] => [fit(n[2 * i], 0, length), fit(n[2 * i + 1], low, high)]);
      lanes.push({ kind, ...flags, name: words.slice(2).join(' '), low, high, unit: rest[at + 1], points });
      at += 3;
    } else return null;
  }
  return {
    ...(beatsPerBar ? { beatsPerBar } : {}),
    length,
    playhead: fit(playhead, 0, length),
    playing: playing !== 0,
    looping: looping !== 0,
    range: [fit(Math.min(a, b), 0, length), fit(Math.max(a, b), 0, length)],
    snap: fit(snap, 0, length),
    selected: sel.length === 2 ? [sel[0], sel[1]] : null,
    lanes,
  };
}

/** Reads `start length offset source #color columns [low high ...] name`, or null when it is bad. */
function clip(field: string): Clip | null {
  const words = field.split(' ');
  const n = words.slice(0, 4).map(Number);
  const color = words[4] ?? '';
  const count = Number(words[5]);
  const v = words.slice(6, 6 + 2 * count).map(Number);
  if (n.length !== 4 || !finite(n) || !COLOR.test(color) || !Number.isInteger(count) || count < 0 || count > MAX_COLUMNS || v.length !== 2 * count || !finite(v)) return null;
  const columns = Array.from({ length: count }, (_, i): [number, number] => [v[2 * i], v[2 * i + 1]]);
  const [start, length, offset, source] = n.map((v) => Math.max(v, 0));
  return { start, length, offset, source, color, columns, name: words.slice(6 + 2 * count).join(' ') };
}

const round3 = (v: number) => Math.round(v * 1000) / 1000;

/** `time` on the snap grid, with 3 decimals. A snap of 0 is no snap. */
export function snapTo(time: number, snap: number): number {
  return round3(snap > 0 ? Math.round(time / snap) * snap : time);
}

/** The clip after a drag of `part` by `dt` seconds on a timeline of `total` seconds. A trim
 * cannot go past the source, and a clip stays on the ruler. */
export function dragClip(c: Clip, part: ClipPart, dt: number, total: number, snap: number): Clip {
  const end = c.source > 0 ? c.source : Infinity;
  if (part === 'move') return { ...c, start: round3(fit(snapTo(c.start + dt, snap), 0, total - c.length)) };
  if (part === 'start') {
    const d = fit(snapTo(c.start + dt, snap) - c.start, Math.max(-c.offset, -c.start), c.length - MIN_CLIP);
    return { ...c, start: round3(c.start + d), length: round3(c.length - d), offset: round3(c.offset + d) };
  }
  const length = fit(snapTo(c.start + c.length + dt, snap) - c.start, MIN_CLIP, Math.min(end - c.offset, total - c.start));
  return { ...c, length: round3(length) };
}

/** The loop range after a drag of its end `end` to `time`. An end stops before the other end. */
export function dragRange(range: [number, number], end: 0 | 1, time: number, total: number, snap: number): [number, number] {
  const r: [number, number] = [range[0], range[1]];
  const v = fit(snapTo(time, snap), 0, total);
  r[end] = round3(end ? Math.max(v, range[0] + MIN_CLIP) : Math.min(v, range[1] - MIN_CLIP));
  return r;
}

/** The point `i` at time `t` and value `v`. A point stays between its neighbors and in the range. */
export function dragPoint(points: [number, number][], i: number, t: number, v: number, low: number, high: number, total: number, snap: number): [number, number] {
  const before = i > 0 ? points[i - 1][0] : 0;
  const after = i + 1 < points.length ? points[i + 1][0] : total;
  return [round3(fit(snapTo(t, snap), before, after)), round3(fit(v, low, high))];
}

/** The SVG path of an automation curve in a box of `total` x 1: the smooth curve of
 * `Automation::value`, flat before the first point and after the last point. */
export function curvePath(points: [number, number][], total: number, low: number, high: number): string {
  const y = (v: number) => round3(high > low ? 1 - (v - low) / (high - low) : 0.5);
  if (points.length === 0) return `M0 ${y(low)}H${total}`;
  // A cubic with the controls at 1/3 and 2/3 of the time is the smoothstep of the SDK.
  const parts = points.slice(1).map(([t, v], i) => {
    const [t0, v0] = points[i];
    return `C${round3(t0 + (t - t0) / 3)} ${y(v0)} ${round3(t0 + ((t - t0) * 2) / 3)} ${y(v)} ${t} ${y(v)}`;
  });
  return `M0 ${y(points[0][1])}H${points[0][0]}${parts.join('')}H${total}`;
}

/** The time as `00:00:01.240`. */
export function clock(seconds: number): string {
  const ms = Math.round(Math.max(seconds, 0) * 1000);
  const pad = (v: number, n = 2) => String(v).padStart(n, '0');
  return `${pad(Math.floor(ms / 3600000))}:${pad(Math.floor(ms / 60000) % 60)}:${pad(Math.floor(ms / 1000) % 60)}.${pad(ms % 1000, 3)}`;
}

/** A value with its unit, for example `2.4 kHz` or `500 Hz`. */
export function quantity(v: number, unit: string): string {
  if (unit && Math.abs(v) >= 1000) return `${(v / 1000).toFixed(1)} k${unit}`;
  const text = String(Number(v.toPrecision(3)));
  return unit ? `${text} ${unit}` : text;
}

const STEPS = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600];

/** The step of the labelled ticks of a ruler of `total` seconds, with 8 labels or fewer, and the
 * decimals of the labels. A small tick comes at each fifth of a step. */
export function ruler(total: number): { step: number; decimals: number } {
  const step = STEPS.find((s) => total / s <= 8) ?? Math.ceil(total / 8 / 600) * 600;
  return { step, decimals: step === 0.25 ? 2 : step < 1 ? 1 : 0 };
}
