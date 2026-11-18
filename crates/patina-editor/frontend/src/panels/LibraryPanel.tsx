// The Library panel (spec 15.2): the nodes and the subgraphs of the libraries, with a preview of
// what a pick brings into the open project (6.2).

import { useEffect, useState, type DragEvent } from 'react';
import { api, docParts, type LibraryNode, type Need, type Pin, type View } from '../api';
import { LIBRARY_MIME } from '../canvas/useAddNode';

const keyOf = (n: LibraryNode) => `${n.path}\n${n.subgraph ? 'graphs' : 'nodes'}/${n.name}`;

/** The text of the state of a subgraph or a folder that a pick brings. */
function needText(n: Need): string {
  switch (n.state) {
    case 'New':
      return 'new';
    case 'Same':
      return 'in the project';
    case 'Keep':
      return 'uses the folder of this project';
    case 'Rename':
      return `as ${n.to}`;
    case 'Clash':
      return `has the crate name of ${n.existing}`;
  }
}

function Pins({ title, pins }: { title: string; pins: Pin[] }) {
  if (pins.length === 0) return null;
  return (
    <ul className="library-rows" aria-label={title}>
      <li className="label">{title}</li>
      {pins.map((p) => (
        <li key={p.key}>
          <span>{p.name ?? p.key}</span>
          <span className="label">{p.ty}</span>
        </li>
      ))}
    </ul>
  );
}

export function LibraryPanel({ view, onAdd }: { view: View; onAdd: (n: LibraryNode) => void }) {
  const [items, setItems] = useState<LibraryNode[] | null>(null);
  const [reads, setReads] = useState(0);
  const [query, setQuery] = useState('');
  const [picked, setPicked] = useState<string | null>(null);
  const [plan, setPlan] = useState<Need[] | string | null>(null);
  useEffect(() => {
    let live = true;
    api.libraryNodes().then(
      (list) => live && setItems(list),
      () => live && setItems([]),
    );
    return () => {
      live = false;
    };
  }, [view.serial, reads]);
  const item = items?.find((n) => keyOf(n) === picked) ?? null;
  // What a pick brings depends on the folders and the subgraphs of this project, so it loads again when they change.
  const folders = JSON.stringify([view.serial, view.nodes.map((n) => n.name), view.widgets, view.views, view.connections, view.types]);
  useEffect(() => {
    setPlan(null);
    if (!item) return;
    let live = true;
    api.libraryPlan(item).then(
      (p) => live && setPlan(p),
      (e) => live && setPlan(String(e)),
    );
    return () => {
      live = false;
    };
  }, [item, folders]);

  const q = query.trim().toLowerCase();
  const groups = new Map<string, LibraryNode[]>();
  for (const n of items ?? []) {
    if (q && !`${n.name}\n${n.library}\n${n.doc ?? ''}`.toLowerCase().includes(q)) continue;
    const list = groups.get(n.path);
    if (list) list.push(n);
    else groups.set(n.path, [n]);
  }
  const startDrag = (e: DragEvent, n: LibraryNode) => {
    e.dataTransfer.setData(LIBRARY_MIME, JSON.stringify(n));
    e.dataTransfer.effectAllowed = 'copy';
  };
  const clash = Array.isArray(plan) && plan.some((n) => n.state === 'Clash');
  const why = !view.trusted
    ? 'Trust the project first.'
    : plan === null
      ? 'Patina reads what the item brings.'
      : typeof plan === 'string'
        ? plan
        : clash
          ? 'A folder has the crate name of a folder of this project. Rename one of them first.'
          : null;
  return (
    <aside className="panel library" aria-label="Library">
      <div className="library-bar">
        <input className="field" type="search" placeholder="Search" aria-label="Search libraries" value={query} onChange={(e) => setQuery(e.target.value)} />
        <button className="icon-btn" aria-label="Read the libraries again" title="Read the libraries again" onClick={() => setReads((r) => r + 1)}>
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
            <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2v3.5H10" />
          </svg>
        </button>
      </div>
      <div className="library-list">
        {items === null && <div className="item empty">Reading...</div>}
        {items?.length === 0 && (
          <div className="item empty library-note">
            <span>
              No library. Add a Patina project as a library in <b>Settings</b>.
            </span>
          </div>
        )}
        {items && items.length > 0 && groups.size === 0 && <div className="item empty">No item matches.</div>}
        {[...groups].map(([path, list]) => (
          <div key={path} className="group">
            <div className="group-title" title={path}>
              {list[0].library}
            </div>
            {list.map((n) => (
              <button
                key={keyOf(n)}
                className={keyOf(n) === picked ? 'item active' : 'item'}
                draggable={view.trusted}
                onDragStart={(e) => startDrag(e, n)}
                onClick={() => setPicked(keyOf(n))}
                title={view.trusted ? 'Show the preview, or drag to the graph to add the item and what it needs' : 'Show the preview'}
              >
                <span className="swatch" style={{ background: n.color ?? undefined }} aria-hidden="true" />
                {n.name}
                {n.error ? <span className="hint error-text">error</span> : n.subgraph && <span className="hint">subgraph</span>}
              </button>
            ))}
          </div>
        ))}
      </div>
      {item && (
        <section className="library-preview" aria-label="Preview">
          <div className="library-head">
            <span className="swatch" style={{ background: item.color ?? undefined }} aria-hidden="true" />
            <b>{item.name}</b>
            <span className="label">
              {item.subgraph ? 'subgraph' : 'node'} of {item.library}
            </span>
          </div>
          {item.doc && <div className="label doc">{docParts(item.doc).map((p, i) => (p.kind === 'bold' ? <b key={i}>{p.text}</b> : p.kind === 'code' ? <code key={i}>{p.text}</code> : <span key={i}>{p.text}</span>))}</div>}
          {item.error && <div className="error-text">{item.error}</div>}
          <Pins title="Inputs" pins={item.inputs} />
          <Pins title="Outputs" pins={item.outputs} />
          <ul className="library-rows" aria-label="Brings">
            <li className="label">Brings</li>
            {plan === null && <li className="label">Reading...</li>}
            {typeof plan === 'string' && <li className="error-text">{plan}</li>}
            {Array.isArray(plan) &&
              plan.map((n) => (
                <li key={n.folder} className={n.state === 'Clash' ? 'error-text' : undefined}>
                  <span>{n.folder}</span> <span className={n.state === 'Clash' ? undefined : 'label'}>{needText(n)}</span>
                </li>
              ))}
          </ul>
          <button className="btn primary" disabled={why !== null} title={why ?? 'Add the item and what it needs to the open graph'} onClick={() => onAdd(item)}>
            Add to graph
          </button>
        </section>
      )}
    </aside>
  );
}
