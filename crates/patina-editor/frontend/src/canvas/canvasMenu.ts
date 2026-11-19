// The right-click menu of the graph canvas (15.2). The entries depend on what the click hits.
import type { GroupColor } from '../api';

/** The colors of a group, in the order of the menu (15.12). */
export const GROUP_COLORS: readonly GroupColor[] = ['gray', 'amber', 'green', 'teal', 'violet', 'pink', 'copper'];

/** What a right-click hits: the empty canvas, the selected nodes, groups or notes, or one connection. */
export type MenuTarget =
  | { kind: 'pane' }
  | {
      kind: 'nodes';
      /** The selected nodes, with the public pins. */
      count: number;
      /** The selected nodes that are not public pins. They can collapse into a subgraph. */
      nodes: number;
      /** The selected uses of subgraphs. */
      uses: number;
      /** One use of a subgraph is selected, and it can open. */
      open: boolean;
      /** One public pin is selected. */
      pin: boolean;
      /** The selected groups and notes. */
      items: number;
    }
  /** Only groups are selected. `color` is their color when they have the same one. */
  | { kind: 'group'; count: number; color: GroupColor | null }
  /** Only notes are selected. */
  | { kind: 'note'; count: number }
  | { kind: 'wire' };

export type MenuAction =
  | 'add' | 'addGroup' | 'addNote' | 'paste' | 'selectAll' | 'fit' | 'arrange' | 'open' | 'rename' | 'collapse' | 'group' | 'expand'
  | 'edit' | 'color' | 'copy' | 'cut' | 'delete' | 'disconnect';
export type MenuEntry = { label: string; keys?: string; action: MenuAction; color?: GroupColor; checked?: boolean } | 'line';

/** The menu entries for a target. An untrusted project gets only the entries that do not edit. */
export function canvasMenu(t: MenuTarget, trusted: boolean): MenuEntry[] {
  const list: (MenuEntry | false)[] = [];
  if (t.kind === 'pane') {
    list.push(
      trusted && { label: 'Add node...', keys: 'Shift+A', action: 'add' },
      trusted && { label: 'Add group', keys: 'Shift+G', action: 'addGroup' },
      trusted && { label: 'Add note', keys: 'Shift+N', action: 'addNote' },
      trusted && { label: 'Paste', keys: 'Ctrl+V', action: 'paste' },
      trusted && 'line',
      { label: 'Select all', keys: 'Ctrl+A', action: 'selectAll' },
      { label: 'Fit view', keys: 'F', action: 'fit' },
      trusted && { label: 'Arrange', keys: 'Shift+L', action: 'arrange' },
    );
  } else if (t.kind === 'nodes') {
    list.push(
      t.open && t.count === 1 && { label: 'Open subgraph', keys: 'Double-click', action: 'open' },
      trusted && t.pin && t.count === 1 && { label: 'Rename pin...', keys: 'Double-click', action: 'rename' },
      trusted && t.nodes > 0 && { label: 'Collapse into subgraph...', keys: 'Ctrl+G', action: 'collapse' },
      trusted && { label: 'Group selection', keys: 'Shift+G', action: 'group' },
      trusted && t.uses > 0 && { label: t.uses === 1 ? 'Expand subgraph' : 'Expand subgraphs', keys: 'Ctrl+Shift+G', action: 'expand' },
      trusted && t.count > 1 && { label: 'Arrange', keys: 'Shift+L', action: 'arrange' },
      { label: t.count === 1 ? 'Fit node' : 'Fit selection', keys: 'F', action: 'fit' },
      ...clip(t.nodes + t.items > 0, trusted),
      trusted && { label: t.count === 1 ? 'Delete node' : 'Delete selection', keys: 'Delete', action: 'delete' },
    );
  } else if (t.kind === 'group' || t.kind === 'note') {
    const one = t.count === 1;
    const name = t.kind === 'group' ? 'group' : 'note';
    list.push(
      trusted && one && { label: t.kind === 'group' ? 'Rename group...' : 'Edit note', keys: 'Double-click', action: 'edit' },
      trusted && one && 'line',
      ...(t.kind === 'group' && trusted ? GROUP_COLORS.map((c): MenuEntry => ({ label: c[0].toUpperCase() + c.slice(1), action: 'color', color: c, checked: t.color === c })) : []),
      t.kind === 'group' && trusted && 'line',
      { label: one ? `Fit ${name}` : 'Fit selection', keys: 'F', action: 'fit' },
      ...clip(true, trusted),
      trusted && { label: one ? `Delete ${name}` : 'Delete selection', keys: 'Delete', action: 'delete' },
    );
  } else if (trusted) {
    list.push({ label: 'Delete connection', keys: 'Delete', action: 'disconnect' });
  }
  return list.filter((e): e is MenuEntry => e !== false);
}

/** Copy, Cut and Paste, then a line before the Delete entry. */
function clip(some: boolean, trusted: boolean): (MenuEntry | false)[] {
  return [
    some && { label: 'Copy', keys: 'Ctrl+C', action: 'copy' },
    trusted && some && { label: 'Cut', keys: 'Ctrl+X', action: 'cut' },
    trusted && { label: 'Paste', keys: 'Ctrl+V', action: 'paste' },
    trusted && 'line',
  ];
}
