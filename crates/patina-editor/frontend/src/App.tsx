// The editor window (spec 15.1): the top bar, the dock with the graph and the panels (15.2), and
// the status bar. The start screen shows when no project is open.

import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { api, folderName, type StartInfo, type View } from './api';
import { analyzerProject, closeCode, onCodeError, reloadCode, showErrors, subscribe, unsavedFiles } from './code/codeState';
import type { Reveal } from './code/reveal';
import { Dock, loadLayout, useLayoutSerial, type Panel } from './panels/Dock';
import { useFloatHost } from './panels/Floats';
import { GraphPath } from './canvas/GraphPath';
import { GraphView } from './canvas/GraphView';
import { Inspector, type InspectorTab } from './panels/Inspector';
import { useEditKeys } from './canvas/keys';
import type { PanelId, ViewSlot } from './panels/layout';
import { Lines } from './run/Lines';
import { ExpandGui, FullGui } from './controls/NodeGui';
import { LayersPanel } from './panels/LayersPanel';
import type { WireData } from './wires/WireLayer';
import { ProjectPanel, type Inspect } from './panels/ProjectPanel';
import { useProjectActions } from './dialogs/ProjectActions';
import { useProjectClose } from './dialogs/useProjectClose';
import { StatusBar, useRun } from './run/RunStatus';
import { DebugPanel } from './panels/DebugPanel';
import { FireTrigger, OpenUse, PortTest } from './canvas/NodeView';
import { TopBar } from './TopBar';
import { useGraphEdits } from './canvas/useGraphEdits';
import { useTrail } from './canvas/useTrail';
import { ViewPanel } from './panels/ViewPanel';
import { SettingsScreen } from './dialogs/SettingsScreen';
import { StartScreen } from './dialogs/StartScreen';

let checkStarted = false;

export function App() {
  const [start, setStart] = useState<StartInfo | null>(null);
  const [settings, setSettings] = useState(false);
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState('');
  const [inspect, setInspect] = useState<Inspect>(null);
  const [tab, setTab] = useState<InspectorTab>('parameters');
  const [reveal, setReveal] = useState<Reveal | null>(null);
  const [focus, setFocus] = useState<{ ids: string[]; n: number } | null>(null);
  // The node whose GUI fills the graph area (4.3). The Inspector shows it.
  const [full, setFull] = useState<string | null>(null);
  const selected = useRef<string[]>([]);
  const serial = useRef(-1);
  const revision = useRef(0);
  const current = useRef<View | null>(null);
  const lockBends = useRef<(to: string) => void>(() => {});
  const unsaved = useSyncExternalStore(subscribe, unsavedFiles);
  const layoutSerial = useLayoutSerial();
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

  const { act, edit, undo, redo, saveAll, startTest, switchInput, collapse, expandUses, copyNodes, paste } = useGraphEdits({ serial, current, show, markStale, log, setError, setFocus });
  const { path, testing, usePath, scoped, domains, goTo, openGraph } = useTrail(view, current, run, act);
  const fire = useCallback((id: string) => api.fire(usePath ? `${usePath}/${id}` : id, '').catch((e) => setError(String(e))), [usePath]);

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
      const check = start.check;
      const shot = start.shot;
      // The self-check and the screenshot code load only with their command-line flags.
      if (shot) import('./check/shot').then((m) => m.runShot(shot, show, openGraph));
      else import('./check/check').then((m) => m.runCheck(check, show, pick, markStale));
    }
  }, [start, show, pick, markStale, openGraph]);

  useEffect(() => showErrors(run.errors), [run.errors]);

  // The serial makes a new process after the same project opens again, because Rust stopped the old one.
  const analyzed = view?.trusted ? view.root : null;
  const opening = view?.serial ?? 0;
  useEffect(() => analyzerProject(analyzed, opening), [analyzed, opening]);

  const dirty = view !== null && (view.dirty || unsaved.length > 0);
  const closing = useProjectClose({ view, dirty, serial, show, setView, saveAll, refreshStart, setError });
  useEditKeys(view !== null, saveAll, undo, redo);

  const expand = useCallback((id: string) => {
    setFull(id);
    setInspect({ kind: 'node', id });
  }, []);
  const closeFull = useCallback(() => setFull(null), []);
  const selectNodes = useCallback((ids: string[]) => setFocus({ ids, n: 0 }), []);
  // The times have flattened IDs (9.1), so only the nodes of the main graph have a name.
  const names = useMemo(() => new Map(view && view.graphName === 'main' ? (view.graph.node ?? []).map((n) => [n.id, folderName(n)] as const) : []), [view]);
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

  const fullEntry = full ? view.graph.node?.find((n) => n.id === full) : undefined;
  const stop = () => {
    watchStop();
    act(api.stop());
  };
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
          onLock={(to) => lockBends.current(to)}
        />
      ),
    },
    logs: { body: <Lines lines={logs} label="Logs" onPick={pick} />, actions: clear(() => setLogs([])) },
    output: { body: <Lines lines={output} label="Output" onPick={pick} />, actions: clear(() => setOutput([])) },
    debug: { body: <DebugPanel times={run.times} domains={view.domains} frameRate={view.frameRate} names={names} onSelect={selectNodes} /> },
  };

  const viewPanel = (slot: ViewSlot, name: string | null) => (
    <ViewPanel name={name && view.views.includes(name) ? name : null} instance={slot} revision={view.codeRevision} trusted={view.trusted} onView={show} selected={selected} onSelect={selectNodes} />
  );

  return (
    <div className="shell">
      <TopBar view={view} run={run} busy={busy} dirty={dirty} full={fullEntry} act={act} onBack={closeFull} onClose={closing.close} onUndo={undo} onRedo={redo} onSave={saveAll} onStop={stop} />
      {/* The dock waits for the layout of the project, so the graph opens at its saved pan and zoom. */}
      {layoutSerial === view.serial ? (
        <ExpandGui.Provider value={expand}>
          <OpenUse.Provider value={openGraph}>
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
                <FireTrigger.Provider value={view.live && (view.graphName === 'main' || usePath) ? fire : null}>
                  <GraphView key={`${view.serial} ${view.graphName}`} view={view} run={scoped} domains={domains} guis={guis} onEdit={edit} onSelect={onSelect} focus={focus} onOpen={openGraph} onCollapse={collapse} onExpand={expandUses} onCopy={copyNodes} onPaste={paste} lockRef={lockBends} />
                </FireTrigger.Provider>
              </PortTest.Provider>
              {view.graphName !== 'main' && (
                <GraphPath view={view} path={path} testing={testing} act={act} onTest={startTest} onOpen={openGraph} onGo={goTo} />
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
          </OpenUse.Provider>
        </ExpandGui.Provider>
      ) : (
        <div className="middle" />
      )}
      <StatusBar view={view} run={run} stuck={stuck} error={error} onPick={pick} />
      {actions.dialogs}
      {closing.dialogs}
    </div>
  );
}
