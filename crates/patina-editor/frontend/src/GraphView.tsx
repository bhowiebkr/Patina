// The graph canvas (spec 15.2). Navigation is snappy: no animation between two views.

import { useCallback, useEffect, useMemo, useState, type DragEvent } from 'react';
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
  type NodeChange,
  type OnSelectionChangeParams,
} from '@xyflow/react';
import { newId, type Edit, type Point, type View } from './api';
import { NodeView, infoOf, nodeState, type PatinaNode, type RunState } from './NodeView';
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
function buildNodes(view: View, run: RunState, prev: PatinaNode[]): PatinaNode[] {
  const old = new Map(prev.map((n) => [n.id, n]));
  const wires = view.graph.wire ?? [];
  return (view.graph.node ?? []).map((entry) => {
    const info = infoOf(entry, view.nodes);
    const [state, message] = nodeState(entry, info, view, run);
    const wired = wires.map((w) => splitPin(w.to)).filter(([id]) => id === entry.id).map(([, key]) => key);
    const position = { x: entry.pos?.[0] ?? 0, y: entry.pos?.[1] ?? 0 };
    const data = { entry, info, wired, state, message, editable: view.trusted };
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
    return { id, type: 'wire', source, sourceHandle, target, targetHandle, selected: selected.has(id), data: { from: w.from, to: w.to, route: w.points ?? NO_POINTS } };
  });
}

function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
}

interface Props {
  view: View;
  run: RunState;
  onEdit: (edit: Edit) => Promise<void>;
  onSelect: (ids: string[], wires: WireData[]) => void;
  /** A request to select one node, for example from a click on a compile error. */
  focus: { id: string; n: number } | null;
}

export function GraphView({ view, run, onEdit, onSelect, focus }: Props) {
  const flow = useReactFlow<PatinaNode, Edge<WireData>>();
  const [nodes, setNodes] = useState<PatinaNode[]>([]);
  const [edges, setEdges] = useState<Edge<WireData>[]>([]);
  const [wires] = useState(() => new WireStore());
  const [glOk, setGlOk] = useState(true);
  const tools = useMemo(
    () => ({ store: wires, onEdit, editable: view.trusted, values: run.values, stale: run.stale }),
    [wires, onEdit, view.trusted, run.values, run.stale],
  );

  useEffect(() => setNodes((prev) => buildNodes(view, run, prev)), [view, run]);
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

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (typing(e.target) || e.ctrlKey || e.altKey || e.metaKey || document.querySelector('.backdrop')) return;
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

  const onDrop = (e: DragEvent) => {
    const folder = e.dataTransfer.getData(NODE_MIME);
    if (!folder || !view.trusted) return;
    e.preventDefault();
    const p = flow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    onEdit({ kind: 'AddNode', id: newId(), folder, pos: [Math.round(p.x), Math.round(p.y)] });
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
        onDragOver={onDragOver}
        onDrop={onDrop}
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
        fitView
        fitViewOptions={FIT}
        colorMode="dark"
      >
        <Background id="minor" variant={BackgroundVariant.Lines} gap={20} color="#202020" />
        <Background id="major" variant={BackgroundVariant.Lines} gap={100} color="#292929" bgColor="transparent" />
        <WireLayer store={wires} onReady={setGlOk} />
      </ReactFlow>
    </WireContext.Provider>
  );
}

/** Fits the graph in the canvas. */
export function useFit() {
  const flow = useReactFlow();
  return useCallback(() => flow.fitView(FIT), [flow]);
}
