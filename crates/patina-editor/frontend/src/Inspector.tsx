// The Inspector (spec 15.5): the parameters, the state and the code of the selected node.

import { useEffect, useState, type FocusEvent, type KeyboardEvent } from 'react';
import { api, folderName, type Edit, type View } from './api';
import { STATE_NAMES, infoOf, nodeState, type RunState } from './NodeView';
import type { Inspect } from './ProjectPanel';

interface Props {
  view: View;
  run: RunState;
  inspect: Inspect;
  onEdit: (edit: Edit) => void;
}

function Code({ name }: { name: string }) {
  const [code, setCode] = useState<{ name: string; text: string; error: boolean } | null>(null);
  useEffect(() => {
    let live = true;
    api
      .nodeSource(name)
      .then((text) => live && setCode({ name, text, error: false }))
      .catch((e) => live && setCode({ name, text: String(e), error: true }));
    return () => {
      live = false;
    };
  }, [name]);
  return (
    <div className="section">
      <h3>Code</h3>
      {/* shortcut: read-only text; use the code editor of spec 15.6 at milestone 3. */}
      {code?.name === name ? <pre className={code.error ? 'code error-text' : 'code'}>{code.text}</pre> : <div className="label">Loading</div>}
    </div>
  );
}

export function Inspector({ view, run, inspect, onEdit }: Props) {
  const entry = inspect?.kind === 'node' ? view.graph.node?.find((n) => n.id === inspect.id) : undefined;
  if (inspect?.kind === 'type') {
    return (
      <aside className="panel right" aria-label="Inspector">
        <div className="panel-title">{inspect.name}</div>
        <Code name={inspect.name} />
      </aside>
    );
  }
  if (!entry) {
    return (
      <aside className="panel right" aria-label="Inspector">
        <div className="panel-title">Inspector</div>
        <div className="section label">Select a node to see its parameters and code.</div>
      </aside>
    );
  }
  const info = infoOf(entry, view.nodes);
  const [state, message] = nodeState(entry, info, view, run);
  const wired = new Set((view.graph.wire ?? []).map((w) => w.to).filter((to) => to.startsWith(`${entry.id}.`)));
  const commit = (key: string, before: string) => (e: FocusEvent<HTMLInputElement>) => {
    const text = e.currentTarget.value;
    if (text !== before) onEdit({ kind: 'SetParam', id: entry.id, key, text: text.trim() === '' ? null : text });
  };
  const keys = (before: string) => (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') e.currentTarget.blur();
    if (e.key === 'Escape') {
      e.currentTarget.value = before;
      e.currentTarget.blur();
    }
  };
  return (
    <aside className="panel right" aria-label="Inspector">
      <div className="panel-title">{folderName(entry)}</div>
      <div className="section">
        <h3>Parameters</h3>
        {(info?.inputs ?? []).length === 0 && <div className="label">No inputs</div>}
        {(info?.inputs ?? []).map((key) => {
          const before = entry.params?.[key] ?? '';
          return (
            <label key={key} className="param">
              <span className="label">{key}</span>
              {wired.has(`${entry.id}.${key}`) ? (
                <span className="label">Wired</span>
              ) : (
                <input
                  key={`${entry.id}.${key}=${before}`}
                  className="field"
                  defaultValue={before}
                  placeholder="Default"
                  disabled={!view.trusted}
                  onBlur={commit(key, before)}
                  onKeyDown={keys(before)}
                  spellCheck={false}
                />
              )}
            </label>
          );
        })}
      </div>
      <div className="section">
        <h3>State</h3>
        <div>{STATE_NAMES[state]}</div>
        {message && <div className={state === 'unresolved' ? 'label' : 'error-text'}>{message}</div>}
        <div className="label">ID {entry.id}</div>
      </div>
      {info && <Code name={info.name} />}
    </aside>
  );
}
