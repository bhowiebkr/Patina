// The self-check of `patina --check <dir>`: it makes a project, edits, runs, saves and opens the
// project again in the real window, then writes the result for the Rust side. The check-*.ts files
// have the groups of steps.

import { api, onEngine, onOutput, outputLines, type EngineEvent, type View } from '../api';
import type { Reveal } from '../code/CodeTab';
import { checkCanvas } from './check-canvas';
import { checkDock } from './check-dock';
import { checkDomains } from './check-domains';
import type { CheckContext } from './check-fixtures';
import { checkIsolation } from './check-isolation';
import { checkPalette } from './check-palette';
import { checkProject, checkRecovery } from './check-project';
import { checkStart } from './check-start';
import { checkSubgraph } from './check-subgraph';
import { checkViews } from './check-views';
import { checkWires } from './check-wires';
import { keepClipInMemory } from '../canvas/clip';

export async function runCheck(dir: string, show: (v: View) => void, pick: (at: Reveal) => void, edited: (v: View) => void) {
  keepClipInMemory();
  const steps: string[] = [];
  const events: EngineEvent[] = [];
  const output: string[] = [];
  let wake = () => {};
  let outputFrom = 0;
  const stopEngine = await onEngine((e) => {
    events.push(e);
    wake();
  });
  const stopOutput = await onOutput((o) => {
    output.push(...outputLines(o.text));
    wake();
  });

  async function until(what: string, ms: number, test: () => boolean) {
    const end = Date.now() + ms;
    while (!test()) {
      const left = end - Date.now();
      if (left <= 0) throw new Error(`no result after ${ms} ms: ${what}`);
      await new Promise<void>((r) => {
        wake = r;
        setTimeout(r, Math.min(left, 250));
      });
    }
  }

  /** Waits for the result of the engine after `from`, and for an output line. */
  async function result(v: View, from: number, line: string, ms: number) {
    let ran: Extract<EngineEvent, { state: 'ran' }> | undefined;
    await until(`a run that prints ${line}`, ms, () => {
      for (const e of events.slice(from)) {
        if (e.serial !== v.serial) continue;
        if (e.state === 'failed') throw new Error(`the build failed: ${e.text}`);
        if (e.state === 'lost') throw new Error(`the node process ended: ${e.text}`);
        if (e.state === 'ran') {
          if (!e.ok) throw new Error(`the run failed: ${JSON.stringify(e.reports)}`);
          ran = e;
        }
      }
      return ran !== undefined && output.slice(outputFrom).includes(line);
    });
    return ran!;
  }

  const step = async (name: string, job: () => Promise<View>) => {
    const v = await job();
    show(v);
    steps.push(name);
    return v;
  };
  const mark = () => {
    outputFrom = output.length;
    return events.length;
  };

  const c: CheckContext = { step, until, result, mark, events, steps, fresh: () => output.slice(outputFrom) };
  try {
    await checkStart(c);
    const made = await checkProject(c, dir, pick);
    const { lib, original } = made;
    const dock = await checkDock(c, await checkIsolation(c, made.v));
    let v = await checkWires(c, dock.v);
    v = await checkCanvas(c, v);
    v = await checkPalette(c, v, dock.at, edited);
    const recovered = await checkRecovery(c, v, lib, original);
    v = await checkDomains(c, recovered.v, lib, recovered.changedCode);
    v = await checkViews(c, v, lib, dock.at);
    await checkSubgraph(c, v, lib);

    await new Promise((r) => setTimeout(r, 300));
    await api.capture('check');
    steps.push('screenshot');
    await api.checkDone({ ok: true, steps });
  } catch (e) {
    await api.checkDone({ ok: false, steps, error: String(e), output: output.slice(-20) });
  } finally {
    stopEngine();
    stopOutput();
  }
}
