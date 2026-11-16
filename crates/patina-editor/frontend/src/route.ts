// The automatic routes of the wires (spec 15.9). A wire with the graph default look goes around
// the node bodies: A* on a sparse grid finds an orthogonal path with few bends, and the wires of
// one output share a trunk. Then a nudge spreads the parallel segments in each channel.

import type { Point } from './api';
import { crossingGaps, junctions, type Gap } from './marks.ts';
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
/** The shortest leg that a nudge leaves next to a bend. */
const MIN_LEG = 10;
/** The widest channel in which a segment moves to the middle. */
const MAX_CENTER = 800;
// shortcut: a graph with a larger grid gets the simple orthogonal paths; use a sparser grid when such graphs matter.
const MAX_CELLS = 2_000_000;
const MAX_EXPAND = 300_000;
/** The time after the last change, in ms, before all nets route again. */
const SETTLE = 250;

interface Leg {
  id: string;
  w: WireIn;
}

// The bits of a grid vertex: inside a body, the edge to the east is blocked, the edge to the south is blocked.
const INSIDE = 1;
const EAST = 2;
const SOUTH = 4;
let cells = new Uint8Array(0);

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
  for (const r of rects) xs.push(r.l - PAD, r.r + PAD), ys.push(r.t - PAD, r.b + PAD);
  for (const { w } of legs) {
    xs.push(w.sx + PAD, w.tx - PAD);
    ys.push(w.sy, w.ty);
    for (const [x, y] of w.via) xs.push(x), ys.push(y);
  }
  unique(xs);
  unique(ys);
  const nx = xs.length;
  const ny = ys.length;
  if (nx * ny > MAX_CELLS) return null;
  if (cells.length < nx * ny) cells = new Uint8Array(nx * ny);
  const cell = cells;
  cell.fill(0, 0, nx * ny);
  for (const r of rects) {
    const i0 = bound(xs, r.l - MARGIN, true);
    const i1 = bound(xs, r.r + MARGIN) - 1;
    const j0 = bound(ys, r.t - MARGIN, true);
    const j1 = bound(ys, r.b + MARGIN) - 1;
    for (let j = j0; j <= j1; j++) {
      for (let i = i0; i <= i1; i++) cell[j * nx + i] |= INSIDE;
      for (let i = Math.max(0, i0 - 1); i <= Math.min(i1, nx - 2); i++) cell[j * nx + i] |= EAST;
    }
    for (let i = i0; i <= i1; i++) for (let j = Math.max(0, j0 - 1); j <= Math.min(j1, ny - 2); j++) cell[j * nx + i] |= SOUTH;
  }
  return { xs, ys, nx, ny, cell };
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

const isVia = (p: Point, via: readonly Point[]) => via.some(([x, y]) => x === p[0] && y === p[1]);

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
function routeVia(g: Grid, w: WireIn): Point[] | null {
  const marks = [free(g, w.sx + PAD, w.sy), ...w.via.map(([x, y]) => free(g, x, y)), free(g, w.tx - PAD, w.ty)];
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
function routeNet(g: Grid, legs: readonly Leg[], out: Map<string, Point[]>, conflicts: Set<string>) {
  const tree: Leg[] = [];
  const lost = ({ id, w }: Leg) => {
    out.set(id, fallback(w));
    conflicts.add(id);
  };
  for (const l of legs) {
    if (!l.w.via.length) tree.push(l);
    else {
      const p = routeVia(g, l.w);
      if (p) out.set(l.id, p);
      else lost(l);
    }
  }
  if (!tree.length) return;
  const { sx, sy } = tree[0].w;
  const far = ({ w }: Leg) => Math.abs(w.tx - sx) + Math.abs(w.ty - sy);
  tree.sort((a, b) => far(a) - far(b));
  const start = free(g, sx + PAD, sy);
  const paths: number[][] = [];
  const owner = new Map<number, [number, number]>();
  for (const l of tree) {
    const { id, w } = l;
    const goal = free(g, w.tx - PAD, w.ty);
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
function routeNets(rects: readonly Rect[], nets: readonly (readonly Leg[])[], conflicts = new Set<string>()): Map<string, Point[]> {
  const out = new Map<string, Point[]>();
  if (!nets.length) return out;
  const g = grid(rects, nets.flat());
  for (const legs of nets) {
    if (g) routeNet(g, legs, out, conflicts);
    else for (const l of legs) out.set(l.id, fallback(l.w));
  }
  return out;
}

/** The segments of one net on one line that touch, which move together. */
interface Unit {
  c: number;
  s0: number;
  s1: number;
  width: number;
  fixed: boolean;
  lo: number;
  hi: number;
  d: number;
  net: string;
  /** Pairs of path index and segment index. */
  segs: number[];
  /** Pairs of position and side (-1 or 1) of each connected segment. */
  ends: number[];
}

/** The segments of `a` and `b` that cross when `a` is before `b`. */
function crossings(a: Unit, b: Unit): number {
  let n = 0;
  for (let k = 0; k < a.ends.length; k += 2) if (a.ends[k + 1] > 0 && b.s0 < a.ends[k] && a.ends[k] < b.s1) n++;
  for (let k = 0; k < b.ends.length; k += 2) if (b.ends[k + 1] < 0 && a.s0 < b.ends[k] && b.ends[k] < a.s1) n++;
  return n;
}

function before(a: Unit, b: Unit): boolean {
  if (Math.abs(a.d - b.d) >= (a.width + b.width) / 2 + GAP) return a.d < b.d;
  const [ab, ba] = [crossings(a, b), crossings(b, a)];
  if (ab !== ba) return ab < ba;
  return a.d !== b.d ? a.d < b.d : a.c !== b.c ? a.c < b.c : a.net < b.net;
}

/**
 * The values nearest to `want` (weighted least squares) that keep the order and the spaces of
 * `off`: x[i] - x[i-1] >= off[i] - off[i-1] (isotonic regression by pool adjacent violators).
 */
export function pav(want: readonly number[], off: readonly number[], weight: readonly number[]): number[] {
  const val: number[] = [];
  const wt: number[] = [];
  const cnt: number[] = [];
  want.forEach((d, i) => {
    let [v, w, m] = [d - off[i], weight[i], 1];
    while (val.length && val[val.length - 1] > v) {
      const w2 = wt.pop()!;
      v = (val.pop()! * w2 + v * w) / (w2 + w);
      w += w2;
      m += cnt.pop()!;
    }
    val.push(v), wt.push(w), cnt.push(m);
  });
  const out: number[] = [];
  val.forEach((v, k) => {
    for (let m = 0; m < cnt[k]; m++) out.push(v + off[out.length]);
  });
  return out;
}

/** Puts the units in order with a gap between neighbors, near their wanted places. */
function place(list: Unit[]) {
  const off = [0];
  for (let i = 1; i < list.length; i++) off.push(off[i - 1] + (list[i - 1].width + list[i].width) / 2 + GAP);
  const at = pav(list.map((u) => u.d), off, list.map((u) => (u.fixed ? 1e6 : 1)));
  list.forEach((u, i) => (u.d = Math.min(Math.max(at[i], u.lo), u.hi)));
}

/** Moves the segments on axis `c` (0 the vertical segments, 1 the horizontal segments) to spread them in their channels. */
function nudgeAxis(paths: readonly Point[][], wires: readonly WireIn[], rects: readonly Rect[], c: 0 | 1) {
  const s = 1 - c;
  const lines = new Map<string, [number, number, number, number][]>();
  paths.forEach((pts, p) => {
    for (let k = 0; k + 1 < pts.length; k++) {
      const [a, b] = [pts[k], pts[k + 1]];
      if (a[c] !== b[c] || a[s] === b[s]) continue;
      const key = `${wires[p].net}\n${a[c]}`;
      const seg: [number, number, number, number] = [p, k, Math.min(a[s], b[s]), Math.max(a[s], b[s])];
      const list = lines.get(key);
      if (list) list.push(seg);
      else lines.set(key, [seg]);
    }
  });
  const units: Unit[] = [];
  for (const list of lines.values()) {
    list.sort((x, y) => x[2] - y[2]);
    let u: Unit | null = null;
    for (const [p, k, s0, s1] of list) {
      const pts = paths[p];
      const w = wires[p];
      if (!u || s0 > u.s1) {
        u = { c: pts[k][c], s0, s1, width: 0, fixed: false, lo: -Infinity, hi: Infinity, d: 0, net: w.net, segs: [], ends: [] };
        units.push(u);
      }
      u.s1 = Math.max(u.s1, s1);
      u.width = Math.max(u.width, w.width);
      u.fixed ||= k === 0 || k === pts.length - 2 || isVia(pts[k], w.via) || isVia(pts[k + 1], w.via);
      u.segs.push(p, k);
      for (const [q, at] of [[k - 1, k], [k + 2, k + 1]]) {
        if (q < 0 || q >= pts.length) continue;
        const side = Math.sign(pts[q][c] - u.c);
        u.ends.push(pts[at][s], side);
        if (side < 0) u.lo = Math.max(u.lo, pts[q][c] + MIN_LEG);
        else if (side > 0) u.hi = Math.min(u.hi, pts[q][c] - MIN_LEG);
      }
    }
  }
  for (const u of units) {
    let [lo, hi] = [u.lo, u.hi];
    for (const r of rects) {
      const [r0, r1, q0, q1] = c === 0 ? [r.l, r.r, r.t, r.b] : [r.t, r.b, r.l, r.r];
      if (q1 + MARGIN <= u.s0 || q0 - MARGIN >= u.s1) continue;
      if (r1 + MARGIN <= u.c) lo = Math.max(lo, r1 + MARGIN + u.width / 2);
      else if (r0 - MARGIN >= u.c) hi = Math.min(hi, r0 - MARGIN - u.width / 2);
    }
    if (u.fixed || lo > hi) [lo, hi] = [u.c, u.c];
    [u.lo, u.hi] = [lo, hi];
    u.d = hi - lo <= MAX_CENTER ? (lo + hi) / 2 : Math.min(Math.max(u.c, lo), hi);
  }
  // The units that overlap along the line and share a channel make one group.
  const order = units.map((_, i) => i).sort((a, b) => units[a].s0 - units[b].s0);
  const root = units.map((_, i) => i);
  const find = (i: number): number => (root[i] === i ? i : (root[i] = find(root[i])));
  for (let x = 0; x < order.length; x++) {
    const a = units[order[x]];
    for (let y = x + 1; y < order.length; y++) {
      const b = units[order[y]];
      if (b.s0 >= a.s1 + GAP) break;
      if (Math.max(a.lo, b.lo) <= Math.min(a.hi, b.hi)) root[find(order[x])] = find(order[y]);
    }
  }
  const groups = new Map<number, Unit[]>();
  units.forEach((u, i) => {
    const list = groups.get(find(i));
    if (list) list.push(u);
    else groups.set(find(i), [u]);
  });
  for (const list of groups.values()) {
    list.sort((a, b) => a.d - b.d || a.c - b.c);
    for (let i = 1; i < list.length; i++) {
      const u = list[i];
      let j = i - 1;
      for (; j >= 0 && before(u, list[j]); j--) list[j + 1] = list[j];
      list[j + 1] = u;
    }
    place(list);
    for (const u of list) {
      for (let k = 0; k < u.segs.length; k += 2) {
        const pts = paths[u.segs[k]];
        pts[u.segs[k + 1]][c] = u.d;
        pts[u.segs[k + 1] + 1][c] = u.d;
      }
    }
  }
}

/** Spreads the parallel segments of the paths in place: first the vertical segments, then the horizontal segments. */
export function nudge(paths: readonly Point[][], wires: readonly WireIn[], rects: readonly Rect[]) {
  nudgeAxis(paths, wires, rects, 0);
  nudgeAxis(paths, wires, rects, 1);
}

function nets(wires: ReadonlyMap<string, WireIn>): Map<string, Leg[]> {
  const out = new Map<string, Leg[]>();
  for (const [id, wire] of wires) {
    // Pins less than 1 px apart in y route as one line, so the path gets no tiny jog.
    const w = Math.abs(wire.ty - wire.sy) < 1 && wire.ty !== wire.sy ? { ...wire, ty: wire.sy } : wire;
    const legs = out.get(w.net);
    if (legs) legs.push({ id, w });
    else out.set(w.net, [{ id, w }]);
  }
  return out;
}

/** The full paths of the wires, from pin to pin, around the bodies. */
export function route(rects: readonly Rect[], wires: ReadonlyMap<string, WireIn>): Map<string, Point[]> {
  const raw = routeNets(rects, [...nets(wires).values()]);
  const ids = [...raw.keys()];
  const paths = ids.map((id) => raw.get(id)!);
  nudge(paths, ids.map((id) => wires.get(id)!), rects);
  return new Map(ids.map((id, i) => [id, paths[i]]));
}

/** True when a segment of the path goes through a body. */
function blocked(path: readonly Point[] | undefined, rects: readonly Rect[]): boolean {
  return !path || blockedAt(path, rects) >= 0;
}

/** The first segment of the path that goes through a body, or -1. The pin ends can be in the margin of their node. */
function blockedAt(path: readonly Point[], rects: readonly Rect[]): number {
  const trim = MARGIN + 1;
  for (let k = 0; k + 1 < path.length; k++) {
    let [x0, y0] = path[k];
    let [x1, y1] = path[k + 1];
    if (k === 0) x0 = Math.min(x0 + trim, x1);
    if (k === path.length - 2) x1 = Math.max(x1 - trim, x0);
    [x0, x1] = [Math.min(x0, x1), Math.max(x0, x1)];
    [y0, y1] = [Math.min(y0, y1), Math.max(y0, y1)];
    for (const r of rects) if (x0 < r.r + MARGIN && x1 > r.l - MARGIN && y0 < r.b + MARGIN && y1 > r.t - MARGIN) return k;
  }
  return -1;
}

/** The place of the conflict mark: the middle of the first segment through a body, or of the middle segment. */
export function conflictAt(path: readonly Point[], rects: readonly Rect[]): Point {
  const b = blockedAt(path, rects);
  const k = b >= 0 ? b : Math.max(0, Math.floor(path.length / 2) - 1);
  const [p, q] = [path[k], path[Math.min(k + 1, path.length - 1)]];
  return [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2];
}

const samePoints = (a: readonly Point[], b: readonly Point[]) => a.length === b.length && a.every((p, i) => p[0] === b[i][0] && p[1] === b[i][1]);

/**
 * The routes of the wires of one graph view. Each change routes again in a microtask: the nets
 * with a changed wire or a blocked path. Then all nets route again when the changes stop.
 */
export class Router {
  private rects: readonly Rect[] = [];
  private wires = new Map<string, WireIn>();
  private raw = new Map<string, Point[]>();
  private keys = new Map<string, string>();
  private out = new Map<string, readonly Point[]>();
  private conflicts = new Set<string>();
  private marks = new Map<string, Point>();
  private joints = new Map<string, readonly Point[]>();
  private cuts = new Map<string, readonly Gap[]>();
  private listeners = new Set<() => void>();
  private queued = false;
  private moved = false;
  private timer: ReturnType<typeof setTimeout> | undefined;

  setObstacles(rects: readonly Rect[]) {
    const old = this.rects;
    if (rects.length === old.length && rects.every((r, i) => r.l === old[i].l && r.t === old[i].t && r.r === old[i].r && r.b === old[i].b)) return;
    this.rects = rects;
    this.moved = true;
    this.queue();
  }

  setWire(id: string, w: WireIn) {
    const o = this.wires.get(id);
    if (o && o.sx === w.sx && o.sy === w.sy && o.tx === w.tx && o.ty === w.ty && o.net === w.net && o.width === w.width && samePoints(o.via, w.via)) return;
    this.wires.set(id, w);
    this.queue();
  }

  removeWire(id: string) {
    if (this.wires.delete(id)) this.queue();
  }

  /** The bends of the routed wire, the same array until the route changes. */
  bends = (id: string): readonly Point[] | undefined => this.out.get(id);

  /** The place of the conflict mark of a wire with no clear path (15.9), the same array until it moves. */
  conflict = (id: string): Point | undefined => this.marks.get(id);

  /** The junction marks of the output of a wire, on the first routed wire of the output only (15.9). */
  junctions = (id: string): readonly Point[] | undefined => this.joints.get(id);

  /** The gaps of a wire where it goes under a wire of another output (15.9). */
  gaps = (id: string): readonly Gap[] | undefined => this.cuts.get(id);

  subscribe = (f: () => void) => {
    this.listeners.add(f);
    return () => void this.listeners.delete(f);
  };

  private queue() {
    if (this.queued) return;
    this.queued = true;
    queueMicrotask(() => {
      this.queued = false;
      this.compute(false);
    });
  }

  private compute(all: boolean) {
    const groups = nets(this.wires);
    const todo: Leg[][] = [];
    const keys = new Map<string, string>();
    for (const [net, legs] of groups) {
      const key = legs.map(({ id, w }) => `${id} ${w.sx} ${w.sy} ${w.tx} ${w.ty} ${w.via.join(' ')}`).join('\n');
      keys.set(net, key);
      if (all || this.keys.get(net) !== key || legs.some(({ id }) => blocked(this.raw.get(id), this.rects))) todo.push(legs);
    }
    for (const legs of todo) for (const { id } of legs) this.conflicts.delete(id);
    for (const [id, p] of routeNets(this.rects, todo, this.conflicts)) this.raw.set(id, p);
    for (const id of this.raw.keys()) if (!this.wires.has(id)) this.raw.delete(id);
    for (const id of this.conflicts) if (!this.wires.has(id)) this.conflicts.delete(id);
    this.keys = keys;
    clearTimeout(this.timer);
    if (!all && todo.length < groups.size && (todo.length > 0 || this.moved)) this.timer = setTimeout(() => this.compute(true), SETTLE);
    this.moved = false;
    const ids = [...this.raw.keys()];
    const paths = ids.map((id) => this.raw.get(id)!.map(([x, y]): Point => [x, y]));
    nudge(paths, ids.map((id) => this.wires.get(id)!), this.rects);
    const out = new Map<string, readonly Point[]>();
    const marks = new Map<string, Point>();
    let changed = ids.length !== this.out.size || this.conflicts.size !== this.marks.size;
    ids.forEach((id, i) => {
      const bends = paths[i].slice(1, -1);
      const old = this.out.get(id);
      const keep = old !== undefined && samePoints(old, bends);
      out.set(id, keep ? old : bends);
      changed ||= !keep;
      if (!this.conflicts.has(id)) return;
      const at = conflictAt(paths[i], this.rects);
      const was = this.marks.get(id);
      const same = was !== undefined && samePoints([was], [at]);
      marks.set(id, same ? was : at);
      changed ||= !same;
    });
    const byNet = new Map<string, number[]>();
    ids.forEach((id, i) => {
      const net = this.wires.get(id)!.net;
      const list = byNet.get(net);
      if (list) list.push(i);
      else byNet.set(net, [i]);
    });
    const joints = new Map<string, readonly Point[]>();
    for (const list of byNet.values()) {
      const at = junctions(list.map((i) => paths[i]));
      if (!at.length) continue;
      const id = ids[list[0]];
      const was = this.joints.get(id);
      const same = was !== undefined && samePoints(was, at);
      joints.set(id, same ? was : at);
      changed ||= !same;
    }
    changed ||= joints.size !== this.joints.size;
    const ws = ids.map((id) => this.wires.get(id)!);
    const cuts = new Map<string, readonly Gap[]>();
    for (const [i, at] of crossingGaps(paths, ws.map((w) => w.net), ws.map((w) => w.width))) {
      const was = this.cuts.get(ids[i]);
      const same = was !== undefined && was.length === at.length && was.every((g, k) => g.every((v, n) => v === at[k][n]));
      cuts.set(ids[i], same ? was : at);
      changed ||= !same;
    }
    changed ||= cuts.size !== this.cuts.size;
    this.out = out;
    this.marks = marks;
    this.joints = joints;
    this.cuts = cuts;
    if (changed) for (const f of this.listeners) f();
  }
}
