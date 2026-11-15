// The widget lines of a node GUI (spec 4.3), as `patina_sdk::Ui::tree` writes them.

/** A widget of a node GUI. `index` is its order in the `ui` call, which events name. */
export type Widget =
  | { kind: 'label'; index: number; text: string }
  | { kind: 'button'; index: number; text: string }
  | { kind: 'toggle'; index: number; text: string; on: boolean }
  | { kind: 'slider'; index: number; text: string; value: number; low: number; high: number }
  | { kind: 'number'; index: number; text: string; value: number }
  | { kind: 'text'; index: number; text: string; value: string }
  | { kind: 'dropdown'; index: number; text: string; selected: number; options: string[] };

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
  });
  return widgets;
}

/** The event line of a user input to widget `index`. A line break in a text becomes a space. */
export function eventLine(index: number, value: number | string): string {
  return `${index} ${String(value).replace(/[\r\n]/g, ' ')}`;
}
