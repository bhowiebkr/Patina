// The nudge of the wire routes (spec 15.9): it spreads the parallel segments in each channel.

import type { Point } from '../api';
import { GAP, isVia, MARGIN, type Rect, type WireIn } from './route.ts';

/** The shortest leg that a nudge leaves next to a bend. */
const MIN_LEG = 10;
/** The widest channel in which a segment moves to the middle. */
const MAX_CENTER = 800;

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
