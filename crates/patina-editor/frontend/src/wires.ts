// The geometry of the connections (spec 15.9): one slot of points for each wire. The WebGL
// layer draws the slots, and each edge of React Flow keeps an invisible hit path on the same curve.

import type { Corners, Point, Style } from './api';
import type { Gap } from './marks.ts';

export const MAX_POINTS = 64;
/** x, y, nx, ny for each point. (nx, ny) is the miter normal, so two segments touch at a joint. */
export const STRIDE = 4;
const BEZIER_SEGMENTS = 32;
/** The route points of one wire at most, as in the engine (spec 6.4). */
export const MAX_ROUTE = 12;
const CORNER_RADIUS = 12;
/** The space between the inner edge of a ribbon and the center of its corner. */
const INNER_RADIUS = 10;
/** The points of a corner at most. A path with many bends gets fewer points for each corner, so it fits in MAX_POINTS. */
const CORNER_POINTS = 12;
/** The horizontal length at both ends of an orthogonal leg that goes back. */
const STUB = 20;
const NO_ROUTE: readonly Point[] = [];

/** The path of a wire (spec 15.9): the bends between the pins, and the corner radius or SMOOTH. */
export interface Shape {
  bends: readonly Point[];
  radius: number;
}
/** The radius of a smooth curve through the bends. */
export const SMOOTH = -1;
const CURVE: Shape = { bends: NO_ROUTE, radius: SMOOTH };

/** The radius of the rounded corners of a wire of this width. */
export const cornerRadius = (width: number) => Math.max(CORNER_RADIUS, width / 2 + INNER_RADIUS);

/**
 * The path of a wire from its style, its corners and its route points. With no style, a wire
 * with no points is curved, and a wire with points is a polyline. A wide ribbon gets a larger
 * corner, so its inner lane does not fold.
 */
export function shape(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], style?: Style, corners?: Corners, width = 0): Shape {
  if (style === 'curved' || (style === undefined && route.length === 0)) return { bends: route, radius: SMOOTH };
  const radius = corners === 'sharp' ? 0 : cornerRadius(width);
  return { bends: style === 'orthogonal' ? orthogonal(sx, sy, tx, ty, route) : route, radius };
}

/**
 * The bends of an orthogonal path: horizontal at the pins, with an elbow before each route point
 * that is not in line. The last leg turns at the middle between the last point and the target.
 */
export function orthogonal(sx: number, sy: number, tx: number, ty: number, route: readonly Point[]): Point[] {
  const bends: Point[] = [];
  let [ax, ay] = [sx, sy];
  for (const [px, py] of route) {
    if (px !== ax && py !== ay) bends.push([px, ay]);
    bends.push([px, py]);
    [ax, ay] = [px, py];
  }
  if (ay === ty && tx >= ax) return bends;
  if (tx >= ax) {
    const mx = (ax + tx) / 2;
    bends.push([mx, ay], [mx, ty]);
  } else {
    const my = (ay + ty) / 2;
    bends.push([ax + STUB, ay], [ax + STUB, my], [tx - STUB, my], [tx - STUB, ty]);
  }
  return bends;
}

/** The pins and the corners of an orthogonal path. */
function legs(sx: number, sy: number, tx: number, ty: number, route: readonly Point[]): Point[] {
  return tidy([[sx, sy], ...orthogonal(sx, sy, tx, ty, route), [tx, ty]]);
}

/** The points with no empty segment and no point inside a straight run. The two ends stay. */
function tidy(points: readonly Point[]): Point[] {
  const v: Point[] = [points[0]];
  for (const p of points.slice(1)) {
    const [a, b] = [v[v.length - 1], v[v.length - 2]];
    if (p[0] === a[0] && p[1] === a[1]) continue;
    if (b && ((b[0] === a[0] && a[0] === p[0]) || (b[1] === a[1] && a[1] === p[1]))) v[v.length - 1] = p;
    else v.push(p);
  }
  return v;
}

/** A segment of an orthogonal path that can move sideways (15.9): the index of its first corner and its middle. */
export interface Segment {
  k: number;
  x: number;
  y: number;
  vertical: boolean;
}

/** The segments of an orthogonal path that can move sideways: all except the segments at the pins. */
export function segments(sx: number, sy: number, tx: number, ty: number, route: readonly Point[]): Segment[] {
  const v = legs(sx, sy, tx, ty, route);
  const out: Segment[] = [];
  for (let k = 1; k < v.length - 2; k++) {
    out.push({ k, x: (v[k][0] + v[k + 1][0]) / 2, y: (v[k][1] + v[k + 1][1]) / 2, vertical: v[k][0] === v[k + 1][0] });
  }
  return out;
}

/**
 * The route points that lock the bends of an automatic route (15.9): whole pixels, with no repeated
 * or in-line point. A route with more than MAX_ROUTE bends keeps MAX_ROUTE of them, spread along it.
 */
export function lockPoints(bends: readonly Point[]): Point[] {
  const out: Point[] = [];
  for (const [x, y] of bends) {
    const p: Point = [Math.round(x), Math.round(y)];
    const [a, b] = [out[out.length - 2], out[out.length - 1]];
    if (b && b[0] === p[0] && b[1] === p[1]) continue;
    if (a && b && ((a[0] === b[0] && b[0] === p[0]) || (a[1] === b[1] && b[1] === p[1]))) out.pop();
    out.push(p);
  }
  if (out.length <= MAX_ROUTE) return out;
  return Array.from({ length: MAX_ROUTE }, (_, i) => out[Math.round((i * (out.length - 1)) / (MAX_ROUTE - 1))]);
}

/**
 * The route after segment k of an orthogonal path moves sideways by d. The two segments next to
 * it stretch, so the path stays connected. The route keeps the corner at the end of each vertical
 * run, because orthogonal() adds the corner before it. A route that gets too long does not change.
 */
export function moveSegment(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], k: number, d: number): Point[] {
  let v = legs(sx, sy, tx, ty, route);
  if (k < 1 || k >= v.length - 2) return [...route];
  const axis = v[k][0] === v[k + 1][0] ? 0 : 1;
  for (const i of [k, k + 1]) v[i] = axis === 0 ? [v[i][0] + d, v[i][1]] : [v[i][0], v[i][1] + d];
  v = tidy(v);
  const flat = (i: number) => v[i][1] === v[i + 1][1];
  const out = v.slice(1, -1).filter((_, j) => j + 1 === v.length - 2 || !flat(j) || flat(j + 1));
  return out.length > MAX_ROUTE ? [...route] : out;
}

/** The horizontal reach of the control points of the default curve. */
function reach(sx: number, tx: number): number {
  return Math.max(40, Math.abs(tx - sx) * 0.5);
}

/** Point k of the polyline from the source pin through the bends to the target pin. */
function vertex(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], k: number, axis: 0 | 1): number {
  if (k === 0) return axis === 0 ? sx : sy;
  if (k > route.length) return axis === 0 ? tx : ty;
  return route[k - 1][axis];
}

/** x0, y0, x1, y1: the ends of the last rounded corner. */
const cut = new Float64Array(4);

/** Sets `cut` for the corner at bend k. Returns false when the corner stays sharp. */
function corner(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], k: number, radius: number): boolean {
  const vx = route[k - 1][0];
  const vy = route[k - 1][1];
  const ix = vx - vertex(sx, sy, tx, ty, route, k - 1, 0);
  const iy = vy - vertex(sx, sy, tx, ty, route, k - 1, 1);
  const ox = vertex(sx, sy, tx, ty, route, k + 1, 0) - vx;
  const oy = vertex(sx, sy, tx, ty, route, k + 1, 1) - vy;
  const lin = Math.hypot(ix, iy);
  const lout = Math.hypot(ox, oy);
  const r = Math.min(radius, lin / 2, lout / 2);
  if (r < 0.5) return false;
  cut[0] = vx - (ix / lin) * r;
  cut[1] = vy - (iy / lin) * r;
  cut[2] = vx + (ox / lout) * r;
  cut[3] = vy + (oy / lout) * r;
  return true;
}

/** A, C1, M, C2 and B (x, y each) of the last S-bend: two quadratic curves that join at M. */
const jog = new Float64Array(10);

/**
 * Sets `jog` when bends k and k + 1 make an S-bend with a leg too short for two corners of the
 * radius. Two arcs of one larger radius then join at the middle of that leg, so the inner lane
 * of a wide ribbon does not fold. At a leg of two radii, the arcs are the two corners.
 */
function sbend(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], k: number, radius: number): boolean {
  if (k >= route.length) return false;
  const [px, py] = route[k - 1];
  const [qx, qy] = route[k];
  const ix = px - vertex(sx, sy, tx, ty, route, k - 1, 0);
  const iy = py - vertex(sx, sy, tx, ty, route, k - 1, 1);
  const ox = vertex(sx, sy, tx, ty, route, k + 2, 0) - qx;
  const oy = vertex(sx, sy, tx, ty, route, k + 2, 1) - qy;
  const lin = Math.hypot(ix, iy);
  const lout = Math.hypot(ox, oy);
  const mid = Math.hypot(qx - px, qy - py);
  if (lin < 1e-6 || lout < 1e-6 || mid < 1e-6) return false;
  const ux = ix / lin;
  const uy = iy / lin;
  // Only an S: the leg out of the second bend goes the same way as the leg into the first.
  if ((ux * ox + uy * oy) / lout < 1 - 1e-6) return false;
  const h = Math.min(radius, lin / 2, lout / 2);
  if (h < 0.5 || mid >= 2 * h) return false;
  // M is the middle of the short leg. Each arc has tangents of length t, so C1 = A + t u and |C1 - M| = t.
  const mx = (qx - px) / 2;
  const my = (qy - py) / 2;
  const along = h + mx * ux + my * uy;
  if (along < 0.5) return false;
  const t = (along * along + mx * mx + my * my - (along - h) * (along - h)) / (2 * along);
  jog[0] = px - ux * h;
  jog[1] = py - uy * h;
  jog[2] = px + ux * (t - h);
  jog[3] = py + uy * (t - h);
  jog[4] = px + mx;
  jog[5] = py + my;
  jog[6] = qx + ux * (h - t);
  jog[7] = qy + uy * (h - t);
  jog[8] = qx + ux * h;
  jog[9] = qy + uy * h;
  return true;
}

/** P0, C1, C2 and P3 (x, y each) of the last smooth segment. */
const ctl = new Float64Array(8);

/** The control offset of the smooth curve at bend k: horizontal at the pins, as the default curve, else a Catmull-Rom tangent. */
function handle(sx: number, sy: number, tx: number, ty: number, bends: readonly Point[], k: number, axis: 0 | 1): number {
  const last = bends.length + 1;
  if (k === 0 || k === last) return axis === 1 ? 0 : reach(vertex(sx, sy, tx, ty, bends, k === 0 ? 0 : last - 1, 0), vertex(sx, sy, tx, ty, bends, k === 0 ? 1 : last, 0));
  return (vertex(sx, sy, tx, ty, bends, k + 1, axis) - vertex(sx, sy, tx, ty, bends, k - 1, axis)) / 6;
}

const AXES = [0, 1] as const;
/** Sets `ctl` for the cubic segment k of the smooth curve through the bends. */
function smooth(sx: number, sy: number, tx: number, ty: number, bends: readonly Point[], k: number) {
  for (const axis of AXES) {
    const a = vertex(sx, sy, tx, ty, bends, k, axis);
    const b = vertex(sx, sy, tx, ty, bends, k + 1, axis);
    ctl[axis] = a;
    ctl[2 + axis] = a + handle(sx, sy, tx, ty, bends, k, axis);
    ctl[4 + axis] = b - handle(sx, sy, tx, ty, bends, k + 1, axis);
    ctl[6 + axis] = b;
  }
}

/** The SVG path from a source pin to a target pin. */
export function wirePath(sx: number, sy: number, tx: number, ty: number, shape: Shape = CURVE): string {
  const { bends, radius } = shape;
  let d = `M${sx} ${sy}`;
  if (radius === SMOOTH) {
    for (let k = 0; k <= bends.length; k++) {
      smooth(sx, sy, tx, ty, bends, k);
      d += `C${ctl[2]} ${ctl[3]} ${ctl[4]} ${ctl[5]} ${ctl[6]} ${ctl[7]}`;
    }
    return d;
  }
  for (let k = 1; k <= bends.length; k++) {
    if (sbend(sx, sy, tx, ty, bends, k, radius)) {
      d += `L${jog[0]} ${jog[1]}Q${jog[2]} ${jog[3]} ${jog[4]} ${jog[5]}Q${jog[6]} ${jog[7]} ${jog[8]} ${jog[9]}`;
      k++;
      continue;
    }
    const [vx, vy] = bends[k - 1];
    d += corner(sx, sy, tx, ty, bends, k, radius) ? `L${cut[0]} ${cut[1]}Q${vx} ${vy} ${cut[2]} ${cut[3]}` : `L${vx} ${vy}`;
  }
  return `${d}L${tx} ${ty}`;
}

/**
 * The place of the value badge and the label (15.9), and the width at most of the label above it.
 * The place is the middle of the curve, or of the longest horizontal segment of a route. A route
 * with no horizontal segment uses its longest segment. The label does not go over the nodes at
 * the two ends or over a corner that turns up into it.
 */
export function anchor(sx: number, sy: number, tx: number, ty: number, shape: Shape): [number, number, number] {
  const route = shape.bends;
  const at = (k: number, axis: 0 | 1) => vertex(sx, sy, tx, ty, route, k, axis);
  const curve = shape.radius === SMOOTH;
  // The middle of a segment is never on a route point, so the handles of a selected wire do not hide the badge.
  let best = 0;
  let longest = -1;
  let flat = false;
  for (let k = 0; k <= route.length; k++) {
    const dy = at(k + 1, 1) - at(k, 1);
    const len = Math.hypot(at(k + 1, 0) - at(k, 0), dy);
    const level = !curve && Math.abs(dy) < 1;
    if ((level && !flat) || (level === flat && len > longest)) [best, longest, flat] = [k, len, level];
  }
  let x = (at(best, 0) + at(best + 1, 0)) / 2;
  let y = (at(best, 1) + at(best + 1, 1)) / 2;
  if (curve) {
    smooth(sx, sy, tx, ty, route, best);
    [x, y] = [(ctl[0] + 3 * ctl[2] + 3 * ctl[4] + ctl[6]) / 8, (ctl[1] + 3 * ctl[3] + 3 * ctl[5] + ctl[7]) / 8];
  }
  if (!flat) return [x, y, Math.min(LABEL_WIDTH, tx > sx ? 2 * Math.min(x - sx, tx - x) - 8 : LABEL_WIDTH)];
  const up = (k: number) => k >= 0 && k <= route.length + 1 && at(k, 1) < y - 0.5;
  const inset = Math.max(best > 0 && up(best - 1) ? shape.radius : 0, best < route.length && up(best + 2) ? shape.radius : 0);
  return [x, y, Math.min(LABEL_WIDTH, longest - 2 * inset - 8)];
}
const LABEL_WIDTH = 160;

/** The distance from (x, y) to the segment from (ax, ay) to (bx, by). */
function segmentDistance(ax: number, ay: number, bx: number, by: number, x: number, y: number): number {
  const dx = bx - ax;
  const dy = by - ay;
  const len2 = dx * dx + dy * dy;
  const t = len2 > 0 ? Math.min(1, Math.max(0, ((x - ax) * dx + (y - ay) * dy) / len2)) : 0;
  return Math.hypot(ax + t * dx - x, ay + t * dy - y);
}

/**
 * The index in the route for a new point at (x, y), from the nearest part of the drawn path in
 * this style (15.9). For an orthogonal path, it is the number of route points before the nearest
 * leg, so the new point does not change the legs around it.
 */
export function insertAt(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], x: number, y: number, style?: Style): number {
  let best = 0;
  let bestDistance = Infinity;
  const near = (k: number, ax: number, ay: number, bx: number, by: number) => {
    const distance = segmentDistance(ax, ay, bx, by, x, y);
    if (distance < bestDistance) [best, bestDistance] = [k, distance];
  };
  if (style === 'orthogonal') {
    const bends = orthogonal(sx, sy, tx, ty, route);
    let before = 0;
    for (let j = 0; j <= bends.length; j++) {
      const b = bends[j - 1];
      if (b && before < route.length && b[0] === route[before][0] && b[1] === route[before][1]) before++;
      near(before, vertex(sx, sy, tx, ty, bends, j, 0), vertex(sx, sy, tx, ty, bends, j, 1), vertex(sx, sy, tx, ty, bends, j + 1, 0), vertex(sx, sy, tx, ty, bends, j + 1, 1));
    }
    return best;
  }
  for (let k = 0; k <= route.length; k++) {
    if (style !== 'curved') {
      near(k, vertex(sx, sy, tx, ty, route, k, 0), vertex(sx, sy, tx, ty, route, k, 1), vertex(sx, sy, tx, ty, route, k + 1, 0), vertex(sx, sy, tx, ty, route, k + 1, 1));
      continue;
    }
    smooth(sx, sy, tx, ty, route, k);
    let [px, py] = [ctl[0], ctl[1]];
    for (let j = 1; j <= BEZIER_SEGMENTS; j++) {
      const t = j / BEZIER_SEGMENTS;
      const u = 1 - t;
      const qx = u * u * u * ctl[0] + 3 * u * u * t * ctl[2] + 3 * u * t * t * ctl[4] + t * t * t * ctl[6];
      const qy = u * u * u * ctl[1] + 3 * u * u * t * ctl[3] + 3 * u * t * t * ctl[5] + t * t * t * ctl[7];
      near(k, px, py, qx, qy);
      [px, py] = [qx, qy];
    }
  }
  return best;
}

function put(out: Float32Array, at: number, n: number, x: number, y: number): number {
  out[at + n * STRIDE] = x;
  out[at + n * STRIDE + 1] = y;
  return n + 1;
}

/** The points of the path from `at`, with their normals. Returns the number of points. */
export function wirePoints(sx: number, sy: number, tx: number, ty: number, shape: Shape, out: Float32Array, at: number): number {
  const { bends, radius } = shape;
  // shortcut: a path of more than MAX_POINTS - 2 bends (only from a file edited by hand) draws its first bends; split the row when this matters.
  const last = Math.min(bends.length, MAX_POINTS - 2);
  let n = put(out, at, 0, sx, sy);
  if (radius === SMOOTH) {
    const steps = Math.min(BEZIER_SEGMENTS, Math.floor((MAX_POINTS - 1) / (last + 1)));
    for (let k = 0; k <= last; k++) {
      smooth(sx, sy, tx, ty, bends, k);
      for (let j = 1; j <= steps; j++) {
        const t = j / steps;
        const u = 1 - t;
        const a = u * u * u;
        const b = 3 * u * u * t;
        const c = 3 * u * t * t;
        const d = t * t * t;
        n = put(out, at, n, a * ctl[0] + b * ctl[2] + c * ctl[4] + d * ctl[6], a * ctl[1] + b * ctl[3] + c * ctl[5] + d * ctl[7]);
      }
    }
    normals(out, at, n);
    return n;
  }
  const steps = Math.min(CORNER_POINTS, Math.floor((MAX_POINTS - 2) / Math.max(1, last)));
  for (let k = 1; k <= last; k++) {
    if (steps >= 2 && k < last && sbend(sx, sy, tx, ty, bends, k, radius)) {
      n = quad(out, at, n, steps, 0, jog[0], jog[1], jog[2], jog[3], jog[4], jog[5]);
      n = quad(out, at, n, steps, 1, jog[4], jog[5], jog[6], jog[7], jog[8], jog[9]);
      k++;
      continue;
    }
    const [vx, vy] = bends[k - 1];
    if (steps < 2 || !corner(sx, sy, tx, ty, bends, k, radius)) {
      n = put(out, at, n, vx, vy);
      continue;
    }
    n = quad(out, at, n, steps, 0, cut[0], cut[1], vx, vy, cut[2], cut[3]);
  }
  n = put(out, at, n, tx, ty);
  normals(out, at, n);
  return n;
}

/** Puts the points `from` to `steps - 1` of the quadratic curve from (x0, y0) by (cx, cy) to (x1, y1). */
function quad(out: Float32Array, at: number, n: number, steps: number, from: number, x0: number, y0: number, cx: number, cy: number, x1: number, y1: number): number {
  for (let j = from; j < steps; j++) {
    const t = j / (steps - 1);
    const u = 1 - t;
    n = put(out, at, n, u * u * x0 + 2 * u * t * cx + t * t * x1, u * u * y0 + 2 * u * t * cy + t * t * y1);
  }
  return n;
}

const dirs = new Float32Array(MAX_POINTS * 2);

/** Sets the miter normals of n points. A segment of zero length takes the direction of a neighbor. */
export function normals(p: Float32Array, at: number, n: number) {
  let valid = -1;
  for (let s = 0; s < n - 1; s++) {
    const dx = p[at + (s + 1) * STRIDE] - p[at + s * STRIDE];
    const dy = p[at + (s + 1) * STRIDE + 1] - p[at + s * STRIDE + 1];
    const len = Math.hypot(dx, dy);
    if (len > 1e-6) {
      dirs[s * 2] = dx / len;
      dirs[s * 2 + 1] = dy / len;
      if (valid < 0) for (let b = 0; b < s; b++) dirs.copyWithin(b * 2, s * 2, s * 2 + 2);
      valid = s;
    } else if (valid >= 0) {
      dirs.copyWithin(s * 2, valid * 2, valid * 2 + 2);
    }
  }
  if (valid < 0) for (let s = 0; s < n - 1; s++) dirs.set([1, 0], s * 2);
  for (let k = 0; k < n; k++) {
    const din = Math.max(k - 1, 0);
    const dout = Math.min(k, n - 2);
    const nx1 = -dirs[dout * 2 + 1];
    const ny1 = dirs[dout * 2];
    let mx = -dirs[din * 2 + 1] + nx1;
    let my = dirs[din * 2] + ny1;
    const len = Math.hypot(mx, my);
    if (len < 1e-6) {
      mx = nx1;
      my = ny1;
    } else {
      mx /= len;
      my /= len;
    }
    // The miter is 4 times the half width at most, at a sharp bend.
    const scale = 1 / Math.max(0.25, mx * nx1 + my * ny1);
    p[at + k * STRIDE + 2] = mx * scale;
    p[at + k * STRIDE + 3] = my * scale;
  }
}

/**
 * Puts a gap of 2 x `half` around (x, y) into a straight axis segment of the points, and marks the
 * gap segment in `holes`. A gap that is not fully in one segment, or not in MAX_POINTS, is left out.
 * Returns the number of points.
 */
export function cutGap(p: Float32Array, at: number, holes: Uint8Array, n: number, x: number, y: number, half: number): number {
  if (n + 2 > MAX_POINTS) return n;
  for (let k = 0; k < n - 1; k++) {
    if (holes[k]) continue;
    const ax = p[at + k * STRIDE];
    const ay = p[at + k * STRIDE + 1];
    const dx = p[at + (k + 1) * STRIDE] - ax;
    const dy = p[at + (k + 1) * STRIDE + 1] - ay;
    const len = Math.hypot(dx, dy);
    if ((Math.abs(dx) >= 0.5 && Math.abs(dy) >= 0.5) || len < 1) continue;
    const [ux, uy] = [dx / len, dy / len];
    const t = (x - ax) * ux + (y - ay) * uy;
    if (Math.abs((x - ax) * uy - (y - ay) * ux) >= 0.5 || t - half <= 0 || t + half >= len) continue;
    p.copyWithin(at + (k + 3) * STRIDE, at + (k + 1) * STRIDE, at + n * STRIDE);
    holes.copyWithin(k + 3, k + 1, n - 1);
    put(p, at, k + 1, ax + ux * (t - half), ay + uy * (t - half));
    put(p, at, k + 2, ax + ux * (t + half), ay + uy * (t + half));
    holes[k + 1] = 1;
    holes[k + 2] = 0;
    return n + 2;
  }
  return n;
}

const NO_GAPS: readonly Gap[] = [];
const cuts = new Uint8Array(MAX_POINTS);

/** The flags of a draw instance. Bits 0 to 5 hold the number of lanes - 1, and bits 10 to 12 the color family (15.9). */
export const HAS_LANES = 64;
export const LIT = 128;
export const STALE = 256;
/** The lane under the pointer. */
export const HOVER = 512;
export const FAMILY_SHIFT = 10;
/** A wire between two clock domains is dashed (15.3). */
export const DASHED = 8192;

/**
 * The draw instances of the wires: slot, segment, lane and flags. Each lane of each segment of
 * each wire, or one instance for each segment of the selected wires.
 */
export function instances(store: WireStore, selectedOnly: boolean): Uint16Array {
  const lanesOf = (s: number) => (selectedOnly ? 1 : Math.max(1, store.bits[s].length));
  let n = 0;
  const drawn = (s: number, k: number) => !store.holes[s * MAX_POINTS + k];
  for (let s = 0; s < store.counts.length; s++) if (!selectedOnly || store.selected[s]) for (let k = 0; k < store.counts[s] - 1; k++) if (drawn(s, k)) n += lanesOf(s);
  const out = new Uint16Array(n * 4);
  let at = 0;
  for (let s = 0; s < store.counts.length; s++) {
    if (selectedOnly && !store.selected[s]) continue;
    const bits = store.bits[s];
    const flags = (Math.max(1, bits.length) - 1) | (bits ? HAS_LANES : 0) | (store.stale[s] ? STALE : 0) | (store.family[s] << FAMILY_SHIFT) | (store.dashed[s] ? DASHED : 0);
    for (let k = 0; k < store.counts[s] - 1; k++) {
      if (!drawn(s, k)) continue;
      for (let lane = 0; lane < lanesOf(s); lane++, at += 4) {
        out[at] = s;
        out[at + 1] = k;
        out[at + 2] = lane;
        out[at + 3] = flags | (bits[lane] === '1' ? LIT : 0) | (store.hover[s] === lane ? HOVER : 0);
      }
    }
  }
  return out;
}

/** The wires that the WebGL layer draws. A free slot has 0 points. */
export class WireStore {
  points = new Float32Array(16 * MAX_POINTS * STRIDE);
  counts = new Uint8Array(16);
  selected = new Uint8Array(16);
  /** The lanes of each wire (6.3), the most significant first, or "" for one plain line. */
  bits: string[] = Array<string>(16).fill('');
  stale = new Uint8Array(16);
  /** 1 for a wire between two clock domains (15.3). */
  dashed = new Uint8Array(16);
  /** The color family of each wire: 0 for neutral, else 1 to 5 (15.9). */
  family = new Uint8Array(16);
  /** The lane under the pointer in each wire, or -1. */
  hover = new Int8Array(16).fill(-1);
  /** 1 for each segment that is not drawn: a gap where the wire goes under a wire of another output (15.9). */
  holes = new Uint8Array(16 * MAX_POINTS);
  /** The slots that changed since the last draw. All slots changed when `grown` is true. */
  readonly changed = new Set<number>();
  grown = true;
  /** True when a wire was added, removed, selected, cleared, or got new lanes or a new number of points since the last draw. */
  layout = true;
  private readonly slots = new Map<string, number>();
  private readonly free: number[] = [];
  private notify = () => {};

  /** Sets the function that runs after each change, for example a request to draw. */
  listen(fn: () => void) {
    this.notify = fn;
  }

  set(id: string, sx: number, sy: number, tx: number, ty: number, shape: Shape, selected: boolean, bits = '', stale = false, family = 0, hover = -1, dashed = false, gaps: readonly Gap[] = NO_GAPS) {
    let slot = this.slots.get(id);
    if (slot === undefined) {
      slot = this.free.pop() ?? this.slots.size;
      if (slot >= this.counts.length) this.grow();
      this.slots.set(id, slot);
      this.layout = true;
    }
    const sel = selected ? 1 : 0;
    const old = stale ? 1 : 0;
    const dash = dashed ? 1 : 0;
    // shortcut: a new lane state rebuilds the instances of all wires; put the lane states in a texture when values come at the display rate (15.6).
    if (this.selected[slot] !== sel || this.bits[slot] !== bits || this.stale[slot] !== old || this.family[slot] !== family || this.hover[slot] !== hover || this.dashed[slot] !== dash) this.layout = true;
    this.selected[slot] = sel;
    this.bits[slot] = bits;
    this.stale[slot] = old;
    this.dashed[slot] = dash;
    this.family[slot] = family;
    this.hover[slot] = hover;
    const at = slot * MAX_POINTS * STRIDE;
    let n = wirePoints(sx, sy, tx, ty, shape, this.points, at);
    cuts.fill(0);
    for (const [x, y, half] of gaps) n = cutGap(this.points, at, cuts, n, x, y, half);
    if (gaps.length) normals(this.points, at, n);
    // A new number of points or new gaps need new segment lists, or the old segments read stale points.
    if (this.counts[slot] !== n) this.layout = true;
    for (let k = 0, base = slot * MAX_POINTS; k < MAX_POINTS; k++) {
      if (this.holes[base + k] !== cuts[k]) this.layout = true;
      this.holes[base + k] = cuts[k];
    }
    this.counts[slot] = n;
    this.changed.add(slot);
    this.notify();
  }

  remove(id: string) {
    const slot = this.slots.get(id);
    if (slot === undefined) return;
    this.slots.delete(id);
    this.free.push(slot);
    this.counts[slot] = 0;
    this.selected[slot] = 0;
    this.bits[slot] = '';
    this.stale[slot] = 0;
    this.dashed[slot] = 0;
    this.family[slot] = 0;
    this.hover[slot] = -1;
    this.layout = true;
    this.notify();
  }

  /**
   * The signed distance from (x, y) to the nearest segment of a wire, along the normal of the
   * segment: negative on the left side of the direction of the wire, where lane 0 is. Null for an unknown wire.
   */
  across(id: string, x: number, y: number): number | null {
    const slot = this.slots.get(id);
    if (slot === undefined || this.counts[slot] < 2) return null;
    const p = this.points;
    const at = slot * MAX_POINTS * STRIDE;
    let best = Infinity;
    let signed = 0;
    for (let k = 0; k < this.counts[slot] - 1; k++) {
      const ax = p[at + k * STRIDE];
      const ay = p[at + k * STRIDE + 1];
      const dx = p[at + (k + 1) * STRIDE] - ax;
      const dy = p[at + (k + 1) * STRIDE + 1] - ay;
      const len2 = dx * dx + dy * dy;
      if (len2 < 1e-9) continue;
      const t = Math.min(1, Math.max(0, ((x - ax) * dx + (y - ay) * dy) / len2));
      const distance = Math.hypot(ax + t * dx - x, ay + t * dy - y);
      if (distance < best) {
        best = distance;
        // The normal (-dy, dx) of normals(): (x - a) . n / |d|.
        signed = ((x - ax) * -dy + (y - ay) * dx) / Math.sqrt(len2);
      }
    }
    return signed;
  }

  get size(): number {
    return this.slots.size;
  }

  private grow() {
    const n = this.counts.length * 2;
    const points = new Float32Array(n * MAX_POINTS * STRIDE);
    points.set(this.points);
    const counts = new Uint8Array(n);
    counts.set(this.counts);
    const selected = new Uint8Array(n);
    selected.set(this.selected);
    const stale = new Uint8Array(n);
    stale.set(this.stale);
    const dashed = new Uint8Array(n);
    dashed.set(this.dashed);
    const family = new Uint8Array(n);
    family.set(this.family);
    const hover = new Int8Array(n).fill(-1);
    hover.set(this.hover);
    const holes = new Uint8Array(n * MAX_POINTS);
    holes.set(this.holes);
    this.holes = holes;
    this.points = points;
    this.counts = counts;
    this.selected = selected;
    this.stale = stale;
    this.dashed = dashed;
    this.family = family;
    this.hover = hover;
    this.bits = this.bits.concat(Array<string>(n - this.bits.length).fill(''));
    this.grown = true;
  }
}
