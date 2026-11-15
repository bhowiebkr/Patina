// The editor window (spec 15.1): the top bar, the dock with the graph and the panels (15.2), and
// the status bar. The start screen shows when no project is open.

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { useViewport } from '@xyflow/react';
import {
  api,
  errorText,
  folderName,
  logKind,
  millis,
  newId,
  onCloseRequested,
  onEngine,
  onOutput,
  outputLines,
  reportText,
  seconds,
  type Diagnostic,
  type EngineEvent,
  type Edit,
  type OutputText,
  type StartInfo,
  type View,
} from './api';
import { HelpMenu } from './About';
import { runCheck } from './check';
import { closeCode, onCodeError, reloadCode, restoreCode, saveCode, showErrors, subscribe, unsavedFiles } from './code';
import { revealOf, type Reveal } from './CodeTab';
import { Dock, flushLayout, loadLayout, useLayoutSerial, ViewsMenu, type Panel } from './Dock';
import { GraphView, useFit } from './GraphView';
import { Inspector, type InspectorTab } from './Inspector';
import type { PanelId } from './layout';
import { addLines, Lines, type Line } from './Lines';
import { ExpandGui, FullGui } from './NodeGui';
import { LayersPanel } from './LayersPanel';
import { NO_RUN, valueStale, type RunState } from './NodeView';
import type { WireData } from './WireLayer';
import { ProjectPanel, type Inspect } from './ProjectPanel';
import { ImportDialog } from './ImportDialog';
import { UpdateDialog } from './UpdateDialog';
import { SettingsScreen } from './SettingsScreen';
import { StartScreen } from './StartScreen';
import { useEvent } from './useEvent';

function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
}

/** True when the edit changes only the drawing, so the result of the run stays current. */
function onlyGeometry(edit: Edit): boolean {
  return edit.kind === 'MoveNode' || edit.kind === 'MovePin' || edit.kind === 'SetRoute' || (edit.kind === 'Group' && edit.edits.every(onlyGeometry));
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
  const [settings, setSettings] = useState(false);
  const [view, setView] = useState<View | null>(null);
  const [run, setRun] = useState<RunState>(NO_RUN);
  // The widget lines of each node GUI, by "<mode> <node id>" (4.3).
  const [guis, setGuis] = useState<ReadonlyMap<string, string>>(new Map());
  const [output, setOutput] = useState<Line[]>([]);
  const [logs, setLogs] = useState<Line[]>([]);
  const [error, setError] = useState('');
  const [importing, setImporting] = useState<string | null>(null);
  const [updating, setUpdating] = useState<{ other: string; folder: string } | null>(null);
  const [inspect, setInspect] = useState<Inspect>(null);
  const [ask, setAsk] = useState<'close' | 'quit' | null>(null);
  const [tab, setTab] = useState<InspectorTab>('parameters');
  const [reveal, setReveal] = useState<Reveal | null>(null);
  const [focus, setFocus] = useState<{ id: string; n: number } | null>(null);
  // True while a build or a run is active, so Stop can stop it after Pause (15.2).
  const [busy, setBusy] = useState(false);
  const [stuck, setStuck] = useState('');
  const stopTimer = useRef<ReturnType<typeof setInterval> | undefined>(undefined);
  const serial = useRef(-1);
  const revision = useRef(0);
  const current = useRef<View | null>(null);
  const unsaved = useSyncExternalStore(subscribe, unsavedFiles);
  const layoutSerial = useLayoutSerial();
  const fit = useFit();

  const log = useCallback((more: Line[]) => setLogs((l) => addLines(l, more)), []);
  useEffect(() => onCodeError(setError), []);

  const endStop = useCallback(() => {
    clearInterval(stopTimer.current);
    stopTimer.current = undefined;
    setStuck('');
  }, []);

  const show = useCallback((v: View) => {
    if (v.serial !== serial.current) {
      // Another project: the old results do not apply.
      serial.current = v.serial;
      revision.current = v.codeRevision;
      loadLayout(v.id, v.serial).then((e) => e && setError(e));
      closeCode();
      setRun(NO_RUN);
      setBusy(false);
      endStop();
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
  }, [endStop]);

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

  // Only the open graph can have unsaved changes, so a change of graph saves all first (6.1).
  const openGraph = useCallback(
    async (name: string) => {
      if (!current.current?.dirty || (await saveAll())) act(api.openGraph(name));
    },
    [act, saveAll],
  );

  // Ctrl+G moves the selected nodes into a new subgraph (6.1).
  const collapse = useCallback(
    async (name: string, ids: string[]) => {
      const v = await api.collapse(name, ids, newId());
      show(v);
      markStale(v);
    },
    [show, markStale],
  );

  const expandUses = useCallback((ids: string[]) => act(api.expand(ids), markStale), [act, markStale]);

  /** Shows a place in the code: selects its node and opens the Code tab at the line (15.5). */
  const pick = useCallback((at: Reveal) => {
    const [group, name = ''] = at.path.split('/');
    const entry = current.current?.graph.node?.find((n) => n.folder === `nodes/${name}`);
    setInspect(group === 'widgets' ? { kind: 'widget', name } : entry ? { kind: 'node', id: entry.id } : { kind: 'type', name });
    if (entry) setFocus({ id: entry.id, n: at.n });
    setTab('code');
    setReveal(at);
  }, []);

  /** Makes a node folder and shows its code in the Inspector (15.2). */
  const newNode = useCallback(
    async (name: string) => {
      show(await api.newNode(name));
      setInspect({ kind: 'type', name });
    },
    [show],
  );

  /** Makes a widget folder and shows its code in the Inspector (4.3). */
  const pickImport = useCallback(
    () =>
      api
        .pickFolder('')
        .then((folder) => folder && setImporting(folder))
        .catch((e) => setError(String(e))),
    [],
  );

  const pickUpdate = useCallback(
    (folder: string) =>
      api
        .updateSource(folder)
        .then((other) => other ?? api.pickFolder(''))
        .then((other) => other && setUpdating({ other, folder }))
        .catch((e) => setError(String(e))),
    [],
  );

  const newWidget = useCallback(
    async (name: string) => {
      show(await api.newWidget(name));
      setInspect({ kind: 'widget', name });
    },
    [show],
  );

  /** Adds a crate to a node or widget folder with `cargo add`. Undo removes it (15.4). */
  const addCrate = useCallback(
    async (folder: string, spec: string) => {
      show(await api.addCrate(folder, spec));
    },
    [show],
  );

  const refreshStart = useCallback(() => api.startInfo().then(setStart).catch((e) => setError(String(e))), []);
  const closeSettings = useCallback(() => setSettings(false), []);

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
    if (e.state === 'gui') {
      setGuis((g) => new Map([...g, ...e.trees.map((t) => [`${t.mode} ${t.id}`, t.tree] as const)]));
      if (e.reports.length > 0) log(e.reports.map((r) => ({ text: `${r.folder} (${r.id}): ${reportText(r)}`, kind: 'err' as const })));
      return;
    }
    const active = e.state === 'building' || e.state === 'started' || e.state === 'frame' || (e.state === 'ran' && e.frames);
    setBusy(active);
    if (!active) endStop();
    if (e.state === 'started') return;
    if (e.state === 'frame') {
      // The values and times of the batch nodes stay. Only the frame nodes ran.
      setRun((r) => ({
        ...r,
        runMs: e.runMs,
        ok: e.ok,
        reports: e.reports,
        values: new Map([...r.values, ...e.values.map((v) => [v.from, v] as const)]),
        idle: new Map([...e.skipped.map((id) => [id, 'skipped'] as const), ...e.stopped.map((id) => [id, 'stopped'] as const)]),
        times: new Map([...r.times, ...e.times.map((t) => [t.id, t.ms] as const)]),
        logs: e.logs,
        frames: r.frames + 1,
      }));
      const lines: Line[] = [];
      for (const l of e.logs) lines.push({ text: `${l.folder} (${l.id}): ${l.level} ${l.text}`, kind: logKind(l.level) });
      for (const r of e.reports) lines.push({ text: `${r.folder} (${r.id}): ${reportText(r)}`, kind: 'err' });
      if (lines.length > 0) log(lines);
      return;
    }
    if (e.state === 'ended') {
      log([{ text: `Ran ${plural(e.frames, 'frame')}`, kind: 'info' }]);
      return;
    }
    if (e.state === 'building') {
      setRun((r) => ({ ...r, phase: 'building' }));
      setGuis(new Map());
      log([{ text: 'Building the graph', kind: 'info' }]);
    } else if (e.state === 'ran') {
      const failed = e.errors.length > 0;
      const idle = new Map([...e.skipped.map((id) => [id, 'skipped'] as const), ...e.stopped.map((id) => [id, 'stopped'] as const)]);
      const values = new Map(e.values.map((v) => [v.from, v]));
      setRun((r) => {
        // A node that did not run keeps its old values, and they show as stale (15.3).
        for (const [from, v] of r.values) if (!values.has(from) && valueStale({ ...r, stale: false, idle }, from)) values.set(from, v);
        return {
        phase: 'ran',
        builtMs: e.builtMs,
        lastBuildMs: e.builtMs !== null && !failed ? e.builtMs : r.lastBuildMs,
        // A build of the base with a node in the slot also builds the slot (7.1).
        baseMs: e.builtMs !== null && !failed && !e.slotOnly ? e.builtMs : r.baseMs,
        slotMs: e.slot === null ? null : e.builtMs !== null && !failed ? e.builtMs : r.slotMs,
        runMs: e.runMs,
        ok: e.ok,
        reports: e.reports,
        values,
        // A run without a build keeps the errors of the last build.
        errors: e.builtMs === null ? r.errors : e.errors,
        text: '',
        stale: false,
        idle,
        times: new Map(e.times.map((t) => [t.id, t.ms])),
        logs: e.logs,
        frames: 0,
        fused: e.fused,
        slot: e.slot,
        };
      });
      const lines: Line[] = [];
      if (failed) lines.push({ text: 'The build failed. Running the last build that compiled.', kind: 'err' }, ...errorLines(e.errors));
      else if (e.builtMs !== null) lines.push({ text: `Built in ${seconds(e.builtMs)}`, kind: 'info' });
      lines.push({ text: `Ran in ${millis(e.runMs)}`, kind: 'info' });
      if (e.stopped.length > 0) lines.push({ text: 'Stop ended the run before all nodes ran.', kind: 'info' });
      for (const l of e.logs) lines.push({ text: `${l.folder} (${l.id}): ${l.level} ${l.text}`, kind: logKind(l.level) });
      for (const r of e.reports) lines.push({ text: `${r.folder} (${r.id}): ${reportText(r)}`, kind: 'err' });
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
    flushLayout()
      .then(() => api.closeProject())
      .then(() => {
        setView(null);
        closeCode();
        serial.current = -1;
        return refreshStart();
      })
      .catch((e) => setError(String(e)));
  }, [refreshStart]);

  // The answer to a recovery file (15.10). The restored code goes into the Code tab as unsaved text.
  const recover = (restore: boolean) =>
    api
      .recover(restore)
      .then(({ view: v, code }) => {
        show(v);
        return restoreCode(code);
      })
      .then((e) => e && setError(e))
      .catch((e) => setError(String(e)));

  const answer = (choice: 'save' | 'discard' | 'cancel') => {
    const what = ask;
    setAsk(null);
    if (choice === 'cancel' || !what) return;
    const go = () => (what === 'quit' ? flushLayout().then(() => api.quit()) : closeProject());
    if (choice === 'save') saveAll().then((ok) => ok && go());
    else go();
  };

  // The node whose GUI fills the graph area (4.3). The Inspector shows it.
  const [full, setFull] = useState<string | null>(null);
  const expand = useCallback((id: string) => {
    setFull(id);
    setInspect({ kind: 'node', id });
  }, []);
  const closeFull = useCallback(() => setFull(null), []);

  const onSelect = useCallback((ids: string[], wires: WireData[]) => {
    setInspect((old) => {
      if (ids.length === 1) return { kind: 'node', id: ids[0] };
      if (ids.length === 0 && wires.length === 1) return { kind: 'wire', from: wires[0].from, to: wires[0].to };
      return old?.kind === 'type' || old?.kind === 'widget' ? old : null;
    });
  }, []);

  if (!view) {
    if (settings) return <SettingsScreen onBack={closeSettings} />;
    return start ? <StartScreen start={start} onOpen={show} onSettings={() => setSettings(true)} /> : null;
  }

  const live = view.live;
  const fullEntry = full ? view.graph.node?.find((n) => n.id === full) : undefined;
  // After Stop, a node call that runs for 2 s or more shows in the status bar (15.2).
  const stop = () => {
    endStop();
    act(api.stop());
    const from = Date.now();
    const timer = setInterval(() => {
      const s = Math.floor((Date.now() - from) / 1000);
      if (s < 2) return;
      api.running().then((id) => {
        if (stopTimer.current !== timer) return;
        if (id === null) endStop();
        else setStuck(`Node ${id} does not stop - ${s} s`);
      }, endStop);
    }, 1000);
    stopTimer.current = timer;
  };
  const dirty = view.dirty || unsaved.length > 0;
  let runText = '';
  if (run.phase === 'building') runText = 'Building';
  else if (run.phase === 'failed') runText = 'Build failed';
  else if (run.phase === 'ran') {
    const parts: string[] = [];
    if (run.errors.length > 0) parts.push('Running the last build that compiled');
    else if (run.builtMs !== null) parts.push(`Built in ${seconds(run.builtMs)}`);
    parts.push(`Run ${millis(run.runMs ?? 0)}`);
    if (run.frames > 0) parts.push(`Frame ${run.frames}`);
    if (run.reports.length > 0) parts.push(plural(run.reports.length, 'run error'));
    const idle = [...run.idle.values()];
    const skipped = idle.filter((s) => s === 'skipped').length;
    if (skipped > 0) parts.push(plural(skipped, 'skipped node'));
    if (run.stale) parts.push('Stale');
    else if (idle.length > skipped) parts.push(plural(idle.length - skipped, 'stale node'));
    runText = parts.join(' - ');
  }
  const firstError = run.errors.map((e) => revealOf(e, Date.now())).find((at) => at !== null);
  const clear = (f: () => void) => (
    <button className="tab" onClick={f}>
      Clear
    </button>
  );
  const panels: Record<PanelId, Panel> = {
    layers: { body: <LayersPanel run={run} /> },
    project: { body: <ProjectPanel view={view} inspect={inspect} onInspect={setInspect} onNewNode={newNode} onNewWidget={newWidget} onImport={pickImport} onUpdate={pickUpdate} /> },
    inspector: {
      body: (
        <Inspector
          view={view}
          run={run}
          guis={guis}
          inspect={inspect}
          tab={tab}
          reveal={reveal}
          onTab={setTab}
          onEdit={edit}
          onSave={saveAll}
          onAddCrate={addCrate}
          onPick={pick}
        />
      ),
    },
    logs: { body: <Lines lines={logs} label="Logs" onPick={pick} />, actions: clear(() => setLogs([])) },
    output: { body: <Lines lines={output} label="Output" onPick={pick} />, actions: clear(() => setOutput([])) },
  };

  return (
    <div className="shell">
      <header className="topbar">
        <div className="brand">
          <b className="brand-name">Patina</b>
        </div>
        <span className="path">
          <b>{view.name}</b> / main{fullEntry && ` / ${folderName(fullEntry)}`}
        </span>
        {fullEntry && (
          <button className="btn" title="Back to the graph (Esc)" onClick={closeFull}>
            Back
          </button>
        )}
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
        <button className="btn" disabled={!live && !busy} onClick={stop}>
          Stop
        </button>
        <button className="btn" title="Fuse the node in the slot back into the base now" disabled={!live || run.fused} onClick={() => act(api.bake())}>
          Bake
        </button>
        <span className="sep" />
        <ViewsMenu />
        <HelpMenu />
      </header>
      {/* The dock waits for the layout of the project, so the graph opens at its saved pan and zoom. */}
      {layoutSerial === view.serial ? (
        <ExpandGui.Provider value={expand}>
          <Dock panels={panels}>
            {!view.trusted && (
              <div className="banner" role="alert">
                <span>This project is not trusted. Patina does not build or run its code until you trust it.</span>
                <button className="btn primary" onClick={() => act(api.trustProject())}>
                  Trust project
                </button>
              </div>
            )}
            <div className="canvas">
              <GraphView key={`${view.serial} ${view.graphName}`} view={view} run={run} guis={guis} onEdit={edit} onSelect={onSelect} focus={focus} onOpen={openGraph} onCollapse={collapse} onExpand={expandUses} />
              {view.graphName !== 'main' && (
                <nav className="graph-path" aria-label="Graph path">
                  <button className="btn" onClick={() => openGraph('main')}>
                    main
                  </button>
                  <span>/ {view.graphName}</span>
                </nav>
              )}
              {fullEntry && (
                <FullGui
                  id={fullEntry.id}
                  tree={guis.get(`2 ${fullEntry.id}`) ?? guis.get(`1 ${fullEntry.id}`) ?? guis.get(`0 ${fullEntry.id}`) ?? ''}
                  disabled={!view.trusted}
                  onBack={closeFull}
                />
              )}
            </div>
          </Dock>
        </ExpandGui.Provider>
      ) : (
        <div className="middle" />
      )}
      <footer className="statusbar">
        <span title={view.problems.join('\n')}>{view.problems.length === 0 ? 'Ready' : `${view.problems.length} problems`}</span>
        {runText && <span>{runText}</span>}
        {run.phase === 'ran' && <span>{run.slot !== null ? `Slot: ${run.slot}` : run.fused ? 'Fused' : 'Edit build'}</span>}
        {stuck && <span className="error-text">{stuck}</span>}
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
      {importing && (
        <ImportDialog
          other={importing}
          onDone={(v) => {
            setImporting(null);
            show(v);
            markStale(v);
          }}
          onClose={() => setImporting(null)}
        />
      )}
      {updating && (
        <UpdateDialog
          {...updating}
          onDone={(v) => {
            setUpdating(null);
            show(v);
            markStale(v);
          }}
          onClose={() => setUpdating(null)}
        />
      )}
      {view.recovery && view.trusted && (
        <div className="backdrop">
          <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="recover-title">
            <h2 id="recover-title">Restore the unsaved changes to {view.name}?</h2>
            <div className="label">Patina stopped before you saved the changes.</div>
            <div className="actions">
              <button className="btn primary" autoFocus onClick={() => recover(true)}>
                Restore
              </button>
              <button className="btn" onClick={() => recover(false)}>
                Discard
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
