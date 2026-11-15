// The editor window (spec 15.1): the top bar, the Project panel, the graph, the Inspector, the
// bottom panel and the status bar. The start screen shows when no project is open.

import { useCallback, useEffect, useRef, useState } from 'react';
import { useViewport } from '@xyflow/react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { api, onCloseRequested, onEngine, onOutput, outputLines, type EngineEvent, type Edit, type OutputText, type StartInfo, type View } from './api';
import { BottomPanel, addLines, type Line } from './BottomPanel';
import { runCheck } from './check';
import { GraphView, useFit } from './GraphView';
import { Inspector } from './Inspector';
import { NO_RUN, type RunState } from './NodeView';
import { ProjectPanel, type Inspect } from './ProjectPanel';
import { StartScreen } from './StartScreen';

/** Listens to a Tauri event while the component is mounted. */
function useEvent<T>(subscribe: (handler: (value: T) => void) => Promise<UnlistenFn>, handler: (value: T) => void) {
  const latest = useRef(handler);
  useEffect(() => {
    latest.current = handler;
  });
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let gone = false;
    subscribe((value) => latest.current(value)).then((f) => (gone ? f() : (unlisten = f)));
    return () => {
      gone = true;
      unlisten?.();
    };
  }, [subscribe]);
}

function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
}

function onlyMoves(edit: Edit): boolean {
  return edit.kind === 'MoveNode' || (edit.kind === 'Group' && edit.edits.every(onlyMoves));
}

const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
const millis = (ms: number) => `${ms < 10 ? ms.toFixed(2) : ms.toFixed(0)} ms`;

function Zoom() {
  const { zoom } = useViewport();
  return <span className="zoom" aria-label="Zoom">{Math.round(zoom * 100)}%</span>;
}

const Icon = ({ d }: { d: string }) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
    <path d={d} />
  </svg>
);

let checkStarted = false;

export function App() {
  const [start, setStart] = useState<StartInfo | null>(null);
  const [view, setView] = useState<View | null>(null);
  const [run, setRun] = useState<RunState>(NO_RUN);
  const [output, setOutput] = useState<Line[]>([]);
  const [logs, setLogs] = useState<Line[]>([]);
  const [error, setError] = useState('');
  const [inspect, setInspect] = useState<Inspect>(null);
  const [ask, setAsk] = useState<'close' | 'quit' | null>(null);
  const serial = useRef(-1);
  const fit = useFit();

  const log = useCallback((more: Line[]) => setLogs((l) => addLines(l, more)), []);

  const show = useCallback((v: View) => {
    if (v.serial !== serial.current) {
      // Another project: the old results do not apply.
      serial.current = v.serial;
      setRun(NO_RUN);
      setOutput([]);
      setLogs([]);
      setInspect(null);
    }
    setView(v);
    setError('');
  }, []);

  const act = useCallback(
    (job: Promise<View>, after?: (v: View) => void) =>
      job
        .then((v) => {
          show(v);
          after?.(v);
        })
        .catch((e) => setError(String(e))),
    [show],
  );

  const markStale = useCallback((v: View) => {
    if (!v.live) setRun((r) => (r.phase === 'none' || r.stale ? r : { ...r, stale: true }));
  }, []);

  const edit = useCallback(
    (e: Edit) => act(api.edit(e), onlyMoves(e) ? undefined : markStale),
    [act, markStale],
  );
  const undo = useCallback(() => act(api.undo(), markStale), [act, markStale]);
  const redo = useCallback(() => act(api.redo(), markStale), [act, markStale]);
  const save = useCallback(() => act(api.save()), [act]);

  const refreshStart = useCallback(() => api.startInfo().then(setStart).catch((e) => setError(String(e))), []);

  useEffect(() => {
    refreshStart();
  }, [refreshStart]);

  useEffect(() => {
    if (start?.check && !checkStarted) {
      checkStarted = true;
      runCheck(start.check, show);
    }
  }, [start, show]);

  useEvent(onEngine, (e: EngineEvent) => {
    if (e.serial !== serial.current) return;
    if (e.state === 'building') {
      setRun((r) => ({ ...r, phase: 'building' }));
      log([{ text: 'Building the graph', kind: 'info' }]);
    } else if (e.state === 'ran') {
      setRun({ phase: 'ran', builtMs: e.builtMs, runMs: e.runMs, ok: e.ok, reports: e.reports, text: '', stale: false });
      const lines: Line[] = [];
      if (e.builtMs !== null) lines.push({ text: `Built in ${seconds(e.builtMs)}`, kind: 'info' });
      lines.push({ text: `Ran in ${millis(e.runMs)}`, kind: 'info' });
      for (const r of e.reports) lines.push({ text: `${r.folder} (${r.id}): ${r.kind}: ${r.text}`, kind: 'err' });
      log(lines);
    } else {
      setRun((r) => ({ ...r, phase: 'failed', text: e.text }));
      log([{ text: e.text, kind: 'err' }]);
    }
  });

  useEvent(onOutput, (o: OutputText) => {
    const kind = o.err ? 'err' : 'out';
    setOutput((l) => addLines(l, outputLines(o.text).map((text) => ({ text, kind }))));
  });
  useEvent(onCloseRequested, () => setAsk('quit'));

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!view || typing(e.target) || !e.ctrlKey || e.altKey || document.querySelector('.backdrop')) return;
      const key = e.key.toLowerCase();
      if (key === 's') save();
      else if (key === 'z' && !e.shiftKey) undo();
      else if (key === 'y' || (key === 'z' && e.shiftKey)) redo();
      else return;
      e.preventDefault();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [view, save, undo, redo]);

  const closeProject = useCallback(() => {
    api
      .closeProject()
      .then(() => {
        setView(null);
        serial.current = -1;
        return refreshStart();
      })
      .catch((e) => setError(String(e)));
  }, [refreshStart]);

  const answer = (choice: 'save' | 'discard' | 'cancel') => {
    const what = ask;
    setAsk(null);
    if (choice === 'cancel' || !what) return;
    const go = () => (what === 'quit' ? api.quit() : closeProject());
    if (choice === 'save') api.save().then(go).catch((e) => setError(String(e)));
    else go();
  };

  const onSelect = useCallback((ids: string[]) => {
    setInspect((old) => (ids.length === 1 ? { kind: 'node', id: ids[0] } : old?.kind === 'type' ? old : null));
  }, []);

  if (!view) {
    return start ? <StartScreen start={start} onOpen={show} /> : null;
  }

  const live = view.live;
  let runText = '';
  if (run.phase === 'building') runText = 'Building';
  else if (run.phase === 'failed') runText = 'Build failed';
  else if (run.phase === 'ran') {
    runText = run.builtMs !== null ? `Built in ${seconds(run.builtMs)} - ` : '';
    runText += `Run ${millis(run.runMs ?? 0)}`;
    if (run.reports.length > 0) runText += ` - ${run.reports.length} errors`;
    if (run.stale) runText += ' - Stale';
  }

  return (
    <div className="shell">
      <header className="topbar">
        <img className="logo" src="/logo.png" alt="Patina" />
        <span className="path">
          <b>{view.name}</b> / main
        </span>
        {view.dirty && <span className="dot" title="Unsaved changes" aria-label="Unsaved changes" />}
        <button className="btn" onClick={() => (view.dirty ? setAsk('close') : closeProject())}>
          Close
        </button>
        <span className="spacer" />
        <span className={live ? 'pill live' : 'pill'}>
          <span className={live ? 'dot ready' : 'dot'} />
          {live ? 'LIVE' : 'PAUSED'}
        </span>
        <span className="sep" />
        <button className="icon-btn" aria-label="Undo" title="Undo (Ctrl+Z)" disabled={!view.canUndo} onClick={undo}>
          <Icon d="M5 3 2 6l3 3M2 6h8a4 4 0 0 1 0 8H7" />
        </button>
        <button className="icon-btn" aria-label="Redo" title="Redo (Ctrl+Y)" disabled={!view.canRedo} onClick={redo}>
          <Icon d="m11 3 3 3-3 3m3-3H6a4 4 0 0 0 0 8h3" />
        </button>
        <span className="sep" />
        <Zoom />
        <button className="btn" title="Fit the graph (F)" onClick={fit}>
          Fit
        </button>
        <span className="sep" />
        <button className="btn" title="Save (Ctrl+S)" disabled={!view.dirty} onClick={save}>
          Save
        </button>
        <button className="btn run" disabled={!view.trusted || live} onClick={() => act(api.run())}>
          <Icon d="M4 2.5v11l9-5.5z" />
          Run
        </button>
        <button className="btn" disabled={!live} onClick={() => act(api.pause())}>
          Pause
        </button>
        {/* Stop is the same as Pause in this version (spec 18, Q10). */}
        <button className="btn" disabled={!live} onClick={() => act(api.pause())}>
          Stop
        </button>
      </header>
      <div className="middle">
        <nav className="strip" aria-label="Panels">
          <button className="icon-btn active" aria-label="Project" title="Project">
            <Icon d="M1.5 3.5h5l1.5 2h6.5v8h-13z" />
          </button>
        </nav>
        <ProjectPanel view={view} inspect={inspect} onInspect={setInspect} />
        <div className="work">
          {view.trusted ? (
            <div />
          ) : (
            <div className="banner" role="alert">
              <span>This project is not trusted. Patina does not build or run its code until you trust it.</span>
              <button className="btn primary" onClick={() => act(api.trustProject())}>
                Trust project
              </button>
            </div>
          )}
          <div className="canvas">
            <GraphView view={view} run={run} onEdit={edit} onSelect={onSelect} />
          </div>
          <BottomPanel output={output} logs={logs} onClear={(tab) => (tab === 'output' ? setOutput([]) : setLogs([]))} />
        </div>
        <Inspector view={view} run={run} inspect={inspect} onEdit={edit} />
      </div>
      <footer className="statusbar">
        <span title={view.problems.join('\n')}>{view.problems.length === 0 ? 'Ready' : `${view.problems.length} problems`}</span>
        {runText && <span>{runText}</span>}
        <span className="spacer" />
        {error && <span className="error-text">{error}</span>}
      </footer>
      {ask && (
        <div className="backdrop" onKeyDown={(e) => e.key === 'Escape' && answer('cancel')}>
          <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="ask-title">
            <h2 id="ask-title">Save the changes to {view.name}?</h2>
            <div className="label">If you do not save, the changes are lost.</div>
            <div className="actions">
              <button className="btn primary" autoFocus onClick={() => answer('save')}>
                Save
              </button>
              <button className="btn" onClick={() => answer('discard')}>
                Discard
              </button>
              <button className="btn" onClick={() => answer('cancel')}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
