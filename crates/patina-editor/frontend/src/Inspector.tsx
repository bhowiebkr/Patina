// The Inspector (spec 15.5): the parameters, the code and the logs of the selected node, in a
// stable tab group (docs/gui-visual-guide/editor-mockups.md).

import type { FocusEvent, KeyboardEvent } from 'react';
import { errorText, folderName, type Edit, type View } from './api';
import { CodeTab, revealOf, type Reveal } from './CodeTab';
import { STATE_NAMES, infoOf, nodeErrors, nodeState, type RunState } from './NodeView';
import type { Inspect } from './ProjectPanel';

export type InspectorTab = 'parameters' | 'code' | 'logs';

const TABS: [InspectorTab, string][] = [
  ['parameters', 'Parameters'],
  ['code', 'Code'],
  ['logs', 'Logs'],
];

interface Props {
  view: View;
  run: RunState;
  inspect: Inspect;
  tab: InspectorTab;
  reveal: Reveal | null;
  onTab: (tab: InspectorTab) => void;
  onEdit: (edit: Edit) => void;
  onSave: () => void;
  onPick: (at: Reveal) => void;
}

function Tabs({ tab, only, onTab }: { tab: InspectorTab; only?: InspectorTab; onTab: (tab: InspectorTab) => void }) {
  return (
    <div className="tabs" role="tablist" aria-label="Inspector">
      {TABS.filter(([key]) => !only || key === only).map(([key, label]) => (
        <button key={key} role="tab" aria-selected={tab === key} className={tab === key ? 'tab active' : 'tab'} onClick={() => onTab(key)}>
          {label}
        </button>
      ))}
    </div>
  );
}

export function Inspector({ view, run, inspect, tab, reveal, onTab, onEdit, onSave, onPick }: Props) {
  const entry = inspect?.kind === 'node' ? view.graph.node?.find((n) => n.id === inspect.id) : undefined;
  const code = (name: string) => <CodeTab name={name} run={run} readOnly={!view.trusted} reveal={reveal} onSave={onSave} />;
  if (inspect?.kind === 'type') {
    return (
      <aside className="panel right" aria-label="Inspector">
        <div className="panel-title">{inspect.name}</div>
        <Tabs tab="code" only="code" onTab={onTab} />
        {code(inspect.name)}
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
  const shown = info ? tab : 'parameters';
  const reports = run.reports.filter((r) => r.id === entry.id);
  const errors = info ? nodeErrors(run, info.name) : [];
  return (
    <aside className="panel right" aria-label="Inspector">
      <div className="panel-title">
        {folderName(entry)}
        <span className="spacer" />
        <span className={state === 'compile-error' || state === 'run-error' ? 'error-text' : 'label'}>{STATE_NAMES[state]}</span>
      </div>
      <Tabs tab={shown} only={info ? undefined : 'parameters'} onTab={onTab} />
      {shown === 'parameters' && (
        <>
          <div className="section">
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
        </>
      )}
      {shown === 'code' && info && code(info.name)}
      {shown === 'logs' && (
        <div className="lines" role="log">
          {reports.length + errors.length === 0 && <div className="info">No errors in the last run</div>}
          {errors.map((e, i) => (
            <button key={`e${i}`} className="line-link err" onClick={() => onPick(revealOf(e, Date.now())!)}>
              {e.file}:{e.start[0]}:{e.start[1]}: {errorText(e)}
            </button>
          ))}
          {reports.map((r, i) => (
            <div key={`r${i}`} className="err">
              {r.kind}: {r.text}
            </div>
          ))}
        </div>
      )}
    </aside>
  );
}
