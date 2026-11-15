// The widget lines of a node GUI (spec 4.3), as `patina_sdk::Ui::tree` writes them.

/** A widget of a node GUI. `index` is its order in the `ui` call, which events name. */
export type Widget =
  | { kind: 'label'; index: number; text: string }
  | { kind: 'button'; index: number; text: string }
  | { kind: 'toggle'; index: number; text: string; on: boolean }
  | { kind: 'slider'; index: number; text: string; value: number; low: number; high: number };

/** Reads one widget from each line. A line of an unknown kind or with bad numbers keeps its index. */
export function parseTree(tree: string): Widget[] {
  const widgets: Widget[] = [];
  tree.split('\n').forEach((line, index) => {
    const [kind, text = '', ...rest] = line.split('\t');
    const [value, low, high] = rest.map(Number);
    if (kind === 'label' || kind === 'button') widgets.push({ kind, index, text });
    else if (kind === 'toggle' && Number.isFinite(value)) widgets.push({ kind, index, text, on: value !== 0 });
    else if (kind === 'slider' && [value, low, high].every(Number.isFinite) && low <= high) widgets.push({ kind, index, text, value, low, high });
  });
  return widgets;
}

/** The event line of a user input to widget `index`. */
export function eventLine(index: number, value: number): string {
  return `${index} ${value}`;
}
