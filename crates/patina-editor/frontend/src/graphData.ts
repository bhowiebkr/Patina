// The nodes and the edges of the graph canvas, made from the view of the engine (spec 15.2, 15.3).

import type { Edge } from '@xyflow/react';
import type { Domain, KindInfo, NodeInfo, Pin, Point, View } from './api';
import { kindOf, socketOf, type Socket } from './bus';
import { ORDER, infoOf, nodeState, outputType, valueStale, type NodeData, type PatinaNode, type PortNode, type RunState } from './NodeView';
import { place, type Frame, type Room } from './frames';
import { outputTypeOf } from './palette';
import type { WireData } from './WireLayer';

const NO_POINTS: readonly Point[] = [];

/** `<node ID>.<pin key>` to the node ID and the pin key. */
export function splitPin(pin: string): [string, string] {
  const dot = pin.indexOf('.');
  return dot < 0 ? [pin, ''] : [pin.slice(0, dot), pin.slice(dot + 1)];
}

export type GraphNode = PatinaNode | PortNode;

/** The type of each public pin of the open subgraph, by "in:<key>" and "out:<key>". */
export function portTypes(view: View): Map<string, string> {
  const info = view.nodes.find((n) => n.subgraph && n.name === view.graphName);
  return new Map([...(info?.inputs ?? []).map((p) => [`in:${p.key}`, p.ty] as const), ...(info?.outputs ?? []).map((p) => [`out:${p.key}`, p.ty] as const)]);
}

/** The name of a pin in a bus name (15.9): a public pin, a pin of a subgraph use, or the key of an input. */
function pinName(view: View, infos: ReadonlyMap<string, NodeInfo | null>, pin: string, output: boolean): string | undefined {
  const [id, key] = splitPin(pin);
  const port = portKey(id);
  if (port !== null) return (output ? view.graph.input : view.graph.output)?.find((p) => p.key === port)?.name;
  const p = (output ? infos.get(id)?.outputs : infos.get(id)?.inputs)?.find((q) => q.key === key);
  // An output key, for example `out`, says nothing, so only the name of a public pin counts.
  return output ? p?.name : p && (p.name ?? p.key);
}

/** The bus sockets of a node (15.9), or undefined when no pin has a kind with lanes. */
function socketsOf(id: string, info: NodeInfo | null, kinds: readonly KindInfo[], run: RunState, sources: ReadonlyMap<string, string>) {
  let out: Record<string, Socket> | undefined;
  const add = (key: string, pin: Pin, from: string, to: string | null) => {
    const s = socketOf(kindOf(kinds, pin.ty), pin.name ?? pin.key, run.values.get(from), from !== '' && valueStale(run, from), from, to);
    if (s) (out ??= {})[key] = s;
  };
  for (const p of info?.outputs ?? []) add(`o:${p.key}`, p, `${id}.${p.key}`, null);
  for (const p of info?.inputs ?? []) add(`i:${p.key}`, p, sources.get(`${id}.${p.key}`) ?? '', `${id}.${p.key}`);
  return out;
}

/** The thumbnail of the first output of a node that has one (5.3). */
function thumbOf(id: string, info: NodeInfo | null, run: RunState): NodeData['thumb'] {
  for (const p of info?.outputs ?? []) {
    const from = `${id}.${p.key}`;
    const text = run.values.get(from)?.thumb;
    if (text) return { text, stale: valueStale(run, from), label: p.name ?? p.key };
  }
  return undefined;
}

/** The key of the public pin "in:<key>" or "out:<key>", or null for a node. */
export function portKey(id: string): string | null {
  return /^(in|out):/.test(id) ? id.slice(id.indexOf(':') + 1) : null;
}

/** The public pins of the open subgraph (6.4). The editor moves, connects, renames and removes them. */
function buildPorts(view: View, run: RunState, rooms: readonly Room[], old: Map<string, GraphNode>): PortNode[] {
  const types = portTypes(view);
  const port = (id: string, name: string, pos: Point | undefined, input: boolean, from?: string): PortNode => {
    const [x, y] = place(rooms, pos ?? [0, 0]);
    const position = { x, y };
    const key = id.slice(id.indexOf(':') + 1);
    // An input shows its test value, and an output shows the value of the use (15.9).
    const value = input ? view.test?.[key] : run.values.get(`${id}.pin`)?.text;
    const ty = types.get(id) ?? '?';
    // The socket of an input gives its lanes to all of its wires. An output gets the lanes of its one wire.
    const pin = `${id}.pin`;
    const socket = socketOf(kindOf(view.kinds, ty), name, run.values.get(pin), valueStale(run, input ? pin : (from ?? pin)), input ? pin : (from ?? ''), input ? null : pin);
    const data = { name, ty, input, value, testing: view.test != null, socket };
    const before = old.get(id);
    return before?.type === 'port' ? { ...before, position, data } : { id, type: 'port', position, deletable: false, data };
  };
  const inputs = (view.graph.input ?? []).map((p) => port(`in:${p.key}`, p.name, p.pos, true));
  return [...inputs, ...(view.graph.output ?? []).map((p) => port(`out:${p.key}`, p.name, p.pos, false, p.from))];
}

/** The output that gives the value of each input, for the bus sockets. */
function sourcesOf(view: View): Map<string, string> {
  const wires = (view.graph.wire ?? []).map((w) => [w.to, w.from] as const);
  return new Map([...wires, ...(view.graph.input ?? []).flatMap((p) => (p.to ?? []).map((to) => [to, `in:${p.key}.pin`] as const))]);
}

/** The inner nodes of each frame, as children of their use. They have no edits (15.3). */
function innerNodes(view: View, run: RunState, guis: ReadonlyMap<string, string>, domains: ReadonlyMap<string, Domain>, frames: readonly Frame[], old: Map<string, GraphNode>): PatinaNode[] {
  const outer = sourcesOf(view);
  const lock = { draggable: false, selectable: false, connectable: false, deletable: false, focusable: false };
  return frames.flatMap((f) => {
    // An inner input that a public input feeds gets the value of the outer wire to the use.
    const fed = f.inputs.flatMap(({ key, to }) => {
      const from = outer.get(`${f.use}.${key}`);
      return from === undefined ? [] : [[to, from] as const];
    });
    const sources = new Map([...f.wires.map((w) => [w.to, w.from] as const), ...fed]);
    const wired = [...f.wires.map((w) => w.to), ...f.inputs.map((p) => p.to)].map(splitPin);
    return f.nodes.map(({ entry, x, y }): PatinaNode => {
      const info = infoOf(entry, view.nodes);
      const [state, message] = nodeState(entry, info, view, run);
      const time = run.phase === 'ran' ? run.times.get(entry.id) : undefined;
      const sockets = socketsOf(entry.id, info, view.kinds, run, sources);
      const keys = wired.filter(([id]) => id === entry.id).map(([, key]) => key);
      const data = { entry, info, wired: keys, state, message, editable: false, time, gui: guis.get(`0 ${entry.id}`), sockets, domain: domains.get(entry.id), inner: true, thumb: thumbOf(entry.id, info, run) };
      const position = { x, y };
      const before = old.get(entry.id);
      return before?.type === 'patina' ? { ...before, position, data } : { id: entry.id, type: 'patina', parentId: f.use, position, data, ...lock };
    });
  });
}

/** Keeps the selection and the measured size of the nodes that stay. */
export function buildNodes(view: View, run: RunState, guis: ReadonlyMap<string, string>, domains: ReadonlyMap<string, Domain>, frames: readonly Frame[], rooms: readonly Room[], prev: GraphNode[]): GraphNode[] {
  const old = new Map(prev.map((n) => [n.id, n]));
  const wires = view.graph.wire ?? [];
  const sources = sourcesOf(view);
  const open = new Set(frames.map((f) => f.use));
  const nodes: GraphNode[] = (view.graph.node ?? []).map((entry) => {
    const info = infoOf(entry, view.nodes);
    const [state, message] = nodeState(entry, info, view, run);
    const wired = wires.map((w) => splitPin(w.to)).filter(([id]) => id === entry.id).map(([, key]) => key);
    const [x, y] = place(rooms, entry.pos ?? [0, 0]);
    const position = { x, y };
    const time = run.phase === 'ran' ? run.times.get(entry.id) : undefined;
    const slot = run.slot !== null && entry.folder === `nodes/${run.slot}`;
    const sockets = socketsOf(entry.id, info, view.kinds, run, sources);
    const domain = domains.get(entry.id);
    const data = { entry, info, wired, state, message, editable: view.trusted, time, gui: guis.get(`0 ${entry.id}`), slot, sockets, domain, open: open.has(entry.id), thumb: thumbOf(entry.id, info, run) };
    const before = old.get(entry.id);
    return before?.type === 'patina' ? { ...before, position, data } : { id: entry.id, type: 'patina' as const, position, data };
  });
  // A parent comes before its children in the list.
  return [...nodes, ...buildPorts(view, run, rooms, old), ...innerNodes(view, run, guis, domains, frames, old)];
}

/** The wires of each frame: between the inner nodes, and between the frame pins and the inner nodes (15.3). */
function innerEdges(view: View, domains: ReadonlyMap<string, Domain>, frames: readonly Frame[], rooms: readonly Room[]): Edge<WireData>[] {
  const outer = sourcesOf(view);
  const lock = { selectable: false, focusable: false, deletable: false, reconnectable: false };
  const edge = (id: string, [source, sourceHandle]: [string, string], [target, targetHandle]: [string, string], data: WireData): Edge<WireData> => ({ id, type: 'wire', source, sourceHandle, target, targetHandle, ...lock, data: { ...data, fixed: true } });
  return frames.flatMap((f) => {
    const use = view.graph.node?.find((n) => n.id === f.use);
    const pins = use && infoOf(use, view.nodes);
    const [ux, uy] = place(rooms, use?.pos ?? [0, 0]);
    const infos = new Map(f.nodes.map((n) => [n.entry.id, infoOf(n.entry, view.nodes)]));
    const ty = (from: string) => outputTypeOf(from, (id) => infos.get(id), (to) => f.wires.find((w) => w.to === to)?.from);
    const wires = f.wires.map((w) => {
      const [a, b] = [domains.get(splitPin(w.from)[0]), domains.get(splitPin(w.to)[0])];
      const bus = w.name ?? pinName(view, infos, w.from, true) ?? pinName(view, infos, w.to, false);
      return edge(`${w.from}>${w.to}`, splitPin(w.from), splitPin(w.to), { from: w.from, to: w.to, route: w.points.map(([x, y]): Point => [x + ux, y + uy]), style: w.style, corners: w.corners, ty: ty(w.from), name: w.name, bus, cross: a !== undefined && b !== undefined && a !== b });
    });
    // A wire from a frame pin shows the value of the outer wire. It routes as its own net, from the frame edge.
    const ins = f.inputs.map(({ key, to }) => {
      const pin = pins?.inputs.find((p) => p.key === key);
      const from = outer.get(`${f.use}.${key}`) ?? `${f.use}.${key}`;
      return edge(`${f.use}.in:${key}>${to}`, [f.use, `in:${key}`], splitPin(to), { from, to, route: NO_POINTS, ty: pin?.ty, bus: pin?.name ?? pin?.key, net: `${f.use}.in:${key}` });
    });
    const outs = f.outputs.map(({ key, from }) => {
      const pin = pins?.outputs.find((p) => p.key === key);
      const to = `${f.use}.out:${key}`;
      return edge(`${from}>${to}`, splitPin(from), [f.use, `out:${key}`], { from, to, route: NO_POINTS, ty: ty(from) ?? pin?.ty, bus: pin?.name });
    });
    return [...wires, ...ins, ...outs];
  });
}

/** A wire between two known clock domains that differ is dashed (15.3). */
export function buildEdges(view: View, domains: ReadonlyMap<string, Domain>, frames: readonly Frame[], rooms: readonly Room[], prev: Edge<WireData>[]): Edge<WireData>[] {
  const selected = new Set(prev.filter((e) => e.selected).map((e) => e.id));
  const types = portTypes(view);
  const infos = new Map((view.graph.node ?? []).map((n) => [n.id, infoOf(n, view.nodes)]));
  const bus = (from: string, to: string, name?: string) => name ?? pinName(view, infos, from, true) ?? pinName(view, infos, to, false);
  const fixed = (from: string, to: string, ty: string | undefined): Edge<WireData> => {
    const [[source, sourceHandle], [target, targetHandle]] = [splitPin(from), splitPin(to)];
    const id = `${from}>${to}`;
    return { id, type: 'wire', source, sourceHandle, target, targetHandle, selected: selected.has(id), data: { from, to, route: NO_POINTS, ty, fixed: true, bus: bus(from, to) } };
  };
  const ports = [
    ...(view.graph.input ?? []).flatMap((p) => (p.to ?? []).map((to) => fixed(`in:${p.key}.pin`, to, types.get(`in:${p.key}`)))),
    ...(view.graph.output ?? []).filter((p) => p.from).map((p) => fixed(p.from, `out:${p.key}.pin`, outputType(view, p.from))),
  ];
  const wires = (view.graph.wire ?? []).map((w): Edge<WireData> => {
    const [source, sourceHandle] = splitPin(w.from);
    const [target, targetHandle] = splitPin(w.to);
    const id = `${w.from}>${w.to}`;
    const [a, b] = [domains.get(source), domains.get(target)];
    const cross = a !== undefined && b !== undefined && a !== b;
    return { id, type: 'wire', source, sourceHandle, target, targetHandle, selected: selected.has(id), data: { from: w.from, to: w.to, route: w.points ? w.points.map((q) => place(rooms, q)) : NO_POINTS, style: w.style, corners: w.corners, ty: outputType(view, w.from), name: w.name, bus: bus(w.from, w.to, w.name), cross } };
  });
  // An order wire has no data, so the Inspector does not show it.
  const orders = (view.graph.order ?? []).map((o): Edge<WireData> => {
    const id = `order:${o.before}>${o.after}`;
    return { id, type: 'order', source: o.before, sourceHandle: ORDER, target: o.after, targetHandle: ORDER, selected: selected.has(id) };
  });
  return [...wires, ...ports, ...orders, ...innerEdges(view, domains, frames, rooms)];
}
