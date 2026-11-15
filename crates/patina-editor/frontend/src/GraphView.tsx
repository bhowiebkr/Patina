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
import { newId, type Edit, type NodeInfo, type Point, type View } from './api';
import { savedViewport, saveViewport } from './Dock';
import { NodeView, infoOf, nodeState, outputType, valueStale, type PatinaNode, type RunState } from './NodeView';
import { matchPin } from './palette';
import { WireContext, WireEdge, WireLayer, type WireData } from './WireLayer';
import { WireStore } from './wires';

export const NODE_MIME = 'application/patina-node';
const FIT = { padding: 0.2, maxZoom: 1, duration: 0 };
const NODE_TYPES = { patina: NodeView };
const EDGE_TYPES = { wire: WireEdge };
const PAN_BUTTONS = [1];
const MULTI_KEYS = ['Shift', 'Control'];
const NO_POINTS: readonly Point[] = [];

/** `<node ID>.<pin key>` to the node ID and the pin key. */
function splitPin(pin: string): [string, string] {
  const dot = pin.indexOf('.');
  return dot < 0 ? [pin, ''] : [pin.slice(0, dot), pin.slice(dot + 1)];
}

/** Keeps the selection and the measured size of the nodes that stay. */
function buildNodes(view: View, run: RunState, guis: ReadonlyMap<string, string>, prev: PatinaNode[]): PatinaNode[] {
  const old = new Map(prev.map((n) => [n.id, n]));
  const wires = view.graph.wire ?? [];
  return (view.graph.node ?? []).map((entry) => {
    const info = infoOf(entry, view.nodes);
    const [state, message] = nodeState(entry, info, view, run);
    const wired = wires.map((w) => splitPin(w.to)).filter(([id]) => id === entry.id).map(([, key]) => key);
    const position = { x: entry.pos?.[0] ?? 0, y: entry.pos?.[1] ?? 0 };
    const time = run.phase === 'ran' ? run.times.get(entry.id) : undefined;
    const data = { entry, info, wired, state, message, editable: view.trusted, time, gui: guis.get(`0 ${entry.id}`) };
    const before = old.get(entry.id);
    return before ? { ...before, position, data } : { id: entry.id, type: 'patina' as const, position, data };
  });
}

function buildEdges(view: View, prev: Edge<WireData>[]): Edge<WireData>[] {
  const selected = new Set(prev.filter((e) => e.selected).map((e) => e.id));
  return (view.graph.wire ?? []).map((w) => {
    const [source, sourceHandle] = splitPin(w.from);
    const [target, targetHandle] = splitPin(w.to);
    const id = `${w.from}>${w.to}`;
    return { id, type: 'wire', source, sourceHandle, target, targetHandle, selected: selected.has(id), data: { from: w.from, to: w.to, route: w.points ?? NO_POINTS, style: w.style, corners: w.corners, ty: outputType(view, w.from) } };
  });
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
  focus: { id: string; n: number } | null;
}

export function GraphView({ view, run, guis, onEdit, onSelect, focus }: Props) {
  const flow = useReactFlow<PatinaNode, Edge<WireData>>();
  const [nodes, setNodes] = useState<PatinaNode[]>([]);
  const [edges, setEdges] = useState<Edge<WireData>[]>([]);
  const [wires] = useState(() => new WireStore());
  const [glOk, setGlOk] = useState(true);
  // The screen point of the open Add node palette, with the pin of a wire that it ends, and the
  // last pointer point over the canvas.
  const [palette, setPalette] = useState<{ x: number; y: number; end?: FreeEnd } | null>(null);
  const pointer = useRef<{ x: number; y: number } | null>(null);
  // The saved pan and zoom of the graph (15.2), or a fit when there is none.
  const [viewport] = useState(() => savedViewport('main'));
  const tools = useMemo(
    () => ({ store: wires, onEdit, editable: view.trusted, values: run.values, stale: (from: string) => valueStale(run, from) }),
    [wires, onEdit, view.trusted, run],
  );

  useEffect(() => setNodes((prev) => buildNodes(view, run, guis, prev)), [view, run, guis]);
  useEffect(() => setEdges((prev) => buildEdges(view, prev)), [view]);
  useEffect(() => {
    if (!focus) return;
    setNodes((list) => list.map((x) => (x.selected === (x.id === focus.id) ? x : { ...x, selected: x.id === focus.id })));
    setEdges((list) => list.map((x) => (x.selected ? { ...x, selected: false } : x)));
  }, [focus]);

  const onNodesChange = useCallback((changes: NodeChange<PatinaNode>[]) => setNodes((n) => applyNodeChanges(changes, n)), []);
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
    (_: unknown, __: PatinaNode, dragged: PatinaNode[]) => {
      const moves: Edit[] = dragged.map((n) => ({ kind: 'MoveNode', id: n.id, pos: [Math.round(n.position.x), Math.round(n.position.y)] }));
      if (moves.length > 0) onEdit(moves.length === 1 ? moves[0] : { kind: 'Group', edits: moves });
    },
    [onEdit],
  );

  const onConnect = useCallback(
    (c: Connection) => onEdit({ kind: 'Connect', from: `${c.source}.${c.sourceHandle}`, to: `${c.target}.${c.targetHandle}` }),
    [onEdit],
  );

  // A drag from a pin that ends on an empty area opens the palette at that point (15.2).
  const onConnectEnd = useCallback(
    (e: globalThis.MouseEvent | TouchEvent, c: FinalConnectionState) => {
      const from = c.fromHandle;
      if (c.isValid || !from?.id || !(e.target instanceof Element) || !e.target.classList.contains('react-flow__pane')) return;
      const pin = `${from.nodeId}.${from.id}`;
      const output = from.type === 'source';
      const entry = view.graph.node?.find((n) => n.id === from.nodeId);
      const ty = output ? outputType(view, pin) : entry && infoOf(entry, view.nodes)?.inputs.find((p) => p.key === from.id)?.ty;
      const point = 'changedTouches' in e ? e.changedTouches[0] : e;
      if (ty !== undefined) setPalette({ x: point.clientX, y: point.clientY, end: { pin, output, ty } });
    },
    [view],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (typing(e.target) || e.ctrlKey || e.altKey || e.metaKey || document.querySelector('.backdrop, .menu-layer')) return;
      if (e.key === 'Delete' && view.trusted) {
        const removed = new Set(nodes.filter((n) => n.selected).map((n) => n.id));
        const edits: Edit[] = [...removed].map((id) => ({ kind: 'RemoveNode', id }));
        for (const edge of edges) {
          if (edge.selected && !removed.has(edge.source) && !removed.has(edge.target)) {
            edits.push({ kind: 'Disconnect', from: edge.data!.from, to: edge.data!.to });
          }
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
  }, [nodes, edges, view.trusted, onEdit, flow]);

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
  const end = palette?.end;
  const choices = end ? view.nodes.filter((n) => matchPin(n, end.output, end.ty) !== undefined) : view.nodes;

  const onDrop = (e: DragEvent) => {
    const folder = e.dataTransfer.getData(NODE_MIME);
    if (!folder || !view.trusted) return;
    e.preventDefault();
    addNode(folder, e.clientX, e.clientY);
  };

  const onDoubleClick = (e: MouseEvent) => {
    if (view.trusted && e.target instanceof Element && e.target.classList.contains('react-flow__pane')) setPalette({ x: e.clientX, y: e.clientY });
  };

  return (
    <WireContext.Provider value={tools}>
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
        onMoveEnd={(_, v) => saveViewport('main', v)}
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
          onAdd={(n) => addNode(n.builtin ? n.name : `nodes/${n.name}`, palette.x, palette.y, end, n)}
          onClose={() => setPalette(null)}
        />
      )}
    </WireContext.Provider>
  );
}

/** Fits the graph in the canvas. */
export function useFit() {
  const flow = useReactFlow();
  return useCallback(() => flow.fitView(FIT), [flow]);
}
