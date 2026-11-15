// The Add node palette: a search for a node of the project, at a point of the canvas (spec 15.2).

import { useEffect, useState, type KeyboardEvent } from 'react';
import type { NodeInfo } from './api';
import { matchNodes } from './palette';

interface Props {
  /** The screen point where the palette opens and the node goes. */
  x: number;
  y: number;
  nodes: NodeInfo[];
  /** The text when `nodes` is empty. */
  none?: string;
  onAdd: (name: string) => void;
  onClose: () => void;
}

/** Type to filter, the arrows to move, Enter or a click to add. Esc or a click out of it closes it. */
export function AddNode({ x, y, nodes, none = 'The project has no nodes.', onAdd, onClose }: Props) {
  const [back] = useState(() => document.activeElement as HTMLElement | null);
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const hits = matchNodes(nodes, query);
  const at = Math.min(active, hits.length - 1);
  useEffect(() => {
    document.getElementById(`add-node-${at}`)?.scrollIntoView({ block: 'nearest' });
  }, [at]);

  const close = () => {
    onClose();
    back?.focus();
  };
  const add = (name: string) => {
    close();
    onAdd(name);
  };
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape' || e.key === 'Tab') close();
    else if (e.key === 'ArrowDown' && hits.length > 0) setActive((at + 1) % hits.length);
    else if (e.key === 'ArrowUp' && hits.length > 0) setActive((at - 1 + hits.length) % hits.length);
    else if (e.key === 'Enter' && hits[at]) add(hits[at].name);
    else return;
    // The keys of the palette do not go to the graph.
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <div className="menu-layer" onPointerDown={(e) => e.target === e.currentTarget && close()}>
      <div className="menu palette" role="dialog" aria-label="Add node" style={{ left: Math.min(x, innerWidth - 250), top: Math.min(y, innerHeight - 300) }}>
        <input
          className="field"
          role="combobox"
          aria-label="Search nodes"
          aria-expanded="true"
          aria-controls="add-node-list"
          aria-autocomplete="list"
          aria-activedescendant={hits[at] ? `add-node-${at}` : undefined}
          placeholder="Search nodes"
          autoFocus
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={onKey}
        />
        <div id="add-node-list" role="listbox" aria-label="Nodes">
          {hits.map((n, i) => (
            <div key={n.name} id={`add-node-${i}`} role="option" aria-selected={i === at} onClick={() => add(n.name)}>
              {n.name}
              {n.error && <span className="hint error-text">error</span>}
            </div>
          ))}
          {hits.length === 0 && <div className="empty">{nodes.length === 0 ? none : 'No node matches.'}</div>}
        </div>
      </div>
    </div>
  );
}
