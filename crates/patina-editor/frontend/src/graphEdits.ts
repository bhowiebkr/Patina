// The edits that the canvas makes from the drags, the drops and the commands (15.2, 15.9).
import type { Connection, Edge, ReactFlowInstance } from '@xyflow/react';
import { arrange, type Box, type Link } from './arrange';
import type { Edit, NodeInfo, Point, View } from './api';
import { infoOf, ORDER, outputType } from './NodeView';
import { socketKey } from './palette';
import type { WireData } from './WireLayer';
import { portKey, portTypes, type GraphNode } from './graphData';

/** What an AddNode edit names: a built-in node, a subgraph or a node folder. */
export function nodeSource(n: NodeInfo): string {
  return n.builtin ? n.name : `${n.subgraph ? 'graphs' : 'nodes'}/${n.name}`;
}

/** The entries of the Add node palette that add a public pin to an open subgraph (6.4). */
const PIN_ENTRY = { inputs: [], outputs: [], error: null, builtin: false, generics: ['?'], full: false, color: null, subgraph: false, kind: 'generic' as const };
export const PUBLIC_INPUT: NodeInfo = { ...PIN_ENTRY, name: 'Public input', outputs: [{ key: 'pin', ty: '?' }] };
export const PUBLIC_OUTPUT: NodeInfo = { ...PIN_ENTRY, name: 'Public output', inputs: [{ key: 'pin', ty: '?' }] };

/** A wire that a drag from a pin into an empty area started: the pin, its direction and its type. */
export interface FreeEnd {
  pin: string;
  output: boolean;
  ty: string;
}

/** The edit that moves a node or a public pin to a graph position. */
export function moveEdit(id: string, pos: Point): Edit {
  const key = portKey(id);
  return key ? { kind: 'MovePin', key, pos } : { kind: 'MoveNode', id, pos };
}

/** The moves of the Arrange command (15.9): the selected nodes, or all nodes when fewer than two are selected. */
export function arrangeEdits(
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
    return [moveEdit(n.id, pos)];
  });
}

/** The edit for a wire between two sockets, or null for a wire between two public pins (6.4). */
export function connectEdit(c: Connection): Edit | null {
  const [from, to, inKey, outKey] = [`${c.source}.${c.sourceHandle}`, `${c.target}.${c.targetHandle}`, portKey(c.source), portKey(c.target)];
  if (inKey && outKey) return null;
  if (c.sourceHandle === ORDER) return { kind: 'ConnectOrder', before: c.source, after: c.target };
  if (inKey) return { kind: 'ConnectPin', key: inKey, end: to };
  if (outKey) return { kind: 'ConnectPin', key: outKey, end: from };
  return { kind: 'Connect', from, to };
}

/** The edit that removes a wire. */
export function disconnectEdit(edge: Edge<WireData>): Edit {
  if (edge.type === 'order') return { kind: 'DisconnectOrder', before: edge.source, after: edge.target };
  const { from, to } = edge.data!;
  const [inKey, outKey] = [portKey(edge.source), portKey(edge.target)];
  if (inKey) return { kind: 'DisconnectPin', key: inKey, end: to };
  if (outKey) return { kind: 'DisconnectPin', key: outKey, end: from };
  return { kind: 'Disconnect', from, to };
}

/** The type key of the socket `key` of `node` for the drag highlight (15.9): 'order', a type key or '*'. */
export function dragKey(view: View, node: string, key: string, output: boolean): string {
  if (key === ORDER) return 'order';
  if (portKey(node)) return socketKey(portTypes(view).get(node) ?? '?', ['?']);
  const entry = view.graph.node?.find((n) => n.id === node);
  const info = entry && infoOf(entry, view.nodes);
  const ty = output ? outputType(view, `${node}.${key}`) : info?.inputs.find((p) => p.key === key)?.ty;
  return ty === undefined ? '*' : socketKey(ty, info?.generics ?? []);
}

/** An order socket connects only to an order socket (9.2). */
export function orderMatch(c: Edge | Connection): boolean {
  return (c.sourceHandle === ORDER) === (c.targetHandle === ORDER);
}
