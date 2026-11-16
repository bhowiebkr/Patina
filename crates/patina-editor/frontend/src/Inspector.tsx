// The Inspector (spec 15.5): the parameters, the code and the logs of the selected node, in a
// stable tab group (docs/gui-visual-guide/editor-mockups.md).

import { useEffect, useState, type FocusEvent, type KeyboardEvent } from 'react';
import { api, errorText, folderName, laneLabel, logKind, reportError, reportText, type Corners, type Edit, type Style, type View } from './api';
import { CodeTab, revealOf, type Reveal } from './CodeTab';
import { STATE_NAMES, infoOf, nodeErrors, nodeState, outputType, valueStale, type RunState } from './NodeView';
import { ExpandButton, NodeGui } from './NodeGui';
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
  /** The widget lines of each node GUI, by "<mode> <node id>" (4.3). */
  guis: ReadonlyMap<string, string>;
  inspect: Inspect;
  tab: InspectorTab;
  reveal: Reveal | null;
  onTab: (tab: InspectorTab) => void;
  onEdit: (edit: Edit) => void;
  onSave: () => void;
  /** Adds a crate to a node or widget folder. It fails with the problem of the crate. */
  onAddCrate: (folder: string, spec: string) => Promise<void>;
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

interface WireProps {
  view: View;
  run: RunState;
  from: string;
  to: string;
  onEdit: (edit: Edit) => void;
}

/** A selected wire: its ends, its look and the full text of its live value (15.7, 15.9). */
function WireView({ view, run, from, to, onEdit }: WireProps) {
  const [full, setFull] = useState<{ from: string; text: string | null } | null>(null);
  useEffect(() => {
    let current = true;
    api.value(from).then((text) => current && setFull({ from, text }), reportError);
    return () => {
      current = false;
    };
  }, [from, run.values]);
  const end = (key: string) => {
    const dot = key.indexOf('.');
    const node = view.graph.node?.find((n) => n.id === key.slice(0, dot));
    return `${node ? folderName(node) : key.slice(0, dot)}.${key.slice(dot + 1)}`;
  };
  // The badge text shows until the full text arrives.
  const value = run.values.get(from);
  const text = (full?.from === from ? full.text : null) ?? value?.text;
  const lanes = value?.lanes;
  const stale = valueStale(run, from);
  const wire = view.graph.wire?.find((w) => w.to === to);
  const locked = !view.trusted || !wire;
  const look = (style?: Style, corners?: Corners) => onEdit({ kind: 'SetLook', to, style, corners });
  // A curve has no corners.
  const curved = wire?.style === 'curved';
  return (
    <aside className="panel" aria-label="Inspector">
      <div className="panel-title">
        Wire
        <span className="spacer" />
        {stale && <span className="label">Stale</span>}
      </div>
      <div className="section">
        <div className="param">
          <span className="label">From</span>
          <span>{end(from)}</span>
        </div>
        <div className="param">
          <span className="label">To</span>
          <span>{end(to)}</span>
        </div>
        <label className="param">
          <span className="label">Name</span>
          <input
            key={`${to}=${wire?.name ?? ''}`}
            className="field"
            defaultValue={wire?.name ?? ''}
            placeholder="Default"
            maxLength={32}
            disabled={locked}
            title="The bus name that the label and the lane tooltips show"
            onBlur={(e) => {
              const name = e.currentTarget.value.trim();
              if (name !== (wire?.name ?? '')) void onEdit({ kind: 'NameWire', to, name: name || null });
            }}
            onKeyDown={(e) => {
              if (e.key === 'Escape') e.currentTarget.value = wire?.name ?? '';
              if (e.key === 'Enter' || e.key === 'Escape') e.currentTarget.blur();
            }}
            spellCheck={false}
          />
        </label>
        <div className="param">
          <span className="label">Type</span>
          <span>{outputType(view, from) ?? 'Unknown'}</span>
        </div>
        {lanes && (
          <div className="param">
            <span className="label">Lanes</span>
            <span title="The most significant lane first">
              {laneLabel(lanes)}: {lanes.bits}
            </span>
          </div>
        )}
      </div>
      <div className="section">
        <h3>Look</h3>
        <label className="param">
          <span className="label">Route</span>
          <select
            className="field"
            value={wire?.style ?? ''}
            disabled={locked}
            onChange={(e) => look((e.currentTarget.value || undefined) as Style | undefined, wire?.corners)}
          >
            <option value="">Automatic</option>
            <option value="curved">Curved</option>
            <option value="orthogonal">Orthogonal</option>
            <option value="polyline">Polyline</option>
          </select>
        </label>
        <label className="param">
          <span className="label">Corners</span>
          <select
            className="field"
            value={curved ? '' : (wire?.corners ?? 'rounded')}
            disabled={locked || curved}
            onChange={(e) => look(wire?.style, e.currentTarget.value === 'sharp' ? 'sharp' : undefined)}
          >
            {curved && <option value="">None</option>}
            <option value="rounded">Rounded</option>
            <option value="sharp">Sharp</option>
          </select>
        </label>
        <button className="btn" disabled={locked || !wire?.points?.length} onClick={() => onEdit({ kind: 'SetRoute', to, points: [] })}>
          Reset route
        </button>
      </div>
      <div className="section">
        <h3>Value</h3>
        {text === undefined ? (
          <div className="label">Run the graph to see the value.</div>
        ) : (
          <pre className={stale ? 'value-text stale' : 'value-text'}>{text}</pre>
        )}
      </div>
    </aside>
  );
}

export function Inspector({ view, run, guis, inspect, tab, reveal, onTab, onEdit, onSave, onAddCrate, onPick }: Props) {
  const entry = inspect?.kind === 'node' ? view.graph.node?.find((n) => n.id === inspect.id) : undefined;
  // A node with a GUI draws it in the Inspector size after each run, once the engine knows (4.3).
  const guiId = entry && guis.has(`0 ${entry.id}`) ? entry.id : null;
  useEffect(() => {
    if (guiId) api.gui(guiId, 1, '').catch(() => {});
  }, [guiId]);
  const code = (folder: string) => (
    <CodeTab name={folder} run={run} readOnly={!view.trusted} reveal={reveal} onSave={onSave} onAddCrate={(spec) => onAddCrate(folder, spec)} />
  );
  if (inspect?.kind === 'type' || inspect?.kind === 'widget' || inspect?.kind === 'view') {
    return (
      <aside className="panel" aria-label="Inspector">
        <div className="panel-title">{inspect.name}</div>
        <Tabs tab="code" only="code" onTab={onTab} />
        {code(`${{ type: 'nodes', widget: 'widgets', view: 'views' }[inspect.kind]}/${inspect.name}`)}
      </aside>
    );
  }
  if (inspect?.kind === 'wire') return <WireView view={view} run={run} from={inspect.from} to={inspect.to} onEdit={onEdit} />;
  if (!entry) {
    return (
      <aside className="panel" aria-label="Inspector">
        <div className="panel-title">Inspector</div>
        <div className="section label">Select a node to see its parameters and code, or a wire to see its value.</div>
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
  // A built-in node has no code folder.
  const hasCode = info && !info.builtin;
  const shown = hasCode ? tab : 'parameters';
  const reports = run.reports.filter((r) => r.id === entry.id);
  const logs = run.logs.filter((l) => l.id === entry.id);
  const errors = info ? nodeErrors(run, info.name) : [];
  return (
    <aside className="panel" aria-label="Inspector">
      <div className="panel-title">
        {folderName(entry)}
        <span className="spacer" />
        {info?.full && <ExpandButton id={entry.id} />}
        <span className={state === 'compile-error' || state === 'run-error' ? 'error-text' : 'label'}>{STATE_NAMES[state]}</span>
      </div>
      <Tabs tab={shown} only={hasCode ? undefined : 'parameters'} onTab={onTab} />
      {shown === 'parameters' && (
        <>
          <div className="section">
            {(info?.inputs ?? []).length === 0 && <div className="label">No inputs</div>}
            {(info?.inputs ?? []).map(({ key, ty }) => {
              const before = entry.params?.[key] ?? '';
              return (
                <label key={key} className="param">
                  <span className="label">{key}</span>
                  {wired.has(`${entry.id}.${key}`) ? (
                    <span className="label">Wired</span>
                  ) : ty === 'bool' ? (
                    <input
                      type="checkbox"
                      checked={before === 'true'}
                      disabled={!view.trusted}
                      onChange={(e) => onEdit({ kind: 'SetParam', id: entry.id, key, text: String(e.currentTarget.checked) })}
                    />
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
          {guiId && (
            <div className="section">
              <h3>Controls</h3>
              <NodeGui id={guiId} mode={1} tree={guis.get(`1 ${guiId}`) ?? guis.get(`0 ${guiId}`) ?? ''} disabled={!view.trusted} />
            </div>
          )}
          <div className="section">
            <h3>State</h3>
            <div>{STATE_NAMES[state]}</div>
            {message && <div className={state === 'unresolved' ? 'label' : 'error-text'}>{message}</div>}
            <div className="label">ID {entry.id}</div>
          </div>
        </>
      )}
      {shown === 'code' && info && code(`nodes/${info.name}`)}
      {shown === 'logs' && (
        <div className="lines" role="log">
          {logs.length + reports.length + errors.length === 0 && <div className="info">No log lines or errors in the last run</div>}
          {logs.map((l, i) => (
            <div key={`l${i}`} className={logKind(l.level)}>
              {l.level} {l.text}
            </div>
          ))}
          {errors.map((e, i) => (
            <button key={`e${i}`} className="line-link err" onClick={() => onPick(revealOf(e, Date.now())!)}>
              {e.file}:{e.start[0]}:{e.start[1]}: {errorText(e)}
            </button>
          ))}
          {reports.map((r, i) => (
            <div key={`r${i}`} className="err">
              {reportText(r)}
            </div>
          ))}
        </div>
      )}
    </aside>
  );
}
