// The graph canvas (spec 15.2). Navigation is snappy: no animation between two views.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type DragEvent, type MouseEvent } from 'react';
import {
  Background,
  BackgroundVariant,
  ReactFlow,
  applyEdgeChanges,
  applyNodeChanges,
  useConnection,
  useReactFlow,
  useStore,
  type Connection,
  type Edge,
  type EdgeChange,
  type FinalConnectionState,
  type NodeChange,
  type OnSelectionChangeParams,
  type ReactFlowInstance,
} from '@xyflow/react';
import { arrange, type Box, type Link } from './arrange';
import { AddNode } from './AddNode';
import { NameField } from './NameField';
import { api, newId, type Domain, type Edit, type GraphFile, type KindInfo, type NodeInfo, type Pin, type Point, type View } from './api';
import { HoverContext, HoverStore, kindOf, socketOf, type Socket } from './bus';
import { Menu, savedViewport, saveViewport, type Item } from './Dock';
import { canvasMenu, type MenuAction, type MenuTarget } from './canvasMenu';
import { FrameToggle, HEADER, NodeEdit, NodeView, ORDER, ROW, PortView, infoOf, nodeState, outputType, valueStale, type PatinaNode, type PortNode, type RunState } from './NodeView';
import { FAR_ZOOM } from './gl';
import { NODE_WIDTH } from './gui';
import { frameOf, graphEdit, place, shownFrames, toggleFrame, type Frame, type Room } from './frames';
import { dragStyle, matchPin, outputTypeOf, socketKey } from './palette';
import { OrderEdge, WireContext, WireEdge, WireLayer, type WireData } from './WireLayer';
import { Labels } from './labels';
import { Router, type Rect } from './route';
import { WireStore } from './wires';

export const NODE_MIME = 'application/patina-node';
const FIT = { padding: 0.2, maxZoom: 1, duration: 0 };
const NODE_TYPES = { patina: NodeView, port: PortView };
const EDGE_TYPES = { wire: WireEdge, order: OrderEdge };
const PAN_BUTTONS = [1];
const MULTI_KEYS = ['Shift', 'Control'];
const NO_POINTS: readonly Point[] = [];

/** `<node ID>.<pin key>` to the node ID and the pin key. */
function splitPin(pin: string): [string, string] {
  const dot = pin.indexOf('.');
  return dot < 0 ? [pin, ''] : [pin.slice(0, dot), pin.slice(dot + 1)];
}

type GraphNode = PatinaNode | PortNode;

/** The type of each public pin of the open subgraph, by "in:<key>" and "out:<key>". */
function portTypes(view: View): Map<string, string> {
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

/** What an AddNode edit names: a built-in node, a subgraph or a node folder. */
function nodeSource(n: NodeInfo): string {
  return n.builtin ? n.name : `${n.subgraph ? 'graphs' : 'nodes'}/${n.name}`;
}

/** The key of the public pin "in:<key>" or "out:<key>", or null for a node. */
function portKey(id: string): string | null {
  return /^(in|out):/.test(id) ? id.slice(id.indexOf(':') + 1) : null;
}

/** The entries of the Add node palette that add a public pin to an open subgraph (6.4). */
const PIN_ENTRY = { inputs: [], outputs: [], error: null, builtin: false, generics: ['?'], full: false, color: null, subgraph: false, kind: 'generic' as const };
const PUBLIC_INPUT: NodeInfo = { ...PIN_ENTRY, name: 'Public input', outputs: [{ key: 'pin', ty: '?' }] };
const PUBLIC_OUTPUT: NodeInfo = { ...PIN_ENTRY, name: 'Public output', inputs: [{ key: 'pin', ty: '?' }] };

/** The moves of the Arrange command (15.9): the selected nodes, or all nodes when fewer than two are selected. */
function arrangeEdits(nodes: GraphNode[], edges: Edge<WireData>[], flow: ReactFlowInstance<GraphNode, Edge<WireData>>): Edit[] {
  const picked = nodes.filter((n) => n.selected);
  const chosen = picked.length > 1 ? picked : nodes.filter((n) => !n.parentId);
  if (chosen.length === 0) return [];
  const ids = new Set(chosen.map((n) => n.id));
  const size = (n: GraphNode) => [n.measured?.width ?? 0, n.measured?.height ?? 0];
  const pinY = (id: string, handle: string | null | undefined, kind: 'source' | 'target') => {
    const b = flow.getInternalNode(id)?.internals.handleBounds?.[kind]?.find((h) => h.id === handle);
    return b ? b.y + b.height / 2 : (flow.getInternalNode(id)?.measured.height ?? 0) / 2;
  };
  const boxes = chosen.map((n): Box => {
    const [w, h] = size(n);
    return { id: n.id, w, h, side: n.id.startsWith('in:') ? 'first' : n.id.startsWith('out:') ? 'last' : undefined };
  });
  const links = edges.flatMap((e): Link[] =>
    ids.has(e.source) && ids.has(e.target)
      ? [{ from: e.source, fy: pinY(e.source, e.sourceHandle, 'source'), to: e.target, ty: pinY(e.target, e.targetHandle, 'target'), net: e.data?.from ?? e.id }]
      : [],
  );
  const at = arrange(boxes, links);
  const x0 = Math.min(...chosen.map((n) => n.position.x));
  const y0 = Math.min(...chosen.map((n) => n.position.y));
  return chosen.flatMap((n): Edit[] => {
    const [x, y] = at.get(n.id)!;
    const pos: Point = [Math.round(x0 + x), Math.round(y0 + y)];
    if (pos[0] === Math.round(n.position.x) && pos[1] === Math.round(n.position.y)) return [];
    const key = portKey(n.id);
    return [key ? { kind: 'MovePin', key, pos } : { kind: 'MoveNode', id: n.id, pos }];
  });
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
      const data = { entry, info, wired: keys, state, message, editable: false, time, gui: guis.get(`0 ${entry.id}`), sockets, domain: domains.get(entry.id), inner: true };
      const position = { x, y };
      const before = old.get(entry.id);
      return before?.type === 'patina' ? { ...before, position, data } : { id: entry.id, type: 'patina', parentId: f.use, position, data, ...lock };
    });
  });
}

/** Keeps the selection and the measured size of the nodes that stay. */
function buildNodes(view: View, run: RunState, guis: ReadonlyMap<string, string>, domains: ReadonlyMap<string, Domain>, frames: readonly Frame[], rooms: readonly Room[], prev: GraphNode[]): GraphNode[] {
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
    const data = { entry, info, wired, state, message, editable: view.trusted, time, gui: guis.get(`0 ${entry.id}`), slot, sockets, domain, open: open.has(entry.id) };
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
function buildEdges(view: View, domains: ReadonlyMap<string, Domain>, frames: readonly Frame[], rooms: readonly Room[], prev: Edge<WireData>[]): Edge<WireData>[] {
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

/** A wire that a drag from a pin into an empty area started: the pin, its direction and its type. */
interface FreeEnd {
  pin: string;
  output: boolean;
  ty: string;
}

/** The edit for a wire between two sockets, or null for a wire between two public pins (6.4). */
function connectEdit(c: Connection): Edit | null {
  const [from, to, inKey, outKey] = [`${c.source}.${c.sourceHandle}`, `${c.target}.${c.targetHandle}`, portKey(c.source), portKey(c.target)];
  if (inKey && outKey) return null;
  if (c.sourceHandle === ORDER) return { kind: 'ConnectOrder', before: c.source, after: c.target };
  if (inKey) return { kind: 'ConnectPin', key: inKey, end: to };
  if (outKey) return { kind: 'ConnectPin', key: outKey, end: from };
  return { kind: 'Connect', from, to };
}

/** The edit that removes a wire. */
function disconnectEdit(edge: Edge<WireData>): Edit {
  if (edge.type === 'order') return { kind: 'DisconnectOrder', before: edge.source, after: edge.target };
  const { from, to } = edge.data!;
  const [inKey, outKey] = [portKey(edge.source), portKey(edge.target)];
  if (inKey) return { kind: 'DisconnectPin', key: inKey, end: to };
  if (outKey) return { kind: 'DisconnectPin', key: outKey, end: from };
  return { kind: 'Disconnect', from, to };
}

/** The type key of the socket `key` of `node` for the drag highlight (15.9): 'order', a type key or '*'. */
function dragKey(view: View, node: string, key: string, output: boolean): string {
  if (key === ORDER) return 'order';
  if (portKey(node)) return socketKey(portTypes(view).get(node) ?? '?', ['?']);
  const entry = view.graph.node?.find((n) => n.id === node);
  const info = entry && infoOf(entry, view.nodes);
  const ty = output ? outputType(view, `${node}.${key}`) : info?.inputs.find((p) => p.key === key)?.ty;
  return ty === undefined ? '*' : socketKey(ty, info?.generics ?? []);
}

/** An order socket connects only to an order socket (9.2). */
function orderMatch(c: Edge | Connection): boolean {
  return (c.sourceHandle === ORDER) === (c.targetHandle === ORDER);
}

function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
}

interface Props {
  view: View;
  run: RunState;
  /** The clock domain of each node of the open graph (9.1). */
  domains: ReadonlyMap<string, Domain>;
  /** The widget lines of each node GUI, by "<mode> <node id>" (4.3). */
  guis: ReadonlyMap<string, string>;
  onEdit: (edit: Edit) => Promise<void>;
  onSelect: (ids: string[], wires: WireData[]) => void;
  /** A request to select one node, for example from a click on a compile error. */
  /** The nodes to select, for example from a view (15.8). */
  focus: { ids: string[]; n: number } | null;
  /** Opens a subgraph, from a double-click on a use (6.1). */
  /** Opens a graph. `use` is the ID of the use that a double-click opened. */
  onOpen: (name: string, use?: string) => void;
  /** Moves the nodes into a new subgraph. It fails with the problem. */
  onCollapse: (name: string, ids: string[]) => Promise<void>;
  /** Puts the nodes of each subgraph use in its place. */
  onExpand: (ids: string[]) => void;
}

export function GraphView({ view, run, domains, guis, onEdit, onSelect, focus, onOpen, onCollapse, onExpand }: Props) {
  const flow = useReactFlow<GraphNode, Edge<WireData>>();
  const [nodes, setNodes] = useState<GraphNode[]>([]);
  const [edges, setEdges] = useState<Edge<WireData>[]>([]);
  const [wires] = useState(() => new WireStore());
  const [router] = useState(() => new Router());
  const [labels] = useState(() => new Labels());
  const [hover] = useState(() => new HoverStore());
  const [glOk, setGlOk] = useState(true);
  // The nodes in view, so that the frames draw only their GUIs (4.3 rule 3). None at a far zoom.
  const visible = useStore((s) => {
    const [tx, ty, zoom] = s.transform;
    if (zoom < FAR_ZOOM) return '';
    const [x0, y0] = [-tx / zoom, -ty / zoom];
    const [x1, y1] = [x0 + s.width / zoom, y0 + s.height / zoom];
    const ids: string[] = [];
    for (const n of s.nodeLookup.values()) {
      const { x, y } = n.internals.positionAbsolute;
      const [w, h] = [n.measured.width ?? 0, n.measured.height ?? 0];
      if (n.type === 'patina' && x <= x1 && x + w >= x0 && y <= y1 && y + h >= y0) ids.push(n.id);
    }
    return ids.join('\n');
  });
  useEffect(() => {
    const t = setTimeout(() => api.visible(visible === '' ? [] : visible.split('\n')).catch(() => {}), 50);
    return () => clearTimeout(t);
  }, [visible]);
  // The uses that show their inner nodes (15.3), and the files of their subgraphs.
  const [shown, setShown] = useState(() => shownFrames(view.graphName));
  const [files, setFiles] = useState<ReadonlyMap<string, GraphFile>>(() => new Map());
  useEffect(() => {
    const names = [...new Set((view.graph.node ?? []).flatMap((n) => (shown.has(n.id) && n.subgraph !== undefined ? [n.subgraph] : [])))];
    let live = true;
    // A subgraph that does not read shows no frame.
    Promise.all(names.map((name) => api.subgraphFile(name).then((file) => [name, file] as const, () => null))).then((list) => {
      if (live) setFiles((prev) => (list.length === 0 && prev.size === 0 ? prev : new Map(list.filter((x) => x !== null))));
    });
    return () => {
      live = false;
    };
  }, [view, shown]);
  const frames = useMemo(
    () =>
      (view.graph.node ?? []).flatMap((n) => {
        const file = shown.has(n.id) && n.subgraph !== undefined ? files.get(n.subgraph) : undefined;
        return file ? [frameOf(n.id, file)] : [];
      }),
    [view, files, shown],
  );
  // shortcut: the route points of the inner wires stay in place during a drag of the frame; move them with the frame when users ask.
  const toggle = useCallback((use: string) => setShown(toggleFrame(view.graphName, use)), [view.graphName]);
  // The room of each open frame, from its measured size and the last closed size of its use (15.3).
  const closed = useRef(new Map<string, readonly [number, number]>());
  const [rooms, setRooms] = useState<readonly Room[]>([]);
  // The view moves the nodes around the open frames, so each edit maps its points back to the graph.
  const edit = useCallback((e: Edit) => onEdit(graphEdit(e, rooms)), [onEdit, rooms]);
  // The socket of a wire drag, as a string so that a pointer move does not render the graph again.
  const from = useConnection((c) => (c.inProgress ? JSON.stringify([c.fromHandle.type, c.fromHandle.nodeId, c.fromHandle.id ?? '']) : ''));
  const dragCss = useMemo(() => {
    if (!from) return '';
    const [side, node, key] = JSON.parse(from) as ['source' | 'target', string, string];
    return dragStyle(side, dragKey(view, node, key, side === 'source'));
  }, [from, view]);
  // The screen point of the open Add node palette, with the pin of a wire that it ends, and the
  // last pointer point over the canvas.
  const [palette, setPalette] = useState<{ x: number; y: number; end?: FreeEnd } | null>(null);
  const [collapsing, setCollapsing] = useState<{ ids: string[]; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<{ key: string; name: string; x: number; y: number } | null>(null);
  const pointer = useRef<{ x: number; y: number } | null>(null);
  // The saved pan and zoom of the graph (15.2), or a fit when there is none.
  const [viewport] = useState(() => savedViewport(view.graphName));
  const tools = useMemo(
    () => ({ store: wires, router, labels, onEdit: edit, editable: view.trusted, values: run.values, stale: (from: string) => valueStale(run, from), kinds: view.kinds }),
    [wires, router, labels, edit, view.trusted, run, view.kinds],
  );
  // The node bodies that the automatic routes go around (15.9).
  useLayoutEffect(() => {
    const at = new Map(nodes.map((n) => [n.id, n.position]));
    const body = (n: GraphNode): Rect[] => {
      const { width, height } = n.measured ?? {};
      if (!width || !height) return [];
      const p = n.parentId ? at.get(n.parentId) : undefined;
      const [l, t] = [n.position.x + (p?.x ?? 0), n.position.y + (p?.y ?? 0)];
      const [r, b] = [l + width, t + height];
      if (n.type !== 'patina' || !n.data.open) return [{ l, t, r, b }];
      // A frame blocks at its header and its thin edges, so the inner wires route inside it. The wires
      // at the frame pins cross the side edges within the pin margin of the router.
      return [{ l, t, r, b: t + HEADER }, { l: l - 1, t: t + HEADER, r: l, b }, { l: r, t: t + HEADER, r: r + 1, b }, { l, t: b - 1, r, b }];
    };
    const rects = nodes.flatMap(body);
    router.setObstacles(rects);
    labels.setObstacles(rects);
  }, [nodes, router, labels]);

  useLayoutEffect(() => {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    for (const n of nodes) {
      if (n.type === 'patina' && !n.data.open && n.measured?.width && n.measured.height) closed.current.set(n.id, [n.measured.width, n.measured.height]);
    }
    const next = frames.flatMap((f): Room[] => {
      const n = byId.get(f.use);
      if (n?.type !== 'patina' || !n.data.open || !n.measured?.width || !n.measured.height) return [];
      // A use that was not closed in this view gets the smallest closed size, so its frame never covers a node.
      const rows = Math.max(n.data.info?.inputs.length ?? 0, n.data.info?.outputs.length ?? 0);
      const [w, h] = closed.current.get(f.use) ?? [NODE_WIDTH, HEADER + rows * ROW];
      const [x, y] = n.data.entry.pos ?? [0, 0];
      return [{ use: f.use, x, y, w, h, dx: n.measured.width - w, dy: n.measured.height - h }];
    });
    setRooms((prev) => (JSON.stringify(prev) === JSON.stringify(next) ? prev : next));
  }, [nodes, frames]);

  useEffect(() => setNodes((prev) => buildNodes(view, run, guis, domains, frames, rooms, prev)), [view, run, guis, domains, frames, rooms]);
  useEffect(() => setEdges((prev) => buildEdges(view, domains, frames, rooms, prev)), [view, domains, frames, rooms]);
  useEffect(() => {
    if (!focus) return;
    const ids = new Set(focus.ids);
    setNodes((list) => list.map((x) => (x.selected === ids.has(x.id) ? x : { ...x, selected: ids.has(x.id) })));
    setEdges((list) => list.map((x) => (x.selected ? { ...x, selected: false } : x)));
  }, [focus]);

  const onNodesChange = useCallback((changes: NodeChange<GraphNode>[]) => setNodes((n) => applyNodeChanges(changes, n)), []);
  const onEdgesChange = useCallback((changes: EdgeChange<Edge<WireData>>[]) => setEdges((e) => applyEdgeChanges(changes, e)), []);

  const onSelectionChange = useCallback(
    ({ nodes: selected, edges: wires }: OnSelectionChangeParams) =>
      onSelect(
        selected.map((n) => n.id),
        wires.flatMap((e) => (e.data ? [e.data as WireData] : [])),
      ),
    [onSelect],
  );

  const onNodeDragStop = useCallback(
    (_: unknown, __: GraphNode, dragged: GraphNode[]) => {
      const moves = dragged.map((n): Edit => {
        const pos: Point = [Math.round(n.position.x), Math.round(n.position.y)];
        const key = portKey(n.id);
        return key ? { kind: 'MovePin', key, pos } : { kind: 'MoveNode', id: n.id, pos };
      });
      if (moves.length > 0) edit(moves.length === 1 ? moves[0] : { kind: 'Group', edits: moves });
    },
    [edit],
  );

  // A wire from a public input or to a public output connects the pin. A wire between two pins is refused.
  const onConnect = useCallback(
    (c: Connection) => {
      const add = connectEdit(c);
      if (add) edit(add);
    },
    [edit],
  );

  // A drag on the end of a wire moves that end to another socket, in one undo step (15.9).
  // A data wire keeps its name and its look. A drop on an empty area keeps the wire.
  const onReconnect = useCallback(
    (old: Edge<WireData>, c: Connection) => {
      const add = connectEdit(c);
      const same = old.source === c.source && old.sourceHandle === c.sourceHandle && old.target === c.target && old.targetHandle === c.targetHandle;
      if (!add || same) return;
      const edits = [disconnectEdit(old), add];
      const d = old.data;
      if (add.kind === 'Connect' && d) {
        if (d.style || d.corners) edits.push({ kind: 'SetLook', to: add.to, style: d.style, corners: d.corners });
        if (d.name) edits.push({ kind: 'NameWire', to: add.to, name: d.name });
      }
      edit({ kind: 'Group', edits });
    },
    [edit],
  );

  // A drag from a pin that ends on an empty area opens the palette at that point (15.2).
  const onConnectEnd = useCallback(
    (e: globalThis.MouseEvent | TouchEvent, c: FinalConnectionState) => {
      const from = c.fromHandle;
      if (c.isValid || !from?.id || from.id === ORDER || !(e.target instanceof Element) || !e.target.classList.contains('react-flow__pane')) return;
      const pin = `${from.nodeId}.${from.id}`;
      const output = from.type === 'source';
      if (portKey(from.nodeId)) return;
      const entry = view.graph.node?.find((n) => n.id === from.nodeId);
      const ty = output ? outputType(view, pin) : entry && infoOf(entry, view.nodes)?.inputs.find((p) => p.key === from.id)?.ty;
      const point = 'changedTouches' in e ? e.changedTouches[0] : e;
      if (ty !== undefined) setPalette({ x: point.clientX, y: point.clientY, end: { pin, output, ty } });
    },
    [view],
  );

  // The commands of the keys and of the right-click menu (15.2). They act on the selection.
  const act = useMemo(() => {
    const selected = nodes.filter((n) => n.selected);
    const patina = selected.filter((n): n is PatinaNode => n.type === 'patina');
    const uses = patina.filter((n) => n.data.entry.subgraph).map((n) => n.id);
    const group = (edits: Edit[]) => edits.length > 0 && edit(edits.length === 1 ? edits[0] : { kind: 'Group', edits });
    const select = (all: boolean) => {
      setNodes((list) => list.map((x) => (!!x.selected === all ? x : { ...x, selected: all })));
      setEdges((list) => list.map((x) => (!!x.selected === all ? x : { ...x, selected: all })));
    };
    return {
      selected,
      patina,
      uses,
      /** The pointer, or the middle of the canvas when the pointer is not on it. */
      middle: () => {
        const r = document.querySelector('.react-flow')!.getBoundingClientRect();
        return pointer.current ?? { x: r.left + r.width / 2, y: r.top + r.height / 2 };
      },
      collapse: (at: { x: number; y: number }) => setCollapsing({ ids: patina.map((n) => n.id), ...at }),
      expand: () => onExpand(uses),
      remove: () => {
        const removed = new Set(selected.map((n) => n.id));
        const edits = [...removed].map((id): Edit => {
          const key = portKey(id);
          return key ? { kind: 'RemovePin', key } : { kind: 'RemoveNode', id };
        });
        for (const edge of edges) {
          if (edge.selected && !removed.has(edge.source) && !removed.has(edge.target)) edits.push(disconnectEdit(edge));
        }
        group(edits);
      },
      fit: () => flow.fitView(selected.length > 0 ? { ...FIT, nodes: selected } : FIT),
      arrange: () => group(arrangeEdits(nodes, edges, flow)),
      selectAll: () => select(true),
      clear: () => select(false),
    };
  }, [nodes, edges, edit, onExpand, flow]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (typing(e.target) || e.altKey || e.metaKey || document.querySelector('.backdrop, .menu-layer')) return;
      const key = e.key.toLowerCase();
      // Ctrl+A selects the nodes only over the canvas, so a panel keeps its own Ctrl+A.
      if (e.ctrlKey && !e.shiftKey && key === 'a' && (pointer.current || (e.target instanceof Element && e.target.closest('.react-flow')))) act.selectAll();
      else if (e.ctrlKey && key === 'g' && view.trusted && (e.shiftKey ? act.uses : act.patina).length > 0) {
        if (e.shiftKey) act.expand();
        else act.collapse(act.middle());
      } else if (e.ctrlKey) return;
      else if (e.key === 'Delete' && view.trusted) act.remove();
      else if (key === 'f') act.fit();
      else if (e.shiftKey && key === 'l' && view.trusted) act.arrange();
      else if (e.shiftKey && key === 'a' && view.trusted) setPalette(act.middle());
      else if (e.key === 'Escape') act.clear();
      else return;
      e.preventDefault();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [act, view.trusted]);

  // The right-click menu: its screen point, and the empty canvas, the selected nodes or one connection.
  const [menu, setMenu] = useState<{ x: number; y: number; on: 'pane' | 'nodes' | Edge<WireData> } | null>(null);
  const openMenu = (e: { preventDefault(): void; clientX: number; clientY: number }, on: 'pane' | 'nodes' | Edge<WireData>) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, on });
  };
  // A right-click out of the selection selects only the node or the connection, as a click does.
  const selectOnly = (id: string, node: boolean) => {
    setNodes((list) => list.map((x) => (!!x.selected === (node && x.id === id) ? x : { ...x, selected: node && x.id === id })));
    setEdges((list) => list.map((x) => (!!x.selected === (!node && x.id === id) ? x : { ...x, selected: !node && x.id === id })));
  };
  const menuTarget = (on: 'pane' | 'nodes' | Edge<WireData>): MenuTarget => {
    if (on === 'pane') return { kind: 'pane' };
    if (on !== 'nodes') return { kind: 'wire' };
    const one = act.selected.length === 1 ? act.selected[0] : undefined;
    const open = one?.type === 'patina' && !!one.data.entry.subgraph && !one.data.inner;
    return { kind: 'nodes', count: act.selected.length, nodes: act.patina.length, uses: act.uses.length, open, pin: one?.type === 'port' };
  };
  const runMenu = (a: MenuAction, at: { x: number; y: number }, on: 'pane' | 'nodes' | Edge<WireData>) => {
    const one = act.selected[0];
    if (a === 'add') setPalette(at);
    else if (a === 'selectAll') act.selectAll();
    else if (a === 'fit') act.fit();
    else if (a === 'arrange') act.arrange();
    else if (a === 'collapse') act.collapse(at);
    else if (a === 'expand') act.expand();
    else if (a === 'delete') act.remove();
    else if (a === 'open' && one?.type === 'patina' && one.data.entry.subgraph) onOpen(one.data.entry.subgraph, one.id);
    else if (a === 'rename' && one?.type === 'port') setRenaming({ key: portKey(one.id)!, name: one.data.name, ...at });
    else if (a === 'disconnect' && typeof on === 'object') edit(disconnectEdit(on));
  };
  const menuItems: Item[] = menu
    ? canvasMenu(menuTarget(menu.on), view.trusted).map((e) => (e === 'line' ? e : { label: e.label, keys: e.keys, run: () => runMenu(e.action, menu, menu.on) }))
    : [];

  const onDragOver = (e: DragEvent) => {
    if (e.dataTransfer.types.includes(NODE_MIME) && view.trusted) {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
    }
  };

  /** Adds a node with its top left corner at a screen point, and its wire to `end` in the same undo step. */
  const addNode = (folder: string, x: number, y: number, end?: FreeEnd, info?: NodeInfo) => {
    const p = flow.screenToFlowPosition({ x, y });
    const id = newId();
    const add: Edit = { kind: 'AddNode', id, folder, builtin: info?.builtin || undefined, pos: [Math.round(p.x), Math.round(p.y)] };
    const key = end && info && matchPin(info, end.output, end.ty);
    const pin = `${id}.${key}`;
    if (!end || !key) edit(add);
    else edit({ kind: 'Group', edits: [add, end.output ? { kind: 'Connect', from: end.pin, to: pin } : { kind: 'Connect', from: pin, to: end.pin }] });
  };
  /** Adds a public pin at a screen point, and its wire to `end` in the same undo step. It takes the key of the end as its name. */
  const addPin = (input: boolean, x: number, y: number, end?: FreeEnd) => {
    const p = flow.screenToFlowPosition({ x, y });
    const key = newId(4);
    const name = end ? splitPin(end.pin)[1] : input ? 'input' : 'output';
    const add: Edit = { kind: 'AddPin', key, name, input, pos: [Math.round(p.x), Math.round(p.y)] };
    edit(end ? { kind: 'Group', edits: [add, { kind: 'ConnectPin', key, end: end.pin }] } : add);
  };
  const end = palette?.end;
  const all = view.graphName === 'main' ? view.nodes : [...view.nodes, PUBLIC_INPUT, PUBLIC_OUTPUT];
  const choices = end ? all.filter((n) => matchPin(n, end.output, end.ty) !== undefined) : all;

  const onDrop = (e: DragEvent) => {
    const info = view.nodes.find((n) => n.name === e.dataTransfer.getData(NODE_MIME));
    if (!info || !view.trusted) return;
    e.preventDefault();
    addNode(nodeSource(info), e.clientX, e.clientY, undefined, info);
  };

  const onDoubleClick = (e: MouseEvent) => {
    if (view.trusted && e.target instanceof Element && e.target.classList.contains('react-flow__pane')) setPalette({ x: e.clientX, y: e.clientY });
  };

  return (
    <WireContext.Provider value={tools}>
      <HoverContext.Provider value={hover}>
      <NodeEdit.Provider value={edit}>
      <FrameToggle.Provider value={toggle}>
        <ReactFlow
          className={glOk ? undefined : 'svg-wires'}
          nodes={nodes}
          edges={edges}
          nodeTypes={NODE_TYPES}
          edgeTypes={EDGE_TYPES}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onSelectionChange={onSelectionChange}
          onNodeDragStop={onNodeDragStop}
          onConnect={onConnect}
          onReconnect={onReconnect}
          edgesReconnectable={view.trusted}
          isValidConnection={orderMatch}
          onConnectEnd={onConnectEnd}
          onDragOver={onDragOver}
          onDrop={onDrop}
          onDoubleClick={onDoubleClick}
          onNodeDoubleClick={(e, n) => {
            if (n.type === 'patina' && n.data.entry.subgraph && !n.data.inner) onOpen(n.data.entry.subgraph, n.id);
            else if (n.type === 'port' && view.trusted) setRenaming({ key: portKey(n.id)!, name: n.data.name, x: e.clientX, y: e.clientY });
          }}
          onPaneContextMenu={(e) => openMenu(e, 'pane')}
          onSelectionContextMenu={(e) => openMenu(e, 'nodes')}
          onNodeContextMenu={(e, n) => {
            if (!n.selected) selectOnly(n.id, true);
            openMenu(e, 'nodes');
          }}
          onEdgeContextMenu={(e, edge) => {
            if (!edge.selected) selectOnly(edge.id, false);
            openMenu(e, edge);
          }}
          onPointerMove={(e) => (pointer.current = { x: e.clientX, y: e.clientY })}
          onPointerLeave={() => (pointer.current = null)}
          nodesDraggable={view.trusted}
          nodesConnectable={view.trusted}
          panOnDrag={PAN_BUTTONS}
          selectionOnDrag
          panActivationKeyCode="Space"
          multiSelectionKeyCode={MULTI_KEYS}
          selectionKeyCode={null}
          deleteKeyCode={null}
          zoomOnDoubleClick={false}
          minZoom={0.1}
          maxZoom={4}
          fitView={!viewport}
          fitViewOptions={FIT}
          defaultViewport={viewport}
          onMoveEnd={(_, v) => saveViewport(view.graphName, v)}
          colorMode="dark"
        >
          <Background id="minor" variant={BackgroundVariant.Lines} gap={20} color="#202020" />
          <Background id="major" variant={BackgroundVariant.Lines} gap={100} color="#292929" bgColor="transparent" />
          <WireLayer store={wires} onReady={setGlOk} />
        </ReactFlow>
        {dragCss && <style>{dragCss}</style>}
        {palette && (
          <AddNode
            x={palette.x}
            y={palette.y}
            nodes={choices}
            none={end && `No node has a pin of type ${end.ty}.`}
            onAdd={(n) =>
              n === PUBLIC_INPUT || n === PUBLIC_OUTPUT
                ? addPin(n === PUBLIC_INPUT, palette.x, palette.y, end)
                : addNode(nodeSource(n), palette.x, palette.y, end, n)
            }
            onClose={() => setPalette(null)}
          />
        )}
        {menu && menuItems.length > 0 && (
          <Menu x={menu.x} y={menu.y} label={menu.on === 'pane' ? 'Canvas' : menu.on === 'nodes' ? 'Node' : 'Connection'} items={menuItems} onClose={() => setMenu(null)} />
        )}
        {collapsing && (
          <div className="menu-layer">
            <div className="menu collapse-name" role="dialog" aria-label="Collapse into a subgraph" style={{ left: Math.min(collapsing.x, innerWidth - 250), top: Math.min(collapsing.y, innerHeight - 80) }}>
              <NameField label="Subgraph name" onMake={(name) => onCollapse(name, collapsing.ids)} onClose={() => setCollapsing(null)} />
            </div>
          </div>
        )}
        {renaming && (
          <div className="menu-layer">
            <div className="menu collapse-name" role="dialog" aria-label="Rename the public pin" style={{ left: Math.min(renaming.x, innerWidth - 250), top: Math.min(renaming.y, innerHeight - 80) }}>
              <NameField label="Pin name" initial={renaming.name} onMake={(name) => edit({ kind: 'RenamePin', key: renaming.key, name })} onClose={() => setRenaming(null)} />
            </div>
          </div>
        )}
      </FrameToggle.Provider>
      </NodeEdit.Provider>
      </HoverContext.Provider>
    </WireContext.Provider>
  );
}

/** Fits the graph in the canvas. */
export function useFit() {
  const flow = useReactFlow();
  return useCallback(() => flow.fitView(FIT), [flow]);
}
