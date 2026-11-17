// The right-click menu of the graph canvas (15.2). The entries depend on what the click hits.

/** What a right-click hits: the empty canvas, the selected nodes, or one connection. */
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
    }
  | { kind: 'wire' };

export type MenuAction = 'add' | 'selectAll' | 'fit' | 'arrange' | 'open' | 'rename' | 'collapse' | 'expand' | 'delete' | 'disconnect';
export type MenuEntry = { label: string; keys?: string; action: MenuAction } | 'line';

/** The menu entries for a target. An untrusted project gets only the entries that do not edit. */
export function canvasMenu(t: MenuTarget, trusted: boolean): MenuEntry[] {
  const list: (MenuEntry | false)[] = [];
  if (t.kind === 'pane') {
    list.push(
      trusted && { label: 'Add node...', keys: 'Shift+A', action: 'add' },
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
      trusted && t.uses > 0 && { label: t.uses === 1 ? 'Expand subgraph' : 'Expand subgraphs', keys: 'Ctrl+Shift+G', action: 'expand' },
      trusted && t.count > 1 && { label: 'Arrange', keys: 'Shift+L', action: 'arrange' },
      { label: t.count === 1 ? 'Fit node' : 'Fit selection', keys: 'F', action: 'fit' },
      trusted && 'line',
      trusted && { label: t.count === 1 ? 'Delete node' : 'Delete selection', keys: 'Delete', action: 'delete' },
    );
  } else if (trusted) {
    list.push({ label: 'Delete connection', keys: 'Delete', action: 'disconnect' });
  }
  return list.filter((e): e is MenuEntry => e !== false);
}
