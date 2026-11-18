// The routes of the wires of one graph view (spec 15.9): the full paths, the conflicts and the marks.

import type { Point } from '../api';
import { crossings, jointSize, junctions, type Dot, type Gap } from './marks.ts';
import { nudge } from './nudge.ts';
import { MARGIN, routeNets, type Leg, type Rect, type WireIn } from './route.ts';

/** The time after the last change, in ms, before all nets route again. */
const SETTLE = 250;

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

/** The full path of a routed wire, from pin to pin, and its width. */
export interface Line {
  points: readonly Point[];
  width: number;
  net: string;
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
  private allJoints: readonly Dot[] = [];
  private lines: ReadonlyMap<string, Line> = new Map();
  private cuts = new Map<string, readonly Gap[]>();
  private corners = new Map<string, readonly number[]>();
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

  /** The junction marks of all outputs, the same array until one moves (15.9). */
  allJunctions = (): readonly Dot[] => this.allJoints;

  /** The full paths of all routed wires, the same map until one moves (15.9). */
  allLines = (): ReadonlyMap<string, Line> => this.lines;

  /** The gaps of a wire where it goes under a wire of another output (15.9). */
  gaps = (id: string): readonly Gap[] | undefined => this.cuts.get(id);

  /** The largest corner radius at each bend of a wire with a bend near a crossing (15.9), or undefined. */
  radii = (id: string): readonly number[] | undefined => this.corners.get(id);

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
      const key = legs.map(({ id, w }) => `${id} ${w.sx} ${w.sy} ${w.tx} ${w.ty} ${w.width} ${w.via.join(' ')}`).join('\n');
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
    const linesMoved = ids.length !== this.lines.size || ids.some((id, i) => {
      const was = this.lines.get(id);
      return was === undefined || was.width !== this.wires.get(id)!.width || !samePoints(was.points, paths[i]);
    });
    if (linesMoved) this.lines = new Map(ids.map((id, i) => [id, { points: paths[i], width: this.wires.get(id)!.width, net: this.wires.get(id)!.net }]));
    const out = new Map<string, readonly Point[]>();
    const marks = new Map<string, Point>();
    let changed = linesMoved || ids.length !== this.out.size || this.conflicts.size !== this.marks.size;
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
    let jointsMoved = false;
    for (const list of byNet.values()) {
      const at = junctions(list.map((i) => paths[i]));
      if (!at.length) continue;
      const id = ids[list[0]];
      const was = this.joints.get(id);
      const same = was !== undefined && samePoints(was, at);
      joints.set(id, same ? was : at);
      jointsMoved ||= !same;
    }
    jointsMoved ||= joints.size !== this.joints.size;
    if (jointsMoved)
      this.allJoints = [...joints].flatMap(([id, at]) => {
        const r = jointSize(this.wires.get(id)!.width) / 2;
        return at.map(([x, y]) => [x, y, r] as const);
      });
    changed ||= jointsMoved;
    const ws = ids.map((id) => this.wires.get(id)!);
    const cuts = new Map<string, readonly Gap[]>();
    const { gaps, radii } = crossings(paths, ws.map((w) => w.net), ws.map((w) => w.width));
    for (const [i, at] of gaps) {
      const was = this.cuts.get(ids[i]);
      const same = was !== undefined && was.length === at.length && was.every((g, k) => g.every((v, n) => v === at[k][n]));
      cuts.set(ids[i], same ? was : at);
      changed ||= !same;
    }
    const corners = new Map<string, readonly number[]>();
    for (const [i, at] of radii) {
      const was = this.corners.get(ids[i]);
      const same = was !== undefined && was.length === at.length && was.every((r, k) => r === at[k]);
      corners.set(ids[i], same ? was : at);
      changed ||= !same;
    }
    changed ||= cuts.size !== this.cuts.size || corners.size !== this.corners.size;
    this.out = out;
    this.marks = marks;
    this.joints = joints;
    this.cuts = cuts;
    this.corners = corners;
    if (changed) for (const f of this.listeners) f();
  }
}
