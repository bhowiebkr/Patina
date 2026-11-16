// The geometry of the connections (spec 15.9): one slot of points for each wire. The WebGL
// layer draws the slots, and each edge of React Flow keeps an invisible hit path on the same curve.

import type { Corners, Point, Style } from './api';

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

/**
 * The path of a wire from its style, its corners and its route points. With no style, a wire
 * with no points is curved, and a wire with points is a polyline. A wide ribbon gets a larger
 * corner, so its inner lane does not fold.
 */
export function shape(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], style?: Style, corners?: Corners, width = 0): Shape {
  if (style === 'curved' || (style === undefined && route.length === 0)) return { bends: route, radius: SMOOTH };
  const radius = corners === 'sharp' ? 0 : Math.max(CORNER_RADIUS, width / 2 + INNER_RADIUS);
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
    const [vx, vy] = bends[k - 1];
    d += corner(sx, sy, tx, ty, bends, k, radius) ? `L${cut[0]} ${cut[1]}Q${vx} ${vy} ${cut[2]} ${cut[3]}` : `L${vx} ${vy}`;
  }
  return `${d}L${tx} ${ty}`;
}

/** The place of the value badge (15.9): the middle of the longest segment of the path. */
export function midpoint(sx: number, sy: number, tx: number, ty: number, shape: Shape): Point {
  const route = shape.bends;
  // The middle of a segment is never on a route point, so the handles of a selected wire do not hide the badge.
  let best = 0;
  let longest = -1;
  for (let k = 0; k <= route.length; k++) {
    const len = Math.hypot(vertex(sx, sy, tx, ty, route, k + 1, 0) - vertex(sx, sy, tx, ty, route, k, 0), vertex(sx, sy, tx, ty, route, k + 1, 1) - vertex(sx, sy, tx, ty, route, k, 1));
    if (len > longest) {
      longest = len;
      best = k;
    }
  }
  if (shape.radius === SMOOTH) {
    smooth(sx, sy, tx, ty, route, best);
    return [(ctl[0] + 3 * ctl[2] + 3 * ctl[4] + ctl[6]) / 8, (ctl[1] + 3 * ctl[3] + 3 * ctl[5] + ctl[7]) / 8];
  }
  const mid = (axis: 0 | 1) => (vertex(sx, sy, tx, ty, route, best, axis) + vertex(sx, sy, tx, ty, route, best + 1, axis)) / 2;
  return [mid(0), mid(1)];
}

/** The index in the route for a new point at (x, y): the place of the nearest straight segment through the route points. */
export function insertAt(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], x: number, y: number): number {
  let best = 0;
  let bestDistance = Infinity;
  for (let k = 0; k <= route.length; k++) {
    const ax = vertex(sx, sy, tx, ty, route, k, 0);
    const ay = vertex(sx, sy, tx, ty, route, k, 1);
    const dx = vertex(sx, sy, tx, ty, route, k + 1, 0) - ax;
    const dy = vertex(sx, sy, tx, ty, route, k + 1, 1) - ay;
    const len2 = dx * dx + dy * dy;
    const t = len2 > 0 ? Math.min(1, Math.max(0, ((x - ax) * dx + (y - ay) * dy) / len2)) : 0;
    const distance = Math.hypot(ax + t * dx - x, ay + t * dy - y);
    if (distance < bestDistance) {
      bestDistance = distance;
      best = k;
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
    const [vx, vy] = bends[k - 1];
    if (steps < 2 || !corner(sx, sy, tx, ty, bends, k, radius)) {
      n = put(out, at, n, vx, vy);
      continue;
    }
    for (let j = 0; j < steps; j++) {
      const t = j / (steps - 1);
      const u = 1 - t;
      n = put(out, at, n, u * u * cut[0] + 2 * u * t * vx + t * t * cut[2], u * u * cut[1] + 2 * u * t * vy + t * t * cut[3]);
    }
  }
  n = put(out, at, n, tx, ty);
  normals(out, at, n);
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

/** The flags of a draw instance. Bits 0 to 5 hold the number of lanes - 1. */
export const HAS_LANES = 64;
export const LIT = 128;
export const STALE = 256;

/**
 * The draw instances of the wires: slot, segment, lane and flags. Each lane of each segment of
 * each wire, or one instance for each segment of the selected wires.
 */
export function instances(store: WireStore, selectedOnly: boolean): Uint16Array {
  const lanesOf = (s: number) => (selectedOnly ? 1 : Math.max(1, store.bits[s].length));
  let n = 0;
  for (let s = 0; s < store.counts.length; s++) if (!selectedOnly || store.selected[s]) n += Math.max(0, store.counts[s] - 1) * lanesOf(s);
  const out = new Uint16Array(n * 4);
  let at = 0;
  for (let s = 0; s < store.counts.length; s++) {
    if (selectedOnly && !store.selected[s]) continue;
    const bits = store.bits[s];
    const flags = (Math.max(1, bits.length) - 1) | (bits ? HAS_LANES : 0) | (store.stale[s] ? STALE : 0);
    for (let k = 0; k < store.counts[s] - 1; k++) {
      for (let lane = 0; lane < lanesOf(s); lane++, at += 4) {
        out[at] = s;
        out[at + 1] = k;
        out[at + 2] = lane;
        out[at + 3] = flags | (bits[lane] === '1' ? LIT : 0);
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

  set(id: string, sx: number, sy: number, tx: number, ty: number, shape: Shape, selected: boolean, bits = '', stale = false) {
    let slot = this.slots.get(id);
    if (slot === undefined) {
      slot = this.free.pop() ?? this.slots.size;
      if (slot >= this.counts.length) this.grow();
      this.slots.set(id, slot);
      this.layout = true;
    }
    const sel = selected ? 1 : 0;
    const old = stale ? 1 : 0;
    // shortcut: a new lane state rebuilds the instances of all wires; put the lane states in a texture when values come at the display rate (15.6).
    if (this.selected[slot] !== sel || this.bits[slot] !== bits || this.stale[slot] !== old) this.layout = true;
    this.selected[slot] = sel;
    this.bits[slot] = bits;
    this.stale[slot] = old;
    const at = slot * MAX_POINTS * STRIDE;
    const n = wirePoints(sx, sy, tx, ty, shape, this.points, at);
    // A new number of points needs new segment lists, or the old segments read stale points.
    if (this.counts[slot] !== n) this.layout = true;
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
    this.layout = true;
    this.notify();
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
    this.points = points;
    this.counts = counts;
    this.selected = selected;
    this.stale = stale;
    this.bits = this.bits.concat(Array<string>(n - this.bits.length).fill(''));
    this.grown = true;
  }
}
