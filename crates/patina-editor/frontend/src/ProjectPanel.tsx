// The Project panel (spec 15.4): the graphs, the nodes, the views, the widgets and the types.

import type { DragEvent } from 'react';
import type { View } from './api';
import { NODE_MIME } from './GraphView';

export type Inspect = { kind: 'node'; id: string } | { kind: 'type'; name: string } | null;

interface Props {
  view: View;
  inspect: Inspect;
  onInspect: (inspect: Inspect) => void;
}

function Empty() {
  return <div className="item empty">None</div>;
}

export function ProjectPanel({ view, inspect, onInspect }: Props) {
  const startDrag = (e: DragEvent, name: string) => {
    e.dataTransfer.setData(NODE_MIME, `nodes/${name}`);
    e.dataTransfer.effectAllowed = 'move';
  };
  return (
    <aside className="panel left" aria-label="Project">
      <div className="panel-title">Project</div>
      <div className="group">
        <div className="group-title">Graphs</div>
        <button className="item active">main</button>
      </div>
      <div className="group">
        <div className="group-title">Nodes</div>
        {view.nodes.length === 0 && <Empty />}
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
