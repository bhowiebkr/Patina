// The shared geometry of the connections: the centerline points, the miter normals, the badge
// positions and the hit test. Both renderers read it.

import type { Graph } from './types';
import { BADGE_CHAR, BADGE_HEIGHT, BOOL_WIDTH, FAR_BUS_WIDTH } from './theme';

/** At this zoom and nearer, a bus shows each lane (15.9). */
export const NEAR_ZOOM = 0.75;
export const MAX_POINTS = 64;
const BEZIER_SEGMENTS = 32;
const CORNER_RADIUS = 12;
const CORNER_STEPS = 5;

export class Wires {
  readonly graph: Graph;
  /** x, y, nx, ny for each point, MAX_POINTS points for each edge. (nx, ny) is the miter normal. */
  readonly points: Float32Array;
  readonly counts: Uint8Array;
  /** min x, min y, max x, max y of each centerline. */
  readonly bounds: Float32Array;
  /** The badge anchor of each edge: the middle point of the centerline. */
  readonly mid: Float32Array;
  private readonly dirs = new Float32Array(MAX_POINTS * 2);

  constructor(graph: Graph) {
    const n = graph.edges.length;
    this.graph = graph;
    this.points = new Float32Array(n * MAX_POINTS * 4);
    this.counts = new Uint8Array(n);
    this.bounds = new Float32Array(n * 4);
    this.mid = new Float32Array(n * 2);
    for (let i = 0; i < n; i++) this.rebuild(i);
  }

  /** The width of the drawn band, in graph pixels. */
  band(i: number, near: boolean): number {
    const w = this.graph.edges[i].width;
    if (w === 1) return BOOL_WIDTH;
    return near ? w * this.graph.lanePitch : FAR_BUS_WIDTH;
  }

  /** The offset of lane j from the centerline. Lane 0 is the most significant bit, at the top. */
  laneOffset(i: number, j: number): number {
    const w = this.graph.edges[i].width;
    return (j - (w - 1) / 2) * this.graph.lanePitch;
  }

  /** The value of the lanes of edge i, the most significant bit first. */
  value(i: number, bits: Uint8Array): number {
    const e = this.graph.edges[i];
    let v = 0;
    for (let j = 0; j < e.width; j++) {
      const lane = e.lane0 + j;
      v = v * 2 + ((bits[lane >> 3] >> (lane & 7)) & 1);
    }
    return v;
  }

  /** The badge text: the value at a near zoom, the width of a bus at a far zoom (15.9). */
  badgeText(i: number, near: boolean, value: number): string {
    const w = this.graph.edges[i].width;
    if (!near) return String(w);
    if (w === 1) return String(value);
    return '0x' + value.toString(16).toUpperCase().padStart(Math.ceil(w / 4), '0');
  }

  /** The badge width in graph pixels, for a fixed-width font. */
  badgeWidth(i: number, near: boolean): number {
    const w = this.graph.edges[i].width;
    const chars = !near ? String(w).length : w === 1 ? 1 : 2 + Math.ceil(w / 4);
    return chars * BADGE_CHAR + 8;
  }

  /** The badge center: above the band, at the middle of the centerline. */
  badgeY(i: number, near: boolean): number {
    return this.mid[i * 2 + 1] - this.band(i, near) / 2 - BADGE_HEIGHT / 2 - 3;
  }

  moveRoutePoint(i: number, k: number, x: number, y: number) {
    this.graph.edges[i].route[k] = [x, y];
    this.rebuild(i);
  }

  rebuild(i: number) {
    const { nodes, edges } = this.graph;
    const e = edges[i];
    const sn = nodes[e.source];
    const tn = nodes[e.target];
    const sx = sn.x + sn.w;
    const sy = sn.y + sn.outputs[e.sourcePin].y;
    const tx = tn.x;
    const ty = tn.y + tn.inputs[e.targetPin].y;
    const p = this.points;
    const base = i * MAX_POINTS * 4;
    let n = 0;
    const put = (x: number, y: number) => {
      p[base + n * 4] = x;
      p[base + n * 4 + 1] = y;
      n++;
    };
    if (e.route.length === 0) {
      const dx = Math.max(40, Math.abs(tx - sx) * 0.5);
      for (let k = 0; k <= BEZIER_SEGMENTS; k++) {
        const t = k / BEZIER_SEGMENTS;
        const u = 1 - t;
        const a = u * u * u;
        const b = 3 * u * u * t;
        const c = 3 * u * t * t;
        const d = t * t * t;
        put(a * sx + b * (sx + dx) + c * (tx - dx) + d * tx, a * sy + b * sy + c * ty + d * ty);
      }
    } else {
      const pts: [number, number][] = [[sx, sy], ...e.route, [tx, ty]];
      put(sx, sy);
      for (let k = 1; k < pts.length - 1; k++) {
        const [ax, ay] = pts[k - 1];
        const [bx, by] = pts[k];
        const [cx, cy] = pts[k + 1];
        const lin = Math.hypot(bx - ax, by - ay);
        const lout = Math.hypot(cx - bx, cy - by);
        const r = Math.min(CORNER_RADIUS, lin / 2, lout / 2);
        const x0 = lin > 0 ? bx - ((bx - ax) / lin) * r : bx;
        const y0 = lin > 0 ? by - ((by - ay) / lin) * r : by;
        const x1 = lout > 0 ? bx + ((cx - bx) / lout) * r : bx;
        const y1 = lout > 0 ? by + ((cy - by) / lout) * r : by;
        for (let s = 0; s < CORNER_STEPS; s++) {
          const t = s / (CORNER_STEPS - 1);
          const u = 1 - t;
          put(u * u * x0 + 2 * u * t * bx + t * t * x1, u * u * y0 + 2 * u * t * by + t * t * y1);
        }
      }
      put(tx, ty);
    }
    this.counts[i] = n;
    this.computeNormals(base, n);
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (let k = 0; k < n; k++) {
      const x = p[base + k * 4];
      const y = p[base + k * 4 + 1];
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    }
    this.bounds.set([minX, minY, maxX, maxY], i * 4);
    const m = base + (n >> 1) * 4;
    this.mid[i * 2] = p[m];
    this.mid[i * 2 + 1] = p[m + 1];
  }

  /** Sets the miter normals. A zero-length segment takes the direction of its neighbor. */
  private computeNormals(base: number, n: number) {
    const p = this.points;
    const dirs = this.dirs;
    let valid = -1;
    for (let s = 0; s < n - 1; s++) {
      const dx = p[base + (s + 1) * 4] - p[base + s * 4];
      const dy = p[base + (s + 1) * 4 + 1] - p[base + s * 4 + 1];
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
      const nx0 = -dirs[din * 2 + 1];
      const ny0 = dirs[din * 2];
      const nx1 = -dirs[dout * 2 + 1];
      const ny1 = dirs[dout * 2];
      let mx = nx0 + nx1;
      let my = ny0 + ny1;
      const len = Math.hypot(mx, my);
      if (len < 1e-6) {
        mx = nx1;
        my = ny1;
      } else {
        mx /= len;
        my /= len;
      }
      const scale = 1 / Math.max(0.25, mx * nx1 + my * ny1);
      p[base + k * 4 + 2] = mx * scale;
      p[base + k * 4 + 3] = my * scale;
    }
  }

  /** The SVG path of the line at `offset` from the centerline. */
  path(i: number, offset: number): string {
    const p = this.points;
    const base = i * MAX_POINTS * 4;
    let d = '';
    for (let k = 0; k < this.counts[i]; k++) {
      const o = base + k * 4;
      const x = p[o] + p[o + 2] * offset;
      const y = p[o + 1] + p[o + 3] * offset;
      d += (k === 0 ? 'M' : 'L') + x.toFixed(1) + ' ' + y.toFixed(1);
    }
    return d;
  }

  /** The edge under the graph point (x, y), or -1. `tol` is in graph pixels. */
  hit(x: number, y: number, tol: number, near: boolean): number {
    const p = this.points;
    const bb = this.bounds;
    let best = -1;
    let bestD = Infinity;
    for (let i = 0; i < this.counts.length; i++) {
      const reach = this.band(i, near) / 2 + tol;
      const b = i * 4;
      if (x < bb[b] - reach || x > bb[b + 2] + reach || y < bb[b + 1] - reach || y > bb[b + 3] + reach) continue;
      const base = i * MAX_POINTS * 4;
      for (let k = 0; k < this.counts[i] - 1; k++) {
        const o = base + k * 4;
        const d = segmentDistance(x, y, p[o], p[o + 1], p[o + 4], p[o + 5]);
        if (d <= reach && d < bestD) {
          bestD = d;
          best = i;
        }
      }
    }
    return best;
  }
}

export function segmentDistance(x: number, y: number, ax: number, ay: number, bx: number, by: number) {
  const dx = bx - ax;
  const dy = by - ay;
  const len2 = dx * dx + dy * dy;
  const t = len2 > 0 ? Math.min(1, Math.max(0, ((x - ax) * dx + (y - ay) * dy) / len2)) : 0;
  return Math.hypot(x - ax - t * dx, y - ay - t * dy);
}
