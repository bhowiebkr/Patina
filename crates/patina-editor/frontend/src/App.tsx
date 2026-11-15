// The editor window (spec 15.1): the top bar, the Project panel, the graph, the Inspector, the
// bottom panel and the status bar. The start screen shows when no project is open.

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { useViewport } from '@xyflow/react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  api,
  errorText,
  millis,
  onCloseRequested,
  onEngine,
  onOutput,
  outputLines,
  seconds,
  type Diagnostic,
  type EngineEvent,
  type Edit,
  type OutputText,
  type StartInfo,
  type View,
} from './api';
import { BottomPanel, addLines, type Line } from './BottomPanel';
import { runCheck } from './check';
import { closeCode, reloadCode, saveCode, showErrors, subscribe, unsavedFiles } from './code';
import { revealOf, type Reveal } from './CodeTab';
import { GraphView, useFit } from './GraphView';
import { Inspector, type InspectorTab } from './Inspector';
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

/** True when the edit changes only the drawing, so the result of the run stays current. */
function onlyGeometry(edit: Edit): boolean {
  return edit.kind === 'MoveNode' || edit.kind === 'SetRoute' || (edit.kind === 'Group' && edit.edits.every(onlyGeometry));
}

/** One Logs line for each compile error. A click on a line with a place shows the code. */
function errorLines(errors: Diagnostic[]): Line[] {
  return errors.map((e): Line => {
    const at = revealOf(e, 0);
    if (!at) return { text: e.rendered.trimEnd(), kind: 'err' };
    return { text: `${e.file}:${e.start[0]}:${e.start[1]}: ${errorText(e)}`, kind: 'err', at };
  });
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

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
  const [tab, setTab] = useState<InspectorTab>('parameters');
  const [reveal, setReveal] = useState<Reveal | null>(null);
  const [focus, setFocus] = useState<{ id: string; n: number } | null>(null);
  const serial = useRef(-1);
  const revision = useRef(0);
  const current = useRef<View | null>(null);
  const unsaved = useSyncExternalStore(subscribe, unsavedFiles);
  const fit = useFit();

  const log = useCallback((more: Line[]) => setLogs((l) => addLines(l, more)), []);

  const show = useCallback((v: View) => {
    if (v.serial !== serial.current) {
      // Another project: the old results do not apply.
      serial.current = v.serial;
      revision.current = v.codeRevision;
      closeCode();
      setRun(NO_RUN);
      setOutput([]);
      setLogs([]);
      setInspect(null);
      setReveal(null);
    } else if (v.codeRevision !== revision.current) {
      // An edit, undo or redo wrote a code file.
      revision.current = v.codeRevision;
      reloadCode();
    }
    current.current = v;
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
    (e: Edit) => act(api.edit(e), onlyGeometry(e) ? undefined : markStale),
    [act, markStale],
  );
  const undo = useCallback(() => act(api.undo(), markStale), [act, markStale]);
  const redo = useCallback(() => act(api.redo(), markStale), [act, markStale]);
  // Ctrl+S saves all: one SetCode edit for each changed code file, then the graph (15.10).
  const saveAll = useCallback(async () => {
    try {
      const coded = await saveCode();
      if (coded) {
        show(coded);
        markStale(coded);
      }
      show(await api.save());
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  }, [show, markStale]);

  /** Shows a place in the code: selects its node and opens the Code tab at the line (15.5). */
  const pick = useCallback((at: Reveal) => {
    const name = at.path.split('/')[1] ?? '';
    const entry = current.current?.graph.node?.find((n) => n.folder === `nodes/${name}`);
    setInspect(entry ? { kind: 'node', id: entry.id } : { kind: 'type', name });
    if (entry) setFocus({ id: entry.id, n: at.n });
    setTab('code');
    setReveal(at);
  }, []);

  const refreshStart = useCallback(() => api.startInfo().then(setStart).catch((e) => setError(String(e))), []);

  useEffect(() => {
    refreshStart();
  }, [refreshStart]);

  useEffect(() => {
    if (start?.check && !checkStarted) {
      checkStarted = true;
      runCheck(start.check, show, pick, markStale);
    }
  }, [start, show, pick, markStale]);

  useEffect(() => showErrors(run.errors), [run.errors]);

  useEvent(onEngine, (e: EngineEvent) => {
    if (e.serial !== serial.current) return;
    if (e.state === 'building') {
      setRun((r) => ({ ...r, phase: 'building' }));
      log([{ text: 'Building the graph', kind: 'info' }]);
    } else if (e.state === 'ran') {
      const failed = e.errors.length > 0;
      setRun((r) => ({
        phase: 'ran',
        builtMs: e.builtMs,
        lastBuildMs: e.builtMs !== null && !failed ? e.builtMs : r.lastBuildMs,
        runMs: e.runMs,
        ok: e.ok,
        reports: e.reports,
        values: new Map(e.values.map((v) => [v.from, v.text])),
        // A run without a build keeps the errors of the last build.
        errors: e.builtMs === null ? r.errors : e.errors,
        text: '',
        stale: false,
      }));
      const lines: Line[] = [];
      if (failed) lines.push({ text: 'The build failed. Running the last build that compiled.', kind: 'err' }, ...errorLines(e.errors));
      else if (e.builtMs !== null) lines.push({ text: `Built in ${seconds(e.builtMs)}`, kind: 'info' });
      lines.push({ text: `Ran in ${millis(e.runMs)}`, kind: 'info' });
      for (const r of e.reports) lines.push({ text: `${r.folder} (${r.id}): ${r.kind}: ${r.text}`, kind: 'err' });
      log(lines);
    } else {
      setRun((r) => ({ ...r, phase: 'failed', text: e.text, values: NO_RUN.values, errors: e.errors }));
      log(e.errors.length > 0 ? [{ text: 'The build failed.', kind: 'err' }, ...errorLines(e.errors)] : [{ text: e.text, kind: 'err' }]);
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
      if (key === 's') saveAll();
      else if (key === 'z' && !e.shiftKey) undo();
      else if (key === 'y' || (key === 'z' && e.shiftKey)) redo();
      else return;
      e.preventDefault();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [view, saveAll, undo, redo]);

  const closeProject = useCallback(() => {
    api
      .closeProject()
      .then(() => {
        setView(null);
        closeCode();
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
    if (choice === 'save') saveAll().then((ok) => ok && go());
    else go();
  };

  const onSelect = useCallback((ids: string[]) => {
    setInspect((old) => (ids.length === 1 ? { kind: 'node', id: ids[0] } : old?.kind === 'type' ? old : null));
  }, []);

  if (!view) {
    return start ? <StartScreen start={start} onOpen={show} /> : null;
  }

  const live = view.live;
  const dirty = view.dirty || unsaved.length > 0;
  let runText = '';
  if (run.phase === 'building') runText = 'Building';
  else if (run.phase === 'failed') runText = 'Build failed';
  else if (run.phase === 'ran') {
    const parts: string[] = [];
    if (run.errors.length > 0) parts.push('Running the last build that compiled');
    else if (run.builtMs !== null) parts.push(`Built in ${seconds(run.builtMs)}`);
    parts.push(`Run ${millis(run.runMs ?? 0)}`);
    if (run.reports.length > 0) parts.push(plural(run.reports.length, 'run error'));
    if (run.stale) parts.push('Stale');
    runText = parts.join(' - ');
  }
  const firstError = run.errors.map((e) => revealOf(e, Date.now())).find((at) => at !== null);

  return (
    <div className="shell">
      <header className="topbar">
        <img className="logo" src="/logo.png" alt="Patina" />
        <span className="path">
          <b>{view.name}</b> / main
        </span>
        {dirty && <span className="dot" title="Unsaved changes" aria-label="Unsaved changes" />}
        <button className="btn" onClick={() => (dirty ? setAsk('close') : closeProject())}>
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
        <button className="btn" title="Save (Ctrl+S)" disabled={!dirty} onClick={saveAll}>
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
            <GraphView view={view} run={run} onEdit={edit} onSelect={onSelect} focus={focus} />
          </div>
          <BottomPanel output={output} logs={logs} onClear={(t) => (t === 'output' ? setOutput([]) : setLogs([]))} onPick={pick} />
        </div>
        <Inspector
          view={view}
          run={run}
          inspect={inspect}
          tab={tab}
          reveal={reveal}
          onTab={setTab}
          onEdit={edit}
          onSave={saveAll}
          onPick={pick}
        />
      </div>
      <footer className="statusbar">
        <span title={view.problems.join('\n')}>{view.problems.length === 0 ? 'Ready' : `${view.problems.length} problems`}</span>
        {runText && <span>{runText}</span>}
        {run.errors.length > 0 && (
          <button className="status-link error-text" onClick={() => firstError && pick(firstError)}>
            {plural(run.errors.length, 'compile error')}
          </button>
        )}
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
