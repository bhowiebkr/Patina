// The Project panel (spec 15.4): the graphs, the nodes, the views, the widgets and the types.

import { useState, type DragEvent } from 'react';
import type { View } from './api';
import { NODE_MIME } from './GraphView';
import { NameField } from './NameField';

export type Inspect =
  | { kind: 'node'; id: string }
  | { kind: 'type'; name: string }
  | { kind: 'widget'; name: string }
  | { kind: 'view'; name: string }
  | { kind: 'wire'; from: string; to: string }
  | null;

interface Props {
  view: View;
  inspect: Inspect;
  onInspect: (inspect: Inspect) => void;
  /** Makes the node folder `nodes/<name>`. It fails with the problem of the name. */
  onNewNode: (name: string) => Promise<void>;
  /** Makes the widget folder `widgets/<name>`. It fails with the problem of the name. */
  onNewWidget: (name: string) => Promise<void>;
  /** Makes the view folder `views/<name>`. It fails with the problem of the name. */
  onNewView: (name: string) => Promise<void>;
  /** Asks for another project, and opens the Import dialog (6.2). */
  onImport: () => void;
  /** Asks for a folder, and makes a package of the project in it (14.1). */
  onPackage: () => void;
  /** Opens the Update dialog for an imported folder (6.2). */
  onUpdate: (folder: string) => void;
  /** Opens the main graph or a subgraph (6.1). */
  onOpen: (name: string) => void;
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

export function ProjectPanel({ view, inspect, onInspect, onNewNode, onNewWidget, onNewView, onImport, onUpdate, onPackage, onOpen }: Props) {
  const [naming, setNaming] = useState<'node' | 'widget' | 'view' | null>(null);
  const close = () => setNaming(null);
  const startDrag = (e: DragEvent, name: string) => {
    e.dataTransfer.setData(NODE_MIME, name);
    e.dataTransfer.effectAllowed = 'move';
  };
  const subgraphs = view.nodes.filter((n) => n.subgraph);
  const nodes = view.nodes.filter((n) => !n.subgraph);
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
            title={n.name === 'main' ? 'Open the main graph' : 'Open the subgraph, or drag it to the graph to add a use'}
          >
            {n.name}
            {n.error && <span className="hint error-text">error</span>}
          </button>
        ))}
      </div>
      <div className="group">
        <div className="group-title">
          Nodes
          {view.trusted && <NewButton label="New node" onClick={() => setNaming('node')} />}
        </div>
        {naming === 'node' && <NameField label="Node name" onMake={onNewNode} onClose={close} />}
        {nodes.length === 0 && naming !== 'node' && <Empty />}
        {nodes.map((n) => (
          <button
            key={n.name}
            className={inspect?.kind === 'type' && inspect.name === n.name ? 'item active' : 'item'}
            draggable={view.trusted}
            onDragStart={(e) => startDrag(e, n.name)}
            onClick={() => onInspect({ kind: 'type', name: n.name })}
            title={view.trusted ? 'Drag to the graph to add the node' : undefined}
          >
            <span className="swatch" style={{ background: n.color ?? undefined }} aria-hidden="true" />
            {n.name}
            {n.error && <span className="hint error-text">error</span>}
          </button>
        ))}
      </div>
      <div className="group">
        <div className="group-title">
          Widgets
          {view.trusted && <NewButton label="New widget" onClick={() => setNaming('widget')} />}
        </div>
        {naming === 'widget' && <NameField label="Widget name" onMake={onNewWidget} onClose={close} />}
        {view.widgets.length === 0 && naming !== 'widget' && <Empty />}
        {view.widgets.map((name) => (
          <button
            key={name}
            className={inspect?.kind === 'widget' && inspect.name === name ? 'item active' : 'item'}
            onClick={() => onInspect({ kind: 'widget', name })}
          >
            {name}
          </button>
        ))}
      </div>
      <div className="group">
        <div className="group-title">
          Views
          {view.trusted && <NewButton label="New view" onClick={() => setNaming('view')} />}
        </div>
        {naming === 'view' && <NameField label="View name" onMake={onNewView} onClose={close} />}
        {view.views.length === 0 && naming !== 'view' && <Empty />}
        {view.views.map((name) => (
          <button
            key={name}
            className={inspect?.kind === 'view' && inspect.name === name ? 'item active' : 'item'}
            onClick={() => onInspect({ kind: 'view', name })}
          >
            {name}
          </button>
        ))}
      </div>
      <div className="group">
        <div className="group-title">Types</div>
        <Empty />
      </div>
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
    </aside>
  );
}
