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
  | { kind: 'canvas'; index: number; text: string; width: number; height: number; shapes: Shape[] };

/** A shape on a canvas, in pixels from its top left corner. */
export type Shape =
  | { kind: 'line'; x1: number; y1: number; x2: number; y2: number; color: string }
  | { kind: 'rect'; x: number; y: number; w: number; h: number; color: string; fill: boolean }
  | { kind: 'circle'; x: number; y: number; r: number; color: string; fill: boolean }
  | { kind: 'text'; x: number; y: number; color: string; text: string };

const COUNTS: Record<string, number> = { line: 4, rect: 4, circle: 3, text: 2 };
// The largest canvas side in pixels, so a bad tree cannot make a huge element.
const MAX_SIDE = 4096;

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
    }
  });
  return widgets;
}

/** The event line of a user input to widget `index`. A line break in a text becomes a space. */
export function eventLine(index: number, value: number | string): string {
  return `${index} ${String(value).replace(/[\r\n]/g, ' ')}`;
}
