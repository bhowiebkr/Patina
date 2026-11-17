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
  type Viewport,
} from '@xyflow/react';
import { arrange, type Box, type Link } from './arrange';
import { AddNode } from './AddNode';
import { NameField } from './NameField';
import { api, newId, type Domain, type Edit, type GraphFile, type LibraryNode, type NodeInfo, type Point, type View } from './api';
import { HoverContext, HoverStore, kindOf, wireBits, wireWidth } from './bus';
import { Menu, savedViewport, saveViewport, type Item } from './Dock';
import { canvasMenu, type MenuAction, type MenuTarget } from './canvasMenu';
import { FrameToggle, HEADER, NodeEdit, NodeView, ORDER, ROW, PortView, infoOf, outputType, valueStale, type PatinaNode, type RunState } from './NodeView';
import { FAR_ZOOM } from './gl';
import { NODE_WIDTH } from './gui';
import { frameOf, graphEdit, shownFrames, toggleFrame, type Room } from './frames';
import { dragStyle, matchPin, socketKey } from './palette';
import { OrderEdge, WireContext, WireEdge, WireLayer, type WireData } from './WireLayer';
import { Labels } from './labels';
import { Router, type Rect } from './route';
import { WireStore } from './wires';
import { useTouchpadPan } from './touchpad';
import { buildEdges, buildNodes, portKey, portTypes, splitPin, type GraphNode } from './graphData';

export const NODE_MIME = 'application/patina-node';
const FIT = { padding: 0.2, maxZoom: 1, duration: 0 };
const NODE_TYPES = { patina: NodeView, port: PortView };
const EDGE_TYPES = { wire: WireEdge, order: OrderEdge };
const PAN_BUTTONS = [1];
const MULTI_KEYS = ['Shift', 'Control'];

/** What an AddNode edit names: a built-in node, a subgraph or a node folder. */
function nodeSource(n: NodeInfo): string {
  return n.builtin ? n.name : `${n.subgraph ? 'graphs' : 'nodes'}/${n.name}`;
}

/** The entries of the Add node palette that add a public pin to an open subgraph (6.4). */
const PIN_ENTRY = { inputs: [], outputs: [], error: null, builtin: false, generics: ['?'], full: false, color: null, subgraph: false, kind: 'generic' as const };
const PUBLIC_INPUT: NodeInfo = { ...PIN_ENTRY, name: 'Public input', outputs: [{ key: 'pin', ty: '?' }] };
const PUBLIC_OUTPUT: NodeInfo = { ...PIN_ENTRY, name: 'Public output', inputs: [{ key: 'pin', ty: '?' }] };

/** The moves of the Arrange command (15.9): the selected nodes, or all nodes when fewer than two are selected. */
function arrangeEdits(
  nodes: GraphNode[],
  edges: Edge<WireData>[],
  flow: ReactFlowInstance<GraphNode, Edge<WireData>>,
  widthOf: (data: WireData) => number,
): Edit[] {
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
      ? [
          {
            from: e.source,
            fy: pinY(e.source, e.sourceHandle, 'source'),
            to: e.target,
            ty: pinY(e.target, e.targetHandle, 'target'),
            net: e.data?.from ?? e.id,
            width: e.data ? widthOf(e.data) : 2,
          },
        ]
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
  /** Copies the nodes to the clipboard, and is true when it did (15.11). */
  onCopy: (ids: string[]) => Promise<boolean>;
  /** Pastes the clipboard, or adds a library node, at a graph position (15.11, 6.2). */
  onPaste: (at: [number, number], library?: LibraryNode) => void;
}

export function GraphView({ view, run, domains, guis, onEdit, onSelect, focus, onOpen, onCollapse, onExpand, onCopy, onPaste }: Props) {
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
  // The palette also shows the nodes of the libraries (6.2). They can change, so they load at each open.
  const [library, setLibrary] = useState<LibraryNode[]>([]);
  const paletteOpen = !!palette;
  useEffect(() => {
    if (paletteOpen) api.libraryNodes().then(setLibrary, () => setLibrary([]));
  }, [paletteOpen]);
  const [collapsing, setCollapsing] = useState<{ ids: string[]; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<{ key: string; name: string; x: number; y: number } | null>(null);
  const pointer = useRef<{ x: number; y: number } | null>(null);
  // The saved pan and zoom of the graph (15.2), or a fit when there is none.
  const [viewport] = useState(() => savedViewport(view.graphName));
  const pane = useRef<HTMLDivElement>(null);
  const save = useCallback((v: Viewport) => saveViewport(view.graphName, v), [view.graphName]);
  const scrolling = useTouchpadPan(pane, flow, save);
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
    const copied = patina.filter((n) => !n.data.inner).map((n) => n.id);
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
      copy: () => copied.length > 0 && onCopy(copied),
      // A cut removes only the copied nodes. A public pin does not go in a clip.
      cut: () => copied.length > 0 && onCopy(copied).then((ok) => void (ok && group(copied.map((id): Edit => ({ kind: 'RemoveNode', id }))))),
      paste: (at: { x: number; y: number }, library?: LibraryNode) => {
        const p = flow.screenToFlowPosition(at);
        onPaste([Math.round(p.x), Math.round(p.y)], library);
      },
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
      arrange: () => group(arrangeEdits(nodes, edges, flow, (d) => wireWidth(wireBits(run.values.get(d.from), kindOf(view.kinds, d.ty))))),
      selectAll: () => select(true),
      clear: () => select(false),
    };
  }, [nodes, edges, edit, onExpand, onCopy, onPaste, flow, run.values, view.kinds]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (typing(e.target) || e.altKey || e.metaKey || document.querySelector('.backdrop, .menu-layer')) return;
      const key = e.key.toLowerCase();
      // Ctrl+A, C, X and V act on the nodes only over the canvas, so a panel keeps its own keys.
      const onCanvas = !!pointer.current || (e.target instanceof Element && !!e.target.closest('.react-flow'));
      if (e.ctrlKey && !e.shiftKey && key === 'a' && onCanvas) act.selectAll();
      else if (e.ctrlKey && key === 'g' && view.trusted && (e.shiftKey ? act.uses : act.patina).length > 0) {
        if (e.shiftKey) act.expand();
        else act.collapse(act.middle());
      } else if (e.ctrlKey && !e.shiftKey && ['c', 'x', 'v'].includes(key) && onCanvas) {
        if (key === 'c') act.copy();
        else if (!view.trusted) return;
        else if (key === 'x') act.cut();
        else act.paste(act.middle());
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
    else if (a === 'copy') act.copy();
    else if (a === 'cut') act.cut();
    else if (a === 'paste') act.paste(at);
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
  const choices = end ? all.filter((n) => matchPin(n, end.output, end.ty) !== undefined) : [...all, ...library];

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
          ref={pane}
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
          onMoveEnd={(_, v) => {
            if (!scrolling.current) save(v);
          }}
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
              'library' in n
                ? act.paste(palette, n as LibraryNode)
                : n === PUBLIC_INPUT || n === PUBLIC_OUTPUT
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
