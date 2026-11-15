// The Project panel (spec 15.4): the graphs, the nodes, the views, the widgets and the types.

import { useState, type DragEvent } from 'react';
import type { View } from './api';
import { NODE_MIME } from './GraphView';
import { NameField } from './NameField';

export type Inspect =
  | { kind: 'node'; id: string }
  | { kind: 'type'; name: string }
  | { kind: 'widget'; name: string }
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

export function ProjectPanel({ view, inspect, onInspect, onNewNode, onNewWidget }: Props) {
  const [naming, setNaming] = useState<'node' | 'widget' | null>(null);
  const close = () => setNaming(null);
  const startDrag = (e: DragEvent, name: string) => {
    e.dataTransfer.setData(NODE_MIME, `nodes/${name}`);
    e.dataTransfer.effectAllowed = 'move';
  };
  return (
    <aside className="panel" aria-label="Project">
      <div className="group">
        <div className="group-title">Graphs</div>
        <button className="item active">main</button>
      </div>
      <div className="group">
        <div className="group-title">
          Nodes
          {view.trusted && <NewButton label="New node" onClick={() => setNaming('node')} />}
        </div>
        {naming === 'node' && <NameField label="Node name" onMake={onNewNode} onClose={close} />}
        {view.nodes.length === 0 && naming !== 'node' && <Empty />}
        {view.nodes.map((n) => (
          <button
            key={n.name}
            className={inspect?.kind === 'type' && inspect.name === n.name ? 'item active' : 'item'}
            draggable={view.trusted}
            onDragStart={(e) => startDrag(e, n.name)}
            onClick={() => onInspect({ kind: 'type', name: n.name })}
            title={view.trusted ? 'Drag to the graph to add the node' : undefined}
          >
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
      {['Views', 'Types'].map((title) => (
        <div key={title} className="group">
          <div className="group-title">{title}</div>
          <Empty />
        </div>
      ))}
    </aside>
  );
}
