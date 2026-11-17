// The Arrange command (spec 15.9): a layered layout. The signal flows from left to right in
// columns, the nodes of a column keep their order to cut the crossings, and the space between two
// columns grows with the wires that bend in it. A wide bus pulls harder than a thin wire, so the
// buses get straight first, and each wire gets space for its width.

import type { Point } from './api.ts';
import { pav } from './nudge.ts';
import { GAP } from './route.ts';

/** A node to place: its size, and `first` or `last` for a public input or output (6.4). */
export interface Box {
  id: string;
  w: number;
  h: number;
  side?: 'first' | 'last';
}

/** A wire: its ends, the height of each pin in its node, the output that it comes from and its width. */
export interface Link {
  from: string;
  fy: number;
  to: string;
  ty: number;
  net: string;
  /** The width of the drawn wire in px. The default is 2. */
  width?: number;
}

/** The base space between two columns. */
const GAP_X = 80;
/** The extra space between two columns for each output that has a wire with a bend in the gap, at least. */
const LANE_X = 12;
/** The space between two nodes in a column. */
const GAP_Y = 32;
/** The half space next to a wire that goes through a column, at least. */
const LANE_Y = 10;
const SWEEPS = 16;
/** A layout more than this many times as tall as wide splits its tall columns. */
const TALL = 2;
/** The most columns that one column splits into. */
const MAX_SPLIT = 4;

/** A neighbor in the column before or after: item, pin height in the neighbor, own pin height, wire width. */
type End = [number, number, number, number];

/** A node, or a point where a long wire goes through a column (box -1). */
interface Item {
  box: number;
  h: number;
  /** The space above and below the item. */
  half: number;
  layer: number;
  y: number;
  prev: End[];
  next: End[];
}

/** The half space next to a wire of this width that goes through a column. */
const halfOf = (width: number) => Math.max(LANE_Y, width / 2 + GAP);
/** The lane in a column gap for a wire of this width. */
const laneOf = (width: number) => Math.max(LANE_X, width + GAP);
/** A wire pulls its ends together with the square of its width, so a bus wins over thin wires. */
const pull = (width: number) => width * width;

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
    return a === undefined || b === undefined || a === b ? [] : [{ a, b, fy: l.fy, ty: l.ty, net: l.net, width: l.width ?? 2 }];
  });
  const back = loops(boxes, edges);
  const dag = edges.map((e, k): [number, number, number, number, number] =>
    back[k] ? [e.b, e.a, e.ty, e.fy, e.width] : [e.a, e.b, e.fy, e.ty, e.width],
  );
  const layer = columns(
    boxes,
    dag.map(([u, v]) => [u, v]),
  );
  const first = place(boxes, dag, back, layer);
  const split = wrap(boxes, layer, first);
  return split ? place(boxes, dag, back, split) : first;
}

/**
 * The columns of a layout that is more than TALL times as tall as wide, with each column that is
 * taller than the side of a square of the same area split into up to MAX_SPLIT columns. The nodes
 * of a split column go to its columns in turn, from top to bottom. Null when no column splits.
 */
function wrap(boxes: readonly Box[], layer: readonly number[], at: Map<string, Point>): number[] | null {
  const pos = boxes.map((b) => at.get(b.id)!);
  const w = Math.max(...boxes.map((b, i) => pos[i][0] + b.w));
  const h = Math.max(...boxes.map((b, i) => pos[i][1] + b.h));
  if (w <= 0 || h <= TALL * w) return null;
  const side = Math.sqrt(w * h);
  const count = Math.max(...layer) + 1;
  const [parts, rank] = [new Array<number>(count).fill(1), new Array<number>(boxes.length).fill(0)];
  for (let l = 0; l < count; l++) {
    const col = layer.flatMap((m, i) => (m === l ? [i] : [])).sort((p, q) => pos[p][1] - pos[q][1]);
    if (!col.length || col.some((i) => boxes[i].side)) continue;
    const span = Math.max(...col.map((i) => pos[i][1] + boxes[i].h)) - pos[col[0]][1];
    parts[l] = Math.min(MAX_SPLIT, col.length, Math.ceil(span / side));
    col.forEach((i, k) => (rank[i] = k % parts[l]));
  }
  if (parts.every((p) => p === 1)) return null;
  const base = [0];
  for (let l = 0; l < count; l++) base.push(base[l] + parts[l]);
  return layer.map((l, i) => base[l] + rank[i]);
}

/** The top left corner of each box in the columns `layer`, with the full layout at (0, 0). */
function place(boxes: readonly Box[], dag: readonly [number, number, number, number, number][], back: Uint8Array, layer: readonly number[]): Map<string, Point> {
  // The items of each column. A long wire gets one item in each column that it goes through, shared by its output.
  const items: Item[] = boxes.map((b, i) => ({ box: i, h: b.h, half: GAP_Y / 2, layer: layer[i], y: 0, prev: [], next: [] }));
  const via = new Map<string, number>();
  // The wire parts in the gap after each column: output, item and pin height at each end, wire width.
  const parts: [string, number, number, number, number, number][][] = [];
  dag.forEach(([u, v, uy, vy, width], k) => {
    let [at, atY] = [u, uy];
    for (let l = layer[u] + 1; l <= layer[v]; l++) {
      let to = v;
      if (l < layer[v]) {
        const key = `${back[k]} ${u} ${uy} ${l}`;
        to = via.get(key) ?? items.push({ box: -1, h: 0, half: 0, layer: l, y: 0, prev: [], next: [] }) - 1;
        items[to].half = Math.max(items[to].half, halfOf(width));
        if (via.has(key)) {
          [at, atY] = [to, 0];
          continue;
        }
        via.set(key, to);
      }
      const toY = to === v ? vy : 0;
      (parts[l - 1] ??= []).push([`${back[k]} ${u} ${uy}`, at, atY, to, toY, width]);
      items[at].next.push([to, toY, atY, width]);
      items[to].prev.push([at, atY, toY, width]);
      [at, atY] = [to, 0];
    }
  });
  // An item with wide wires moves less when a column is full, so its wires stay straight.
  const weight = items.map((it) => Math.max(1, [...it.prev, ...it.next].reduce((s, e) => s + pull(e[3]), 0)));

  const count = Math.max(0, ...layer) + 1;
  const cols: number[][] = Array.from({ length: count }, () => []);
  items.forEach((it, i) => cols[it.layer].push(i));
  const space = (a: Item, b: Item) => a.h + a.half + b.half;
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
      list.map((i) => weight[i]),
    );
    list.forEach((i, k) => (items[i].y = ys[k]));
    col.splice(0, col.length, ...list);
  };
  // The top that the neighbors want, with each wire weighted by its pull.
  const mean = (ends: readonly End[]) => {
    let [sum, total] = [0, 0];
    for (const [i, at, own, w] of ends) (sum += pull(w) * (items[i].y + at - own)), (total += pull(w));
    return total ? sum / total : null;
  };
  for (const col of cols) settle(col, () => null, false);
  for (let s = 0; s < SWEEPS; s++) {
    const sort = s < SWEEPS - 4;
    if (s % 2 === 0) for (let l = 1; l < count; l++) settle(cols[l], (it) => mean(it.prev), sort);
    else for (let l = count - 2; l >= 0; l--) settle(cols[l], (it) => mean(it.next), sort);
  }
  // A long wire stays at the height of its output, so it bends only next to its input.
  for (let l = 0; l < count; l++) settle(cols[l], (it) => (it.box < 0 ? mean(it.prev) : mean([...it.prev, ...it.next])), false);

  // A straight wire needs no lane. The bends of two outputs share a lane when their heights do not
  // overlap. A lane is as wide as the widest wire of its output, plus the space between two wires.
  const lanes = (l: number) => {
    const span = new Map<string, [number, number, number]>();
    for (const [net, a, ay, b, by, w] of parts[l] ?? []) {
      const [y0, y1] = [items[a].y + ay, items[b].y + by];
      if (Math.abs(y0 - y1) <= 1) continue;
      const [lo, hi, wide] = span.get(net) ?? [Infinity, -Infinity, 0];
      span.set(net, [Math.min(lo, y0, y1), Math.max(hi, y0, y1), Math.max(wide, w)]);
    }
    const ends = [...span.values()]
      .flatMap(([lo, hi, w]): [number, number][] => [
        [lo - w / 2, laneOf(w)],
        [hi + w / 2, -laneOf(w)],
      ])
      .sort((p, q) => p[0] - q[0] || p[1] - q[1]);
    let [depth, most] = [0, 0];
    for (const [, d] of ends) most = Math.max(most, (depth += d));
    return most;
  };
  const xs = [0];
  for (let l = 0; l + 1 < count; l++) {
    const wide = Math.max(0, ...cols[l].map((i) => (items[i].box < 0 ? 0 : boxes[items[i].box].w)));
    xs.push(xs[l] + wide + GAP_X + lanes(l));
  }
  const top = Math.min(...items.map((it) => it.y));
  const out = new Map<string, Point>();
  boxes.forEach((b, i) => out.set(b.id, [Math.round(xs[layer[i]]), Math.round(items[i].y - top)]));
  return out;
}
