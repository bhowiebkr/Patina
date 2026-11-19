// The nodes of a subgraph use in place, in a neutral frame in the graph that uses it (spec 15.3).

import type { Corners, Edit, GraphFile, NodeEntry, Path, Point } from '../api';
import { pathOf } from '../wires/wires.ts';
import { GROUP_MIN } from './groups.ts';

/** The space of the frame at the left of the inner nodes, for the input names. */
export const FRAME_LEFT = 90;
/** The space of the frame at the right of the inner nodes, for the output names. */
export const FRAME_RIGHT = 90;
/** The space of the frame below the inner nodes. */
export const FRAME_BOTTOM = 24;
/** The top of the inner nodes in the frame, below the header. */
export const FRAME_TOP = 56;

/** A wire between two inner nodes, with the use prefix and its route points from the top left corner of the use. */
export interface InnerWire {
  from: string;
  to: string;
  points: Point[];
  style?: Path;
  corners?: Corners;
  name?: string;
}

/** The inner nodes and wires of one use. The IDs have the prefix `<use ID>/`, as in the run (6.4). */
export interface Frame {
  use: string;
  /** The inner nodes, at their position in the frame. */
  nodes: { entry: NodeEntry; x: number; y: number }[];
  wires: InnerWire[];
  /** The inner end of each wire from a public input, by the key of the input. */
  inputs: { key: string; to: string }[];
  /** The inner start of each public output, by its key. */
  outputs: { key: string; from: string }[];
}

/** The frame of the use `use`, from the file of its subgraph. */
export function frameOf(use: string, file: GraphFile): Frame {
  const entries = file.node ?? [];
  const prefix = (id: string) => `${use}/${id}`;
  const least = (axis: 0 | 1) => (entries.length === 0 ? 0 : Math.min(...entries.map((n) => n.pos?.[axis] ?? 0)));
  const [dx, dy] = [FRAME_LEFT - least(0), FRAME_TOP - least(1)];
  return {
    use,
    nodes: entries.map((n) => ({ entry: { ...n, id: prefix(n.id) }, x: (n.pos?.[0] ?? 0) + dx, y: (n.pos?.[1] ?? 0) + dy })),
    wires: (file.wire ?? []).map((w) => ({
      from: prefix(w.from),
      to: prefix(w.to),
      points: (w.points ?? []).map(([x, y]): Point => [x + dx, y + dy]),
      style: pathOf(w.style, file),
      corners: w.corners,
      name: w.name,
    })),
    inputs: (file.input ?? []).flatMap((p) => (p.to ?? []).map((to) => ({ key: p.key, to: prefix(to) }))),
    outputs: (file.output ?? []).flatMap((p) => (p.from ? [{ key: p.key, from: prefix(p.from) }] : [])),
  };
}

/** The use of an inner wire, and the view place of the use when the wire got its route points. */
export interface FrameAt {
  use: string;
  at: Point;
}

/** `route` moved by the drag of its use from `frame.at` to `now`, so the route points of an inner wire move with the frame (15.3). */
export function dragRoute(route: readonly Point[], frame: FrameAt | undefined, now: { x: number; y: number } | undefined): readonly Point[] {
  if (!frame || !now) return route;
  const [dx, dy] = [now.x - frame.at[0], now.y - frame.at[1]];
  return dx === 0 && dy === 0 ? route : route.map(([x, y]): Point => [x + dx, y + dy]);
}

/** The uses that show their inner nodes, by graph name. They stay while the editor runs. */
const shown = new Map<string, ReadonlySet<string>>();

export function shownFrames(graph: string): ReadonlySet<string> {
  return shown.get(graph) ?? new Set();
}

/** Shows or hides the inner nodes of the use `use` of the graph `graph`. */
export function toggleFrame(graph: string, use: string): ReadonlySet<string> {
  const next = new Set(shownFrames(graph));
  if (!next.delete(use)) next.add(use);
  shown.set(graph, next);
  return next;
}

/** The room of an open frame in the view: the use at `x`, `y` with the closed size `w` x `h` grows by `dx` and `dy`. */
export interface Room {
  use: string;
  x: number;
  y: number;
  w: number;
  h: number;
  dx: number;
  dy: number;
}

/** A span of one axis: it starts at `start`, is `len` long and grows by `extra`. */
type Span = readonly [start: number, len: number, extra: number];

function spans(rooms: readonly Room[], axis: 0 | 1): Span[] {
  return rooms.map((r) => (axis === 0 ? [r.x, Math.max(r.w, 1), Math.max(r.dx, 0)] : [r.y, Math.max(r.h, 1), Math.max(r.dy, 0)]));
}

// The growth is linear across the span, so the map is continuous, has a slope of 1 or more and has an inverse.
function forward(a: number, list: readonly Span[]): number {
  return list.reduce((v, [s, l, e]) => v + (a <= s ? 0 : a >= s + l ? e : ((a - s) * e) / l), a);
}

function inverse(v: number, list: readonly Span[]): number {
  const marks = [...new Set(list.flatMap(([s, l]) => [s, s + l]))].sort((a, b) => a - b);
  // Nothing grows before the first start.
  if (marks.length === 0 || v <= marks[0]) return v;
  let [a0, v0] = [marks[0], marks[0]];
  for (const a1 of marks.slice(1)) {
    const v1 = forward(a1, list);
    if (v <= v1) return a0 + ((v - v0) * (a1 - a0)) / (v1 - v0);
    [a0, v0] = [a1, v1];
  }
  return a0 + v - v0;
}

/**
 * The view point of a graph point. An open frame moves the nodes at its right and below it, and
 * keeps their order, so it covers no node that its closed use did not cover (15.3).
 */
export function place(rooms: readonly Room[], p: Point): Point {
  return rooms.length === 0 ? p : [forward(p[0], spans(rooms, 0)), forward(p[1], spans(rooms, 1))];
}

/** The graph point of a view point: the inverse of `place`. */
export function unplace(rooms: readonly Room[], p: Point): Point {
  return rooms.length === 0 ? p : [inverse(p[0], spans(rooms, 0)), inverse(p[1], spans(rooms, 1))];
}

/** An edit of the view, in graph coordinates. A moved use takes its new place first, so the other points of the edit agree with its frame. */
export function graphEdit(edit: Edit, rooms: readonly Room[]): Edit {
  if (rooms.length === 0) return edit;
  const moves = new Map<string, Point>();
  const collect = (e: Edit) => {
    if (e.kind === 'Group') e.edits.forEach(collect);
    else if (e.kind === 'MoveNode') moves.set(e.id, e.pos);
  };
  collect(edit);
  const round = ([x, y]: Point): Point => [Math.round(x), Math.round(y)];
  // A use does not grow its own place. A room grows only the points after its start, so the moved
  // uses take their places in the order of the view on each axis, and each sees the earlier ones.
  const solve = (axis: 0 | 1) => {
    const known = rooms.filter((r) => !moves.has(r.use));
    const moved = rooms.filter((r) => moves.has(r.use)).sort((a, b) => moves.get(a.use)![axis] - moves.get(b.use)![axis]);
    for (const r of moved) {
      const at = Math.round(inverse(moves.get(r.use)![axis], spans(known, axis)));
      known.push(axis === 0 ? { ...r, x: at } : { ...r, y: at });
    }
    return new Map(known.map((r) => [r.use, axis === 0 ? r.x : r.y]));
  };
  const [xs, ys] = [solve(0), solve(1)];
  const next = rooms.map((r) => (moves.has(r.use) ? { ...r, x: xs.get(r.use)!, y: ys.get(r.use)! } : r));
  const at = (p: Point) => round(unplace(next, p));
  const map = (e: Edit): Edit => {
    switch (e.kind) {
      case 'Group':
        return { ...e, edits: e.edits.map(map) };
      case 'MoveNode': {
        const r = next.find((o) => o.use === e.id);
        return { ...e, pos: r ? [r.x, r.y] : at(e.pos) };
      }
      case 'AddNode':
      case 'AddPin':
      case 'MovePin':
        return { ...e, pos: at(e.pos) };
      case 'SetRoute':
        return { ...e, points: e.points.map(at) };
      case 'SetGroup': {
        const { pos, size } = e.group;
        const [[l, t], [r, b]] = [at(pos), at([pos[0] + size[0], pos[1] + size[1]])];
        return { ...e, group: { ...e.group, pos: [l, t], size: [Math.max(GROUP_MIN[0], r - l), Math.max(GROUP_MIN[1], b - t)] } };
      }
      case 'SetNote':
        return { ...e, note: { ...e.note, pos: at(e.note.pos) } };
      default:
        return e;
    }
  };
  return map(edit);
}
