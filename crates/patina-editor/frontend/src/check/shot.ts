// The screenshots of `patina --shot <project> <dir>`: it opens the project, runs it for some
// frames, and writes a screenshot of the main graph and of each subgraph to <dir>. It opens each
// subgraph through a use, so the screenshot shows the live values of that use (15.9).

import { api, onEngine, type EngineEvent, type View } from '../api';
import { mouseWheel } from './check-fixtures';

const LIMIT_MS = 150_000;
const FRAMES = 8;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Open = (name: string, use?: string) => Promise<View | undefined>;

/** A node GUI input of an example: the canvas label before, the points in canvas units (one point is a tap), and the labels after. */
type Poke = { from: RegExp; path: [number, number][]; to: RegExp; also?: RegExp };

const POKES: Poke[] = [
  // hello: C and 7 on the keypad give 7 x 3 = 21 on the tape, and the + key gives 7 + 3 = 10.
  { from: /^Keypad 12$/, path: [[141, 92.5]], to: /^Keypad 0$/ },
  { from: /^Keypad 0$/, path: [[27, 92.5]], to: /^Keypad 7$/, also: /^Tape 21$/ },
  { from: /^Operation x = 21$/, path: [[27, 92.5]], to: /^Operation \+ = 10$/, also: /^Tape 10$/ },
  // gpu: a drag up turns the knob, a drag on the track moves the fader, and a tap on INV inverts the gradient.
  { from: /^Knob 0\.75$/, path: [[66, 70], [66, 55], [66, 40]], to: /^Knob 0\.95$/ },
  { from: /^Mix 0\.30$/, path: [[67, 54], [100, 54], [148, 54]], to: /^Mix 1\.00$/ },
  { from: /^Gradient \d+ x \d+$/, path: [[59, 69]], to: /^Gradient \d+ x \d+ INV$/ },
  // computer: a tap turns a switch on.
  { from: /^Switch off$/, path: [[38, 16]], to: /^Switch on$/ },
];

/** Gives the input of `poke` to the first canvas with its label, and waits for the labels after. Returns false when no canvas has the label. */
async function poke({ from, path, to, also }: Poke) {
  const label = (c: Element) => c.getAttribute('aria-label') ?? '';
  const canvases = () => [...document.querySelectorAll<SVGSVGElement>('.gui-canvas')];
  const pad = canvases().find((c) => from.test(label(c)));
  if (!pad) return false;
  const box = pad.getBoundingClientRect();
  const s = box.width / pad.viewBox.baseVal.width;
  const at = ([x, y]: [number, number]) => ({ bubbles: true, button: 0, buttons: 1, pointerId: 1, isPrimary: true, clientX: box.left + x * s, clientY: box.top + y * s });
  pad.dispatchEvent(new PointerEvent('pointerdown', at(path[0])));
  for (const p of path.slice(1)) {
    await sleep(100);
    pad.dispatchEvent(new PointerEvent('pointermove', at(p)));
  }
  await sleep(100);
  pad.dispatchEvent(new PointerEvent('pointerup', at(path[path.length - 1])));
  const end = Date.now() + 30_000;
  while (!to.test(label(pad)) || (also && !canvases().some((c) => also.test(label(c))))) {
    if (Date.now() > end) throw new Error(`after the input on "${from.source}", the labels are: ${canvases().map(label).join(', ')}`);
    await sleep(100);
  }
  return true;
}

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
    // The taps and the drags of the node GUIs, which a person does with the mouse (4.3).
    const before = events.length;
    const poked = [];
    for (const p of POKES) if (await poke(p)) poked.push(`poke ${p.to.source}`);
    const failed = events.slice(before).find((e) => e.state === 'failed' || ((e.state === 'ran' || e.state === 'frame') && !e.ok));
    if (failed) throw new Error(`a run after a node GUI input failed: ${JSON.stringify(failed)}`);
    steps.push(...poked);
    if (poked.length) {
      await sleep(500);
      await api.capture('main-poke');
      steps.push('main-poke');
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
