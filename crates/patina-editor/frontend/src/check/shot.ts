// The screenshots of `patina --shot <project> <dir>`: it opens the project, runs it for some
// frames, and writes a screenshot of the main graph and of each subgraph to <dir>. It opens each
// subgraph through a use, so the screenshot shows the live values of that use (15.9).

import { api, onEngine, type EngineEvent, type View } from '../api';
import { mouseWheel } from './check-fixtures';

const LIMIT_MS = 150_000;
const FRAMES = 8;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Open = (name: string, use?: string) => Promise<View | undefined>;

export async function runShot(project: string, show: (v: View) => void, open: Open) {
  const events: EngineEvent[] = [];
  const stop = await onEngine((e) => events.push(e));
  const steps: string[] = [];
  try {
    let v = await api.openProject(project);
    if (!v.trusted) v = await api.trustProject();
    show(v);
    v = await api.run();
    show(v);
    const serial = v.serial;
    const end = Date.now() + LIMIT_MS;
    const ofRun = () => events.filter((e) => e.serial === serial);
    // A project without frames runs one time, so its 'ran' event ends the wait.
    const done = () => ofRun().some((e) => e.state === 'ran' && !e.frames) || ofRun().filter((e) => e.state === 'frame').length >= FRAMES;
    while (!done()) {
      const failed = ofRun().find((e) => e.state === 'failed' || ((e.state === 'ran' || e.state === 'frame') && !e.ok));
      if (failed) throw new Error(`the run failed: ${JSON.stringify(failed)}`);
      if (Date.now() > end) throw new Error(`no ${FRAMES} frames after ${LIMIT_MS} ms`);
      await sleep(250);
    }
    await sleep(500);
    await api.capture('main');
    steps.push('main');
    // The node GUIs and the bus contacts hide at a far zoom (15.1), so a second screenshot zooms in.
    const zoom = async (node: Element | null | undefined, name: string) => {
      const box = node?.getBoundingClientRect();
      const pane = document.querySelector('.react-flow__pane');
      if (!box || !pane) return;
      for (let i = 0; i < 12; i++) pane.dispatchEvent(mouseWheel(-100, box.left + box.width / 2, box.top + box.height / 2));
      await sleep(1000);
      await api.capture(`${name}-zoom`);
      steps.push(`${name}-zoom`);
    };
    const nodes = document.querySelectorAll('.react-flow__node');
    await zoom(nodes[nodes.length - 1], 'main');
    // Depth first: open each subgraph one time, through its first use, then go back.
    const visit = async (graph: View, path: [string, string][]) => {
      for (const n of graph.graph.node ?? []) {
        if (!n.subgraph || steps.includes(n.subgraph)) continue;
        const inner = await open(n.subgraph, n.id);
        if (!inner) throw new Error(`cannot open ${n.subgraph}`);
        await sleep(500);
        await api.capture(n.subgraph);
        steps.push(n.subgraph);
        // A public pin of a bus type shows its contacts only near (15.9).
        await zoom(document.querySelector('.patina-node.port.bus'), n.subgraph);
        await visit(inner, [...path, [n.subgraph, n.id]]);
        await open('main');
        for (const [name, id] of path) await open(name, id);
      }
    };
    await visit(v, []);
    await api.checkDone({ ok: true, steps });
  } catch (e) {
    await api.checkDone({ ok: false, steps, error: String(e) });
  } finally {
    stop();
  }
}
