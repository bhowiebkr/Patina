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

/** An axis-aligned segment: the fixed coordinate, the low and the high end, and the index of its path. */
type Run = readonly [at: number, lo: number, hi: number, path: number];

/**
 * The gaps where the paths of two outputs cross (15.9), by path index. The narrower wire goes under
 * and gets the gap. Of two wires of one width, the vertical wire goes under. The order comes from
 * the routes only, so a pan, a zoom or a selection does not change it.
 */
export function crossingGaps(paths: readonly (readonly Point[])[], nets: readonly string[], widths: readonly number[]): Map<number, Gap[]> {
  const across: Run[] = [];
  const down: Run[] = [];
  paths.forEach((p, i) =>
    p.slice(1).forEach(([x1, y1], k) => {
      const [x0, y0] = p[k];
      if (Math.abs(y1 - y0) < 0.5 && Math.abs(x1 - x0) >= 1) across.push([y0, Math.min(x0, x1), Math.max(x0, x1), i]);
      else if (Math.abs(x1 - x0) < 0.5 && Math.abs(y1 - y0) >= 1) down.push([x0, Math.min(y0, y1), Math.max(y0, y1), i]);
    }),
  );
  const out = new Map<number, Gap[]>();
  const add = (i: number, x: number, y: number, half: number) => {
    const list = out.get(i) ?? [];
    if (!list.some(([gx, gy]) => Math.abs(gx - x) < 0.5 && Math.abs(gy - y) < 0.5)) list.push([x, y, half]);
    out.set(i, list);
  };
  for (const [y, x0, x1, i] of across) {
    for (const [x, y0, y1, j] of down) {
      if (nets[i] === nets[j] || x <= x0 || x >= x1 || y <= y0 || y >= y1) continue;
      const under = widths[j] <= widths[i] ? j : i;
      // The upper wire must be straight at the crossing, away from its rounded corners. The lower
      // wire gets the gap only where its drawn path is straight (cutGap in wires.ts).
      const r = cornerRadius(widths[under === j ? i : j]);
      if (under === j ? x - x0 < r || x1 - x < r : y - y0 < r || y1 - y < r) continue;
      add(under, x, y, widths[under === j ? i : j] / 2 + CLEAR);
    }
  }
  return out;
}
