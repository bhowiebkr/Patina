// The marks of the routed wires (spec 15.9): a junction dot where the wires of one output part,
// and a gap where the wires of two outputs cross.

import type { Point } from '../api';
import { cornerRadius } from './wires.ts';

/** A gap in a wire: its center and its half length along the wire. */
export type Gap = readonly [x: number, y: number, half: number];

/** The clear space on each side of the upper wire. */
const CLEAR = 3;

/** True when the point is on the segment between its ends, 0.5 px or nearer. */
export function onSegment([x, y]: Point, [x0, y0]: Point, [x1, y1]: Point): boolean {
  const [dx, dy] = [x1 - x0, y1 - y0];
  const t = ((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy);
  return t > 0 && t < 1 && Math.abs((x - x0) * dy - (y - y0) * dx) < 0.5 * Math.hypot(dx, dy);
}

/** The width and the height of the junction dot of a wire of `width`, a ring on a bus (canvas.css). */
export const jointSize = (width: number) => (width > 2 ? width + 6 : 8);

/** A junction dot: its center and its radius (15.9). */
export type Dot = readonly [x: number, y: number, r: number];

/**
 * The points where the paths of one output part (15.9): a point on 2 or more paths, with segments in
 * 3 or more directions. A path that goes out to a route point and back is not a junction.
 */
export function junctions(paths: readonly (readonly Point[])[]): Point[] {
  if (paths.length < 2) return [];
  const segs = paths.flatMap((p, i) => p.slice(1).map((q, k): [Point, Point, number] => [p[k], q, i]));
  const near = (a: Point, b: Point) => Math.abs(a[0] - b[0]) < 0.5 && Math.abs(a[1] - b[1]) < 0.5;
  const dir = (a: Point, b: Point) => Math.round(Math.atan2(b[1] - a[1], b[0] - a[0]) / (Math.PI / 4)) & 7;
  const out: Point[] = [];
  for (const v of paths.flatMap((p) => p.slice(1, -1))) {
    if (out.some((o) => near(o, v))) continue;
    const dirs = new Set<number>();
    const on = new Set<number>();
    for (const [a, b, i] of segs) {
      if (near(a, b)) continue;
      if (near(v, a)) dirs.add(dir(a, b));
      else if (near(v, b)) dirs.add(dir(b, a));
      else if (onSegment(v, a, b)) dirs.add(dir(v, a)).add(dir(v, b));
      else continue;
      on.add(i);
    }
    if (dirs.size >= 3 && on.size >= 2) out.push(v);
  }
  return out;
}

/** An axis-aligned segment: the fixed coordinate, the low and the high end, its path, and the point index at each end. */
type Run = readonly [at: number, lo: number, hi: number, path: number, kLo: number, kHi: number];

/** The marks where the paths of two outputs cross, by path index. */
export interface Crossings {
  /** The gaps of the wires that go under. */
  gaps: Map<number, Gap[]>;
  /** The largest corner radius at each bend, for a path with a bend near a crossing. */
  radii: Map<number, number[]>;
}

/**
 * The crossings of the paths of two outputs (15.9). The narrower wire goes under and gets the gap.
 * Of two wires of one width, the vertical wire goes under. A corner near a crossing gets a smaller
 * radius, so both wires are straight over the gap. The order comes from the routes only, so a pan,
 * a zoom or a selection does not change it.
 */
export function crossings(paths: readonly (readonly Point[])[], nets: readonly string[], widths: readonly number[]): Crossings {
  const across: Run[] = [];
  const down: Run[] = [];
  paths.forEach((p, i) =>
    p.slice(1).forEach(([x1, y1], k) => {
      const [x0, y0] = p[k];
      if (Math.abs(y1 - y0) < 0.5 && Math.abs(x1 - x0) >= 1) across.push([y0, Math.min(x0, x1), Math.max(x0, x1), i, x0 < x1 ? k : k + 1, x0 < x1 ? k + 1 : k]);
      else if (Math.abs(x1 - x0) < 0.5 && Math.abs(y1 - y0) >= 1) down.push([x0, Math.min(y0, y1), Math.max(y0, y1), i, y0 < y1 ? k : k + 1, y0 < y1 ? k + 1 : k]);
    }),
  );
  const hits: [x: number, y: number, a: Run, d: Run][] = [];
  for (const a of across) {
    for (const d of down) {
      const [[y, x0, x1, i], [x, y0, y1, j]] = [a, d];
      if (nets[i] !== nets[j] && x > x0 && x < x1 && y > y0 && y < y1) hits.push([x, y, a, d]);
    }
  }
  const radii = new Map<number, number[]>();
  const limit = (i: number, k: number, r: number) => {
    const p = paths[i];
    if (k <= 0 || k >= p.length - 1 || r >= cornerRadius(widths[i])) return;
    let list = radii.get(i);
    if (!list) radii.set(i, (list = new Array<number>(p.length - 2).fill(Infinity)));
    list[k - 1] = Math.min(list[k - 1], Math.max(0, r));
  };
  for (const [x, y, a, d] of hits) {
    // The gap of the lower wire, and 1 px more, stays out of the rounded corners of both wires.
    const room = Math.max(widths[a[3]], widths[d[3]]) / 2 + CLEAR + 1;
    limit(a[3], a[4], x - a[1] - room);
    limit(a[3], a[5], a[2] - x - room);
    limit(d[3], d[4], y - d[1] - room);
    limit(d[3], d[5], d[2] - y - room);
  }
  // The wires of one output that share a bend get one corner, so a trunk does not show two arcs.
  const shared = new Map<string, number>();
  const key = (i: number, v: Point) => `${nets[i]} ${v[0]} ${v[1]}`;
  for (const [i, list] of radii) {
    list.forEach((r, k) => shared.set(key(i, paths[i][k + 1]), Math.min(shared.get(key(i, paths[i][k + 1])) ?? Infinity, r)));
  }
  if (shared.size) paths.forEach((p, i) => p.forEach((v, k) => limit(i, k, shared.get(key(i, v)) ?? Infinity)));
  const radius = (i: number, k: number) => Math.min(cornerRadius(widths[i]), radii.get(i)?.[k - 1] ?? Infinity);
  const gaps = new Map<number, Gap[]>();
  for (const [x, y, a, d] of hits) {
    const [i, j] = [a[3], d[3]];
    const under = widths[j] <= widths[i] ? j : i;
    // The upper wire must be straight at the crossing, away from its rounded corners and its pins. The
    // lower wire gets the gap only where its drawn path is straight (cutGap in wires.ts).
    if (under === j ? x - a[1] < radius(i, a[4]) || a[2] - x < radius(i, a[5]) : y - d[1] < radius(j, d[4]) || d[2] - y < radius(j, d[5])) continue;
    const list = gaps.get(under) ?? [];
    if (!list.some(([gx, gy]) => Math.abs(gx - x) < 0.5 && Math.abs(gy - y) < 0.5)) list.push([x, y, widths[under === j ? i : j] / 2 + CLEAR]);
    gaps.set(under, list);
  }
  return { gaps, radii };
}
