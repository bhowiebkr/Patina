// The graph canvas (spec 15.2). Navigation is snappy: no animation between two views.

import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent, type MouseEvent } from 'react';
import {
  Background,
  BackgroundVariant,
  ReactFlow,
  applyEdgeChanges,
  applyNodeChanges,
  useReactFlow,
  type Connection,
  type Edge,
  type EdgeChange,
  type FinalConnectionState,
  type NodeChange,
  type OnSelectionChangeParams,
} from '@xyflow/react';
import { AddNode } from './AddNode';
import { NameField } from './NameField';
import { newId, type Edit, type NodeInfo, type Point, type View } from './api';
import { savedViewport, saveViewport } from './Dock';
import { NodeEdit, NodeView, PortView, infoOf, nodeState, outputType, valueStale, type PatinaNode, type PortNode, type RunState } from './NodeView';
import { matchPin } from './palette';
import { WireContext, WireEdge, WireLayer, type WireData } from './WireLayer';
import { WireStore } from './wires';

export const NODE_MIME = 'application/patina-node';
const FIT = { padding: 0.2, maxZoom: 1, duration: 0 };
const NODE_TYPES = { patina: NodeView, port: PortView };
const EDGE_TYPES = { wire: WireEdge };
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

/** What an AddNode edit names: a built-in node, a subgraph or a node folder. */
function nodeSource(n: NodeInfo): string {
  return n.builtin ? n.name : `${n.subgraph ? 'graphs' : 'nodes'}/${n.name}`;
}

/** The key of the public pin "in:<key>" or "out:<key>", or null for a node. */
function portKey(id: string): string | null {
  return /^(in|out):/.test(id) ? id.slice(id.indexOf(':') + 1) : null;
}

/** The entries of the Add node palette that add a public pin to an open subgraph (6.4). */
const PIN_ENTRY = { inputs: [], outputs: [], error: null, builtin: false, generics: ['?'], full: false, color: null, subgraph: false };
const PUBLIC_INPUT: NodeInfo = { ...PIN_ENTRY, name: 'Public input', outputs: [{ key: 'pin', ty: '?' }] };
const PUBLIC_OUTPUT: NodeInfo = { ...PIN_ENTRY, name: 'Public output', inputs: [{ key: 'pin', ty: '?' }] };

/** The public pins of the open subgraph (6.4). The editor moves, connects, renames and removes them. */
function buildPorts(view: View, run: RunState, old: Map<string, GraphNode>): PortNode[] {
  const types = portTypes(view);
  const port = (id: string, name: string, pos: Point | undefined, input: boolean): PortNode => {
    const position = { x: pos?.[0] ?? 0, y: pos?.[1] ?? 0 };
    const key = id.slice(id.indexOf(':') + 1);
    // An input shows its test value, and an output shows the value of the use (15.9).
    const value = input ? view.test?.[key] : run.values.get(`${id}.pin`)?.text;
    const data = { name, ty: types.get(id) ?? '?', input, value, testing: view.test != null };
    const before = old.get(id);
    return before?.type === 'port' ? { ...before, position, data } : { id, type: 'port', position, deletable: false, data };
  };
  const inputs = (view.graph.input ?? []).map((p) => port(`in:${p.key}`, p.name, p.pos, true));
  return [...inputs, ...(view.graph.output ?? []).map((p) => port(`out:${p.key}`, p.name, p.pos, false))];
}

/** Keeps the selection and the measured size of the nodes that stay. */
function buildNodes(view: View, run: RunState, guis: ReadonlyMap<string, string>, prev: GraphNode[]): GraphNode[] {
  const old = new Map(prev.map((n) => [n.id, n]));
  const wires = view.graph.wire ?? [];
  const nodes: GraphNode[] = (view.graph.node ?? []).map((entry) => {
    const info = infoOf(entry, view.nodes);
    const [state, message] = nodeState(entry, info, view, run);
    const wired = wires.map((w) => splitPin(w.to)).filter(([id]) => id === entry.id).map(([, key]) => key);
    const position = { x: entry.pos?.[0] ?? 0, y: entry.pos?.[1] ?? 0 };
    const time = run.phase === 'ran' ? run.times.get(entry.id) : undefined;
    const slot = run.slot !== null && entry.folder === `nodes/${run.slot}`;
    const data = { entry, info, wired, state, message, editable: view.trusted, time, gui: guis.get(`0 ${entry.id}`), slot };
    const before = old.get(entry.id);
    return before?.type === 'patina' ? { ...before, position, data } : { id: entry.id, type: 'patina' as const, position, data };
  });
  return [...nodes, ...buildPorts(view, run, old)];
}

function buildEdges(view: View, prev: Edge<WireData>[]): Edge<WireData>[] {
  const selected = new Set(prev.filter((e) => e.selected).map((e) => e.id));
  const types = portTypes(view);
  const fixed = (from: string, to: string, ty: string | undefined): Edge<WireData> => {
    const [[source, sourceHandle], [target, targetHandle]] = [splitPin(from), splitPin(to)];
    const id = `${from}>${to}`;
    return { id, type: 'wire', source, sourceHandle, target, targetHandle, selected: selected.has(id), data: { from, to, route: NO_POINTS, ty, fixed: true } };
  };
  const ports = [
    ...(view.graph.input ?? []).flatMap((p) => (p.to ?? []).map((to) => fixed(`in:${p.key}.pin`, to, types.get(`in:${p.key}`)))),
    ...(view.graph.output ?? []).filter((p) => p.from).map((p) => fixed(p.from, `out:${p.key}.pin`, outputType(view, p.from))),
  ];
  const wires = (view.graph.wire ?? []).map((w): Edge<WireData> => {
    const [source, sourceHandle] = splitPin(w.from);
    const [target, targetHandle] = splitPin(w.to);
    const id = `${w.from}>${w.to}`;
    return { id, type: 'wire', source, sourceHandle, target, targetHandle, selected: selected.has(id), data: { from: w.from, to: w.to, route: w.points ?? NO_POINTS, style: w.style, corners: w.corners, ty: outputType(view, w.from) } };
  });
  return [...wires, ...ports];
}

/** A wire that a drag from a pin into an empty area started: the pin, its direction and its type. */
interface FreeEnd {
  pin: string;
  output: boolean;
  ty: string;
}

function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
}

interface Props {
  view: View;
  run: RunState;
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

export function GraphView({ view, run, guis, onEdit, onSelect, focus, onOpen, onCollapse, onExpand }: Props) {
  const flow = useReactFlow<GraphNode, Edge<WireData>>();
  const [nodes, setNodes] = useState<GraphNode[]>([]);
  const [edges, setEdges] = useState<Edge<WireData>[]>([]);
  const [wires] = useState(() => new WireStore());
  const [glOk, setGlOk] = useState(true);
  // The screen point of the open Add node palette, with the pin of a wire that it ends, and the
  // last pointer point over the canvas.
  const [palette, setPalette] = useState<{ x: number; y: number; end?: FreeEnd } | null>(null);
  const [collapsing, setCollapsing] = useState<{ ids: string[]; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<{ key: string; name: string; x: number; y: number } | null>(null);
  const pointer = useRef<{ x: number; y: number } | null>(null);
  // The saved pan and zoom of the graph (15.2), or a fit when there is none.
  const [viewport] = useState(() => savedViewport(view.graphName));
  const tools = useMemo(
    () => ({ store: wires, onEdit, editable: view.trusted, values: run.values, stale: (from: string) => valueStale(run, from) }),
    [wires, onEdit, view.trusted, run],
  );

  useEffect(() => setNodes((prev) => buildNodes(view, run, guis, prev)), [view, run, guis]);
  useEffect(() => setEdges((prev) => buildEdges(view, prev)), [view]);
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
        wires.map((e) => e.data as WireData),
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
      if (moves.length > 0) onEdit(moves.length === 1 ? moves[0] : { kind: 'Group', edits: moves });
    },
    [onEdit],
  );

  // A wire from a public input or to a public output connects the pin. A wire between two pins is refused.
  const onConnect = useCallback(
    (c: Connection) => {
      const [from, to, inKey, outKey] = [`${c.source}.${c.sourceHandle}`, `${c.target}.${c.targetHandle}`, portKey(c.source), portKey(c.target)];
      if (inKey && outKey) return;
      if (inKey) onEdit({ kind: 'ConnectPin', key: inKey, end: to });
      else if (outKey) onEdit({ kind: 'ConnectPin', key: outKey, end: from });
      else onEdit({ kind: 'Connect', from, to });
    },
    [onEdit],
  );

  // A drag from a pin that ends on an empty area opens the palette at that point (15.2).
  const onConnectEnd = useCallback(
    (e: globalThis.MouseEvent | TouchEvent, c: FinalConnectionState) => {
      const from = c.fromHandle;
      if (c.isValid || !from?.id || !(e.target instanceof Element) || !e.target.classList.contains('react-flow__pane')) return;
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

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (typing(e.target) || e.altKey || e.metaKey || document.querySelector('.backdrop, .menu-layer')) return;
      if (e.ctrlKey) {
        const selected = nodes.filter((n): n is PatinaNode => !!n.selected && n.type === 'patina');
        const uses = selected.filter((n) => n.data.entry.subgraph).map((n) => n.id);
        if (e.key.toLowerCase() !== 'g' || !view.trusted || (e.shiftKey ? uses : selected).length === 0) return;
        const r = document.querySelector('.react-flow')!.getBoundingClientRect();
        if (e.shiftKey) onExpand(uses);
        else setCollapsing({ ids: selected.map((n) => n.id), ...(pointer.current ?? { x: r.left + r.width / 2, y: r.top + r.height / 2 }) });
      } else if (e.key === 'Delete' && view.trusted) {
        const removed = new Set(nodes.filter((n) => n.selected).map((n) => n.id));
        const edits = [...removed].map((id): Edit => {
          const key = portKey(id);
          return key ? { kind: 'RemovePin', key } : { kind: 'RemoveNode', id };
        });
        for (const edge of edges) {
          if (!edge.selected || removed.has(edge.source) || removed.has(edge.target)) continue;
          const { from, to } = edge.data!;
          const inKey = portKey(edge.source);
          const outKey = portKey(edge.target);
          if (inKey) edits.push({ kind: 'DisconnectPin', key: inKey, end: to });
          else if (outKey) edits.push({ kind: 'DisconnectPin', key: outKey, end: from });
          else edits.push({ kind: 'Disconnect', from, to });
        }
        if (edits.length > 0) onEdit(edits.length === 1 ? edits[0] : { kind: 'Group', edits });
      } else if (e.key === 'f' || e.key === 'F') {
        flow.fitView(FIT);
      } else if (e.shiftKey && e.key.toLowerCase() === 'a' && view.trusted) {
        const r = document.querySelector('.react-flow')!.getBoundingClientRect();
        setPalette(pointer.current ?? { x: r.left + r.width / 2, y: r.top + r.height / 2 });
      } else if (e.key === 'Escape') {
        setNodes((n) => n.map((x) => (x.selected ? { ...x, selected: false } : x)));
        setEdges((list) => list.map((x) => (x.selected ? { ...x, selected: false } : x)));
      } else {
        return;
      }
      e.preventDefault();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [nodes, edges, view.trusted, onEdit, onExpand, flow]);

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
    if (!end || !key) onEdit(add);
    else onEdit({ kind: 'Group', edits: [add, end.output ? { kind: 'Connect', from: end.pin, to: pin } : { kind: 'Connect', from: pin, to: end.pin }] });
  };
  /** Adds a public pin at a screen point, and its wire to `end` in the same undo step. It takes the key of the end as its name. */
  const addPin = (input: boolean, x: number, y: number, end?: FreeEnd) => {
    const p = flow.screenToFlowPosition({ x, y });
    const key = newId(4);
    const name = end ? splitPin(end.pin)[1] : input ? 'input' : 'output';
    const add: Edit = { kind: 'AddPin', key, name, input, pos: [Math.round(p.x), Math.round(p.y)] };
    onEdit(end ? { kind: 'Group', edits: [add, { kind: 'ConnectPin', key, end: end.pin }] } : add);
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
      <NodeEdit.Provider value={onEdit}>
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
          onConnectEnd={onConnectEnd}
          onDragOver={onDragOver}
          onDrop={onDrop}
          onDoubleClick={onDoubleClick}
          onNodeDoubleClick={(e, n) => {
            if (n.type === 'patina' && n.data.entry.subgraph) onOpen(n.data.entry.subgraph, n.id);
            else if (n.type === 'port' && view.trusted) setRenaming({ key: portKey(n.id)!, name: n.data.name, x: e.clientX, y: e.clientY });
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
              <NameField label="Pin name" initial={renaming.name} onMake={(name) => onEdit({ kind: 'RenamePin', key: renaming.key, name })} onClose={() => setRenaming(null)} />
            </div>
          </div>
        )}
      </NodeEdit.Provider>
    </WireContext.Provider>
  );
}

/** Fits the graph in the canvas. */
export function useFit() {
  const flow = useReactFlow();
  return useCallback(() => flow.fitView(FIT), [flow]);
}
