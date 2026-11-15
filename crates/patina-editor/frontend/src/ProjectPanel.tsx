// The Project panel (spec 15.4): the graphs, the nodes, the views, the widgets and the types.

import { useState, type DragEvent, type KeyboardEvent } from 'react';
import type { View } from './api';
import { NODE_MIME } from './GraphView';

export type Inspect =
  | { kind: 'node'; id: string }
  | { kind: 'type'; name: string }
  | { kind: 'wire'; from: string; to: string }
  | null;

interface Props {
  view: View;
  inspect: Inspect;
  onInspect: (inspect: Inspect) => void;
  /** Makes the node folder `nodes/<name>`. It fails with the problem of the name. */
  onNewNode: (name: string) => Promise<void>;
}

function Empty() {
  return <div className="item empty">None</div>;
}

/** The name field of a new node folder: Enter makes it, Esc or a click out of it cancels. */
function NewNode({ onMake, onClose }: { onMake: (name: string) => Promise<void>; onClose: () => void }) {
  const [name, setName] = useState('');
  const [problem, setProblem] = useState('');
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') onClose();
    else if (e.key === 'Enter') onMake(name.trim()).then(onClose, (err) => setProblem(String(err)));
    else return;
    e.preventDefault();
    e.stopPropagation();
  };
  return (
    <div className="new-item">
      <input
        className="field"
        aria-label="Node name"
        aria-invalid={problem !== ''}
        aria-describedby={problem ? 'new-node-problem' : undefined}
        placeholder="Node name"
        autoFocus
        value={name}
        onChange={(e) => {
          setName(e.target.value);
          setProblem('');
        }}
        onKeyDown={onKey}
        onBlur={onClose}
      />
      {problem && (
        <div id="new-node-problem" className="error-text" role="alert">
          {problem}
        </div>
      )}
    </div>
  );
}

export function ProjectPanel({ view, inspect, onInspect, onNewNode }: Props) {
  const [naming, setNaming] = useState(false);
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
          {view.trusted && (
            <button className="icon-btn" aria-label="New node" title="New node" onClick={() => setNaming(true)}>
              <svg width="12" height="12" viewBox="0 0 12 12" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
                <path d="M6 1v10M1 6h10" />
              </svg>
            </button>
          )}
        </div>
        {naming && <NewNode onMake={onNewNode} onClose={() => setNaming(false)} />}
        {view.nodes.length === 0 && !naming && <Empty />}
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
      {['Views', 'Widgets', 'Types'].map((title) => (
        <div key={title} className="group">
          <div className="group-title">{title}</div>
          <Empty />
        </div>
      ))}
    </aside>
  );
}
