// The face node of a subgraph (4.3): a node with a free layout whose GUI draws each use of the
// subgraph. The code shape makes one, with Rust code that draws the symbol that the use had.

import { api, newId, type NodeInfo, type Point, type View } from '../api.ts';
import { lanePath, symbolLayout, type SymbolLayout } from '../canvas/symbols.ts';

const RGB = {
  body: [38, 38, 38],
  line: [238, 238, 235],
  chip: [28, 28, 27],
  edge: [93, 93, 88],
  leg: [168, 168, 162],
};
/** The width of a character of the 12 px canvas text, for the anchor of a symbol text. */
const CHAR = 7;

const num = (n: number) => {
  const t = String(Math.round(n * 100) / 100);
  return t.includes('.') ? t : `${t}.0`;
};
const pt = ([x, y]: Point) => `[${num(x)}, ${num(y)}]`;
const rgb = (hex: string | null | undefined) => (hex ? [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) : RGB.body);
const rust = (c: number[]) => `[${c.join(', ')}]`;

/** The Rust source of a face node that draws `layout`. `names` has the pin name of each lead key. */
export function faceCode(folder: string, graph: string, layout: SymbolLayout, names: ReadonlyMap<string, string>, color?: string | null): string {
  const type = folder
    .split(/[_-]/)
    .filter((w) => w !== '')
    .map((w) => w[0].toUpperCase() + w.slice(1))
    .join('');
  const body = rgb(color);
  const light = 0.299 * body[0] + 0.587 * body[1] + 0.114 * body[2] > 140;
  const draw: string[] = [];
  const path = (d: string, c: number[], fill: boolean) => draw.push(`c.path("${d}", ${rust(c)}, ${fill});`);
  for (const l of layout.leads) path(lanePath(l, 0), RGB.leg, false);
  for (const p of layout.parts) {
    if (p.fill === 'body' || p.fill === 'chip') {
      path(p.d, p.fill === 'body' ? body : color ? body : RGB.chip, true);
      path(p.d, p.fill === 'body' ? RGB.line : RGB.edge, false);
    } else if (p.fill === 'mark') path(p.d, RGB.edge, true);
    else path(p.d, p.width ? RGB.leg : RGB.line, false);
  }
  for (const l of layout.leads.filter((l) => l.leg)) {
    draw.push(`c.rect(${pt([l.key.startsWith('i:') ? l.body[0] - 6 : l.body[0], l.body[1] - 2.5])}, [6.0, 5.0], ${rust(RGB.leg)}, true);`);
  }
  for (const t of layout.texts) {
    const w = t.text.length * CHAR;
    const x = t.anchor === 'middle' ? t.at[0] - w / 2 : t.anchor === 'end' ? t.at[0] - w : t.at[0];
    draw.push(`c.text(${pt([x, t.at[1]])}, ${rust(light ? RGB.chip : RGB.line)}, ${JSON.stringify(t.text)});`);
  }
  const pins = layout.leads.map((l) => `ui.pin(${JSON.stringify(l.key.slice(2))}, ${pt(l.end)}); // ${l.key.startsWith('i:') ? 'input' : 'output'} ${names.get(l.key) ?? ''}`);
  return `use patina_sdk::{Ui, node};

/// The look of each use of the subgraph \`${graph}\` (spec 4.3). \`ui\` draws the body of the use, and
/// each \`ui.pin\` puts a public pin, by its key or its name. Add an input to this node and wire it in
/// the subgraph to show a value from inside the subgraph.
#[node(free_layout)]
pub struct ${type};

impl ${type} {
    pub fn init() -> Self {
        ${type}
    }

    pub fn process(&mut self) {}

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut c = ui.canvas(${JSON.stringify(graph)}, ${pt([layout.width, layout.height])});
${draw.map((l) => `            ${l}`).join('\n')}
        }
${pins.map((l) => `        ${l.trimEnd()}`).join('\n')}
    }
}
`;
}

/** The node folder name of the face of the open subgraph: "<graph>_look", with a number when another folder has the name. */
export function faceName(view: View): string {
  const key = (n: string) => n.toLowerCase().replace(/-/g, '_');
  const others = [...view.widgets, ...view.views, ...view.connections, ...view.types, ...view.nodes.filter((n) => n.subgraph || n.builtin).map((n) => n.name)].map(key);
  const base = `${view.graphName.slice(0, 54)}_look`;
  for (let i = 1; ; i++) {
    const name = i === 1 ? base : `${base}${i}`;
    if (!others.includes(key(name))) return name;
  }
}

/** Gives the open subgraph the code shape: a face node that draws its symbol. A face folder from before keeps its code. */
export async function makeFace(view: View, info: NodeInfo | undefined, lanes: (key: string) => number): Promise<View> {
  const symbol = view.graph.symbol;
  const name = faceName(view);
  if (!view.nodes.some((n) => n.name === name && !n.subgraph && !n.builtin)) {
    const [inputs, outputs] = [info?.inputs ?? [], info?.outputs ?? []];
    const layout = symbolLayout(symbol && symbol.shape !== 'code' ? symbol : { shape: 'chip', label: symbol?.label }, view.graphName, inputs, outputs, lanes);
    const names = new Map([...inputs.map((p) => [`i:${p.key}`, p.name ?? p.key] as const), ...outputs.map((p) => [`o:${p.key}`, p.name ?? p.key] as const)]);
    await api.newNode(name);
    await api.edit({ kind: 'SetCode', path: `nodes/${name}/src/lib.rs`, text: faceCode(name, view.graphName, layout, names, symbol?.color) });
  }
  const id = newId();
  const nodes = view.graph.node ?? [];
  const pos: Point = [Math.min(0, ...nodes.map((n) => n.pos?.[0] ?? 0)), Math.min(0, ...nodes.map((n) => n.pos?.[1] ?? 0)) - 200];
  return api.edit({ kind: 'Group', edits: [{ kind: 'AddNode', id, folder: `nodes/${name}`, pos }, { kind: 'SetSymbol', symbol: { shape: 'code', face: id, label: symbol?.label ?? null, color: symbol?.color ?? null } }] });
}
