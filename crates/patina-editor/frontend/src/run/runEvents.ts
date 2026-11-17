// The engine events of a run (15.2): the run state after each event, and its Logs lines.

import { errorText, logKind, millis, reportText, seconds, type Diagnostic, type EngineEvent, type NodeLog, type Report } from '../api';
import { revealOf } from '../code/reveal';
import type { Line } from './Lines';
import { NO_RUN, valueStale, type RunState } from '../canvas/NodeView';

/** An engine event that changes the run state or the Logs. */
export type RunEvent = Exclude<EngineEvent, { state: 'gui' | 'started' }>;

/** One Logs line for each compile error. A click on a line with a place shows the code. */
function errorLines(errors: Diagnostic[]): Line[] {
  return errors.map((e): Line => {
    const at = revealOf(e, 0);
    if (!at) return { text: e.rendered.trimEnd(), kind: 'err' };
    return { text: `${e.file}:${e.start[0]}:${e.start[1]}: ${errorText(e)}`, kind: 'err', at };
  });
}

/** One Logs line for each node log and each run error of a run or a frame. */
export function nodeLines(e: { logs: NodeLog[]; reports: Report[] }): Line[] {
  return [
    ...e.logs.map((l): Line => ({ text: `${l.folder} (${l.id}): ${l.level} ${l.text}`, kind: logKind(l.level) })),
    ...e.reports.map((r): Line => ({ text: `${r.folder} (${r.id}): ${reportText(r)}`, kind: 'err' })),
  ];
}

export const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

const idleOf = (skipped: string[], stopped: string[]) =>
  new Map([...skipped.map((id) => [id, 'skipped'] as const), ...stopped.map((id) => [id, 'stopped'] as const)]);

/** The run state after an engine event. */
export function nextRun(r: RunState, e: RunEvent): RunState {
  switch (e.state) {
    case 'frame':
      // The values and times of the batch nodes stay. Only the frame nodes ran.
      return {
        ...r,
        runMs: e.runMs,
        ok: e.ok,
        // The reports of the event runs stay, because the event nodes are not in the frame domain.
        reports: [...e.reports, ...r.eventReports],
        // A frame sends only the values that changed, and the times at 10 Hz (15.9).
        values: e.values.length > 0 ? new Map([...r.values, ...e.values.map((v) => [v.from, v] as const)]) : r.values,
        idle: idleOf(e.skipped, e.stopped),
        times: e.times.length > 0 ? new Map([...r.times, ...e.times.map((t) => [t.id, t] as const)]) : r.times,
        logs: e.logs,
        frames: r.frames + e.frames,
      };
    case 'fired': {
      // Only the nodes of this run ran. A run replaces the reports of the nodes that it reached (9.4).
      const reached = new Set([...e.times.map((t) => t.id), ...e.skipped, ...e.stopped, ...e.reports.map((r) => r.id)]);
      const merge = (old: Report[]) => [...old.filter((r) => !reached.has(r.id)), ...e.reports];
      return {
        ...r,
        ok: r.ok && e.ok,
        reports: merge(r.reports),
        eventReports: merge(r.eventReports),
        values: new Map([...r.values, ...e.values.map((v) => [v.from, v] as const)]),
        times: new Map([...r.times, ...e.times.map((t) => [t.id, t] as const)]),
      };
    }
    case 'ended':
      return r;
    case 'building':
      return { ...r, phase: 'building', building: { base: e.base, slot: e.slot } };
    case 'ran': {
      const failed = e.errors.length > 0;
      const idle = idleOf(e.skipped, e.stopped);
      const values = new Map(e.values.map((v) => [v.from, v]));
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
        times: new Map(e.times.map((t) => [t.id, t])),
        logs: e.logs,
        frames: 0,
        framed: e.frames,
        eventReports: [],
        fused: e.fused,
        slot: e.slot,
        building: r.building,
      };
    }
    case 'loaded': {
      const failed = e.errors.length > 0;
      return {
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
      };
    }
    case 'lost':
      return { ...r, phase: 'lost', text: e.text, values: NO_RUN.values };
    case 'failed':
      return { ...r, phase: 'failed', text: e.text, values: NO_RUN.values, errors: e.errors };
  }
}

/** The Logs lines of an engine event. */
export function eventLines(e: RunEvent): Line[] {
  switch (e.state) {
    case 'frame':
      return nodeLines(e);
    case 'fired': {
      const prefix = e.id > 0 ? `Run ${e.id}: ` : '';
      return nodeLines(e).map((l) => ({ ...l, text: prefix + l.text }));
    }
    case 'ended':
      return [{ text: e.frames > 0 ? `Ran ${plural(e.frames, 'frame')}` : 'The live run ended', kind: 'info' }];
    case 'building':
      return [{ text: e.base ? 'Building the graph' : `Building the slot ${e.slot}`, kind: 'info' }];
    case 'ran': {
      const lines: Line[] = [];
      if (e.errors.length > 0) lines.push({ text: 'The build failed. Running the last build that compiled.', kind: 'err' }, ...errorLines(e.errors));
      else if (e.builtMs !== null) lines.push({ text: `Built in ${seconds(e.builtMs)}`, kind: 'info' });
      lines.push({ text: `Ran in ${millis(e.runMs)}`, kind: 'info' });
      if (e.stopped.length > 0) lines.push({ text: 'Stop ended the run before all nodes ran.', kind: 'info' });
      return [...lines, ...nodeLines(e)];
    }
    case 'loaded': {
      const failed = e.errors.length > 0;
      const lines: Line[] = failed ? [{ text: 'The build failed. The last build that compiled is loaded.', kind: 'err' }, ...errorLines(e.errors)] : [];
      if (!failed && e.builtMs !== null) lines.push({ text: `Built in ${seconds(e.builtMs)}`, kind: 'info' });
      return [...lines, { text: 'The nodes of the last build loaded. Click Run to run the graph.', kind: 'info' }];
    }
    case 'lost':
      return [{ text: e.text, kind: 'err' }];
    case 'failed':
      return e.errors.length > 0 ? [{ text: 'The build failed.', kind: 'err' }, ...errorLines(e.errors)] : [{ text: e.text, kind: 'err' }];
  }
}
