// The right-click menu of the canvas (15.2): the empty canvas, the selected nodes, groups or notes, or one connection.
import { useState } from 'react';
import type { Edge } from '@xyflow/react';
import type { Edit, GroupColor } from '../api';
import { Menu, type Item } from '../panels/Dock';
import { Confirm } from '../dialogs/Confirm';
import { canvasMenu, type MenuAction, type MenuTarget } from './canvasMenu';
import type { At, CanvasActions } from './canvasActions';
import type { WireData } from '../wires/WireLayer';
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
  // The menu asks before a delete. The Delete key does not, because undo gives the nodes back (15.2).
  const [ask, setAsk] = useState<{ count: number; kind: string } | null>(null);
  const open = (e: Click, on: On) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, on });
  };
  const target = (on: On): MenuTarget => {
    if (on === 'pane') return { kind: 'pane' };
    if (on !== 'nodes') return { kind: 'wire' };
    const count = act.selected.length;
    const colors = new Set(act.groups.map((n) => n.data.group.color));
    if (count > 0 && act.groups.length === count) return { kind: 'group', count, color: colors.size === 1 ? [...colors][0] : null };
    if (count > 0 && act.items.length === count && act.groups.length === 0) return { kind: 'note', count };
    const one = count === 1 ? act.selected[0] : undefined;
    const use = one?.type === 'patina' && !!one.data.entry.subgraph && !one.data.inner;
    return { kind: 'nodes', count, nodes: act.patina.length, uses: act.uses.length, open: use, pin: one?.type === 'port', items: act.items.length };
  };
  const run = (a: MenuAction, at: At, on: On, color?: GroupColor) => {
    const one = act.selected[0];
    if (a === 'add') onAdd(at);
    else if (a === 'addGroup') act.addGroup(at, false);
    else if (a === 'addNote') act.addNote(at);
    else if (a === 'group') act.addGroup(at, true);
    else if (a === 'edit') act.rename();
    else if (a === 'color' && color) act.setColor(color);
    else if (a === 'selectAll') act.selectAll();
    else if (a === 'fit') act.fit();
    else if (a === 'arrange') act.arrange();
    else if (a === 'collapse') act.collapse(at);
    else if (a === 'expand') act.expand();
    else if (a === 'delete') setAsk({ count: act.selected.length, kind: one?.type === 'groupBox' ? 'group' : one?.type === 'note' ? 'note' : 'node' });
    else if (a === 'copy') act.copy();
    else if (a === 'cut') act.cut();
    else if (a === 'paste') act.paste(at);
    else if (a === 'open' && one?.type === 'patina' && one.data.entry.subgraph) onOpen(one.data.entry.subgraph, one.id);
    else if (a === 'rename' && one?.type === 'port') onRename(portKey(one.id)!, one.data.name, at);
    else if (a === 'disconnect' && typeof on === 'object') edit(disconnectEdit(on));
  };
  const shown = menu ? target(menu.on) : null;
  const items: Item[] = menu && shown ? canvasMenu(shown, trusted).map((e) => (e === 'line' ? e : { label: e.label, keys: e.keys, checked: e.checked, run: () => run(e.action, menu, menu.on, e.color) })) : [];
  const label = shown?.kind === 'pane' ? 'Canvas' : shown?.kind === 'wire' ? 'Connection' : shown?.kind === 'group' ? 'Group' : shown?.kind === 'note' ? 'Note' : 'Node';
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
    element: (
      <>
        {menu && items.length > 0 && (
          <Menu x={menu.x} y={menu.y} label={label} items={items} onClose={() => setMenu(null)} />
        )}
        {ask !== null && (
          <Confirm
            title={ask.count === 1 ? `Delete the ${ask.kind}?` : `Delete ${ask.count} items?`}
            label="Undo (Ctrl+Z) gives it back."
            answers={[
              ['Delete', () => { setAsk(null); act.remove(); }],
              ['Cancel', () => setAsk(null)],
            ]}
            onCancel={() => setAsk(null)}
          />
        )}
      </>
    ),
  };
}
