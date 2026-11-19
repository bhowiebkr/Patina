// The screenshots of `patina --shot <project> <dir>`: it opens the project, runs it for some
// frames, and writes a screenshot of the main graph and of each subgraph to <dir>. It opens each
// subgraph through a use, so the screenshot shows the live values of that use (15.9).

import { api, onEngine, type EngineEvent, type View } from '../api';
import { changeLayout, savedViewport } from '../panels/Dock';
import { openPanel } from '../panels/layout';
import { BUILD_MS, button, enterText, entry, mouseWheel, rightClick } from './check-fixtures';
import { lowContrast } from './contrast';
import { skippedShape } from '../controls/gui';
import { frameStats } from './benchGraph';
import { synthDocks } from './shot-synth';
import { materialWorkbench } from './shot-material';
import { songWorkbench } from './shot-song';
import { modelWorkbench } from './shot-model';
import { sleep, wait, placesOnCanvas, textsClear, guiFit, unnamed, chromeFit, contrastOk, focusOk, menusFit, pinNames, wiresClear, openFit } from './shot-layout';

const LIMIT_MS = 150_000;
const FRAMES = 8;

/** The time of the next page frame. A hidden page gives no frames, so the wait ends after 200 ms. */
const nextFrame = () => new Promise<number>((r) => (requestAnimationFrame(r), setTimeout(() => r(performance.now()), 200)));

/** The mean stage times of the frame events of a run (9.1), and the page frame times while it runs, for the report. */
async function tickTimes(events: EngineEvent[]): Promise<string> {
  // The first ticks draw all node GUIs one time, so they do not count.
  const ticks = events.flatMap((e) => (e.state === 'frame' ? [e] : [])).slice(2);
  const mean = (of: (e: (typeof ticks)[number]) => number) => (ticks.reduce((sum, e) => sum + of(e), 0) / Math.max(ticks.length, 1)).toFixed(2);
  const times: number[] = [];
  for (let i = 0, last = await nextFrame(); i < 30; i++) {
    const now = await nextFrame();
    times.push(now - last);
    last = now;
  }
  const page = frameStats(times);
  return `tick of ${ticks.length} events: frames ${mean((e) => e.runMs)} ms, values ${mean((e) => e.stages.keepMs)} ms, send ${mean((e) => e.stages.sendMs)} ms, node GUIs ${mean((e) => e.stages.guiMs)} ms, page frame ${page.mean} ms (p95 ${page.p95} ms)`;
}

type Open = (name: string, use?: string) => Promise<View | undefined>;

/** A node GUI input of an example: the canvas label before, the points in canvas units (one point is a tap) or a text for the field of the node, and the labels after. */
type Poke = { from: RegExp; path: [number, number][] | string; to: RegExp; also?: RegExp };

const POKES: Poke[] = [
  // synth: switch the oscillator, drag detune, and stop and restart the score.
  { from: /^Timbre shape 1$/, path: [[30, 80]], to: /^Timbre shape 0$/ },
  { from: /^DETUNE 9\.00 ct$/, path: [[44, 39], [44, 11]], to: /^DETUNE 15\.00 ct$/ },
  { from: /^Ember sequencer playing$/, path: [[380, 30]], to: /^Ember sequencer hold$/ },
  { from: /^Ember sequencer hold$/, path: [[380, 30]], to: /^Ember sequencer playing$/ },
  { from: /^Workbench scene 0$/, path: [[210, 85]], to: /^Workbench scene 1$/ },
  { from: /^Workbench scene 1$/, path: [[380, 85]], to: /^Workbench scene 2$/ },
  { from: /^Workbench scene 2$/, path: [[70, 85]], to: /^Workbench scene 0$/ },
  // hello: C and 7 on the keypad give 7 x 3 = 21 on the tape, and the + key gives 7 + 3 = 10.
  { from: /^Keypad 12$/, path: [[141, 92.5]], to: /^Keypad 0$/ },
  { from: /^Keypad 0$/, path: [[27, 92.5]], to: /^Keypad 7$/, also: /^Tape 21$/ },
  { from: /^Operation x = 21$/, path: [[27, 92.5]], to: /^Operation \+ = 10$/, also: /^Tape 10$/ },
  // gpu: a drag up turns the knob, a drag on the track moves the fader, and a tap on INV inverts the gradient.
  { from: /^Knob 0\.75$/, path: [[66, 70], [66, 55], [66, 40]], to: /^Knob 0\.95$/ },
  { from: /^Mix 0\.30$/, path: [[67, 54], [100, 54], [148, 54]], to: /^Mix 1\.00$/ },
  { from: /^Gradient \d+ x \d+$/, path: [[59, 69]], to: /^Gradient \d+ x \d+ INV$/ },
  // computer: a tap turns a switch on, and a text with Enter goes into the key buffer of the keyboard.
  { from: /^Switch off$/, path: [[38, 16]], to: /^Switch on$/ },
  { from: /^Buffer ?$/, path: 'HI', to: /^Buffer HI\|$/ },
];

/** Gives the input of `poke` to the first canvas with its label, and waits for the labels after. Returns false when no canvas has the label. */
async function poke({ from, path, to, also }: Poke) {
  const label = (c: Element) => c.getAttribute('aria-label') ?? '';
  const canvases = () => [...document.querySelectorAll<SVGSVGElement>('.gui-canvas')];
  const pad = canvases().find((c) => from.test(label(c)));
  if (!pad) return false;
  // The frames draw only the GUIs of the nodes in view (4.3 rule 3), so Fit node shows the node first.
  const node = pad.closest('.react-flow__node');
  const pane = document.querySelector('.react-flow__pane')!.getBoundingClientRect();
  let box = pad.getBoundingClientRect();
  if (node && (box.left < pane.left || box.top < pane.top || box.right > pane.right || box.bottom > pane.bottom)) {
    rightClick(node);
    await wait(() => !!entry('Node', 'Fit node'), () => `no Fit node entry for "${from.source}"`);
    entry('Node', 'Fit node')!.click();
    await sleep(1000);
    box = pad.getBoundingClientRect();
  }
  if (typeof path === 'string') {
    enterText(node!.querySelector<HTMLInputElement>('input.gui-field')!, path);
  } else {
    const s = box.width / pad.viewBox.baseVal.width;
    const at = ([x, y]: [number, number]) => ({ bubbles: true, button: 0, buttons: 1, pointerId: 1, isPrimary: true, clientX: box.left + x * s, clientY: box.top + y * s });
    pad.dispatchEvent(new PointerEvent('pointerdown', at(path[0])));
    for (const p of path.slice(1)) {
      await sleep(100);
      pad.dispatchEvent(new PointerEvent('pointermove', at(p)));
    }
    await sleep(100);
    pad.dispatchEvent(new PointerEvent('pointerup', at(path[path.length - 1])));
  }
  await wait(
    () => to.test(label(pad)) && (!also || canvases().some((c) => also.test(label(c)))),
    () => `after the input on "${from.source}", the labels are: ${canvases().map(label).join(', ')}`,
  );
  return true;
}

/** Selects each node, and throws on an Inspector that scrolls to the side, a widget out of it, a label that cuts its text, a GUI that cannot draw or a node with an error (15.7). */
async function inspectNodes(where: string, ids = [...document.querySelectorAll<HTMLElement>('.react-flow__node')].map((n) => n.dataset.id ?? '').filter((id) => !/^(note|group):/.test(id))) {
  changeLayout((l) => openPanel(l, 'inspector'));
  const aside = () => document.querySelector<HTMLElement>('aside[aria-label="Inspector"]');
  const bad: string[] = [];
  for (const id of ids) {
    const node = document.querySelector<HTMLElement>(`.react-flow__node[data-id="${id}"]`);
    if (!node) continue;
    node.click();
    await wait(() => [...(aside()?.querySelectorAll('.label') ?? [])].some((l) => l.textContent === `ID ${id}`), () => `the Inspector does not show node ${id}`);
    // The engine draws the GUI again in the Inspector size (4.3).
    if (node.querySelector('.node-gui')) await sleep(600);
    const a = aside()!;
    const b = a.getBoundingClientRect();
    if (a.scrollWidth > a.clientWidth + 1) bad.push(`${id} scrolls the Inspector to the side`);
    if (a.querySelector('.panel-title .error-text')) bad.push(`${id} has an error`);
    if (a.querySelector('.node-gui [role=alert]')) bad.push(`the GUI of ${id} cannot draw`);
    for (const w of a.querySelectorAll('.node-gui :is(svg.gui-canvas, canvas, input, select, button, textarea, .label), .param > *')) {
      const r = w.getBoundingClientRect();
      if (r.width === 0) continue;
      const what = `${id} ${w.tagName.toLowerCase()} ${w.getAttribute('aria-label') ?? w.textContent?.slice(0, 20) ?? ''}`;
      if (r.left < b.left - 1 || r.right > b.right + 1) bad.push(`out ${what}`);
      if (w instanceof HTMLElement && w.scrollWidth > w.clientWidth + 1 && !(w instanceof HTMLInputElement || w instanceof HTMLTextAreaElement)) bad.push(`cut ${what}`);
    }
    bad.push(...unnamed(a).map((b) => `${id} ${b}`), ...textsClear(a));
    pinNames(`${where} ${id}`);
    placesOnCanvas();
  }
  if (bad.length) throw new Error(`the Inspector of ${where} is wrong: ${bad.join(', ')}`);
  return `inspect ${where}: ${ids.length} nodes`;
}

/** The outputs of the workbenches with a `Viewer` thumbnail, and the image labels of the Inspector: the frame of the computer GPU with its screen widget, and the image of the gpu gradient. */
const VIEWED: { from: string; shows: RegExp }[] = [
  { from: 'ad357059907f778c.out0', shows: /^Value: 32 x 32 pixels, Screen: 32 x 32 pixels$/ },
  { from: '1a2b3c4d5e6f7081.out', shows: /^Value: 64 x 16 pixels$/ },
];

/** Selects the first wire from an output of `VIEWED`, and waits for the thumbnail and the widgets of its Viewer in the Inspector (5.3). Returns the labels, or undefined when the graph has no such wire. */
async function inspectThumb() {
  const viewed = VIEWED.find(({ from }) => document.querySelector(`.react-flow__edge[data-id^="${from}>"]`));
  const hit = viewed && document.querySelector<SVGPathElement>(`.react-flow__edge[data-id^="${viewed.from}>"] .wire-hit`);
  if (!hit) return undefined;
  changeLayout((l) => openPanel(l, 'inspector'));
  const p = hit.getPointAtLength(hit.getTotalLength() / 2);
  const m = hit.getScreenCTM()!;
  hit.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: m.a * p.x + m.c * p.y + m.e, clientY: m.b * p.x + m.d * p.y + m.f }));
  const shown = () => [...document.querySelectorAll('aside[aria-label="Inspector"] canvas.gui-image')].map((c) => c.getAttribute('aria-label'));
  await wait(() => viewed.shows.test(shown().join(', ')), () => `the Inspector of the wire from ${viewed.from} shows: ${shown().join(', ')}`);
  return shown().join(', ');
}

/** No node covers the badge of the selected wire, which shows all of its text (15.9). Returns false when the wire has no badge. */
function badgeOnTop() {
  const b = document.querySelector<HTMLElement>('.wire-value.selected');
  if (!b) return false;
  const r = b.getBoundingClientRect();
  // The hit test skips a box without pointer events.
  b.style.pointerEvents = 'auto';
  const tops = [r.left + 2, r.right - 2].map((x) => document.elementFromPoint(x, r.top + r.height / 2));
  b.style.pointerEvents = '';
  const over = tops.find((t) => !b.contains(t));
  if (over) throw new Error(`a ${over.tagName} ${over.getAttribute('class')} covers the selected badge "${b.textContent}"`);
  return true;
}

/** Opens the full size of each node that has one, and throws on a side scroll, a widget out of it, a label that cuts its text or a GUI that cannot draw (4.3). Captures `main-full` on the first code editor with line numbers and the band on the current line. */
async function fullSizes(where: string, ids = [...document.querySelectorAll<HTMLElement>('.react-flow__node')].map((n) => n.dataset.id ?? '')) {
  const expand = 'button[aria-label="Open the full size"]';
  const full = () => document.querySelector<HTMLElement>('.full-gui');
  const ready = () => !!full()?.querySelector('.gui-code-gutter div') && !!full()?.querySelector('.gui-code-line');
  const bad: string[] = [];
  let [code, seen] = [false, 0];
  for (const id of ids) {
    const button = document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${id}"] ${expand}`);
    if (!button) continue;
    seen++;
    button.click();
    await wait(() => !!full(), () => `no full size of node ${id}`);
    // The engine draws the GUI again in the full size (4.3).
    await sleep(600);
    if (!code && full()!.querySelector('.gui-code-box')) {
      const end = Date.now() + 5_000;
      while (!ready() && Date.now() < end) await sleep(100);
      code = ready();
      if (code) await api.capture('main-full');
    }
    const f = full()!;
    const b = f.getBoundingClientRect();
    if (f.querySelector('svg.gui-canvas'))
      await api.capture(`${where}-panel-${id}`, [b.left, b.top, b.width, b.height].map((v) => v * devicePixelRatio));
    if (f.scrollWidth > f.clientWidth + 1) bad.push(`${id} scrolls to the side`);
    if (f.querySelector('[role=alert]')) bad.push(`the GUI of ${id} cannot draw`);
    for (const w of f.querySelectorAll('.node-gui :is(svg.gui-canvas, canvas, input, select, button, textarea, .label)')) {
      const r = w.getBoundingClientRect();
      if (r.width === 0) continue;
      const what = `${id} ${w.tagName.toLowerCase()} ${w.getAttribute('aria-label') ?? w.textContent?.slice(0, 20) ?? ''}`;
      if (r.left < b.left - 1 || r.right > b.right + 1) bad.push(`out ${what}`);
      if (w instanceof HTMLElement && w.scrollWidth > w.clientWidth + 1 && !(w instanceof HTMLInputElement || w instanceof HTMLTextAreaElement)) bad.push(`cut ${what}`);
    }
    bad.push(...unnamed(f).map((b) => `${id} ${b}`), ...textsClear(f));
    f.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await wait(() => !full(), () => 'Esc did not close the full size');
  }
  if (bad.length) throw new Error(`the full size GUIs of ${where} are wrong: ${bad.join(', ')}`);
  return { text: `full ${where}: ${seen} nodes`, code };
}

/** The views of the workbenches: an input on the viewport, and the text of the view after it. gpu: a wheel turn up sets the light to 60 %. computer: the monitor shows the frame of the run. */
const VIEWS: Record<string, { input?: () => Event; shows: RegExp }> = {
  scene: { input: () => new WheelEvent('wheel', { deltaY: -100, bubbles: true, cancelable: true }), shows: /light: 60 %/ },
  monitor: { shows: /Click the monitor/ },
};

/** Opens view `name` from the Views menu, gives it its input of `VIEWS`, and throws on a view that does not build, a side scroll, a widget out of the panel or a label that cuts its text (15.8). Captures `view-<name>`. */
async function openView(name: string) {
  const panel = () => document.querySelector<HTMLElement>(`aside[data-view="${CSS.escape(name)}"]`);
  const text = () => {
    const failed = panel()?.querySelector('.error-text')?.textContent;
    if (failed) throw new Error(`the view ${name} failed: ${failed}`);
    return panel()?.querySelector('.node-gui')?.textContent ?? '';
  };
  button('Views')!.click();
  const item = () => [...document.querySelectorAll<HTMLElement>('[role="menuitemcheckbox"]')].find((e) => e.textContent?.trim() === name);
  await wait(() => !!item(), () => `no item ${name} in the Views menu`);
  item()!.click();
  await wait(() => text() !== '', () => `the view ${name} shows: ${panel()?.textContent ?? 'no panel'}`, BUILD_MS);
  const known = VIEWS[name];
  const area = panel()!.querySelector('.gui-viewport');
  if (known?.input) {
    if (!area) throw new Error(`the view ${name} has no viewport: ${text()}`);
    area.dispatchEvent(known.input());
  }
  if (area) {
    // The page is transparent over a viewport, so only its parents can be under it (15.8).
    const root = document.documentElement.classList;
    await wait(() => root.contains('native-viewport'), () => `the page is not transparent over the viewport of the view ${name}`);
    const r = area.getBoundingClientRect();
    for (let i = 1; i < 8; i++)
      for (let j = 1; j < 8; j++) {
        const under = document.elementsFromPoint(r.left + (r.width * i) / 8, r.top + (r.height * j) / 8).find((e) => !e.contains(area));
        if (under) throw new Error(`a ${under.tagName} ${under.getAttribute('class')} covers the viewport of the view ${name} at ${i}/8, ${j}/8`);
      }
    for (let e: Element | null = area; e; e = e.parentElement) {
      const bg = getComputedStyle(e);
      if (bg.backgroundImage !== 'none' || !/^(transparent|rgba\(.*, 0\))$/.test(bg.backgroundColor))
        throw new Error(`the ${e.tagName} ${e.id || e.getAttribute('class')} has a background over the viewport of the view ${name}`);
    }
  }
  if (known) await wait(() => known.shows.test(text()), () => `the view ${name} shows: ${text()}`);
  await sleep(500);
  const r = area?.getBoundingClientRect();
  const drawn = await api.capture(`view-${name}`, r && [r.left, r.top, r.width, r.height].map((v) => v * devicePixelRatio));
  // The composed window shows the surface of the view through the page (15.8).
  if (r && drawn !== null && drawn < 0.9) throw new Error(`the view ${name} drew ${Math.round(drawn * 100)} % of its viewport in the window`);
  const a = panel()!;
  const b = a.getBoundingClientRect();
  const bad: string[] = [];
  if (a.scrollWidth > a.clientWidth + 1) bad.push('a side scroll');
  for (const w of a.querySelectorAll('.node-gui :is(svg.gui-canvas, canvas, input, select, button, textarea, .label, .gui-viewport)')) {
    const r = w.getBoundingClientRect();
    if (r.width === 0) continue;
    const what = `${w.tagName.toLowerCase()} ${w.getAttribute('aria-label') ?? w.textContent?.slice(0, 20) ?? ''}`;
    if (r.left < b.left - 1 || r.right > b.right + 1) bad.push(`out ${what}`);
    if (w instanceof HTMLElement && w.scrollWidth > w.clientWidth + 1 && !(w instanceof HTMLInputElement || w instanceof HTMLTextAreaElement)) bad.push(`cut ${what}`);
  }
  bad.push(...lowContrast(a).bad);
  if (bad.length) throw new Error(`the view ${name} is wrong: ${bad.join(', ')}`);
  return [`view-${name}`, `view ${name}: ${text()}`, chromeFit(`view ${name}`)];
}

/** computer: picks the echo program, and waits for the keys of the key buffer on the terminal. Returns false when no node can pick `echo.asm`. */
async function echo() {
  const pick = document.querySelector<HTMLSelectElement>('.react-flow__node select:has(option[value$="echo.asm"])');
  const screen = () => document.querySelector<SVGSVGElement>('.gui-canvas[aria-label="Terminal"]');
  if (!pick || !screen()) return false;
  pick.value = pick.querySelector<HTMLOptionElement>('option[value$="echo.asm"]')!.value;
  pick.dispatchEvent(new Event('change', { bubbles: true }));
  // The frames draw only the GUIs of the nodes in view (4.3 rule 3).
  rightClick(screen()!.closest('.react-flow__node')!);
  await wait(() => !!entry('Node', 'Fit node'), () => 'no Fit node entry for the terminal');
  entry('Node', 'Fit node')!.click();
  const text = () => [...(screen()?.querySelectorAll('text') ?? [])].map((t) => t.textContent ?? '').filter((t) => t.length === 1).join('');
  await wait(() => text().includes('HI'), () => `the terminal shows "${text()}"`);
  await api.capture('main-echo');
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
    steps.push('main', openFit('main'));
    if (ofRun().some((e) => e.state === 'frame')) steps.push(await tickTimes(ofRun()));
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
    // A free layout with bus pins shows its sockets beside the drawing (4.3). The widest one shows the most.
    const socketed = [...document.querySelectorAll('.patina-node.socketed')].sort((a, b) => b.getBoundingClientRect().width - a.getBoundingClientRect().width)[0];
    if (socketed) {
      button('Fit')?.click();
      await sleep(500);
      await zoom(socketed, 'main-socket');
    }
    // The taps and the drags of the node GUIs, which a person does with the mouse (4.3).
    const before = events.length;
    const poked = [];
    for (const p of POKES) if (await poke(p)) poked.push(`poke ${p.to.source}`);
    const failed = events.slice(before).find((e) => e.state === 'failed' || ((e.state === 'ran' || e.state === 'frame') && !e.ok));
    if (failed) throw new Error(`a run after a node GUI input failed: ${JSON.stringify(failed)}`);
    steps.push(...poked);
    if (poked.length) {
      // The zoom step leaves the view on one node, so Fit shows the inputs on all nodes.
      button('Fit')?.click();
      await sleep(500);
      await api.capture('main-poke');
      steps.push('main-poke');
      if (placesOnCanvas()) steps.push('placed widgets on their canvas');
      steps.push(guiFit('main'));
      steps.push(wiresClear('main', v.graph));
      steps.push(chromeFit('main'), contrastOk('main'), focusOk('main'));
      steps.push(await menusFit('main'));
      // A laptop of 1920 x 1080 pixels at 150% gives a window of 1280 x 720 (15.2).
      await api.checkResize(1280, 720);
      await sleep(1000);
      await api.capture('main-small');
      const small = [chromeFit('small'), guiFit('small'), await menusFit('small')];
      await api.checkResize(1440, 900);
      await sleep(1000);
      steps.push(...small);
    }
    steps.push(await inspectNodes('main'));
    const inspected = await inspectThumb();
    if (inspected) {
      steps.push(`inspect ${inspected}`, chromeFit('main-inspect'), contrastOk('main-inspect'), focusOk('main-inspect'));
      if (badgeOnTop()) steps.push('the selected badge is above the nodes');
      await api.capture('main-inspect');
      steps.push('main-inspect');
    }
    const full = await fullSizes('main');
    if (full.code) steps.push('main-full');
    steps.push(full.text);
    if (await echo()) steps.push('main-echo');
    for (const name of v.views) steps.push(...(await openView(name)));
    steps.push(...(await synthDocks(v, show)));
    steps.push(...(await songWorkbench(v, show)));
    steps.push(...(await materialWorkbench(v, show, () => guiFit('materials'))));
    steps.push(...(await modelWorkbench(v, show)));
    // A subgraph inspects the first node of each kind that the shot did not inspect yet.
    const kinds = new Set((v.graph.node ?? []).map((n) => n.folder ?? n.subgraph ?? n.builtin));
    // Depth first: open each subgraph one time, through its first use, then go back.
    const visit = async (graph: View, path: [string, string][]) => {
      for (const n of graph.graph.node ?? []) {
        if (!n.subgraph || steps.includes(n.subgraph)) continue;
        const inner = await open(n.subgraph, n.id);
        if (!inner) throw new Error(`cannot open ${n.subgraph}`);
        // A change of graph shows the Graph tab, so an open view does not cover the subgraph (15.2).
        await wait(() => !document.querySelector('.center-view'), () => `a view covers the graph ${n.subgraph}`);
        await sleep(500);
        await api.capture(n.subgraph);
        steps.push(n.subgraph, openFit(n.subgraph));
        steps.push(guiFit(n.subgraph));
        steps.push(wiresClear(n.subgraph, inner.graph));
        steps.push(chromeFit(n.subgraph), contrastOk(n.subgraph), focusOk(n.subgraph));
        steps.push(await menusFit(n.subgraph), pinNames(n.subgraph));
        const fresh = (inner.graph.node ?? []).filter((m) => !kinds.has(m.folder ?? m.subgraph ?? m.builtin) && kinds.add(m.folder ?? m.subgraph ?? m.builtin));
        if (fresh.length) steps.push(await inspectNodes(n.subgraph, fresh.map((m) => m.id)), (await fullSizes(n.subgraph, fresh.map((m) => m.id))).text);
        // A public pin of a bus type shows its contacts only near (15.9).
        await zoom(document.querySelector('.patina-node.port.bus'), n.subgraph);
        await visit(inner, [...path, [n.subgraph, n.id]]);
        await open('main');
        for (const [name, id] of path) await open(name, id);
      }
    };
    await visit(v, []);
    // The other top graphs, for example the guide graphs of a workbench, run and get the layout checks of main (6.1).
    for (const name of v.graphs.filter((g) => g !== 'main')) {
      show(await api.stop());
      // A graph that an earlier step opened keeps its pan and zoom, so it does not fit at open.
      const saved = savedViewport(name);
      const top = await open(name);
      if (!top) throw new Error(`cannot open ${name}`);
      const from = events.length;
      show(await api.run());
      const ofTop = () => events.slice(from).filter((e) => e.state === 'ran' || e.state === 'frame' || e.state === 'failed');
      await wait(() => ofTop().length > 0, () => `the graph ${name} did not run`, LIMIT_MS);
      const failed = ofTop().find((e) => e.state === 'failed' || !e.ok);
      if (failed) throw new Error(`the run of ${name} failed: ${JSON.stringify(failed)}`);
      await wait(() => !document.querySelector('.center-view'), () => `a view covers the graph ${name}`);
      await sleep(500);
      await api.capture(name);
      steps.push(name, saved ? `open fit ${name}: a saved pan and zoom` : openFit(name), guiFit(name), wiresClear(name, top.graph), chromeFit(name), contrastOk(name), focusOk(name), await menusFit(name));
      const fresh = (top.graph.node ?? []).filter((m) => !kinds.has(m.folder ?? m.subgraph ?? m.builtin) && kinds.add(m.folder ?? m.subgraph ?? m.builtin)).map((m) => m.id);
      if (fresh.length) steps.push(await inspectNodes(name, fresh), (await fullSizes(name, fresh)).text);
    }
    if (skippedShape) throw new Error(`the canvas "${skippedShape}" has a shape that the editor skips (4.3)`);
    steps.push('no skipped canvas shapes');
    await api.checkDone({ ok: true, steps });
  } catch (e) {
    await api.checkDone({ ok: false, steps, error: String(e) });
  } finally {
    stop();
  }
}
