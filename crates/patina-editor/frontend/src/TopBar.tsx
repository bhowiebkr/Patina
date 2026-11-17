// The top bar of an open project: the path, Close, the run controls and the menus (15.1).
import { useViewport } from '@xyflow/react';
import { api, folderName, type NodeEntry, type View } from './api';
import { HelpMenu } from './dialogs/About';
import { ViewsMenu } from './panels/Dock';
import { Field } from './controls/Field';
import { useFit } from './canvas/GraphView';
import type { RunState } from './canvas/NodeView';

function Zoom() {
  const { zoom } = useViewport();
  return <span className="zoom" aria-label="Zoom">{Math.round(zoom * 100)}%</span>;
}

const Icon = ({ d }: { d: string }) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
    <path d={d} />
  </svg>
);

export function TopBar(p: {
  view: View;
  run: RunState;
  busy: boolean;
  dirty: boolean;
  full: NodeEntry | undefined;
  act: (job: Promise<View>) => void;
  onBack: () => void;
  onClose: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
  onStop: () => void;
}) {
  const { view, run, act } = p;
  const live = view.live;
  const fit = useFit();
  return (
    <header className="topbar">
      <div className="brand">
        <b className="brand-name">Patina</b>
      </div>
      <span className="path">
        <b>{view.name}</b> / {view.graphName}{p.full && ` / ${folderName(p.full)}`}
      </span>
      {p.full && (
        <button className="btn" title="Back to the graph (Esc)" onClick={p.onBack}>
          Back
        </button>
      )}
      {p.dirty && <span className="dot" title="Unsaved changes" aria-label="Unsaved changes" />}
      <button className="btn" onClick={p.onClose}>
        Close
      </button>
      <span className="spacer" />
      <span className={live ? 'pill live' : 'pill'}>
        <span className={live ? 'dot ready' : 'dot'} />
        {live ? 'LIVE' : 'PAUSED'}
      </span>
      <span className="sep" />
      <button className="icon-btn" aria-label="Undo" title="Undo (Ctrl+Z)" disabled={!view.canUndo} onClick={p.onUndo}>
        <Icon d="M5 3 2 6l3 3M2 6h8a4 4 0 0 1 0 8H7" />
      </button>
      <button className="icon-btn" aria-label="Redo" title="Redo (Ctrl+Y)" disabled={!view.canRedo} onClick={p.onRedo}>
        <Icon d="m11 3 3 3-3 3m3-3H6a4 4 0 0 0 0 8h3" />
      </button>
      <span className="sep" />
      <Zoom />
      <button className="btn" title="Fit the graph (F)" onClick={fit}>
        Fit
      </button>
      <span className="sep" />
      <button className="btn" title="Save (Ctrl+S)" disabled={!p.dirty} onClick={p.onSave}>
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
      {/* After a run, a node GUI or a state drop can still run and not return (15.2). */}
      <button className="btn" disabled={!live && !p.busy && run.phase !== 'ran'} onClick={p.onStop}>
        Stop
      </button>
      <button className="btn" title="Fuse the node in the slot back into the base now" disabled={!live || run.fused} onClick={() => act(api.bake())}>
        Bake
      </button>
      <span className="fps" title="The frames each second of the project. The clock node ticks each 4 frames.">
        <Field w={{ kind: 'number', index: 0, text: 'FPS', value: view.frameRate, step: 0, unit: '' }} send={(rate) => act(api.frameRate(Math.round(Number(rate))))} disabled={false} />
      </span>
      <span className="sep" />
      <ViewsMenu views={view.views} />
      <HelpMenu />
    </header>
  );
}
