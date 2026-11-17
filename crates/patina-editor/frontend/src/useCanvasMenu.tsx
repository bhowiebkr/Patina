// The right-click menu of the canvas (15.2): the empty canvas, the selected nodes or one connection.
import { useState } from 'react';
import type { Edge } from '@xyflow/react';
import type { Edit } from './api';
import { Menu, type Item } from './Dock';
import { canvasMenu, type MenuAction, type MenuTarget } from './canvasMenu';
import type { At, CanvasActions } from './canvasActions';
import type { WireData } from './WireLayer';
import { disconnectEdit } from './graphEdits';
import { portKey, type GraphNode } from './graphData';

type On = 'pane' | 'nodes' | Edge<WireData>;
type Click = { preventDefault(): void; clientX: number; clientY: number };

export function useCanvasMenu(p: {
  act: CanvasActions;
  trusted: boolean;
  edit: (edit: Edit) => Promise<void>;
  onOpen: (name: string, use?: string) => void;
  onAdd: (at: At) => void;
  onRename: (key: string, name: string, at: At) => void;
}) {
  const { act, trusted, edit, onOpen, onAdd, onRename } = p;
  const [menu, setMenu] = useState<{ x: number; y: number; on: On } | null>(null);
  const open = (e: Click, on: On) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, on });
  };
  const target = (on: On): MenuTarget => {
    if (on === 'pane') return { kind: 'pane' };
    if (on !== 'nodes') return { kind: 'wire' };
    const one = act.selected.length === 1 ? act.selected[0] : undefined;
    const use = one?.type === 'patina' && !!one.data.entry.subgraph && !one.data.inner;
    return { kind: 'nodes', count: act.selected.length, nodes: act.patina.length, uses: act.uses.length, open: use, pin: one?.type === 'port' };
  };
  const run = (a: MenuAction, at: At, on: On) => {
    const one = act.selected[0];
    if (a === 'add') onAdd(at);
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
    else if (a === 'rename' && one?.type === 'port') onRename(portKey(one.id)!, one.data.name, at);
    else if (a === 'disconnect' && typeof on === 'object') edit(disconnectEdit(on));
  };
  const items: Item[] = menu ? canvasMenu(target(menu.on), trusted).map((e) => (e === 'line' ? e : { label: e.label, keys: e.keys, run: () => run(e.action, menu, menu.on) })) : [];
  return {
    onPane: (e: Click) => open(e, 'pane'),
    onSelection: (e: Click) => open(e, 'nodes'),
    // A right-click out of the selection selects only the node or the connection, as a click does.
    onNode: (e: Click, n: GraphNode) => {
      if (!n.selected) act.only(n.id, true);
      open(e, 'nodes');
    },
    onEdge: (e: Click, edge: Edge<WireData>) => {
      if (!edge.selected) act.only(edge.id, false);
      open(e, edge);
    },
    element: menu && items.length > 0 && (
      <Menu x={menu.x} y={menu.y} label={menu.on === 'pane' ? 'Canvas' : menu.on === 'nodes' ? 'Node' : 'Connection'} items={items} onClose={() => setMenu(null)} />
    ),
  };
}
