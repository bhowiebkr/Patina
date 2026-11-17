// The Add node palette and the drops of the canvas: a node from the palette, the library or a drag (15.2, 6.2).
import { useCallback, useEffect, useState, type DragEvent, type MouseEvent } from 'react';
import type { Edge, FinalConnectionState, ReactFlowInstance } from '@xyflow/react';
import { AddNode } from './AddNode';
import { api, newId, type DragLink, type Edit, type LibraryNode, type NodeInfo, type View } from './api';
import type { At, CanvasActions } from './canvasActions';
import { infoOf, ORDER, outputType } from './NodeView';
import { matchPin } from './palette';
import type { WireData } from './WireLayer';
import { nodeSource, PUBLIC_INPUT, PUBLIC_OUTPUT, type FreeEnd } from './graphEdits';
import { portKey, splitPin, type GraphNode } from './graphData';

export const NODE_MIME = 'application/patina-node';

export function useAddNode(p: { view: View; flow: ReactFlowInstance<GraphNode, Edge<WireData>>; act: CanvasActions; edit: (edit: Edit) => Promise<void> }) {
  const { view, flow, act, edit } = p;
  // The screen point of the open Add node palette, with the pin of a wire that it ends.
  const [palette, setPalette] = useState<(At & { end?: FreeEnd }) | null>(null);
  // The palette also shows the nodes of the libraries (6.2). They can change, so they load at each open.
  const [library, setLibrary] = useState<LibraryNode[]>([]);
  const paletteOpen = !!palette;
  useEffect(() => {
    if (paletteOpen) api.libraryNodes().then(setLibrary, () => setLibrary([]));
  }, [paletteOpen]);

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
  const choices = [...all, ...library].filter((n) => !end || matchPin(n, end.output, end.ty) !== undefined);
  /** The wire from the drag end to a new library node, or undefined without a drag. */
  const link = (n: LibraryNode): DragLink | undefined => end && { end: end.pin, output: end.output, key: matchPin(n, end.output, end.ty)! };

  return {
    open: setPalette,
    onConnectEnd,
    onDragOver: (e: DragEvent) => {
      if (e.dataTransfer.types.includes(NODE_MIME) && view.trusted) {
        e.preventDefault();
        e.dataTransfer.dropEffect = 'move';
      }
    },
    onDrop: (e: DragEvent) => {
      const info = view.nodes.find((n) => n.name === e.dataTransfer.getData(NODE_MIME));
      if (!info || !view.trusted) return;
      e.preventDefault();
      addNode(nodeSource(info), e.clientX, e.clientY, undefined, info);
    },
    onDoubleClick: (e: MouseEvent) => {
      if (view.trusted && e.target instanceof Element && e.target.classList.contains('react-flow__pane')) setPalette({ x: e.clientX, y: e.clientY });
    },
    element: palette && (
      <AddNode
        x={palette.x}
        y={palette.y}
        nodes={choices}
        none={end && `No node has a pin of type ${end.ty}.`}
        onAdd={(n) =>
          'library' in n
            ? act.paste(palette, n as LibraryNode, link(n as LibraryNode))
            : n === PUBLIC_INPUT || n === PUBLIC_OUTPUT
            ? addPin(n === PUBLIC_INPUT, palette.x, palette.y, end)
            : addNode(nodeSource(n), palette.x, palette.y, end, n)
        }
        onClose={() => setPalette(null)}
      />
    ),
  };
}
