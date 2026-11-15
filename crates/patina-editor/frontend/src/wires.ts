// The geometry of the connections (spec 15.9): one slot of points for each wire. The WebGL
// layer draws the slots, and each edge of React Flow keeps an invisible hit path on the same curve.

export const MAX_POINTS = 64;
/** x, y, nx, ny for each point. (nx, ny) is the miter normal, so two segments touch at a joint. */
export const STRIDE = 4;
const BEZIER_SEGMENTS = 32;

/** The horizontal reach of the control points of the default curve. */
function reach(sx: number, tx: number): number {
  return Math.max(40, Math.abs(tx - sx) * 0.5);
}

/** The SVG path of the default curve from a source pin to a target pin. */
export function wirePath(sx: number, sy: number, tx: number, ty: number): string {
  const dx = reach(sx, tx);
  return `M${sx} ${sy}C${sx + dx} ${sy} ${tx - dx} ${ty} ${tx} ${ty}`;
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

/** The wires that the WebGL layer draws. A free slot has 0 points. */
export class WireStore {
  points = new Float32Array(16 * MAX_POINTS * STRIDE);
  counts = new Uint8Array(16);
  selected = new Uint8Array(16);
  /** The slots that changed since the last draw. All slots changed when `grown` is true. */
  readonly changed = new Set<number>();
  grown = true;
  /** True when a wire was added, removed, selected or cleared since the last draw. */
  layout = true;
  private readonly slots = new Map<string, number>();
  private readonly free: number[] = [];
  private notify = () => {};

  /** Sets the function that runs after each change, for example a request to draw. */
  listen(fn: () => void) {
    this.notify = fn;
  }

  set(id: string, sx: number, sy: number, tx: number, ty: number, selected: boolean) {
    let slot = this.slots.get(id);
    if (slot === undefined) {
      slot = this.free.pop() ?? this.slots.size;
      if (slot >= this.counts.length) this.grow();
      this.slots.set(id, slot);
      this.layout = true;
    }
    const sel = selected ? 1 : 0;
    if (this.selected[slot] !== sel) this.layout = true;
    this.selected[slot] = sel;
    this.counts[slot] = bezierPoints(sx, sy, tx, ty, this.points, slot * MAX_POINTS * STRIDE);
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
    this.points = points;
    this.counts = counts;
    this.selected = selected;
    this.grown = true;
  }
}
