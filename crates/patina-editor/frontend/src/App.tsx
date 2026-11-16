// The editor window (spec 15.1): the top bar, the dock with the graph and the panels (15.2), and
// the status bar. The start screen shows when no project is open.

import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { useViewport } from '@xyflow/react';
import { api, folderName, newId, onCloseRequested, type Edit, type StartInfo, type View } from './api';
import { HelpMenu } from './About';
import { runCheck } from './check';
import { runShot } from './shot';
import { closeCode, onCodeError, reloadCode, restoreCode, saveCode, showErrors, subscribe, unsavedFiles } from './code';
import type { Reveal } from './CodeTab';
import { Confirm } from './Confirm';
import { Dock, endLayout, flushLayout, loadLayout, useLayoutSerial, ViewsMenu, type Panel } from './Dock';
import { useFloatHost } from './Floats';
import { GraphView, useFit } from './GraphView';
import { Inspector, type InspectorTab } from './Inspector';
import type { PanelId, ViewSlot } from './layout';
import { Lines } from './Lines';
import { ExpandGui, FullGui } from './NodeGui';
import { LayersPanel } from './LayersPanel';
import type { WireData } from './WireLayer';
import { ProjectPanel, type Inspect } from './ProjectPanel';
import { useProjectActions } from './ProjectActions';
import { StatusBar, useRun } from './RunStatus';
import { PortTest, scopeRun, TEST_USE } from './NodeView';
import { ViewPanel } from './ViewPanel';
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
  const [error, setError] = useState('');
  const [inspect, setInspect] = useState<Inspect>(null);
  const [ask, setAsk] = useState<'close' | 'quit' | null>(null);
  const [tab, setTab] = useState<InspectorTab>('parameters');
  const [reveal, setReveal] = useState<Reveal | null>(null);
  const [focus, setFocus] = useState<{ ids: string[]; n: number } | null>(null);
  // The node whose GUI fills the graph area (4.3). The Inspector shows it.
  const [full, setFull] = useState<string | null>(null);
  const selected = useRef<string[]>([]);
  const serial = useRef(-1);
  const revision = useRef(0);
  const current = useRef<View | null>(null);
  const unsaved = useSyncExternalStore(subscribe, unsavedFiles);
  const layoutSerial = useLayoutSerial();
  const fit = useFit();
  const { run, guis, output, setOutput, logs, setLogs, busy, stuck, log, reset, markStale, watchStop } = useRun(serial);

  useEffect(() => onCodeError(setError), []);

  const show = useCallback(
    (v: View) => {
      if (v.serial !== serial.current) {
        // Another project: the old results do not apply.
        serial.current = v.serial;
        revision.current = v.codeRevision;
        loadLayout(v.id, v.serial).then((e) => e && setError(e));
        closeCode();
        reset();
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
    },
    [reset],
  );

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
  // A double-click on a use opens the subgraph with the live values of that use (15.9).
  const [scope, setScope] = useState('');
  const openGraph = useCallback(
    async (name: string, use?: string) => {
      if (current.current?.dirty && !(await saveAll())) return undefined;
      setScope((s) => (use === undefined ? '' : s ? `${s}/${use}` : use));
      let opened: View | undefined;
      await act(api.openGraph(name), (v) => (opened = v));
      return opened;
    },
    [act, saveAll],
  );
  const testing = view?.test != null;
  const scoped = useMemo(() => scopeRun(run, view?.graphName === 'main' ? '' : testing ? TEST_USE : scope), [run, scope, view?.graphName, testing]);

  // The subgraph runs alone. Each bool public input starts off, and a click switches it (15.9).
  const startTest = useCallback(() => {
    const info = current.current?.nodes.find((n) => n.subgraph && n.name === current.current?.graphName);
    act(api.test(Object.fromEntries((info?.inputs ?? []).filter((p) => p.ty === 'bool').map((p) => [p.key, 'false']))));
  }, [act]);
  const switchInput = useCallback(
    (key: string) => {
      const inputs = current.current?.test;
      if (inputs) act(api.test({ ...inputs, [key]: inputs[key] === 'true' ? 'false' : 'true' }));
    },
    [act],
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
    setInspect(group !== 'nodes' ? { kind: 'folder', folder: `${group}/${name}` } : entry ? { kind: 'node', id: entry.id } : { kind: 'type', name });
    if (entry) setFocus({ ids: [entry.id], n: at.n });
    setTab('code');
    setReveal(at);
  }, []);

  const actions = useProjectActions({ show, markStale, log, setInspect, setError });

  const refreshStart = useCallback(() => api.startInfo().then(setStart).catch((e) => setError(String(e))), []);
  const closeSettings = useCallback(() => setSettings(false), []);

  useEffect(() => {
    refreshStart();
  }, [refreshStart]);

  useEffect(() => {
    if (start?.check && !checkStarted) {
      checkStarted = true;
      if (start.shot) runShot(start.shot, show, openGraph);
      else runCheck(start.check, show, pick, markStale);
    }
  }, [start, show, pick, markStale, openGraph]);

  useEffect(() => showErrors(run.errors), [run.errors]);

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
    endLayout()
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

  const expand = useCallback((id: string) => {
    setFull(id);
    setInspect({ kind: 'node', id });
  }, []);
  const closeFull = useCallback(() => setFull(null), []);
  const selectNodes = useCallback((ids: string[]) => setFocus({ ids, n: 0 }), []);
  const sendFloat = useFloatHost(view, selected, show, selectNodes);

  const onSelect = useCallback((ids: string[], wires: WireData[]) => {
    selected.current = ids;
    sendFloat();
    setInspect((old) => {
      if (ids.length === 1) return { kind: 'node', id: ids[0] };
      if (ids.length === 0 && wires.length === 1) return { kind: 'wire', from: wires[0].from, to: wires[0].to };
      return old?.kind === 'type' || old?.kind === 'folder' ? old : null;
    });
  }, [sendFloat]);

  if (!view) {
    if (settings) return <SettingsScreen onBack={closeSettings} />;
    return start ? <StartScreen start={start} onOpen={show} onSettings={() => setSettings(true)} /> : null;
  }

  const live = view.live;
  const fullEntry = full ? view.graph.node?.find((n) => n.id === full) : undefined;
  const stop = () => {
    watchStop();
    act(api.stop());
  };
  const dirty = view.dirty || unsaved.length > 0;
  const clear = (f: () => void) => (
    <button className="tab" onClick={f}>
      Clear
    </button>
  );
  const panels: Record<Exclude<PanelId, ViewSlot>, Panel> = {
    layers: { body: <LayersPanel run={run} /> },
    project: {
      body: (
        <ProjectPanel
          view={view}
          inspect={inspect}
          onInspect={setInspect}
          onNew={actions.newFolder}
          onImport={actions.pickImport}
          onUpdate={actions.pickUpdate}
          onPackage={actions.makePackage}
          onOpen={openGraph}
        />
      ),
    },
    inspector: {
      body: (
        <Inspector
          view={view}
          run={scoped}
          guis={guis}
          inspect={inspect}
          tab={tab}
          reveal={reveal}
          onTab={setTab}
          onEdit={edit}
          onSave={saveAll}
          onAddCrate={actions.addCrate}
          onPick={pick}
        />
      ),
    },
    logs: { body: <Lines lines={logs} label="Logs" onPick={pick} />, actions: clear(() => setLogs([])) },
    output: { body: <Lines lines={output} label="Output" onPick={pick} />, actions: clear(() => setOutput([])) },
  };

  const viewPanel = (slot: ViewSlot, name: string | null) => (
    <ViewPanel name={name && view.views.includes(name) ? name : null} instance={slot} revision={view.codeRevision} trusted={view.trusted} onView={show} selected={selected} onSelect={selectNodes} />
  );

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
        <button className="btn" title="Run one frame of the paused graph" disabled={!view.trusted || live || !run.framed || run.stale} onClick={() => act(api.step())}>
          Step
        </button>
        <button className="btn" disabled={!live && !busy} onClick={stop}>
          Stop
        </button>
        <button className="btn" title="Fuse the node in the slot back into the base now" disabled={!live || run.fused} onClick={() => act(api.bake())}>
          Bake
        </button>
        <span className="sep" />
        <ViewsMenu views={view.views} />
        <HelpMenu />
      </header>
      {/* The dock waits for the layout of the project, so the graph opens at its saved pan and zoom. */}
      {layoutSerial === view.serial ? (
        <ExpandGui.Provider value={expand}>
          <Dock panels={panels} view={viewPanel}>
            {!view.trusted && (
              <div className="banner" role="alert">
                <span>This project is not trusted. Patina does not build or run its code until you trust it.</span>
                <button className="btn primary" onClick={() => act(api.trustProject())}>
                  Trust project
                </button>
              </div>
            )}
            <div className="canvas">
              <PortTest.Provider value={switchInput}>
                <GraphView key={`${view.serial} ${view.graphName}`} view={view} run={scoped} guis={guis} onEdit={edit} onSelect={onSelect} focus={focus} onOpen={openGraph} onCollapse={collapse} onExpand={expandUses} />
              </PortTest.Provider>
              {view.graphName !== 'main' && (
                <nav className="graph-path" aria-label="Graph path">
                  <button className="btn" onClick={() => openGraph('main')}>
                    main
                  </button>
                  <span>/ {view.graphName}</span>
                  <button
                    className={testing ? 'btn primary' : 'btn'}
                    aria-pressed={testing}
                    disabled={!view.trusted}
                    onClick={() => (testing ? act(api.test(null)) : startTest())}
                    title={testing ? 'End the test. The main graph runs again.' : 'Run this subgraph alone. Click an input switch to change it.'}
                  >
                    {testing ? 'End test' : 'Test'}
                  </button>
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
      <StatusBar view={view} run={run} stuck={stuck} error={error} onPick={pick} />
      {ask && (
        <Confirm
          title={`Save the changes to ${view.name}?`}
          label="If you do not save, the changes are lost."
          answers={[
            ['Save', () => answer('save')],
            ['Discard', () => answer('discard')],
            ['Cancel', () => answer('cancel')],
          ]}
          onCancel={() => answer('cancel')}
        />
      )}
      {actions.dialogs}
      {view.recovery && view.trusted && (
        <Confirm
          title={`Restore the unsaved changes to ${view.name}?`}
          label="Patina stopped before you saved the changes."
          answers={[
            ['Restore', () => recover(true)],
            ['Discard', () => recover(false)],
          ]}
        />
      )}
    </div>
  );
}
