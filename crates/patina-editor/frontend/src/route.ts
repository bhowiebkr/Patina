// The automatic routes of the wires (spec 15.9). A wire with the graph default look goes around
// the node bodies: A* on a sparse grid finds an orthogonal path with few bends, and the wires of
// one output share a trunk. Then a nudge spreads the parallel segments in each channel (nudge.ts),
// and the Router keeps the routes of one view (router.ts).

import type { Point } from './api';
import { orthogonal } from './wires.ts';

/** A node body on the canvas. */
export interface Rect {
  l: number;
  t: number;
  r: number;
  b: number;
}

/** A wire to route: its pins, its route points, its output and its drawn width. */
export interface WireIn {
  sx: number;
  sy: number;
  tx: number;
  ty: number;
  via: readonly Point[];
  net: string;
  width: number;
}

/** The distance of the grid lines from a node body, and the length of a pin stub. */
export const PAD = 20;
/** The space between two parallel wires. */
export const GAP = 8;
/** The space that a wire keeps from each node body. */
export const MARGIN = 6;
/** The cost of a bend, as a length in pixels. */
const BEND = 60;
// shortcut: a graph with a larger grid gets the simple orthogonal paths; use a sparser grid when such graphs matter.
const MAX_CELLS = 2_000_000;
const MAX_EXPAND = 300_000;

export interface Leg {
  id: string;
  w: WireIn;
}

// The bits of a grid vertex: inside a body, the edge to the east is blocked, the edge to the south is blocked.
const INSIDE = 1;
const EAST = 2;
const SOUTH = 4;
let cells = new Uint8Array(0);
let wideCells = new Uint8Array(0);

/** The space that a wire keeps from each node body: a bus also keeps its half width. */
const clearance = (width: number) => (width > 2 ? MARGIN + width / 2 : MARGIN);
/** The distance of the grid lines from a body and the stub length for a wire: more than PAD for a bus that needs it. */
function reach(width: number): number {
  const c = clearance(width);
  return c > PAD ? c + GAP : PAD;
}

interface Grid {
  xs: number[];
  ys: number[];
  nx: number;
  ny: number;
  cell: Uint8Array;
}

function unique(a: number[]): number[] {
  a.sort((p, q) => p - q);
  let n = 0;
  for (const v of a) if (n === 0 || v !== a[n - 1]) a[n++] = v;
  a.length = n;
  return a;
}

/** The first index with a[i] >= v, or with a[i] > v when `after`. */
function bound(a: readonly number[], v: number, after = false): number {
  let lo = 0;
  let hi = a.length;
  while (lo < hi) {
    const m = (lo + hi) >> 1;
    if (a[m] < v || (after && a[m] === v)) lo = m + 1;
    else hi = m;
  }
  return lo;
}

/** The lines next to each body and through each pin stub and route point, with the blocked parts. */
function grid(rects: readonly Rect[], legs: readonly Leg[]): Grid | null {
  const xs: number[] = [];
  const ys: number[] = [];
  const offs = new Set([PAD]);
  for (const { w } of legs) {
    const o = reach(w.width);
    offs.add(o);
    xs.push(w.sx + PAD, w.tx - PAD, w.sx + o, w.tx - o);
    ys.push(w.sy, w.ty);
    for (const [x, y] of w.via) xs.push(x), ys.push(y);
  }
  for (const o of offs) for (const r of rects) xs.push(r.l - o, r.r + o), ys.push(r.t - o, r.b + o);
  unique(xs);
  unique(ys);
  const nx = xs.length;
  const ny = ys.length;
  if (nx * ny > MAX_CELLS) return null;
  if (cells.length < nx * ny) cells = new Uint8Array(nx * ny);
  const g = { xs, ys, nx, ny, cell: cells };
  block(g, rects, MARGIN);
  return g;
}

/** Marks the vertices and the edges of the grid that are nearer than `clear` to a body. */
function block(g: Grid, rects: readonly Rect[], clear: number) {
  const { xs, ys, nx, ny, cell } = g;
  cell.fill(0, 0, nx * ny);
  for (const r of rects) {
    const i0 = bound(xs, r.l - clear, true);
    const i1 = bound(xs, r.r + clear) - 1;
    const j0 = bound(ys, r.t - clear, true);
    const j1 = bound(ys, r.b + clear) - 1;
    for (let j = j0; j <= j1; j++) {
      for (let i = i0; i <= i1; i++) cell[j * nx + i] |= INSIDE;
      for (let i = Math.max(0, i0 - 1); i <= Math.min(i1, nx - 2); i++) cell[j * nx + i] |= EAST;
    }
    for (let i = i0; i <= i1; i++) for (let j = Math.max(0, j0 - 1); j <= Math.min(j1, ny - 2); j++) cell[j * nx + i] |= SOUTH;
  }
}

/** The vertex at the point, or -1 when the point is not on the grid or is in a body. */
function free(g: Grid, x: number, y: number): number {
  const i = bound(g.xs, x);
  const j = bound(g.ys, y);
  if (g.xs[i] !== x || g.ys[j] !== y) return -1;
  const v = j * g.nx + i;
  return g.cell[v] & INSIDE ? -1 : v;
}

// The search buffers, used again by each search. A state is vertex * 2 + axis (0 horizontal, 1 vertical).
let cost = new Float32Array(0);
let back = new Int32Array(0);
let stamp = new Uint16Array(0);
let gen = 0;
let heapKey = new Float64Array(1024);
let heapVal = new Int32Array(1024);
let heapN = 0;

function push(k: number, v: number) {
  if (heapN === heapKey.length) {
    const [keys, vals] = [new Float64Array(heapN * 2), new Int32Array(heapN * 2)];
    keys.set(heapKey);
    vals.set(heapVal);
    [heapKey, heapVal] = [keys, vals];
  }
  let i = heapN++;
  while (i > 0) {
    const p = (i - 1) >> 1;
    if (heapKey[p] <= k) break;
    heapKey[i] = heapKey[p];
    heapVal[i] = heapVal[p];
    i = p;
  }
  heapKey[i] = k;
  heapVal[i] = v;
}

function pop(): number {
  const top = heapVal[0];
  const k = heapKey[--heapN];
  const v = heapVal[heapN];
  let i = 0;
  for (;;) {
    let c = 2 * i + 1;
    if (c >= heapN) break;
    if (c + 1 < heapN && heapKey[c + 1] < heapKey[c]) c++;
    if (heapKey[c] >= k) break;
    heapKey[i] = heapKey[c];
    heapVal[i] = heapVal[c];
    i = c;
  }
  heapKey[i] = k;
  heapVal[i] = v;
  return top;
}

/** The fewest bends from axis `a` to a goal at (dx, dy) that the path reaches on `axis` (-1 for each axis). */
function turns(dx: number, dy: number, a: number, axis: number): number {
  const across = a === 0 ? dy > 0 : dx > 0;
  if (across) return axis === a ? 2 : 1;
  return axis >= 0 && axis !== a ? 1 : 0;
}

/** A* from the seed states to the goal vertex on `axis`. Returns the end state, or -1. */
function search(g: Grid, seeds: readonly number[], goal: number, axis: number): number {
  const { xs, ys, nx, ny, cell } = g;
  const n = nx * ny * 2;
  if (stamp.length < n) [cost, back, stamp] = [new Float32Array(n), new Int32Array(n), new Uint16Array(n)];
  if (++gen > 32767) {
    stamp.fill(0);
    gen = 1;
  }
  const open = gen * 2;
  const shut = open + 1;
  heapN = 0;
  const gx = xs[goal % nx];
  const gy = ys[Math.floor(goal / nx)];
  const h = (v: number, a: number) => {
    const dx = Math.abs(xs[v % nx] - gx);
    const dy = Math.abs(ys[Math.floor(v / nx)] - gy);
    return dx + dy + turns(dx, dy, a, axis) * BEND;
  };
  for (const s of seeds) {
    cost[s] = 0;
    back[s] = -1;
    stamp[s] = open;
    push(h(s >> 1, s & 1), s);
  }
  for (let count = 0; heapN > 0; ) {
    const s = pop();
    if (stamp[s] === shut) continue;
    stamp[s] = shut;
    const v = s >> 1;
    const a = s & 1;
    if (v === goal && (axis < 0 || a === axis)) return s;
    if (++count > MAX_EXPAND) return -1;
    const i = v % nx;
    const j = (v - i) / nx;
    for (let e = 0; e < 4; e++) {
      let w: number;
      if (e === 0) {
        if (i + 1 >= nx || cell[v] & EAST) continue;
        w = v + 1;
      } else if (e === 1) {
        if (i === 0 || cell[v - 1] & EAST) continue;
        w = v - 1;
      } else if (e === 2) {
        if (j + 1 >= ny || cell[v] & SOUTH) continue;
        w = v + nx;
      } else {
        if (j === 0 || cell[v - nx] & SOUTH) continue;
        w = v - nx;
      }
      if (cell[w] & INSIDE) continue;
      const b = e >> 1;
      const len = b ? Math.abs(ys[(w - i) / nx] - ys[j]) : Math.abs(xs[w - j * nx] - xs[i]);
      const c = cost[s] + len + (b === a ? 0 : BEND);
      const t = w * 2 + b;
      if (stamp[t] === shut || (stamp[t] === open && cost[t] <= c)) continue;
      cost[t] = c;
      back[t] = s;
      stamp[t] = open;
      push(c + h(w, b) * (1 + 1e-6), t);
    }
  }
  return -1;
}

function trace(s: number): number[] {
  const out: number[] = [];
  for (; s >= 0; s = back[s]) out.push(s);
  return out.reverse();
}

export const isVia = (p: Point, via: readonly Point[]) => via.some(([x, y]) => x === p[0] && y === p[1]);

/** Removes the repeated points and the points in line, but keeps each route point. */
export function simplify(pts: readonly Point[], via: readonly Point[]): Point[] {
  const out: Point[] = [];
  for (const p of pts) {
    const n = out.length;
    if (n && out[n - 1][0] === p[0] && out[n - 1][1] === p[1]) continue;
    if (n >= 2) {
      const [a, b] = [out[n - 2], out[n - 1]];
      if (((a[0] === b[0] && b[0] === p[0]) || (a[1] === b[1] && b[1] === p[1])) && !isVia(b, via)) out.pop();
    }
    out.push(p);
  }
  return out;
}

function toPoints(g: Grid, states: readonly number[], w: WireIn): Point[] {
  const pts: Point[] = [[w.sx, w.sy]];
  for (const s of states) {
    const v = s >> 1;
    pts.push([g.xs[v % g.nx], g.ys[Math.floor(v / g.nx)]]);
  }
  pts.push([w.tx, w.ty]);
  return simplify(pts, w.via);
}

/** The simple orthogonal path, for a wire with no clear path or a graph that is too large. */
function fallback(w: WireIn): Point[] {
  return simplify([[w.sx, w.sy], ...orthogonal(w.sx, w.sy, w.tx, w.ty, w.via), [w.tx, w.ty]], w.via);
}

/** A wire with route points goes alone, through each point in turn. Null when it has no clear path. */
function routeVia(g: Grid, w: WireIn, pad: number): Point[] | null {
  const marks = [free(g, w.sx + pad, w.sy), ...w.via.map(([x, y]) => free(g, x, y)), free(g, w.tx - pad, w.ty)];
  if (marks.includes(-1)) return null;
  const states = [marks[0] * 2];
  for (let k = 1; k < marks.length; k++) {
    const end = search(g, [states[states.length - 1]], marks[k], k === marks.length - 1 ? 0 : -1);
    if (end < 0) return null;
    states.push(...trace(end).slice(1));
  }
  return toPoints(g, states, w);
}

/**
 * The wires of one output: the nearest target first, then each next target from the nearest point of the tree.
 * A wire with no clear path gets the simple path, and its ID goes into `conflicts`.
 */
function routeNet(g: Grid, legs: readonly Leg[], out: Map<string, Point[]>, conflicts: Set<string>, pad: number) {
  const tree: Leg[] = [];
  const lost = ({ id, w }: Leg) => {
    out.set(id, fallback(w));
    conflicts.add(id);
  };
  for (const l of legs) {
    if (!l.w.via.length) tree.push(l);
    else {
      const p = routeVia(g, l.w, pad);
      if (p) out.set(l.id, p);
      else lost(l);
    }
  }
  if (!tree.length) return;
  const { sx, sy } = tree[0].w;
  const far = ({ w }: Leg) => Math.abs(w.tx - sx) + Math.abs(w.ty - sy);
  tree.sort((a, b) => far(a) - far(b));
  const start = free(g, sx + pad, sy);
  const paths: number[][] = [];
  const owner = new Map<number, [number, number]>();
  for (const l of tree) {
    const { id, w } = l;
    const goal = free(g, w.tx - pad, w.ty);
    const end = start < 0 || goal < 0 ? -1 : search(g, paths.length ? [...owner.keys()] : [start * 2], goal, 0);
    if (end < 0) {
      lost(l);
      continue;
    }
    const branch = trace(end);
    const o = owner.get(branch[0]);
    const states = o ? [...paths[o[0]].slice(0, o[1]), ...branch] : branch;
    states.forEach((s, k) => owner.has(s) || owner.set(s, [paths.length, k]));
    paths.push(states);
    out.set(id, toPoints(g, states, w));
  }
}

/** The paths of the nets from pin to pin, before the nudge. A graph that is too large has no conflicts. */
export function routeNets(rects: readonly Rect[], nets: readonly (readonly Leg[])[], conflicts = new Set<string>()): Map<string, Point[]> {
  const out = new Map<string, Point[]>();
  if (!nets.length) return out;
  const g = grid(rects, nets.flat());
  if (!g) {
    for (const legs of nets) for (const l of legs) out.set(l.id, fallback(l.w));
    return out;
  }
  // A bus keeps its clearance from the bodies. A bus with no such path routes as a plain wire.
  const width = (legs: readonly Leg[]) => legs.reduce((m, l) => Math.max(m, l.w.width), 0);
  const wide: Grid = { ...g, cell: wideCells };
  let clear = MARGIN;
  for (const legs of [...nets].sort((a, b) => width(a) - width(b))) {
    const c = clearance(width(legs));
    if (c > MARGIN) {
      if (c !== clear) {
        if (wideCells.length < g.nx * g.ny) wide.cell = wideCells = new Uint8Array(g.nx * g.ny);
        block(wide, rects, (clear = c));
      }
      const lost = new Set<string>();
      routeNet(wide, legs, out, lost, reach(width(legs)));
      if (!lost.size) continue;
    }
    routeNet(g, legs, out, conflicts, PAD);
  }
  return out;
}
