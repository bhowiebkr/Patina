// The state of the run (15.2): the engine events, the Logs and Output lines, Stop and the status
// bar.

import { useCallback, useRef, useState, type RefObject } from 'react';
import { api, errorLine, firstSentence, millis, onEngine, onOutput, outputLines, seconds, type EngineEvent, type OutputText, type View } from '../api';
import { revealOf, type Reveal } from '../code/reveal';
import { addLines, withHints, type Line } from './Lines';
import { NO_RUN, type RunState } from '../canvas/NodeView';
import { eventLines, nextRun, nodeLines, plural } from './runEvents';
import { useEvent } from '../useEvent';

/** The run state of the open project. `serial` is the serial of the open project: other events are old. */
export function useRun(serial: RefObject<number>) {
  const [run, setRun] = useState<RunState>(NO_RUN);
  // The widget lines of each node GUI, by "<mode> <node id>" (4.3).
  const [guis, setGuis] = useState<ReadonlyMap<string, string>>(new Map());
  const [output, setOutput] = useState<Line[]>([]);
  const [logs, setLogs] = useState<Line[]>([]);
  // True while a build or a run is active, so Stop can stop it after Pause (15.2).
  const [busy, setBusy] = useState(false);
  const [stuck, setStuck] = useState('');
  const stopTimer = useRef<ReturnType<typeof setInterval> | undefined>(undefined);

  const log = useCallback((more: Line[]) => {
    const time = Date.now();
    setLogs((l) => addLines(l, withHints(more).map((line) => ({ ...line, time }))));
  }, []);

  const endStop = useCallback(() => {
    clearInterval(stopTimer.current);
    stopTimer.current = undefined;
    setStuck('');
  }, []);

  /** Clears all results, for another project. */
  const reset = useCallback(() => {
    setRun(NO_RUN);
    setGuis(new Map());
    setBusy(false);
    endStop();
    setOutput([]);
    setLogs([]);
  }, [endStop]);

  /** After an edit that changes the run, the results are for an old graph. */
  const markStale = useCallback((v: View) => {
    if (!v.live) setRun((r) => (r.phase === 'none' || r.stale ? r : { ...r, stale: true }));
  }, []);

  // After Stop, a node call that runs for 2 s or more shows in the status bar (15.2).
  const watchStop = useCallback(() => {
    endStop();
    const from = Date.now();
    const timer = setInterval(() => {
      const s = Math.floor((Date.now() - from) / 1000);
      if (s < 2) return;
      api.running().then((id) => {
        if (stopTimer.current !== timer) return;
        if (id === null) endStop();
        // An empty ID is a node Default, a node Drop or a view, which names no node (16.1).
        else setStuck(`${id ? `Node ${id}` : 'A node Default, a node Drop or a view'} does not stop - ${s} s. Click Stop to end the node process.`);
      }, endStop);
    }, 1000);
    stopTimer.current = timer;
  }, [endStop]);

  useEvent(onEngine, (e: EngineEvent) => {
    if (e.serial !== serial.current) return;
    if (e.state === 'gui') {
      // The same map when no lines changed, so that the canvas does not draw again.
      setGuis((g) => {
        const changed = e.trees.filter((t) => g.get(`${t.mode} ${t.id}`) !== t.tree);
        if (changed.length === 0) return g;
        const next = new Map(g);
        for (const t of changed) next.set(`${t.mode} ${t.id}`, t.tree);
        return next;
      });
      if (e.reports.length > 0) log(nodeLines({ logs: [], reports: e.reports }));
      return;
    }
    const active = e.state === 'building' || e.state === 'started' || e.state === 'frame' || e.state === 'fired' || (e.state === 'ran' && (e.frames || e.events));
    setBusy(active);
    if (!active) endStop();
    if (e.state === 'started') return;
    setRun((r) => nextRun(r, e));
    const lines = eventLines(e);
    if (lines.length > 0) log(lines);
  });

  useEvent(onOutput, (o: OutputText) => {
    const kind = o.err ? 'err' : 'out';
    setOutput((l) => addLines(l, outputLines(o.text).map((text) => ({ text, kind }))));
  });

  return { run, guis, output, setOutput, logs, setLogs, busy, stuck, log, reset, markStale, watchStop };
}

/** The text of the run in the status bar. */
function runText(run: RunState): string {
  if (run.phase === 'building') return 'Building';
  // The end of the node process (16.1) and a failure without compile errors show their first sentence.
  if (run.phase === 'lost') return firstSentence(run.text);
  if (run.phase === 'failed') return run.errors.length > 0 ? 'Build failed' : firstSentence(run.text);
  if (run.phase === 'loaded') return `${run.errors.length > 0 ? 'Loaded the last build that compiled' : run.builtMs !== null ? `Built in ${seconds(run.builtMs)}` : 'Loaded'} - Not run`;
  if (run.phase !== 'ran') return '';
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
  return parts.join(' - ');
}

/** The times of the stages of the last frame, for the tooltip of the run text. */
function stagesText(run: RunState): string | undefined {
  const s = run.stages;
  return s ? `Frames ${millis(run.runMs ?? 0)}
Values ${millis(s.keepMs)}
Send ${millis(s.sendMs)}
Node GUIs ${millis(s.guiMs)}` : undefined;
}

export function StatusBar({ view, run, stuck, error, onPick, onLogs }: { view: View; run: RunState; stuck: string; error: string; onPick: (at: Reveal) => void; onLogs: () => void }) {
  const text = runText(run);
  const firstError = () => run.errors.map((e) => revealOf(e, Date.now())).find((at) => at !== null);
  // The status bar has one line. A click opens Logs, which have the full text (15.8).
  const first = error.split(/\r?\n/)[0];
  // One count for the compile errors and the graph problems (15.5).
  const errors = run.errors.length + view.problems.length;
  return (
    <footer className="statusbar">
      {errors === 0 ? (
        <span>Ready</span>
      ) : (
        <button
          className="status-link error-text"
          title={`${[...run.errors.map(errorLine), ...view.problems].join('\n')}\n\nClick to show the first error.`}
          onClick={() => {
            const at = firstError();
            if (at) onPick(at);
            else onLogs();
          }}
        >
          <span className="status-dot" aria-hidden="true" />
          Errors: {errors}
        </button>
      )}
      {text && <span title={stagesText(run)}>{text}</span>}
      {(run.phase === 'ran' || run.phase === 'loaded') && <span>{run.slot !== null ? `Slot: ${run.slot}` : run.fused ? 'Fused' : 'Edit build'}</span>}
      {stuck && <span className="error-text">{stuck}</span>}
      <span className="spacer" />
      {error && (
        <button className="status-link error-text status-error" title={`${error}\n\nClick to open Logs.`} onClick={onLogs}>
          {first}
        </button>
      )}
    </footer>
  );
}
