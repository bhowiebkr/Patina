// The graph canvas (spec 15.2). Navigation is snappy: no animation between two views.

import { useCallback, useEffect, useMemo, useRef, useState, type MutableRefObject, type ReactNode } from 'react';
import {
  Background,
  BackgroundVariant,
  ReactFlow,
  applyEdgeChanges,
  applyNodeChanges,
  useConnection,
  useReactFlow,
  type Connection,
  type Edge,
  type EdgeChange,
  type NodeChange,
  type OnSelectionChangeParams,
  type Viewport,
} from '@xyflow/react';
import { NameField } from '../controls/NameField';
import type { Domain, Edit, LibraryNode, DragLink, View } from '../api';
import { HoverContext, HoverStore, PITCH } from './bus';
import { savedViewport, saveViewport } from '../panels/Dock';
import { FrameToggle, gutter, NodeEdit, NodeView, PortView, type RunState } from './NodeView';
import { graphEdit } from './frames';
import { dragLanes, dragStyle } from './palette';
import { OrderEdge, WireEdge } from '../wires/WireEdge';
import { WireContext, WireLayer, type WireData } from '../wires/WireLayer';
import { Labels } from '../wires/labels';
import { Router } from '../wires/router';
import { lockPoints, WireStore } from '../wires/wires';
import { useTouchpadPan } from './touchpad';
import { useCanvasKeys } from './keys';
import { buildEdges, buildNodes, portKey, type GraphNode } from './graphData';
import { withValues } from './liveValues';
import { FIT, useCanvasActions, type At } from './canvasActions';
import { useFrames, useObstacles, useVisibleGuis } from './canvasLayout';
import { connectEdit, disconnectEdit, dragKey, moveEdit, orderMatch } from './graphEdits';
import { useAddNode } from './useAddNode';
import { useCanvasMenu } from './useCanvasMenu';

const NODE_TYPES = { patina: NodeView, port: PortView };
const EDGE_TYPES = { wire: WireEdge, order: OrderEdge };
const PAN_BUTTONS = [1];
const MULTI_KEYS = ['Shift', 'Control'];
const NO_IDS: ReadonlySet<string> = new Set();

/** A dialog at a screen point that asks for a name. */
function NameDialog({ label, at, children }: { label: string; at: At; children: ReactNode }) {
  return (
    <div className="menu-layer">
      <div className="menu collapse-name" role="dialog" aria-label={label} style={{ left: Math.min(at.x, innerWidth - 250), top: Math.min(at.y, innerHeight - 80) }}>
        {children}
      </div>
    </div>
  );
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
  /** The nodes to select, for example from a click on a compile error or from a view (15.8). */
  focus: { ids: string[]; n: number } | null;
  /** Opens a graph. `use` is the ID of the use that a double-click opened (6.1). */
  onOpen: (name: string, use?: string) => void;
  /** Moves the nodes into a new subgraph. It fails with the problem. */
  onCollapse: (name: string, ids: string[]) => Promise<void>;
  /** Puts the nodes of each subgraph use in its place. */
  onExpand: (ids: string[]) => void;
  /** Copies the nodes to the clipboard, and is true when it did (15.11). */
  onCopy: (ids: string[]) => Promise<boolean>;
  /** Pastes the clipboard, or adds a library node with the wire of a pin drag, at a graph position (15.11, 6.2). */
  onPaste: (at: [number, number], library?: LibraryNode, link?: DragLink) => void;
  /** Gets the function of the Lock bends button of the Inspector for the wire to an input (15.9). */
  lockRef: MutableRefObject<(to: string) => void>;
  /** Gets the function of the **Add to graph** button of the Library panel (15.2). */
  addRef: MutableRefObject<(n: LibraryNode) => void>;
}

export function GraphView({ view, run, domains, guis, onEdit, onSelect, focus, onOpen, onCollapse, onExpand, onCopy, onPaste, lockRef, addRef }: Props) {
  const flow = useReactFlow<GraphNode, Edge<WireData>>();
  const [nodes, setNodes] = useState<GraphNode[]>([]);
  const [edges, setEdges] = useState<Edge<WireData>[]>([]);
  const [wires] = useState(() => new WireStore());
  const [router] = useState(() => new Router());
  const [labels] = useState(() => new Labels());
  // A badge hides on a junction dot, and a label hides where another wire goes through it (15.9).
  useEffect(() => {
    const update = () => {
      labels.setDots(router.allJunctions());
      labels.setLines(router.allLines());
    };
    update();
    return router.subscribe(update);
  }, [router, labels]);
  const [hover] = useState(() => new HoverStore());
  const [glOk, setGlOk] = useState(true);
  useVisibleGuis();
  const { frames, rooms, toggle } = useFrames(view, nodes);
  // The view moves the nodes around the open frames, so each edit maps its points back to the graph.
  const edit = useCallback((e: Edit) => onEdit(graphEdit(e, rooms)), [onEdit, rooms]);
  // Lock bends saves the bends of the automatic route as its route points (15.9).
  useEffect(() => {
    lockRef.current = (to) => {
      const id = edges.find((e) => e.type === 'wire' && e.data?.to === to)?.id;
      const bends = id === undefined ? undefined : router.bends(id);
      if (bends?.length) void edit({ kind: 'SetRoute', to, points: lockPoints(bends) });
    };
  }, [lockRef, edges, router, edit]);
  // The socket of a wire drag, as a string so that a pointer move does not render the graph again.
  const from = useConnection((c) => (c.inProgress ? JSON.stringify([c.fromHandle.type, c.fromHandle.nodeId, c.fromHandle.id ?? '']) : ''));
  const dragCss = useMemo(() => {
    if (!from) return '';
    const [side, node, key] = JSON.parse(from) as ['source' | 'target', string, string];
    return dragStyle(side, dragKey(view, node, key, side === 'source'));
  }, [from, view]);
  // A drag from a bus socket draws a ribbon as wide as the wire, never a thin line (15.9).
  const dragLine = useMemo(() => {
    if (!from) return undefined;
    const [side, node, key] = JSON.parse(from) as ['source' | 'target', string, string];
    const n = dragLanes(flow.getNode(node), side, key);
    return n > 1 ? { strokeWidth: n * PITCH } : undefined;
  }, [from, flow]);
  const [collapsing, setCollapsing] = useState<{ ids: string[]; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<{ key: string; name: string; x: number; y: number } | null>(null);
  // The last pointer point over the canvas.
  const pointer = useRef<At | null>(null);
  // The saved pan and zoom of the graph (15.2), or a fit when there is none.
  const [viewport] = useState(() => savedViewport(view.graphName));
  const pane = useRef<HTMLDivElement>(null);
  const save = useCallback((v: Viewport) => saveViewport(view.graphName, v), [view.graphName]);
  const scrolling = useTouchpadPan(pane, flow, save);
  const tools = useMemo(
    () => ({ store: wires, router, labels, onEdit: edit, editable: view.trusted, kinds: view.kinds }),
    [wires, router, labels, edit, view.trusted, view.kinds],
  );
  useObstacles(nodes, router, labels);

  // The nodes of the last drag keep their place until the view of their move comes back.
  const moved = useRef<{ view: View; ids: ReadonlySet<string> } | null>(null);
  useEffect(() => {
    const held = moved.current?.view === view ? moved.current.ids : NO_IDS;
    setNodes((prev) => buildNodes(view, run, guis, domains, frames, rooms, prev, held));
  }, [view, run, guis, domains, frames, rooms]);
  useEffect(() => setEdges((prev) => buildEdges(view, domains, frames, rooms, prev)), [view, domains, frames, rooms]);
  // After the effect above, so a new build of the edges gets the values too.
  useEffect(() => setEdges((prev) => withValues(prev, run)), [view, domains, frames, rooms, run]);
  useEffect(() => {
    if (!focus) return;
    const ids = new Set(focus.ids);
    setNodes((list) => list.map((x) => (x.selected === ids.has(x.id) ? x : { ...x, selected: ids.has(x.id) })));
    setEdges((list) => list.map((x) => (x.selected ? { ...x, selected: false } : x)));
  }, [focus]);

  // With no saved pan and zoom, the graph fits again when a node GUI gets its size, until the user acts on the canvas (15.2).
  const autoFit = useRef(!viewport);
  const stopFit = useCallback(() => (autoFit.current = false), []);
  const onNodesChange = useCallback(
    (changes: NodeChange<GraphNode>[]) => {
      setNodes((n) => applyNodeChanges(changes, n));
      if (autoFit.current && changes.some((c) => c.type === 'dimensions')) requestAnimationFrame(() => autoFit.current && flow.fitView(FIT));
    },
    [flow],
  );
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
      const moves = dragged.map((n) => moveEdit(n.id, [Math.round(n.position.x + gutter(n)), Math.round(n.position.y)]));
      if (moves.length === 0) return;
      const hold = { view, ids: new Set(dragged.map((n) => n.id)) };
      moved.current = hold;
      // The edit shows its view before it ends. After a failed move, the next build puts the nodes back.
      void edit(moves.length === 1 ? moves[0] : { kind: 'Group', edits: moves }).then(() => {
        if (moved.current === hold) moved.current = null;
      });
    },
    [edit, view],
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
      // The look comes from the file, because the drawn path has the graph default in it.
      const look = view.graph.wire?.find((w) => w.to === d?.to);
      if (add.kind === 'Connect' && d) {
        if (look?.style || look?.corners) edits.push({ kind: 'SetLook', to: add.to, style: look.style, corners: look.corners });
        if (d.name) edits.push({ kind: 'NameWire', to: add.to, name: d.name });
      }
      edit({ kind: 'Group', edits });
    },
    [edit, view.graph],
  );

  const collapse = useCallback((ids: string[], at: At) => setCollapsing({ ids, ...at }), []);
  const act = useCanvasActions({ view, run, nodes, edges, setNodes, setEdges, flow, pointer, edit, onCollapse: collapse, onExpand, onCopy, onPaste });
  const adding = useAddNode({ view, flow, act, edit });
  useEffect(() => {
    addRef.current = (n) => act.paste(act.middle(), n);
  }, [addRef, act]);
  useCanvasKeys(act, view.trusted, pointer, adding.open);
  const menu = useCanvasMenu({ act, trusted: view.trusted, edit, onOpen, onAdd: adding.open, onRename: (key, name, at) => setRenaming({ key, name, ...at }) });

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
          connectionLineStyle={dragLine}
          onConnectEnd={adding.onConnectEnd}
          onDragOver={adding.onDragOver}
          onDrop={adding.onDrop}
          onDoubleClick={adding.onDoubleClick}
          onNodeDoubleClick={(e, n) => {
            if (n.type === 'patina' && n.data.entry.subgraph && !n.data.inner) onOpen(n.data.entry.subgraph, n.id);
            else if (n.type === 'port' && view.trusted) setRenaming({ key: portKey(n.id)!, name: n.data.name, x: e.clientX, y: e.clientY });
          }}
          onPaneContextMenu={menu.onPane}
          onSelectionContextMenu={menu.onSelection}
          onNodeContextMenu={menu.onNode}
          onEdgeContextMenu={menu.onEdge}
          onPointerMove={(e) => (pointer.current = { x: e.clientX, y: e.clientY })}
          onPointerLeave={() => (pointer.current = null)}
          onPointerDownCapture={stopFit}
          onKeyDownCapture={stopFit}
          onMoveStart={(e) => e && stopFit()}
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
        {adding.element}
        {menu.element}
        {collapsing && (
          <NameDialog label="Collapse into a subgraph" at={collapsing}>
            <NameField label="Subgraph name" onMake={(name) => onCollapse(name, collapsing.ids)} onClose={() => setCollapsing(null)} />
          </NameDialog>
        )}
        {renaming && (
          <NameDialog label="Rename the public pin" at={renaming}>
            <NameField label="Pin name" initial={renaming.name} onMake={(name) => edit({ kind: 'RenamePin', key: renaming.key, name })} onClose={() => setRenaming(null)} />
          </NameDialog>
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
