// The nudge of the wire routes (spec 15.9): it spreads the parallel segments in each channel.

import type { Point } from '../api';
import { GAP, isVia, MARGIN, type Rect, type WireIn } from './route.ts';

/** The shortest leg that a nudge leaves next to a bend. */
const MIN_LEG = 10;
/** The widest channel in which a segment moves to the middle. */
const MAX_CENTER = 800;
/** The over-relaxation of the placement solver. */
const OMEGA = 1.5;
/** The space between a segment and a node body in a channel too narrow for the gaps. */
const NARROW_MARGIN = 1;

/** The segments of one net on one line that touch, which move together. */
interface Unit {
  c: number;
  s0: number;
  s1: number;
  width: number;
  fixed: boolean;
  lo: number;
  hi: number;
  /** The bounds with the margin of a narrow channel. */
  lo1: number;
  hi1: number;
  d: number;
  net: string;
  /** Pairs of path index and segment index. */
  segs: number[];
  /** Pairs of position and side of each connected segment: -1 or 1, or -2 or 2 when that segment cannot move later. */
  ends: number[];
}

/** The segments of `a` and `b` that cross or touch when `a` is before `b`. */
function crossings(a: Unit, b: Unit): number {
  const t = (a.width + b.width) / 2;
  let n = 0;
  for (let k = 0; k < a.ends.length; k += 2) if (a.ends[k + 1] > 0 && b.s0 - t < a.ends[k] && a.ends[k] < b.s1 + t) n++;
  for (let k = 0; k < b.ends.length; k += 2) if (b.ends[k + 1] < 0 && a.s0 - t < b.ends[k] && b.ends[k] < a.s1 + t) n++;
  return n;
}

/** The smallest distance between a segment out of `a` and a segment into `b`, when `a` goes before `b`. A segment that moves later adds a gap. */
function near(a: Unit, b: Unit): number {
  let d = Infinity;
  for (let k = 0; k < a.ends.length; k += 2) {
    if (a.ends[k + 1] <= 0) continue;
    for (let m = 0; m < b.ends.length; m += 2) {
      if (b.ends[m + 1] < 0) d = Math.min(d, Math.abs(a.ends[k] - b.ends[m]) + (a.ends[k + 1] === 2 && b.ends[m + 1] === -2 ? 0 : GAP));
    }
  }
  return d;
}

/** True when `a` and `b` overlap along the line, so that they keep a gap. */
function overlaps(a: Unit, b: Unit): boolean {
  return a.s0 < b.s1 + GAP && b.s0 < a.s1 + GAP;
}

function before(a: Unit, b: Unit): boolean {
  const t = (a.width + b.width) / 2 + GAP;
  // Only one order can fit in the bounds.
  if (overlaps(a, b) && a.lo + t <= b.hi !== b.lo + t <= a.hi) return a.lo + t <= b.hi;
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

/**
 * Puts the units near their wanted places, each with a gap from the units before it that overlap
 * it along the line (least squares, by projected SOR on the dual). A channel that is too narrow
 * for the gaps first uses a body margin of 1 px, then gets smaller gaps, so that no two lines meet.
 */
function place(list: Unit[]) {
  const n = list.length;
  let lo = list.map((u) => u.lo);
  let hi = list.map((u) => u.hi);
  // The units i < j that overlap along the line, and their gap: x[j] - x[i] >= gap. The bounds of the two can make the gap smaller.
  const gaps = () => {
    const out: number[] = [];
    for (let j = 1; j < n; j++) {
      for (let i = 0; i < j; i++) {
        if (!overlaps(list[i], list[j])) continue;
        const gap = Math.min((list[i].width + list[j].width) / 2 + GAP, hi[j] - lo[i]);
        if (gap > 0) out.push(i, j, gap);
      }
    }
    return out;
  };
  let pairs = gaps();
  // True when the lowest places with the gaps times f stay under the upper bounds.
  const fits = (f: number) => {
    const at = [...lo];
    for (let k = 0; k < pairs.length; k += 3) at[pairs[k + 1]] = Math.max(at[pairs[k + 1]], at[pairs[k]] + f * pairs[k + 2]);
    return at.every((v, i) => v <= hi[i] + 1e-6);
  };
  if (!fits(1)) {
    lo = list.map((u) => u.lo1);
    hi = list.map((u) => u.hi1);
    pairs = gaps();
  }
  let f = 1;
  if (!fits(1)) {
    let [a, b] = [0, 1];
    for (let k = 0; k < 12; k++) [a, b] = fits((a + b) / 2) ? [(a + b) / 2, b] : [a, (a + b) / 2];
    f = a;
  }
  const x = list.map((u) => u.d);
  if (fits(f)) {
    const w = list.map((u) => (u.fixed ? 1e6 : 1));
    const m = pairs.length / 3;
    const lam = new Float64Array(m + 2 * n);
    let move = Infinity;
    // One dual step of constraint k, with the violation r and the inverse curvature inv. It gives the change of the multiplier.
    const step = (k: number, r: number, inv: number) => {
      const l = Math.max(0, lam[k] + (OMEGA * r) / inv);
      const dl = l - lam[k];
      lam[k] = l;
      move = Math.max(move, Math.abs(dl) * inv);
      return dl;
    };
    for (let sweep = 0; sweep < 1000 && move > 1e-4; sweep++) {
      move = 0;
      for (let k = 0; k < m; k++) {
        const i = pairs[3 * k];
        const j = pairs[3 * k + 1];
        const dl = step(k, x[i] + f * pairs[3 * k + 2] - x[j], 1 / w[i] + 1 / w[j]);
        x[i] -= dl / w[i];
        x[j] += dl / w[j];
      }
      for (let i = 0; i < n; i++) {
        x[i] += step(m + 2 * i, lo[i] - x[i], 1 / w[i]) / w[i];
        x[i] -= step(m + 2 * i + 1, x[i] - hi[i], 1 / w[i]) / w[i];
      }
    }
  }
  list.forEach((u, i) => (u.d = Math.min(Math.max(x[i], lo[i]), hi[i])));
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
        u = { c: pts[k][c], s0, s1, width: 0, fixed: false, lo: -Infinity, hi: Infinity, lo1: 0, hi1: 0, d: 0, net: w.net, segs: [], ends: [] };
        units.push(u);
      }
      u.s1 = Math.max(u.s1, s1);
      u.width = Math.max(u.width, w.width);
      u.fixed ||= k === 0 || k === pts.length - 2 || isVia(pts[k], w.via) || isVia(pts[k + 1], w.via);
      u.segs.push(p, k);
      for (const [q, at] of [[k - 1, k], [k + 2, k + 1]]) {
        if (q < 0 || q >= pts.length) continue;
        const side = Math.sign(pts[q][c] - u.c);
        // The next segments on the other axis move later, except the pin segments and the via segments.
        const pin = c === 1 || q === 0 || q === pts.length - 1 || isVia(pts[q], w.via) || isVia(pts[at], w.via);
        u.ends.push(pts[at][s], pin ? 2 * side : side);
        if (side < 0) u.lo = Math.max(u.lo, pts[q][c] + MIN_LEG);
        else if (side > 0) u.hi = Math.min(u.hi, pts[q][c] - MIN_LEG);
      }
    }
  }
  for (const u of units) {
    let [lo, hi, lo1, hi1] = [u.lo, u.hi, u.lo, u.hi];
    for (const r of rects) {
      const [r0, r1, q0, q1] = c === 0 ? [r.l, r.r, r.t, r.b] : [r.t, r.b, r.l, r.r];
      if (q1 + MARGIN <= u.s0 || q0 - MARGIN >= u.s1) continue;
      if (r1 + MARGIN <= u.c) {
        lo = Math.max(lo, r1 + MARGIN + u.width / 2);
        lo1 = Math.max(lo1, r1 + NARROW_MARGIN + u.width / 2);
      } else if (r0 - MARGIN >= u.c) {
        hi = Math.min(hi, r0 - MARGIN - u.width / 2);
        hi1 = Math.min(hi1, r0 - NARROW_MARGIN - u.width / 2);
      }
    }
    if (u.fixed || lo > hi) [lo, hi, lo1, hi1] = [u.c, u.c, u.c, u.c];
    [u.lo, u.hi, u.lo1, u.hi1] = [lo, hi, lo1, hi1];
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
    // A unit moves to the place that most lowers the sum of max(0, t - near) with the units that it passes, when the bounds let it.
    for (let pass = 0, moved = true; moved && pass < 4; pass++) {
      moved = false;
      for (let j = 0; j < list.length; j++) {
        const b = list[j];
        let [best, at] = [-1e-9, -1];
        for (const dir of [-1, 1]) {
          let sum = 0;
          // The units of one net keep their order, so that the legs between them keep their direction.
          for (let k = j + dir; k >= 0 && k < list.length && list[k].net !== b.net; k += dir) {
            const a = list[k];
            if (!overlaps(a, b)) continue;
            const t = (a.width + b.width) / 2 + GAP;
            const [x, y] = dir < 0 ? [b, a] : [a, b];
            if (x.lo + t > y.hi) break;
            sum += Math.max(0, t - near(x, y)) - Math.max(0, t - near(y, x));
            if (sum < best) [best, at] = [sum, k];
          }
        }
        if (at < 0) continue;
        list.splice(j, 1);
        list.splice(at, 0, b);
        moved = true;
      }
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
