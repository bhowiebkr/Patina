// The widget lines of a node GUI (spec 4.3), as `patina_sdk::Ui::tree` writes them.

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
  | { kind: 'dropdown'; index: number; text: string; selected: number; options: string[] }
  | { kind: 'tabs'; index: number; text: string; selected: number; options: string[] }
  | { kind: 'section'; index: number; text: string; open: boolean }
  | { kind: 'end'; index: number; text: string }
  | { kind: 'canvas'; index: number; text: string; width: number; height: number; shapes: Shape[] }
  | { kind: 'viewport'; index: number; text: string; height: number }
  | { kind: 'color'; index: number; text: string; value: string }
  | { kind: 'plot'; index: number; text: string; low: number; high: number; values: number[]; axes: Axes | null }
  | { kind: 'scatter'; index: number; text: string; low: number; high: number; values: number[]; axes: Axes | null }
  | { kind: 'waveform'; index: number; text: string; playhead: number | null; columns: [number, number][]; trim: [number, number] | null }
  | { kind: 'table'; index: number; text: string; headings: string[]; rows: string[][]; selected: number | null }
  | { kind: 'curve'; index: number; text: string; points: [number, number][]; histogram: number[] }
  | { kind: 'ramp'; index: number; text: string; blend: Blend; selected: number; stops: Stop[] }
  | { kind: 'image'; index: number; text: string; width: number; height: number; rgba: string };

/** The position from 0 to 1 and the color of a color ramp stop. */
export type Stop = [number, string];

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
// The largest canvas side in pixels, so a bad tree cannot make a huge element.
const MAX_SIDE = 4096;
// The limits of the value widgets. `patina_sdk::Ui` keeps to them, and a bad tree cannot pass them.
const MAX_VALUES = 2048;
const MAX_ROWS = 1000;
const MAX_COLUMNS = 32;
const MAX_POINTS = 256;
const MAX_BINS = 256;
const MAX_PIXELS = 256 * 256;
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
  if (kind === 'image') {
    const [width, height] = rest.map(Number);
    const rgba = rest[2] ?? '';
    const ok = Number.isInteger(width) && Number.isInteger(height) && width >= 0 && height >= 0 && width * height <= MAX_PIXELS;
    // Base64 has 4 digits for each 3 bytes, and each pixel has 4 bytes.
    return ok && rgba.length === 4 * Math.ceil((width * height * 4) / 3) ? { kind, index, text, width, height, rgba } : null;
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
    else if ((kind === 'dropdown' || kind === 'tabs') && Number.isInteger(value)) widgets.push({ kind, index, text, selected: value, options: rest.slice(1) });
    else if (kind === 'section' && Number.isFinite(value)) widgets.push({ kind, index, text, open: value !== 0 });
    else if (kind === 'end') widgets.push({ kind, index, text });
    else if (kind === 'canvas' && value > 0 && low > 0) {
      const shapes = rest.slice(2).flatMap((f) => parseShape(f) ?? []);
      widgets.push({ kind, index, text, width: Math.min(value, MAX_SIDE), height: Math.min(low, MAX_SIDE), shapes });
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

/** The event line of a user input to widget `index`. A line break in a text becomes a space. */
export function eventLine(index: number, value: number | string): string {
  return `${index} ${String(value).replace(/[\r\n]/g, ' ')}`;
}
