// The widget lines of a node GUI (spec 4.3), as `patina_sdk::Ui::tree` writes them.

import { parseTimeline, type TimelineData } from './timelineData.ts';

/** A widget of a node GUI. `index` is its order in the `ui` call, which events name. */
export type Widget =
  | { kind: 'label'; index: number; text: string }
  | { kind: 'button'; index: number; text: string }
  | { kind: 'icon'; index: number; text: string; icon: string }
  | { kind: 'progress'; index: number; text: string; done: number }
  | { kind: 'toggle'; index: number; text: string; on: boolean }
  | { kind: 'slider'; index: number; text: string; value: number; low: number; high: number }
  | { kind: 'number'; index: number; text: string; value: number; step: number; unit: string }
  | { kind: 'text'; index: number; text: string; value: string }
  /** `editor` adds line numbers, the `current` line (-1 for none) and the `errors` by line, from 0. */
  | { kind: 'code'; index: number; text: string; value: string; editor: boolean; current: number; errors: [number, string][] }
  | { kind: 'path'; index: number; text: string; pick: 'open' | 'save' | 'folder'; value: string }
  | { kind: 'asset'; index: number; text: string; extensions: string[]; value: string }
  | { kind: 'dropdown'; index: number; text: string; selected: number; options: string[] }
  | { kind: 'tabs'; index: number; text: string; selected: number; options: string[] }
  | { kind: 'section'; index: number; text: string; open: boolean }
  | { kind: 'end'; index: number; text: string }
  | { kind: 'canvas'; index: number; text: string; width: number; height: number; shapes: Shape[]; keys: boolean }
  | { kind: 'viewport'; index: number; text: string; height: number }
  | { kind: 'color'; index: number; text: string; value: string }
  | { kind: 'plot'; index: number; text: string; low: number; high: number; values: number[]; axes: Axes | null }
  | { kind: 'scatter'; index: number; text: string; low: number; high: number; values: number[]; axes: Axes | null }
  | { kind: 'waveform'; index: number; text: string; playhead: number | null; columns: [number, number][]; trim: [number, number] | null }
  | { kind: 'table'; index: number; text: string; headings: string[]; rows: string[][]; selected: number | null }
  | { kind: 'curve'; index: number; text: string; points: [number, number][]; histogram: number[] }
  | { kind: 'ramp'; index: number; text: string; blend: Blend; selected: number; stops: Stop[] }
  | { kind: 'layers'; index: number; text: string; selected: number; layers: Layer[] }
  | ({ kind: 'image'; index: number; text: string } & Picture)
  /** A 2D transform on a frame of `frame` pixels, with an image of the frame behind it or none. */
  | { kind: 'transform'; index: number; text: string; frame: [number, number]; t: Transform; image: Picture | null }
  | { kind: 'timeline'; index: number; text: string; t: TimelineData };

/** The size and the base64 RGBA pixels of an image. */
export interface Picture {
  width: number;
  height: number;
  rgba: string;
}

/** A move in pixels, a rotation in degrees and a scale around a pivot from 0 to 1, as `patina_sdk::Transform2d`. */
export interface Transform {
  x: number;
  y: number;
  rotation: number;
  sx: number;
  sy: number;
  px: number;
  py: number;
  lock: boolean;
}

/** The position from 0 to 1 and the color of a color ramp stop. */
export type Stop = [number, string];

/** A row of a layer stack. A layer without an amount, for example a base, has no amount field. */
export interface Layer {
  visible: boolean;
  amount: number | null;
  color: string;
  name: string;
}

/** How a color ramp blends the colors between two stops, as `patina_sdk::Blend`. */
export const BLENDS = ['linear', 'ease', 'constant'] as const;
export type Blend = (typeof BLENDS)[number];

/** The x range and the units of x and y of a plot. */
export interface Axes {
  x: [number, number];
  units: [string, string];
}

/** A shape on a canvas, in pixels from its top left corner. */
export type Shape =
  | { kind: 'line'; x1: number; y1: number; x2: number; y2: number; color: string }
  | { kind: 'rect'; x: number; y: number; w: number; h: number; color: string; fill: boolean }
  | { kind: 'circle'; x: number; y: number; r: number; color: string; fill: boolean }
  | { kind: 'text'; x: number; y: number; color: string; text: string };

const COUNTS: Record<string, number> = { line: 4, rect: 4, circle: 3, text: 2 };
const COLOR = /^#[0-9a-f]{6}$/;
// A layer: visible (0 or 1), the amount or '-', the color and the name.
const LAYER = /^([01]) (\S+) (#[0-9a-f]{6}) (.*)$/;
// The largest canvas side in pixels, so a bad tree cannot make a huge element.
const MAX_SIDE = 4096;
// The limits of the value widgets. `patina_sdk::Ui` keeps to them, and a bad tree cannot pass them.
const MAX_VALUES = 2048;
const MAX_ROWS = 1000;
const MAX_COLUMNS = 32;
const MAX_POINTS = 256;
const MAX_BINS = 256;
const MAX_PIXELS = 256 * 256;
/** The digits of base64 with no end marks. */
const BASE64 = /^[A-Za-z0-9+/]*$/;
const MAX_STOPS = 32;
// The deepest section in a section. A deeper section shows its widgets in its parent.
const MAX_DEPTH = 8;

/** Reads one shape field of a canvas line, or null when it is bad. */
function parseShape(field: string): Shape | null {
  const [kind, ...words] = field.split(' ');
  const count = COUNTS[kind];
  if (count === undefined) return null;
  const n = words.slice(0, count).map(Number);
  const color = words[count] ?? '';
  if (n.length < count || !n.every(Number.isFinite) || !COLOR.test(color)) return null;
  const fill = words[count + 1] === '1';
  if (kind === 'line') return { kind, x1: n[0], y1: n[1], x2: n[2], y2: n[3], color };
  if (kind === 'rect') return n[2] >= 0 && n[3] >= 0 ? { kind, x: n[0], y: n[1], w: n[2], h: n[3], color, fill } : null;
  if (kind === 'circle') return n[2] >= 0 ? { kind, x: n[0], y: n[1], r: n[2], color, fill } : null;
  return { kind: 'text', x: n[0], y: n[1], color, text: words.slice(count + 1).join(' ') };
}

/** The numbers of a field with spaces between them. A word that is not finite, for example `NaN`, is NaN. */
function numbers(field: string | undefined, max: number): number[] {
  return field ? field.split(' ', max).map(Number) : [];
}

/** Reads the width, the height and the base64 RGBA of an image, or null when they are bad. */
function pixels(fields: string[]): Picture | null {
  const [width, height] = fields.map(Number);
  const rgba = fields[2] ?? '';
  const ok = Number.isInteger(width) && Number.isInteger(height) && width >= 0 && height >= 0 && width * height <= MAX_PIXELS;
  // Base64 has 4 digits for each 3 bytes, and each pixel has 4 bytes. Other digits or ends make
  // the decode fail or give the wrong length, so the line is bad.
  const end = '='.repeat((3 - ((width * height * 4) % 3)) % 3);
  const digits = rgba.slice(0, rgba.length - end.length);
  const data = rgba.length === 4 * Math.ceil((width * height * 4) / 3) && rgba.endsWith(end) && BASE64.test(digits);
  return ok && data ? { width, height, rgba } : null;
}

/** Reads a value widget line, or null when it is bad. */
function parseValue(kind: string, index: number, text: string, rest: string[]): Widget | null {
  if (kind === 'color') return COLOR.test(rest[0] ?? '') ? { kind, index, text, value: rest[0] } : null;
  if (kind === 'plot' || kind === 'scatter') {
    const [low, high, , xLow, xHigh] = rest.map(Number);
    const axes: Axes | null = rest.length > 3 ? { x: [xLow, xHigh], units: [rest[5] ?? '', rest[6] ?? ''] } : null;
    if (![low, high, ...(axes?.x ?? [])].every(Number.isFinite)) return null;
    return { kind, index, text, low, high, values: numbers(rest[2], MAX_VALUES), axes };
  }
  if (kind === 'waveform') {
    const at = rest[0] ? Number(rest[0]) : null;
    const n = numbers(rest[1], MAX_VALUES);
    const columns = Array.from({ length: n.length >> 1 }, (_, i): [number, number] => [n[2 * i], n[2 * i + 1]]);
    const [start, end] = rest[2] ? numbers(rest[2], 2) : [];
    const trim: [number, number] | null = rest[2] ? [start, end] : null;
    if (trim && !(start >= 0 && start <= end && end <= 1)) return null;
    return at === null || (at >= 0 && at <= 1) ? { kind, index, text, playhead: at, columns, trim } : null;
  }
  if (kind === 'table') {
    const count = Number(rest[0]);
    const selected = rest[1] ? Number(rest[1]) : null;
    if (!Number.isInteger(count) || count < 0 || count > MAX_COLUMNS || (selected !== null && !Number.isInteger(selected))) return null;
    const cells = rest.slice(2 + count, 2 + count + count * MAX_ROWS);
    const rows = count === 0 ? [] : Array.from({ length: Math.ceil(cells.length / count) }, (_, r) => cells.slice(r * count, r * count + count));
    return { kind, index, text, headings: rest.slice(2, 2 + count), rows, selected };
  }
  if (kind === 'curve') {
    const points = rest.slice(1, 1 + MAX_POINTS).map((f) => numbers(f, 2));
    if (!points.every((p) => p.length === 2 && p.every(Number.isFinite))) return null;
    // A bar that is not a positive number is empty.
    const histogram = numbers(rest[0], MAX_BINS).map((v) => (v > 0 && Number.isFinite(v) ? v : 0));
    return { kind, index, text, points: points.map(([x, y]): [number, number] => [Math.min(Math.max(x, 0), 1), Math.min(Math.max(y, 0), 1)]), histogram };
  }
  if (kind === 'ramp') {
    const blend = BLENDS.find((b) => b === rest[0]);
    const selected = Number(rest[1]);
    const stops = rest.slice(2, 2 + MAX_STOPS).map((f): Stop => [Number(f.split(' ')[0]), f.split(' ')[1] ?? '']);
    if (!blend || !Number.isInteger(selected) || !stops.every(([at, color]) => Number.isFinite(at) && COLOR.test(color))) return null;
    return { kind, index, text, blend, selected, stops: stops.map(([at, color]): Stop => [Math.min(Math.max(at, 0), 1), color]) };
  }
  if (kind === 'layers') {
    const selected = Number(rest[0]);
    const layers = rest.slice(1, 1 + MAX_ROWS).map(layer);
    if (!Number.isInteger(selected) || !layers.every((l): l is Layer => l !== null)) return null;
    return { kind, index, text, selected, layers };
  }
  if (kind === 'image') {
    const image = pixels(rest);
    return image && { kind, index, text, ...image };
  }
  if (kind === 'timeline') {
    const t = parseTimeline(rest);
    return t && { kind, index, text, t };
  }
  if (kind === 'transform') {
    const [fw, fh] = numbers(rest[0], 2);
    const [x, y] = numbers(rest[1], 2);
    const [sx, sy] = numbers(rest[3], 2);
    const [px, py] = numbers(rest[4], 2);
    const rotation = Number(rest[2]);
    const image = pixels(rest.slice(6));
    if (![fw, fh, x, y, rotation, sx, sy, px, py].every(Number.isFinite) || !(fw > 0 && fh > 0 && sx > 0 && sy > 0) || !image) return null;
    const t = { x, y, rotation, sx, sy, px: Math.min(Math.max(px, 0), 1), py: Math.min(Math.max(py, 0), 1), lock: rest[5] === '1' };
    return { kind, index, text, frame: [fw, fh], t, image: image.width > 0 && image.height > 0 ? image : null };
  }
  return null;
}

// A node is 200 px wide. A canvas makes it wider, up to the limit (4.3). 26 px is the padding and the border.
export const NODE_WIDTH = 200;
const MAX_NODE_WIDTH = 480;
const GUI_PAD = 26;

/** The width of a node from the canvas sizes of its GUI. The values that it shows never change it. */
export function nodeWidth(tree: string): number {
  let width = 0;
  for (const line of tree.split('\n')) {
    const [kind, , value] = line.split('\t', 3);
    if (kind === 'canvas') width = Math.max(width, Number(value) || 0);
  }
  return Math.min(Math.max(NODE_WIDTH, Math.ceil(width) + GUI_PAD), MAX_NODE_WIDTH);
}

/** Reads one widget from each line. A line of an unknown kind or with bad numbers keeps its index. */
export function parseTree(tree: string): Widget[] {
  const widgets: Widget[] = [];
  tree.split('\n').forEach((line, index) => {
    const [kind, text = '', ...rest] = line.split('\t');
    const [value, low, high] = rest.map(Number);
    if (kind === 'label' || kind === 'button') widgets.push({ kind, index, text });
    else if (kind === 'icon') widgets.push({ kind, index, text, icon: rest[0] ?? '' });
    else if (kind === 'progress' && value >= 0 && value <= 1) widgets.push({ kind, index, text, done: value });
    else if (kind === 'toggle' && Number.isFinite(value)) widgets.push({ kind, index, text, on: value !== 0 });
    else if (kind === 'slider' && [value, low, high].every(Number.isFinite) && low <= high) widgets.push({ kind, index, text, value, low, high });
    else if (kind === 'number' && Number.isFinite(value)) widgets.push({ kind, index, text, value, step: low > 0 && Number.isFinite(low) ? low : 0, unit: rest[2] ?? '' });
    else if (kind === 'text') widgets.push({ kind, index, text, value: rest[0] ?? '' });
    else if (kind === 'code') widgets.push({ kind, index, text, value: unescapeCode(rest[0] ?? ''), ...codeLines(rest.slice(1)) });
    else if (kind === 'path' && (rest[0] === 'open' || rest[0] === 'save' || rest[0] === 'folder')) widgets.push({ kind, index, text, pick: rest[0], value: rest[1] ?? '' });
    else if (kind === 'asset') widgets.push({ kind, index, text, extensions: (rest[0] ?? '').split(' ').filter(Boolean), value: rest[1] ?? '' });
    else if ((kind === 'dropdown' || kind === 'tabs') && Number.isInteger(value)) widgets.push({ kind, index, text, selected: value, options: rest.slice(1) });
    else if (kind === 'section' && Number.isFinite(value)) widgets.push({ kind, index, text, open: value !== 0 });
    else if (kind === 'end') widgets.push({ kind, index, text });
    else if (kind === 'canvas' && value > 0 && low > 0) {
      const shapes = rest.slice(2).flatMap((f) => parseShape(f) ?? []);
      const keys = rest.slice(2).includes('keys');
      widgets.push({ kind, index, text, width: Math.min(value, MAX_SIDE), height: Math.min(low, MAX_SIDE), shapes, keys });
    } else if (kind === 'viewport' && value > 0) widgets.push({ kind, index, text, height: Math.min(value, MAX_SIDE) });
    else {
      const widget = parseValue(kind, index, text, rest);
      if (widget) widgets.push(widget);
    }
  });
  return widgets;
}

type IconButton = Extract<Widget, { kind: 'icon' }>;

/** A widget, a section with the widgets up to its end line, or a row of icon buttons. */
export type Block =
  | Exclude<Widget, { kind: 'section' | 'end' | 'icon' }>
  | (Extract<Widget, { kind: 'section' }> & { items: Block[] })
  | { kind: 'icons'; index: number; items: IconButton[] };

/** Puts the widgets of each section into it, and icon buttons on adjacent lines into one row.
 * An end line without a section does nothing. */
export function nest(widgets: Widget[]): Block[] {
  const root: Block[] = [];
  const stack = [root];
  for (const w of widgets) {
    const top = stack[stack.length - 1];
    const last = top[top.length - 1];
    if (w.kind === 'end') {
      if (stack.length > 1) stack.pop();
    } else if (w.kind === 'icon') {
      if (last?.kind === 'icons' && last.items[last.items.length - 1].index === w.index - 1) last.items.push(w);
      else top.push({ kind: 'icons', index: w.index, items: [w] });
    } else if (w.kind !== 'section') top.push(w);
    else if (stack.length > MAX_DEPTH) stack.push(top);
    else {
      const section = { ...w, items: [] };
      top.push(section);
      stack.push(section.items);
    }
  }
  return root;
}

/** A color between `a` and `b`, at `t` from 0 to 1. */
function mix(a: string, b: string, t: number): string {
  const channel = (c: string, i: number) => parseInt(c.slice(1 + 2 * i, 3 + 2 * i), 16);
  return `#${[0, 1, 2].map((i) => Math.round(channel(a, i) + (channel(b, i) - channel(a, i)) * t).toString(16).padStart(2, '0')).join('')}`;
}

function layer(field: string): Layer | null {
  const m = LAYER.exec(field);
  const amount = m?.[2] === '-' ? null : Number(m?.[2]);
  if (!m || (amount !== null && !Number.isFinite(amount))) return null;
  return { visible: m[1] === '1', amount: amount === null ? null : Math.min(Math.max(amount, 0), 1), color: m[3], name: m[4] };
}

/** The CSS gradient of a color ramp, with the colors of `patina_sdk::ColorRamp::color`. An ease
 * blend gets 8 parts between two stops. */
export function rampGradient(stops: Stop[], blend: Blend): string {
  if (stops.length === 0) return 'none';
  const pct = (at: number) => `${Math.round(at * 10000) / 100}%`;
  const parts = [`${stops[0][1]} 0%`];
  stops.forEach(([at, color], i) => {
    const before = stops[i - 1];
    if (before && blend === 'constant') parts.push(`${before[1]} ${pct(at)}`);
    if (before && blend === 'ease') {
      for (let k = 1; k < 8; k++) {
        const t = k / 8;
        parts.push(`${mix(before[1], color, t * t * (3 - 2 * t))} ${pct(before[0] + (at - before[0]) * t)}`);
      }
    }
    parts.push(`${color} ${pct(at)}`);
  });
  parts.push(`${stops[stops.length - 1][1]} 100%`);
  return `linear-gradient(to right, ${parts.join(', ')})`;
}

/** `value` plus `count` steps. The rounding to 12 digits removes the float error, so 0.1 + 0.2 is 0.3. */
export function stepped(value: number, step: number, count: number): number {
  return Number((value + step * count).toPrecision(12));
}

/** A code field line keeps its line breaks and tabs as `\n` and `\t`, and `\` as `\\` (spec 4.3). */
export function escapeCode(text: string): string {
  return text.replace(/\r/g, '').replace(/[\\\n\t]/g, (c) => (c === '\\' ? '\\\\' : c === '\n' ? '\\n' : '\\t'));
}

/** Reads the text that `escapeCode` writes. A `\` before another character stays. */
export function unescapeCode(text: string): string {
  return text.replace(/\\(.?)/gs, (all, c: string) => (c === 'n' ? '\n' : c === 't' ? '\t' : c === '\\' ? '\\' : all));
}

/** The line marks of a code editor: the current line, then `<line> <error>` for each error. */
function codeLines(fields: string[]): { editor: boolean; current: number; errors: [number, string][] } {
  const line = (text: string) => (/^\d+$/.test(text) ? Number(text) : -1);
  const errors = fields.slice(1).flatMap((f): [number, string][] => {
    const at = f.indexOf(' ');
    return at > 0 && line(f.slice(0, at)) >= 0 ? [[line(f.slice(0, at)), unescapeCode(f.slice(at + 1))]] : [];
  });
  return { editor: fields.length > 0, current: line(fields[0] ?? ''), errors };
}

/** The event line of a user input to widget `index`. A line break in a text becomes a space. */
export function eventLine(index: number, value: number | string): string {
  return `${index} ${String(value).replace(/[\r\n]/g, ' ')}`;
}

/** A point in pixels. */
export type Point = [number, number];
/** The part of a transform gizmo that a drag holds. */
export type Part = 'move' | 'x' | 'y' | 'rotate' | 'corner';
const MIN_SCALE = 0.01;
const MAX_SCALE = 100;
const limit = (s: number) => Math.min(Math.max(s, MIN_SCALE), MAX_SCALE);
const r3 = (v: number) => Math.round(v * 1000) / 1000;

/** An angle in degrees from -180 to 180, as `patina_sdk::Transform2d` keeps it. */
function wrap(degrees: number): number {
  const r = degrees % 360;
  return r > 180 ? r - 360 : r <= -180 ? r + 360 : r;
}

/** The place of the point `p` of a frame of `size` after the transform, as `patina_sdk::Transform2d::apply`. */
export function applyTransform(t: Transform, size: Point, [x, y]: Point): Point {
  const a = (t.rotation * Math.PI) / 180;
  const [cx, cy] = [t.px * size[0], t.py * size[1]];
  const [dx, dy] = [(x - cx) * t.sx, (y - cy) * t.sy];
  return [cx + dx * Math.cos(a) - dy * Math.sin(a) + t.x, cy + dx * Math.sin(a) + dy * Math.cos(a) + t.y];
}

/** The transform with the pivot at `pivot`, with a move that keeps the box in its place. */
export function movePivot(t: Transform, size: Point, pivot: Point): Transform {
  const moved = { ...t, px: pivot[0], py: pivot[1] };
  // The two transforms differ only by a move, so one point that stays keeps all points.
  const [a, b] = [applyTransform(t, size, [0, 0]), applyTransform(moved, size, [0, 0])];
  return { ...moved, x: r3(t.x + a[0] - b[0]), y: r3(t.y + a[1] - b[1]) };
}

/** A scale of `value` for x. The aspect lock keeps the ratio of the two scales. */
export function scaleTo(t: Transform, value: number): Transform {
  return t.lock ? { ...t, sx: limit(value), sy: limit((t.sy * value) / t.sx) } : { ...t, sx: limit(value) };
}

/** The transform after a drag of `part` from `from` to `to`. The points are on a display with `k`
 * units for each pixel of the frame, and `at` is the pivot on it. `snap` turns in steps of 15 degrees. */
export function dragTransform(t: Transform, part: Part, from: Point, to: Point, at: Point, k: number, snap: boolean): Transform {
  const [dx, dy] = [(to[0] - from[0]) / k, (to[1] - from[1]) / k];
  if (part === 'move') return { ...t, x: r3(t.x + dx), y: r3(t.y + dy) };
  if (part === 'x') return { ...t, x: r3(t.x + dx) };
  if (part === 'y') return { ...t, y: r3(t.y + dy) };
  const angle = (p: Point) => Math.atan2(p[1] - at[1], p[0] - at[0]);
  if (part === 'rotate') {
    const r = t.rotation + ((angle(to) - angle(from)) * 180) / Math.PI;
    return { ...t, rotation: r3(wrap(snap ? Math.round(r / 15) * 15 : r)) };
  }
  // A corner: the points in the axes of the box, from the pivot.
  const a = (-t.rotation * Math.PI) / 180;
  const local = ([x, y]: Point): Point => [(x - at[0]) * Math.cos(a) - (y - at[1]) * Math.sin(a), (x - at[0]) * Math.sin(a) + (y - at[1]) * Math.cos(a)];
  const [p0, p1] = [local(from), local(to)];
  if (t.lock) {
    const f = (p1[0] * p0[0] + p1[1] * p0[1]) / (p0[0] ** 2 + p0[1] ** 2 || 1);
    return { ...t, sx: r3(limit(t.sx * f)), sy: r3(limit(t.sy * f)) };
  }
  // A corner on the line of the pivot does not change the scale of that axis.
  const ratio = (i: number) => (Math.abs(p0[i]) > 1 ? p1[i] / p0[i] : 1);
  return { ...t, sx: r3(limit(t.sx * ratio(0))), sy: r3(limit(t.sy * ratio(1))) };
}

/** The event of a transform, as `patina_sdk::Ui::transform` reads it. */
export function transformEvent(t: Transform): string {
  return `set ${[t.x, t.y, t.rotation, t.sx, t.sy, t.px, t.py].map(r3).join(' ')} ${t.lock ? 1 : 0}`;
}
