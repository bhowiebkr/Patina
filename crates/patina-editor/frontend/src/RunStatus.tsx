// The state of the run (15.2): the engine events, the Logs and Output lines, Stop and the status
// bar.

import { useCallback, useRef, useState, type RefObject } from 'react';
import {
  api,
  errorText,
  firstSentence,
  logKind,
  millis,
  onEngine,
  onOutput,
  outputLines,
  reportText,
  seconds,
  type Diagnostic,
  type NodeLog,
  type Report,
  type EngineEvent,
  type OutputText,
  type View,
} from './api';
import { revealOf, type Reveal } from './CodeTab';
import { addLines, type Line } from './Lines';
import { NO_RUN, valueStale, type RunState } from './NodeView';
import { useEvent } from './useEvent';

/** One Logs line for each compile error. A click on a line with a place shows the code. */
function errorLines(errors: Diagnostic[]): Line[] {
  return errors.map((e): Line => {
    const at = revealOf(e, 0);
    if (!at) return { text: e.rendered.trimEnd(), kind: 'err' };
    return { text: `${e.file}:${e.start[0]}:${e.start[1]}: ${errorText(e)}`, kind: 'err', at };
  });
}

/** One Logs line for each node log and each run error of a run or a frame. */
function nodeLines(e: { logs: NodeLog[]; reports: Report[] }): Line[] {
  return [
    ...e.logs.map((l): Line => ({ text: `${l.folder} (${l.id}): ${l.level} ${l.text}`, kind: logKind(l.level) })),
    ...e.reports.map((r): Line => ({ text: `${r.folder} (${r.id}): ${reportText(r)}`, kind: 'err' })),
  ];
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

const idleOf = (skipped: string[], stopped: string[]) =>
  new Map([...skipped.map((id) => [id, 'skipped'] as const), ...stopped.map((id) => [id, 'stopped'] as const)]);

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

  const log = useCallback((more: Line[]) => setLogs((l) => addLines(l, more)), []);

  const endStop = useCallback(() => {
    clearInterval(stopTimer.current);
    stopTimer.current = undefined;
    setStuck('');
  }, []);

  /** Clears all results, for another project. */
  const reset = useCallback(() => {
    setRun(NO_RUN);
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
      setGuis((g) => new Map([...g, ...e.trees.map((t) => [`${t.mode} ${t.id}`, t.tree] as const)]));
      if (e.reports.length > 0) log(nodeLines({ logs: [], reports: e.reports }));
      return;
    }
    const active = e.state === 'building' || e.state === 'started' || e.state === 'frame' || e.state === 'fired' || (e.state === 'ran' && (e.frames || e.events));
    setBusy(active);
    if (!active) endStop();
    if (e.state === 'started') return;
    if (e.state === 'frame') {
      // The values and times of the batch nodes stay. Only the frame nodes ran.
      setRun((r) => ({
        ...r,
        runMs: e.runMs,
        ok: e.ok,
        // The reports of the event runs stay, because the event nodes are not in the frame domain.
        reports: [...e.reports, ...r.eventReports],
        values: new Map([...r.values, ...e.values.map((v) => [v.from, v] as const)]),
        idle: idleOf(e.skipped, e.stopped),
        times: new Map([...r.times, ...e.times.map((t) => [t.id, t.ms] as const)]),
        logs: e.logs,
        frames: r.frames + 1,
      }));
      const lines = nodeLines(e);
      if (lines.length > 0) log(lines);
      return;
    }
    if (e.state === 'fired') {
      // Only the nodes of this run ran. A run replaces the reports of the nodes that it reached (9.4).
      const reached = new Set([...e.times.map((t) => t.id), ...e.skipped, ...e.stopped, ...e.reports.map((r) => r.id)]);
      const merge = (old: Report[]) => [...old.filter((r) => !reached.has(r.id)), ...e.reports];
      setRun((r) => ({
        ...r,
        ok: r.ok && e.ok,
        reports: merge(r.reports),
        eventReports: merge(r.eventReports),
        values: new Map([...r.values, ...e.values.map((v) => [v.from, v] as const)]),
        times: new Map([...r.times, ...e.times.map((t) => [t.id, t.ms] as const)]),
      }));
      const prefix = e.id > 0 ? `Run ${e.id}: ` : '';
      const lines = nodeLines(e).map((l) => ({ ...l, text: prefix + l.text }));
      if (lines.length > 0) log(lines);
      return;
    }
    if (e.state === 'ended') {
      log([{ text: e.frames > 0 ? `Ran ${plural(e.frames, 'frame')}` : 'The live run ended', kind: 'info' }]);
      return;
    }
    if (e.state === 'building') {
      setRun((r) => ({ ...r, phase: 'building', building: { base: e.base, slot: e.slot } }));
      setGuis(new Map());
      log([{ text: e.base ? 'Building the graph' : `Building the slot ${e.slot}`, kind: 'info' }]);
    } else if (e.state === 'ran') {
      const failed = e.errors.length > 0;
      const idle = idleOf(e.skipped, e.stopped);
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
          framed: e.frames,
          eventReports: [],
          fused: e.fused,
          slot: e.slot,
          building: r.building,
        };
      });
      const lines: Line[] = [];
      if (failed) lines.push({ text: 'The build failed. Running the last build that compiled.', kind: 'err' }, ...errorLines(e.errors));
      else if (e.builtMs !== null) lines.push({ text: `Built in ${seconds(e.builtMs)}`, kind: 'info' });
      lines.push({ text: `Ran in ${millis(e.runMs)}`, kind: 'info' });
      if (e.stopped.length > 0) lines.push({ text: 'Stop ended the run before all nodes ran.', kind: 'info' });
      log([...lines, ...nodeLines(e)]);
    } else if (e.state === 'loaded') {
      const failed = e.errors.length > 0;
      setRun((r) => ({
        ...r,
        phase: 'loaded',
        builtMs: e.builtMs,
        lastBuildMs: e.builtMs !== null && !failed ? e.builtMs : r.lastBuildMs,
        baseMs: e.builtMs !== null && !failed ? e.builtMs : r.baseMs,
        slotMs: e.slot === null ? null : r.slotMs,
        errors: e.builtMs === null ? r.errors : e.errors,
        text: '',
        stale: false,
        fused: e.fused,
        slot: e.slot,
      }));
      const lines: Line[] = failed ? [{ text: 'The build failed. The last build that compiled is loaded.', kind: 'err' }, ...errorLines(e.errors)] : [];
      if (!failed && e.builtMs !== null) lines.push({ text: `Built in ${seconds(e.builtMs)}`, kind: 'info' });
      log([...lines, { text: 'The nodes of the last build loaded. Click Run to run the graph.', kind: 'info' }]);
    } else if (e.state === 'lost') {
      setRun((r) => ({ ...r, phase: 'lost', text: e.text, values: NO_RUN.values }));
      log([{ text: e.text, kind: 'err' }]);
    } else {
      setRun((r) => ({ ...r, phase: 'failed', text: e.text, values: NO_RUN.values, errors: e.errors }));
      log(e.errors.length > 0 ? [{ text: 'The build failed.', kind: 'err' }, ...errorLines(e.errors)] : [{ text: e.text, kind: 'err' }]);
    }
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

export function StatusBar({ view, run, stuck, error, onPick }: { view: View; run: RunState; stuck: string; error: string; onPick: (at: Reveal) => void }) {
  const text = runText(run);
  const firstError = () => run.errors.map((e) => revealOf(e, Date.now())).find((at) => at !== null);
  return (
    <footer className="statusbar">
      <span title={view.problems.join('\n')}>{view.problems.length === 0 ? 'Ready' : `${view.problems.length} problems`}</span>
      {text && <span>{text}</span>}
      {(run.phase === 'ran' || run.phase === 'loaded') && <span>{run.slot !== null ? `Slot: ${run.slot}` : run.fused ? 'Fused' : 'Edit build'}</span>}
      {stuck && <span className="error-text">{stuck}</span>}
      {run.errors.length > 0 && (
        <button
          className="status-link error-text"
          onClick={() => {
            const at = firstError();
            if (at) onPick(at);
          }}
        >
          {plural(run.errors.length, 'compile error')}
        </button>
      )}
      <span className="spacer" />
      {error && <span className="error-text">{error}</span>}
    </footer>
  );
}
