// The commands of the canvas keys and of the right-click menu (15.2). They act on the selection.
import { useMemo, type Dispatch, type RefObject, type SetStateAction } from 'react';
import type { Edge, ReactFlowInstance } from '@xyflow/react';
import type { DragLink, Edit, LibraryNode, View } from '../api';
import { kindOf, wireBits, wireWidth } from './bus';
import type { PatinaNode, RunState } from './NodeView';
import type { WireData } from '../wires/WireLayer';
import { arrangeEdits, disconnectEdit } from './graphEdits';
import { portKey, type GraphNode } from './graphData';

export const FIT = { padding: 0.2, maxZoom: 1, duration: 0 };

export type At = { x: number; y: number };

export function useCanvasActions(p: {
  view: View;
  run: RunState;
  nodes: GraphNode[];
  edges: Edge<WireData>[];
  setNodes: Dispatch<SetStateAction<GraphNode[]>>;
  setEdges: Dispatch<SetStateAction<Edge<WireData>[]>>;
  flow: ReactFlowInstance<GraphNode, Edge<WireData>>;
  pointer: RefObject<At | null>;
  edit: (edit: Edit) => Promise<void>;
  onCollapse: (ids: string[], at: At) => void;
  onExpand: (ids: string[]) => void;
  onCopy: (ids: string[]) => Promise<boolean>;
  onPaste: (at: [number, number], library?: LibraryNode, link?: DragLink) => void;
}) {
  const { view, run, nodes, edges, setNodes, setEdges, flow, pointer, edit, onCollapse, onExpand, onCopy, onPaste } = p;
  return useMemo(() => {
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
      middle: (): At => {
        const r = document.querySelector('.react-flow')!.getBoundingClientRect();
        return pointer.current ?? { x: r.left + r.width / 2, y: r.top + r.height / 2 };
      },
      collapse: (at: At) => onCollapse(patina.map((n) => n.id), at),
      expand: () => onExpand(uses),
      copy: () => copied.length > 0 && onCopy(copied),
      // A cut removes only the copied nodes. A public pin does not go in a clip.
      cut: () => copied.length > 0 && onCopy(copied).then((ok) => void (ok && group(copied.map((id): Edit => ({ kind: 'RemoveNode', id }))))),
      paste: (at: At, library?: LibraryNode, link?: DragLink) => {
        const p = flow.screenToFlowPosition(at);
        onPaste([Math.round(p.x), Math.round(p.y)], library, link);
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
      /** Selects only the node or the connection, as a click does. */
      only: (id: string, node: boolean) => {
        setNodes((list) => list.map((x) => (!!x.selected === (node && x.id === id) ? x : { ...x, selected: node && x.id === id })));
        setEdges((list) => list.map((x) => (!!x.selected === (!node && x.id === id) ? x : { ...x, selected: !node && x.id === id })));
      },
    };
  }, [nodes, edges, setNodes, setEdges, edit, onCollapse, onExpand, onCopy, onPaste, flow, pointer, run.values, view.kinds]);
}

export type CanvasActions = ReturnType<typeof useCanvasActions>;
