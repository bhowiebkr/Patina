// The Project panel (spec 15.4): the graphs, the nodes, the widgets, the views, the connections
// and the types.

import { useState, type DragEvent, type KeyboardEvent, type MouseEvent } from 'react';
import type { View } from '../api';
import { NODE_MIME } from '../canvas/useAddNode';
import { NameField } from '../controls/NameField';
import { Confirm } from '../dialogs/Confirm';
import { Menu } from './Dock';

/** `type` is a node folder. `folder` is a widget, view, connection or type folder, for example `types/segments`. */
export type Inspect =
  | { kind: 'node'; id: string }
  | { kind: 'type'; name: string }
  | { kind: 'folder'; folder: string }
  | { kind: 'wire'; from: string; to: string }
  | null;

/** The code groups after **Nodes**: the folder group, the title and the field label. */
const GROUPS = [
  ['widgets', 'Widgets', 'Widget'],
  ['views', 'Views', 'View'],
  ['connections', 'Connections', 'Connection'],
  ['types', 'Types', 'Type'],
] as const;
export type Group = 'nodes' | (typeof GROUPS)[number][0];

interface Props {
  view: View;
  inspect: Inspect;
  onInspect: (inspect: Inspect) => void;
  /** Makes the folder `<group>/<name>`. It fails with the problem of the name. */
  onNew: (group: Group, name: string) => Promise<void>;
  /** Asks for another project, and opens the Import dialog (6.2). */
  onImport: () => void;
  /** Asks for a folder, and makes a package of the project in it (14.1). */
  onPackage: () => void;
  /** Opens the Update dialog for an imported folder (6.2). */
  onUpdate: (folder: string) => void;
  /** Opens the main graph or a subgraph (6.1). */
  onOpen: (name: string) => void;
  /** Moves a folder, for example `nodes/switch` or `graphs/filter`, to the Recycle Bin (15.4). */
  onDelete: (folder: string) => void;
}

function Empty() {
  return <div className="item empty">None</div>;
}

function NewButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button className="icon-btn" aria-label={label} title={label} onClick={onClick}>
      <svg width="12" height="12" viewBox="0 0 12 12" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
        <path d="M6 1v10M1 6h10" />
      </svg>
    </button>
  );
}

export function ProjectPanel({ view, inspect, onInspect, onNew, onImport, onUpdate, onPackage, onOpen, onDelete }: Props) {
  const [naming, setNaming] = useState<Group | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; folder: string } | null>(null);
  const [asking, setAsking] = useState<string | null>(null);
  // A right-click or the Delete key on an item asks to delete its folder (15.4).
  const deleting = (folder: string) =>
    view.trusted
      ? {
          onContextMenu: (e: MouseEvent) => {
            e.preventDefault();
            setMenu({ x: e.clientX, y: e.clientY, folder });
          },
          onKeyDown: (e: KeyboardEvent) => e.key === 'Delete' && setAsking(folder),
        }
      : {};
  const close = () => setNaming(null);
  const startDrag = (e: DragEvent, name: string) => {
    e.dataTransfer.setData(NODE_MIME, name);
    e.dataTransfer.effectAllowed = 'move';
  };
  const subgraphs = view.nodes.filter((n) => n.subgraph);
  const nodes = view.nodes.filter((n) => !n.subgraph);
  const title = (group: Group, text: string, one: string) => (
    <div className="group-title">
      {text}
      {view.trusted && <NewButton label={`New ${one.toLowerCase()}`} onClick={() => setNaming(group)} />}
    </div>
  );
  const field = (group: Group, one: string) =>
    naming === group && <NameField label={`${one} name`} onMake={(name) => onNew(group, name)} onClose={close} />;
  return (
    <aside className="panel" aria-label="Project">
      <div className="group">
        <div className="group-title">Graphs</div>
        {[{ name: 'main', error: null }, ...subgraphs].map((n) => (
          <button
            key={n.name}
            className={view.graphName === n.name ? 'item active' : 'item'}
            draggable={view.trusted && n.name !== 'main'}
            onDragStart={(e) => startDrag(e, n.name)}
            onClick={() => onOpen(n.name)}
            {...(n.name === 'main' ? {} : deleting(`graphs/${n.name}`))}
            title={n.name === 'main' ? 'Open the main graph' : 'Open the subgraph, or drag it to the graph to add a use'}
          >
            {n.name}
            {n.error && <span className="hint error-text">error</span>}
          </button>
        ))}
      </div>
      <div className="group">
        {title('nodes', 'Nodes', 'Node')}
        {field('nodes', 'Node')}
        {nodes.length === 0 && naming !== 'nodes' && <Empty />}
        {nodes.map((n) => (
          <button
            key={n.name}
            className={inspect?.kind === 'type' && inspect.name === n.name ? 'item active' : 'item'}
            draggable={view.trusted}
            onDragStart={(e) => startDrag(e, n.name)}
            onClick={() => onInspect({ kind: 'type', name: n.name })}
            {...deleting(`nodes/${n.name}`)}
            title={[n.doc, view.trusted && 'Drag to the graph to add the node'].filter(Boolean).join('\n\n') || undefined}
          >
            <span className="swatch" style={{ background: n.color ?? undefined }} aria-hidden="true" />
            {n.name}
            {n.error && <span className="hint error-text">error</span>}
          </button>
        ))}
      </div>
      {GROUPS.map(([group, text, one]) => (
        <div key={group} className="group">
          {title(group, text, one)}
          {field(group, one)}
          {view[group].length === 0 && naming !== group && <Empty />}
          {view[group].map((name) => {
            const folder = `${group}/${name}`;
            return (
              <button
                key={name}
                className={inspect?.kind === 'folder' && inspect.folder === folder ? 'item active' : 'item'}
                onClick={() => onInspect({ kind: 'folder', folder })}
                {...deleting(folder)}
              >
                {name}
              </button>
            );
          })}
        </div>
      ))}
      {view.trusted && (
        <div className="group">
          {view.imports.length > 0 && <div className="group-title">Imports</div>}
          {view.imports.map((f) => (
            <button key={f} className="item" onClick={() => onUpdate(f)} title="Compare with the other project and update">
              {f}
              <span className="hint">Update</span>
            </button>
          ))}
          <button className="btn" onClick={onImport} title="Copy a folder from another project">
            Import
          </button>
          <button className="btn" onClick={onPackage} title="Save the project and make a program that runs it">
            Package
          </button>
        </div>
      )}
      {menu && <Menu x={menu.x} y={menu.y} label={menu.folder} items={[{ label: 'Delete', keys: 'Del', run: () => setAsking(menu.folder) }]} onClose={() => setMenu(null)} />}
      {asking && (
        <Confirm
          title={`Delete ${asking}?`}
          label={`Delete moves ${asking} to the Recycle Bin. Undo cannot get it back, but the Recycle Bin can. The nodes and the uses that need it show an error until you delete them.`}
          answers={[
            [
              'Delete',
              () => {
                onDelete(asking);
                setAsking(null);
              },
            ],
            ['Cancel', () => setAsking(null)],
          ]}
          onCancel={() => setAsking(null)}
        />
      )}
    </aside>
  );
}
