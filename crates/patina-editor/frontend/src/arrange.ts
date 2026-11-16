// The Arrange command (spec 15.9): a layered layout. The signal flows from left to right in
// columns, the nodes of a column keep their order to cut the crossings, and the space between two
// columns grows with the wires that bend in it.

import type { Point } from './api.ts';
import { pav } from './route.ts';

/** A node to place: its size, and `first` or `last` for a public input or output (6.4). */
export interface Box {
  id: string;
  w: number;
  h: number;
  side?: 'first' | 'last';
}

/** A wire: its ends, the height of each pin in its node and the output that it comes from. */
export interface Link {
  from: string;
  fy: number;
  to: string;
  ty: number;
  net: string;
}

/** The base space between two columns. */
const GAP_X = 80;
/** The extra space between two columns for each output that has a wire with a bend in the gap. */
const LANE_X = 12;
/** The space between two nodes in a column. */
const GAP_Y = 32;
/** The half space next to a wire that goes through a column. */
const LANE_Y = 10;
const SWEEPS = 16;

/** A node, or a point where a long wire goes through a column (box -1). */
interface Item {
  box: number;
  h: number;
  layer: number;
  y: number;
  /** The neighbors in the column before and after: item, pin height in the neighbor, own pin height. */
  prev: [number, number, number][];
  next: [number, number, number][];
}

function columns(boxes: readonly Box[], dag: readonly [number, number][]): number[] {
  const n = boxes.length;
  const preds: number[][] = boxes.map(() => []);
  const succs: number[][] = boxes.map(() => []);
  for (const [u, v] of dag) succs[u].push(v), preds[v].push(u);
  const deg = preds.map((p) => p.length);
  const topo = boxes.map((_, i) => i).filter((i) => deg[i] === 0);
  for (let k = 0; k < topo.length; k++) for (const v of succs[topo[k]]) if (--deg[v] === 0) topo.push(v);
  const layer = new Array<number>(n).fill(0);
  const fix = () => {
    for (const v of topo) for (const u of preds[v]) layer[v] = Math.max(layer[v], layer[u] + 1);
  };
  fix();
  // A node with no inputs moves right, next to the first node that it feeds.
  for (let k = n - 1; k >= 0; k--) {
    const v = topo[k];
    if (!preds[v].length && succs[v].length && boxes[v].side !== 'first') layer[v] = Math.max(0, Math.min(...succs[v].map((s) => layer[s])) - 1);
  }
  const first = boxes.some((b) => b.side === 'first');
  boxes.forEach((b, i) => (layer[i] = b.side === 'first' ? 0 : first ? Math.max(layer[i], 1) : layer[i]));
  fix();
  const inner = boxes.map((b, i) => (b.side === 'last' ? 0 : layer[i]));
  const last = Math.max(0, ...inner) + 1;
  boxes.forEach((b, i) => b.side === 'last' && (layer[i] = last));
  return layer;
}

/** Marks the wires that close a loop: the wires that go back in an order with few such wires (Eades, Lin and Smyth). */
function loops(boxes: readonly Box[], edges: readonly { a: number; b: number }[]): Uint8Array {
  const n = boxes.length;
  const [ins, outs] = [new Array<number>(n).fill(0), new Array<number>(n).fill(0)];
  for (const e of edges) outs[e.a]++, ins[e.b]++;
  const done = new Uint8Array(n);
  const [head, tail]: number[][] = [[], []];
  const take = (v: number, end: number[]) => {
    done[v] = 1;
    end.push(v);
    for (const e of edges) if (e.a === v && !done[e.b]) ins[e.b]--;
    for (const e of edges) if (e.b === v && !done[e.a]) outs[e.a]--;
  };
  boxes.forEach((b, i) => b.side === 'first' && take(i, head));
  boxes.forEach((b, i) => b.side === 'last' && take(i, tail));
  for (let left = n - head.length - tail.length; left > 0; ) {
    const free = boxes.map((_, i) => i).filter((i) => !done[i]);
    const sink = free.find((i) => outs[i] === 0);
    const source = free.find((i) => ins[i] === 0);
    const v = sink ?? source ?? free.reduce((p, q) => (outs[q] - ins[q] > outs[p] - ins[p] ? q : p));
    take(v, sink === undefined ? head : tail);
    left--;
  }
  const at = new Array<number>(n);
  [...head, ...tail.reverse()].forEach((v, k) => (at[v] = k));
  return Uint8Array.from(edges, (e) => (at[e.a] > at[e.b] ? 1 : 0));
}

/** The top left corner of each box, with the full layout at (0, 0). */
export function arrange(boxes: readonly Box[], links: readonly Link[]): Map<string, Point> {
  const index = new Map(boxes.map((b, i) => [b.id, i]));
  const edges = links.flatMap((l) => {
    const [a, b] = [index.get(l.from), index.get(l.to)];
    return a === undefined || b === undefined || a === b ? [] : [{ a, b, fy: l.fy, ty: l.ty, net: l.net }];
  });
  const back = loops(boxes, edges);
  const dag = edges.map((e, k): [number, number, number, number] => (back[k] ? [e.b, e.a, e.ty, e.fy] : [e.a, e.b, e.fy, e.ty]));
  const layer = columns(
    boxes,
    dag.map(([u, v]) => [u, v]),
  );

  // The items of each column. A long wire gets one item in each column that it goes through, shared by its output.
  const items: Item[] = boxes.map((b, i) => ({ box: i, h: b.h, layer: layer[i], y: 0, prev: [], next: [] }));
  const via = new Map<string, number>();
  // The wire parts in the gap after each column: output, item and pin height at each end.
  const parts: [string, number, number, number, number][][] = [];
  dag.forEach(([u, v, uy, vy], k) => {
    let [at, atY] = [u, uy];
    for (let l = layer[u] + 1; l <= layer[v]; l++) {
      let to = v;
      if (l < layer[v]) {
        const key = `${back[k]} ${u} ${uy} ${l}`;
        to = via.get(key) ?? items.push({ box: -1, h: 0, layer: l, y: 0, prev: [], next: [] }) - 1;
        if (via.has(key)) {
          [at, atY] = [to, 0];
          continue;
        }
        via.set(key, to);
      }
      const toY = to === v ? vy : 0;
      (parts[l - 1] ??= []).push([`${back[k]} ${u} ${uy}`, at, atY, to, toY]);
      items[at].next.push([to, toY, atY]);
      items[to].prev.push([at, atY, toY]);
      [at, atY] = [to, 0];
    }
  });

  const count = Math.max(0, ...layer) + 1;
  const cols: number[][] = Array.from({ length: count }, () => []);
  items.forEach((it, i) => cols[it.layer].push(i));
  const space = (a: Item, b: Item) => a.h + (a.box < 0 ? LANE_Y : GAP_Y / 2) + (b.box < 0 ? LANE_Y : GAP_Y / 2);
  // Puts a column near the wanted tops, in order of the wanted tops when `sort`.
  const settle = (col: number[], want: (it: Item) => number | null, sort: boolean) => {
    const wanted = col.map((i) => want(items[i]) ?? items[i].y);
    const order = col.map((_, k) => k);
    if (sort) order.sort((p, q) => wanted[p] - wanted[q] || p - q);
    const list = order.map((k) => col[k]);
    const off = [0];
    for (let k = 1; k < list.length; k++) off.push(off[k - 1] + space(items[list[k - 1]], items[list[k]]));
    const ys = pav(
      order.map((k) => wanted[k]),
      off,
      list.map(() => 1),
    );
    list.forEach((i, k) => (items[i].y = ys[k]));
    col.splice(0, col.length, ...list);
  };
  const mean = (ends: [number, number, number][]) => (ends.length ? ends.reduce((s, [i, at, own]) => s + items[i].y + at - own, 0) / ends.length : null);
  for (const col of cols) settle(col, () => null, false);
  for (let s = 0; s < SWEEPS; s++) {
    const sort = s < SWEEPS - 4;
    if (s % 2 === 0) for (let l = 1; l < count; l++) settle(cols[l], (it) => mean(it.prev), sort);
    else for (let l = count - 2; l >= 0; l--) settle(cols[l], (it) => mean(it.next), sort);
  }
  // A long wire stays at the height of its output, so it bends only next to its input.
  for (let l = 0; l < count; l++) settle(cols[l], (it) => (it.box < 0 ? mean(it.prev) : mean([...it.prev, ...it.next])), false);

  // A straight wire needs no lane. The bends of two outputs share a lane when their heights do not overlap.
  const lanes = (l: number) => {
    const span = new Map<string, [number, number]>();
    for (const [net, a, ay, b, by] of parts[l] ?? []) {
      const [y0, y1] = [items[a].y + ay, items[b].y + by];
      if (Math.abs(y0 - y1) <= 1) continue;
      const [lo, hi] = span.get(net) ?? [Infinity, -Infinity];
      span.set(net, [Math.min(lo, y0, y1), Math.max(hi, y0, y1)]);
    }
    const ends = [...span.values()].flatMap(([lo, hi]): [number, number][] => [[lo, 1], [hi, -1]]).sort((p, q) => p[0] - q[0] || p[1] - q[1]);
    let [depth, most] = [0, 0];
    for (const [, d] of ends) most = Math.max(most, (depth += d));
    return most;
  };
  const xs = [0];
  for (let l = 0; l + 1 < count; l++) {
    const wide = Math.max(0, ...cols[l].map((i) => (items[i].box < 0 ? 0 : boxes[items[i].box].w)));
    xs.push(xs[l] + wide + GAP_X + LANE_X * lanes(l));
  }
  const top = Math.min(...items.map((it) => it.y));
  const out = new Map<string, Point>();
  boxes.forEach((b, i) => out.set(b.id, [Math.round(xs[layer[i]]), Math.round(items[i].y - top)]));
  return out;
}
