// The widget lines of a node GUI (spec 4.3), as `patina_sdk::Ui::tree` writes them.

/** A widget of a node GUI. `index` is its order in the `ui` call, which events name. */
export type Widget =
  | { kind: 'label'; index: number; text: string }
  | { kind: 'button'; index: number; text: string }
  | { kind: 'toggle'; index: number; text: string; on: boolean }
  | { kind: 'slider'; index: number; text: string; value: number; low: number; high: number }
  | { kind: 'number'; index: number; text: string; value: number }
  | { kind: 'text'; index: number; text: string; value: string }
  | { kind: 'dropdown'; index: number; text: string; selected: number; options: string[] }
  | { kind: 'canvas'; index: number; text: string; width: number; height: number; shapes: Shape[] }
  | { kind: 'viewport'; index: number; text: string; height: number }
  | { kind: 'color'; index: number; text: string; value: string }
  | { kind: 'plot'; index: number; text: string; low: number; high: number; values: number[] }
  | { kind: 'waveform'; index: number; text: string; playhead: number | null; columns: [number, number][] }
  | { kind: 'table'; index: number; text: string; headings: string[]; rows: string[][]; selected: number | null }
  | { kind: 'curve'; index: number; text: string; points: [number, number][] }
  | { kind: 'image'; index: number; text: string; width: number; height: number; rgba: string };

/** A shape on a canvas, in pixels from its top left corner. */
export type Shape =
  | { kind: 'line'; x1: number; y1: number; x2: number; y2: number; color: string }
  | { kind: 'rect'; x: number; y: number; w: number; h: number; color: string; fill: boolean }
  | { kind: 'circle'; x: number; y: number; r: number; color: string; fill: boolean }
  | { kind: 'text'; x: number; y: number; color: string; text: string };

const COUNTS: Record<string, number> = { line: 4, rect: 4, circle: 3, text: 2 };
// The largest canvas side in pixels, so a bad tree cannot make a huge element.
const MAX_SIDE = 4096;
// The limits of the value widgets. `patina_sdk::Ui` keeps to them, and a bad tree cannot pass them.
const MAX_VALUES = 2048;
const MAX_ROWS = 1000;
const MAX_COLUMNS = 32;
const MAX_POINTS = 256;
const MAX_PIXELS = 256 * 256;

/** Reads one shape field of a canvas line, or null when it is bad. */
function parseShape(field: string): Shape | null {
  const [kind, ...words] = field.split(' ');
  const count = COUNTS[kind];
  if (count === undefined) return null;
  const n = words.slice(0, count).map(Number);
  const color = words[count] ?? '';
  if (n.length < count || !n.every(Number.isFinite) || !/^#[0-9a-f]{6}$/.test(color)) return null;
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
  if (kind === 'color') return /^#[0-9a-f]{6}$/.test(rest[0] ?? '') ? { kind, index, text, value: rest[0] } : null;
  if (kind === 'plot') {
    const [low, high] = rest.map(Number);
    return Number.isFinite(low) && Number.isFinite(high) ? { kind, index, text, low, high, values: numbers(rest[2], MAX_VALUES) } : null;
  }
  if (kind === 'waveform') {
    const at = rest[0] ? Number(rest[0]) : null;
    const n = numbers(rest[1], MAX_VALUES);
    const columns = Array.from({ length: n.length >> 1 }, (_, i): [number, number] => [n[2 * i], n[2 * i + 1]]);
    return at === null || (at >= 0 && at <= 1) ? { kind, index, text, playhead: at, columns } : null;
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
    const points = rest.slice(0, MAX_POINTS).map((f) => numbers(f, 2));
    if (!points.every((p) => p.length === 2 && p.every(Number.isFinite))) return null;
    return { kind, index, text, points: points.map(([x, y]): [number, number] => [Math.min(Math.max(x, 0), 1), Math.min(Math.max(y, 0), 1)]) };
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

/** Reads one widget from each line. A line of an unknown kind or with bad numbers keeps its index. */
export function parseTree(tree: string): Widget[] {
  const widgets: Widget[] = [];
  tree.split('\n').forEach((line, index) => {
    const [kind, text = '', ...rest] = line.split('\t');
    const [value, low, high] = rest.map(Number);
    if (kind === 'label' || kind === 'button') widgets.push({ kind, index, text });
    else if (kind === 'toggle' && Number.isFinite(value)) widgets.push({ kind, index, text, on: value !== 0 });
    else if (kind === 'slider' && [value, low, high].every(Number.isFinite) && low <= high) widgets.push({ kind, index, text, value, low, high });
    else if (kind === 'number' && Number.isFinite(value)) widgets.push({ kind, index, text, value });
    else if (kind === 'text') widgets.push({ kind, index, text, value: rest[0] ?? '' });
    else if (kind === 'dropdown' && Number.isInteger(value)) widgets.push({ kind, index, text, selected: value, options: rest.slice(1) });
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

/** The event line of a user input to widget `index`. A line break in a text becomes a space. */
export function eventLine(index: number, value: number | string): string {
  return `${index} ${String(value).replace(/[\r\n]/g, ' ')}`;
}
