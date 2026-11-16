// The screenshots of `patina --shot <project> <dir>`: it opens the project, runs it for some
// frames, and writes a screenshot of the main graph and of each subgraph to <dir>. It opens each
// subgraph through a use, so the screenshot shows the live values of that use (15.9).

import { api, onEngine, type EngineEvent, type View } from './api';

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
    while (ofRun().filter((e) => e.state === 'frame').length < FRAMES) {
      const failed = ofRun().find((e) => e.state === 'failed' || ((e.state === 'ran' || e.state === 'frame') && !e.ok));
      if (failed) throw new Error(`the run failed: ${JSON.stringify(failed)}`);
      if (Date.now() > end) throw new Error(`no ${FRAMES} frames after ${LIMIT_MS} ms`);
      await sleep(250);
    }
    await sleep(500);
    await api.capture('main');
    steps.push('main');
    // The node GUIs hide at a far zoom (15.1), so a second screenshot zooms in on the last node.
    const nodes = document.querySelectorAll('.react-flow__node');
    const box = nodes[nodes.length - 1]?.getBoundingClientRect();
    const pane = document.querySelector('.react-flow__pane');
    if (box && pane) {
      const at = { clientX: box.left + box.width / 2, clientY: box.top + box.height / 2 };
      for (let i = 0; i < 12; i++) pane.dispatchEvent(new WheelEvent('wheel', { deltaY: -100, ...at, bubbles: true, cancelable: true }));
      await sleep(1000);
      await api.capture('main-zoom');
      steps.push('main-zoom');
    }
    // Depth first: open each subgraph one time, through its first use, then go back.
    const visit = async (graph: View, path: [string, string][]) => {
      for (const n of graph.graph.node ?? []) {
        if (!n.subgraph || steps.includes(n.subgraph)) continue;
        const inner = await open(n.subgraph, n.id);
        if (!inner) throw new Error(`cannot open ${n.subgraph}`);
        await sleep(500);
        await api.capture(n.subgraph);
        steps.push(n.subgraph);
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
