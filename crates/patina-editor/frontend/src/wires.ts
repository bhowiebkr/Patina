// The geometry of the connections (spec 15.9): one slot of points for each wire. The WebGL
// layer draws the slots, and each edge of React Flow keeps an invisible hit path on the same curve.

import type { Point } from './api';

export const MAX_POINTS = 64;
/** x, y, nx, ny for each point. (nx, ny) is the miter normal, so two segments touch at a joint. */
export const STRIDE = 4;
const BEZIER_SEGMENTS = 32;
/** The route points of one wire at most, as in the engine (spec 6.4). */
export const MAX_ROUTE = 12;
const CORNER_RADIUS = 12;
/** 2 + MAX_ROUTE x CORNER_POINTS fits in MAX_POINTS. */
const CORNER_POINTS = 5;
const NO_ROUTE: readonly Point[] = [];

/** The horizontal reach of the control points of the default curve. */
function reach(sx: number, tx: number): number {
  return Math.max(40, Math.abs(tx - sx) * 0.5);
}

/** Point k of the polyline from the source pin through the route points to the target pin. */
function vertex(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], k: number, axis: 0 | 1): number {
  if (k === 0) return axis === 0 ? sx : sy;
  if (k > route.length) return axis === 0 ? tx : ty;
  return route[k - 1][axis];
}

/** x0, y0, x1, y1: the ends of the last rounded corner. */
const cut = new Float64Array(4);

/** Sets `cut` for the corner at route point k. Returns false when the corner stays sharp. */
function corner(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], k: number): boolean {
  if (route.length > MAX_ROUTE) return false;
  const vx = route[k - 1][0];
  const vy = route[k - 1][1];
  const ix = vx - vertex(sx, sy, tx, ty, route, k - 1, 0);
  const iy = vy - vertex(sx, sy, tx, ty, route, k - 1, 1);
  const ox = vertex(sx, sy, tx, ty, route, k + 1, 0) - vx;
  const oy = vertex(sx, sy, tx, ty, route, k + 1, 1) - vy;
  const lin = Math.hypot(ix, iy);
  const lout = Math.hypot(ox, oy);
  const r = Math.min(CORNER_RADIUS, lin / 2, lout / 2);
  if (r < 0.5) return false;
  cut[0] = vx - (ix / lin) * r;
  cut[1] = vy - (iy / lin) * r;
  cut[2] = vx + (ox / lout) * r;
  cut[3] = vy + (oy / lout) * r;
  return true;
}

/** The SVG path from a source pin to a target pin: the default curve, or the rounded route. */
export function wirePath(sx: number, sy: number, tx: number, ty: number, route: readonly Point[] = NO_ROUTE): string {
  if (route.length === 0) {
    const dx = reach(sx, tx);
    return `M${sx} ${sy}C${sx + dx} ${sy} ${tx - dx} ${ty} ${tx} ${ty}`;
  }
  let d = `M${sx} ${sy}`;
  for (let k = 1; k <= route.length; k++) {
    const [vx, vy] = route[k - 1];
    d += corner(sx, sy, tx, ty, route, k) ? `L${cut[0]} ${cut[1]}Q${vx} ${vy} ${cut[2]} ${cut[3]}` : `L${vx} ${vy}`;
  }
  return `${d}L${tx} ${ty}`;
}

/** The place of the value badge (15.9): the middle of the default curve, or of the longest segment of the route. */
export function midpoint(sx: number, sy: number, tx: number, ty: number, route: readonly Point[]): Point {
  // At t = 0.5 the control points of the default curve cancel, so its middle is the middle of the ends.
  if (route.length === 0) return [(sx + tx) / 2, (sy + ty) / 2];
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
  const mid = (axis: 0 | 1) => (vertex(sx, sy, tx, ty, route, best, axis) + vertex(sx, sy, tx, ty, route, best + 1, axis)) / 2;
  return [mid(0), mid(1)];
}

/** The index in the route for a new point at (x, y): the place of the nearest segment. */
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

/** The points of the rounded route from `at`, with their normals. Returns the number of points. */
export function routePoints(sx: number, sy: number, tx: number, ty: number, route: readonly Point[], out: Float32Array, at: number): number {
  // shortcut: a route of more than MAX_POINTS - 2 points (only from a file edited by hand) draws its first points; split the row when this matters.
  const last = Math.min(route.length, MAX_POINTS - 2);
  let n = put(out, at, 0, sx, sy);
  for (let k = 1; k <= last; k++) {
    const [vx, vy] = route[k - 1];
    if (!corner(sx, sy, tx, ty, route, k)) {
      n = put(out, at, n, vx, vy);
      continue;
    }
    for (let j = 0; j < CORNER_POINTS; j++) {
      const t = j / (CORNER_POINTS - 1);
      const u = 1 - t;
      n = put(out, at, n, u * u * cut[0] + 2 * u * t * vx + t * t * cut[2], u * u * cut[1] + 2 * u * t * vy + t * t * cut[3]);
    }
  }
  n = put(out, at, n, tx, ty);
  normals(out, at, n);
  return n;
}

/** The points of the default curve from `at`, with their normals. Returns the number of points. */
export function bezierPoints(sx: number, sy: number, tx: number, ty: number, out: Float32Array, at: number): number {
  const dx = reach(sx, tx);
  for (let k = 0; k <= BEZIER_SEGMENTS; k++) {
    const t = k / BEZIER_SEGMENTS;
    const u = 1 - t;
    const b = 3 * u * u * t;
    const c = 3 * u * t * t;
    const d = t * t * t;
    out[at + k * STRIDE] = (u * u * u + b) * sx + c * tx + d * tx + (b - c) * dx;
    out[at + k * STRIDE + 1] = (u * u * u + b) * sy + (c + d) * ty;
  }
  normals(out, at, BEZIER_SEGMENTS + 1);
  return BEZIER_SEGMENTS + 1;
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

  set(id: string, sx: number, sy: number, tx: number, ty: number, route: readonly Point[], selected: boolean, bits = '', stale = false) {
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
    const n = route.length === 0 ? bezierPoints(sx, sy, tx, ty, this.points, at) : routePoints(sx, sy, tx, ty, route, this.points, at);
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
