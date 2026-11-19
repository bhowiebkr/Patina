// The commands of the canvas keys and of the right-click menu (15.2). They act on the selection.
import { useMemo, type Dispatch, type RefObject, type SetStateAction } from 'react';
import type { Edge, ReactFlowInstance } from '@xyflow/react';
import { newId, type DragLink, type Edit, type Group, type GroupColor, type LibraryNode, type Note, type Point, type View } from '../api';
import { kindOf, wireBits, wireWidth } from './bus';
import type { PatinaNode, RunState } from './NodeView';
import type { WireData } from '../wires/WireLayer';
import { arrangeEdits, bodyBox, disconnectEdit } from './graphEdits';
import { portKey, type GraphNode } from './graphData';
import { groupAround, itemEdit, itemOf, NEW_GROUP, NOTE_WIDTH, type GroupBoxNode, type ItemNode } from './groups';

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
  /** Sends an edit, and the nodes `ids` keep their place in the view until it ends. */
  send: (edit: Edit, ids: Iterable<string>) => void;
  setEditing: (id: string | null) => void;
  onCollapse: (ids: string[], at: At) => void;
  onExpand: (ids: string[]) => void;
  onCopy: (ids: string[]) => Promise<boolean>;
  onPaste: (at: [number, number], library?: LibraryNode, link?: DragLink) => void;
}) {
  const { view, run, nodes, edges, setNodes, setEdges, flow, pointer, edit, send, setEditing, onCollapse, onExpand, onCopy, onPaste } = p;
  return useMemo(() => {
    const selected = nodes.filter((n) => n.selected);
    const patina = selected.filter((n): n is PatinaNode => n.type === 'patina');
    const uses = patina.filter((n) => n.data.entry.subgraph).map((n) => n.id);
    const items = selected.filter((n): n is ItemNode => n.type === 'groupBox' || n.type === 'note');
    const groups = items.filter((n): n is GroupBoxNode => n.type === 'groupBox');
    // A public pin does not go in a clip. A clip names a group or a note by its file ID.
    const clipped: GraphNode[] = [...patina.filter((n) => !n.data.inner), ...items];
    const copied = clipped.map((n) => itemOf(n.id)?.id ?? n.id);
    const removeEdit = (id: string): Edit => {
      const item = itemOf(id);
      if (item) return item.kind === 'group' ? { kind: 'RemoveGroup', id: item.id } : { kind: 'RemoveNote', id: item.id };
      const key = portKey(id);
      return key ? { kind: 'RemovePin', key } : { kind: 'RemoveNode', id };
    };
    const group = (edits: Edit[]) => edits.length > 0 && edit(edits.length === 1 ? edits[0] : { kind: 'Group', edits });
    const at = (s: At): Point => {
      const p = flow.screenToFlowPosition(s);
      return [Math.round(p.x), Math.round(p.y)];
    };
    const select = (all: boolean) => {
      setNodes((list) => list.map((x) => (!!x.selected === all ? x : { ...x, selected: all })));
      setEdges((list) => list.map((x) => (!!x.selected === all ? x : { ...x, selected: all })));
    };
    return {
      selected,
      patina,
      uses,
      items,
      groups,
      /** The pointer, or the middle of the canvas when the pointer is not on it. */
      middle: (): At => {
        const r = document.querySelector('.react-flow')!.getBoundingClientRect();
        return pointer.current ?? { x: r.left + r.width / 2, y: r.top + r.height / 2 };
      },
      collapse: (at: At) => onCollapse(patina.map((n) => n.id), at),
      expand: () => onExpand(uses),
      copy: () => copied.length > 0 && onCopy(copied),
      // A cut removes only the copied nodes. A public pin does not go in a clip.
      cut: () => copied.length > 0 && onCopy(copied).then((ok) => void (ok && group(clipped.map((n) => removeEdit(n.id))))),
      paste: (s: At, library?: LibraryNode, link?: DragLink) => onPaste(at(s), library, link),
      remove: () => {
        const removed = new Set(selected.map((n) => n.id));
        const edits = [...removed].map(removeEdit);
        for (const edge of edges) {
          if (edge.selected && !removed.has(edge.source) && !removed.has(edge.target)) edits.push(disconnectEdit(edge));
        }
        group(edits);
      },
      fit: () => flow.fitView(selected.length > 0 ? { ...FIT, nodes: selected } : FIT),
      arrange: () => group(arrangeEdits(nodes, edges, flow, (d) => wireWidth(wireBits(run.values.get(d.from), kindOf(view.kinds, d.ty))))),
      /** Adds a group around the selected nodes when `around` is true, else at the screen point (15.12). */
      addGroup: (s: At, around: boolean) => {
        const top = selected.filter((n) => !n.parentId);
        const place = around && top.length > 0 ? groupAround(top.map(bodyBox)) : { pos: at(s), size: NEW_GROUP };
        void edit({ kind: 'SetGroup', group: { id: newId(), title: 'Group', color: 'amber', ...place } });
      },
      /** Adds an empty note at the screen point, and opens its text. */
      addNote: (s: At) => {
        const id = newId();
        void edit({ kind: 'SetNote', note: { id, pos: at(s), width: NOTE_WIDTH, text: '' } });
        setEditing(`note:${id}`);
      },
      setColor: (color: GroupColor) => group(groups.map((n) => itemEdit(n, { color }))),
      /** Opens the title of the selected group or the text of the selected note. */
      rename: () => items.length === 1 && setEditing(items[0].id),
      /** Sets a part of a group or a note. A place or a size is in the view. */
      change: (id: string, change: Partial<Group & Note>) => {
        const n = nodes.find((x) => x.id === id);
        if (n?.type === 'groupBox' || n?.type === 'note') send(itemEdit(n, change), [id]);
      },
      selectAll: () => select(true),
      clear: () => select(false),
      /** Selects only the node or the connection, as a click does. */
      only: (id: string, node: boolean) => {
        setNodes((list) => list.map((x) => (!!x.selected === (node && x.id === id) ? x : { ...x, selected: node && x.id === id })));
        setEdges((list) => list.map((x) => (!!x.selected === (!node && x.id === id) ? x : { ...x, selected: !node && x.id === id })));
      },
    };
  }, [nodes, edges, setNodes, setEdges, edit, send, setEditing, onCollapse, onExpand, onCopy, onPaste, flow, pointer, run.values, view.kinds]);
}

export type CanvasActions = ReturnType<typeof useCanvasActions>;
