// The screenshots of `patina --shot <project> <dir>`: it opens the project, runs it for some
// frames, and writes a screenshot of the main graph and of each subgraph to <dir>. It opens each
// subgraph through a use, so the screenshot shows the live values of that use (15.9).

import { api, onEngine, type EngineEvent, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { openPanel } from '../panels/layout';
import { BUILD_MS, button, closeMenu, enterText, entry, mouseWheel, rightClick } from './check-fixtures';
import { lowContrast } from './contrast';
import { noFocusLook } from './focus';
import { textsMeet } from './boxes';

const LIMIT_MS = 150_000;
const FRAMES = 8;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Waits up to `ms` for `test`, or throws the text of `what`. */
async function wait(test: () => boolean, what: () => string, ms = 30_000) {
  const end = Date.now() + ms;
  while (!test()) {
    if (Date.now() > end) throw new Error(what());
    await sleep(100);
  }
}

type Open = (name: string, use?: string) => Promise<View | undefined>;

/** A node GUI input of an example: the canvas label before, the points in canvas units (one point is a tap) or a text for the field of the node, and the labels after. */
type Poke = { from: RegExp; path: [number, number][] | string; to: RegExp; also?: RegExp };

const POKES: Poke[] = [
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

/** Each placed widget is on its canvas (4.3), on the graph and in the Inspector. Returns false when no GUI has a placed widget. */
function placesOnCanvas() {
  const guis = [...document.querySelectorAll('.node-gui.placed')];
  for (const gui of guis) {
    const pad = gui.querySelector('svg.gui-canvas')!.getBoundingClientRect();
    for (const p of gui.querySelectorAll('.gui-place')) {
      const r = p.getBoundingClientRect();
      if (r.left < pad.left || r.top < pad.top || r.right > pad.right + 1 || r.bottom > pad.bottom + 1) throw new Error(`a placed widget at ${r.left}, ${r.top} is outside its canvas at ${pad.left}, ${pad.top} in ${gui.getAttribute('aria-label')}`);
    }
  }
  return guis.length > 0;
}

/** The texts of the node GUI drawings and the symbols in `root` that are out of the drawing or cover half of another text (4.3). A symbol can draw a text out of its body. */
function textsClear(root: ParentNode) {
  const bad: string[] = [];
  for (const svg of root.querySelectorAll('svg.gui-canvas, .patina-node.symbol > svg')) {
    const s = svg.getBoundingClientRect();
    const owner = svg.closest('[data-id]')?.getAttribute('data-id') ?? svg.closest('[aria-label]')?.getAttribute('aria-label');
    const texts = [...svg.querySelectorAll('text')].map((t) => [t.textContent?.trim() ?? '', t.getBoundingClientRect()] as const).filter(([t, r]) => t && r.width > 0);
    for (const [i, [t, r]] of texts.entries()) {
      if (svg.matches('.gui-canvas') && (r.left < s.left - 2 || r.top < s.top - 2 || r.right > s.right + 2 || r.bottom > s.bottom + 2)) bad.push(`text out ${owner} "${t}"`);
      for (const [u, q] of texts.slice(i + 1))
        if (textsMeet(r, q)) bad.push(`text over ${owner} "${t}" "${u}"`);
    }
  }
  return bad;
}

/** Throws on a widget out of its node or the Inspector, a label that cuts its text, a GUI that cannot draw, or two nodes that overlap (4.3). */
function guiFit(where: string) {
  const bad: string[] = [];
  let seen = 0;
  const name = (w: Element) => `${w.tagName.toLowerCase()} ${w.getAttribute('aria-label') ?? w.textContent?.slice(0, 20) ?? ''}`;
  const boxes = [...document.querySelectorAll('.react-flow__node'), ...document.querySelectorAll('aside[aria-label="Inspector"]')];
  for (const box of boxes) {
    const b = box.getBoundingClientRect();
    for (const w of box.querySelectorAll('.node-gui :is(svg.gui-canvas, canvas, input, select, button, textarea, .label)')) {
      const r = w.getBoundingClientRect();
      if (r.width === 0) continue;
      seen++;
      if (r.left < b.left - 1 || r.top < b.top - 1 || r.right > b.right + 1 || r.bottom > b.bottom + 1) bad.push(`out ${box.getAttribute('data-id') ?? 'inspector'} ${name(w)}`);
      if (w instanceof HTMLElement && w.scrollWidth > w.clientWidth + 1 && !(w instanceof HTMLInputElement || w instanceof HTMLTextAreaElement)) bad.push(`cut ${box.getAttribute('data-id') ?? 'inspector'} ${name(w)}`);
    }
  }
  const nodes = [...document.querySelectorAll('.react-flow__node')].map((n) => [n.getAttribute('data-id'), n.getBoundingClientRect()] as const);
  for (const e of document.querySelectorAll('.node-gui [role=alert]')) bad.push(`a GUI cannot draw: ${e.textContent}`);
  bad.push(...textsClear(document));
  // A pin shows on hover, so it must not cover the text of the title (15.3).
  for (const node of document.querySelectorAll('.react-flow__node'))
    for (const t of node.querySelectorAll('.title')) {
      const range = document.createRange();
      range.selectNodeContents(t);
      // The text can clip with an ellipsis, so only its visible part counts.
      const [r, c] = [range.getBoundingClientRect(), t.getBoundingClientRect()];
      const [left, right] = [Math.max(r.left, c.left), Math.min(r.right, c.right)];
      for (const h of node.querySelectorAll('.react-flow__handle')) {
        const q = h.getBoundingClientRect();
        if (r.width && Math.min(right, q.right) - Math.max(left, q.left) > 1 && Math.min(r.bottom, q.bottom) - Math.max(r.top, q.top) > 1) bad.push(`a pin covers the title ${t.textContent} of ${node.getAttribute('data-id')}`);
      }
    }
  // The order pins are in the row of the caption of a free node, so they must not cover its icons (15.3).
  for (const icon of document.querySelectorAll('.free-caption > :not(.title)')) {
    const [r, node] = [icon.getBoundingClientRect(), icon.closest('.react-flow__node')];
    for (const q of [...(node?.querySelectorAll('.order-pin') ?? [])].map((h) => h.getBoundingClientRect())) {
      const x = Math.min(r.right, q.right) - Math.max(r.left, q.left);
      if (r.width && x > 0.5 && Math.min(r.bottom, q.bottom) - Math.max(r.top, q.top) > 0.5) bad.push(`an order pin covers ${x.toFixed(1)} px of the ${icon.getAttribute('aria-label') ?? icon.className} of ${node?.getAttribute('data-id')}, caption ${icon.parentElement?.getBoundingClientRect().width.toFixed(1)} px of node ${node?.getBoundingClientRect().width.toFixed(1)} px`);
    }
  }
  for (const [i, [a, p]] of nodes.entries())
    for (const [b, q] of nodes.slice(i + 1)) if (Math.min(p.right, q.right) - Math.max(p.left, q.left) > 2 && Math.min(p.bottom, q.bottom) - Math.max(p.top, q.top) > 2) bad.push(`over ${a} ${b}`);
  if (bad.length) throw new Error(`the layout of ${where} is wrong: ${bad.join(', ')}`);
  return `fit ${where}: ${seen} widgets, ${document.querySelectorAll('svg.gui-canvas text, .patina-node.symbol > svg text').length} drawn texts and ${nodes.length} nodes`;
}

/** The buttons, fields and tabs in `root` that have no accessible name, for a screen reader (15.3). */
function unnamed(root: ParentNode) {
  const bad: string[] = [];
  for (const w of root.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href], [role=tab], [role=button], [role=slider], [role=menuitem], [role=checkbox], [role=switch]')) {
    if (w.closest('.monaco-editor') || w.getAttribute('type') === 'hidden' || w.getBoundingClientRect().width === 0) continue;
    const by = (w.getAttribute('aria-labelledby') ?? '').split(' ').map((i) => document.getElementById(i)?.textContent ?? '').join('');
    const labels = 'labels' in w ? [...((w as HTMLInputElement).labels ?? [])].map((l) => l.textContent).join('') : '';
    const field = w instanceof HTMLInputElement || w instanceof HTMLSelectElement || w instanceof HTMLTextAreaElement;
    const name = [w.getAttribute('aria-label'), by, labels, w.title, w.getAttribute('placeholder'), field ? '' : w.textContent].find((n) => n?.trim());
    if (!name) bad.push(`no name ${w.tagName.toLowerCase()}.${w.className} in ${w.closest('[aria-label], [data-id]')?.getAttribute('aria-label') ?? w.closest('[data-id]')?.getAttribute('data-id') ?? 'the page'}`);
  }
  return bad;
}

/** Throws on a button, a tab, a title or a label of the panels and the bars that cuts its text or is out of the window (15.2). */
function chromeFit(where: string) {
  const bad: string[] = [];
  let seen = 0;
  for (const w of document.querySelectorAll<HTMLElement>('button, [role=tab], .panel-title, .label, .topbar *, .statusbar *, h3')) {
    if (w.closest('.react-flow__node, .node-gui, .monaco-editor')) continue;
    if (w.getBoundingClientRect().width === 0) continue;
    seen++;
    const what = `${w.tagName.toLowerCase()}.${w.className} "${(w.getAttribute('aria-label') ?? w.textContent ?? '').slice(0, 30)}"`;
    // An item in a scroll area can scroll out of view, so the area must be in the window.
    let box: Element = w;
    for (let a = w.parentElement; a; a = a.parentElement) {
      const s = getComputedStyle(a);
      if (/auto|scroll/.test(s.overflowX + s.overflowY)) {
        box = a;
        break;
      }
    }
    const r = box.getBoundingClientRect();
    if (r.left < -1 || r.right > innerWidth + 1 || r.bottom > innerHeight + 1) bad.push(`out ${what}`);
    if (w.scrollWidth > w.clientWidth + 1) bad.push(`cut ${what}`);
    // Two buttons side by side need a gap, so they do not read as one control (15.2).
    const next = w.nextElementSibling;
    if (w.classList.contains('btn') && next?.classList.contains('btn')) {
      const [a, b] = [w.getBoundingClientRect(), next.getBoundingClientRect()];
      if (b.width && Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1 && Math.abs(b.left - a.right) < 2) bad.push(`no gap after ${what}`);
    }
  }
  bad.push(...unnamed(document));
  if (bad.length) throw new Error(`the chrome of ${where} is wrong: ${bad.join(', ')}`);
  return `chrome ${where}: ${seen} items`;
}

/** Throws on a text of the page with too little contrast (15.3). */
function contrastOk(where: string) {
  const { bad, seen } = lowContrast(document);
  if (bad.length) throw new Error(`the contrast of ${where} is too low: ${bad.join(', ')}`);
  return `contrast ${where}: ${seen} texts`;
}

/** Throws on a button, a field or a tab of the page that a keyboard focus does not mark (WCAG 2.4.7). */
function focusOk(where: string) {
  const { bad, seen } = noFocusLook(document);
  const name = (w: Element) => `${w.tagName.toLowerCase()}.${w.className} "${(w.getAttribute('aria-label') ?? w.textContent ?? '').slice(0, 24)}" in ${w.closest('[data-id]')?.getAttribute('data-id') ?? w.closest('[aria-label]')?.getAttribute('aria-label') ?? 'the page'}`;
  if (bad.length) throw new Error(`no focus look in ${where}: ${bad.map(name).join(', ')}`);
  return `focus ${where}: ${seen} controls`;
}

/** Opens the menus of the canvas at its right bottom corner, a node, a wire, the Views button, a dock tab and a Project item, and throws on a menu out of the window or an item that cuts its text (15.2). */
async function menusFit(where: string) {
  const bad: string[] = [];
  const menus: string[] = [];
  const check = async (open: () => void) => {
    open();
    await wait(() => !!document.querySelector('.menu'), () => `no menu in ${where} after the menu ${menus.length + 1}`);
    const m = document.querySelector<HTMLElement>('.menu')!;
    const label = m.getAttribute('aria-label') ?? '';
    menus.push(label);
    const r = m.getBoundingClientRect();
    if (r.left < -1 || r.top < -1 || r.right > innerWidth + 1 || r.bottom > innerHeight + 1) bad.push(`out ${label}`);
    for (const b of m.querySelectorAll('button'))
      if (b.scrollWidth > b.clientWidth + 1 || b.scrollHeight > b.clientHeight + 1) bad.push(`cut ${label} "${b.textContent}"`);
    bad.push(...unnamed(m).map((b) => `${label} ${b}`), ...lowContrast(m).bad.map((b) => `${label} ${b}`));
    closeMenu();
    await wait(() => !document.querySelector('.menu'), () => `the menu ${label} does not close`);
  };
  const pane = document.querySelector('.react-flow__pane')!;
  const p = pane.getBoundingClientRect();
  await check(() => rightClick(pane, p.right - 4, p.bottom - 4));
  const node = document.querySelector('.react-flow__node');
  if (node) await check(() => rightClick(node));
  const hit = document.querySelector<SVGPathElement>('.react-flow__edge .wire-hit');
  if (hit) {
    const at = hit.getPointAtLength(hit.getTotalLength() / 2);
    const m = hit.getScreenCTM()!;
    await check(() => rightClick(hit, m.a * at.x + m.c * at.y + m.e, m.b * at.x + m.d * at.y + m.f));
  }
  await check(() => button('Views')!.click());
  const tab = [...document.querySelectorAll('[role=tab][draggable=true]')].pop();
  if (tab) await check(() => rightClick(tab));
  // The last item in the window of a group with a menu: not a None item and not an import.
  const item = [...document.querySelectorAll('aside[aria-label="Project"] button.item:not([title^="Compare"])')]
    .filter((w) => w.getBoundingClientRect().width > 0 && w.getBoundingClientRect().bottom <= innerHeight)
    .pop();
  if (item) await check(() => rightClick(item));
  if (bad.length) throw new Error(`the menus of ${where} are wrong: ${bad.join(', ')}`);
  return `menus ${where}: ${menus.join(', ')}`;
}

/** Throws when an Inspector row shows the random key of a public pin of a subgraph, not its name (6.4). */
function pinNames(where: string) {
  const rows = [...document.querySelectorAll('aside[aria-label="Inspector"] .param > span')].map((s) => s.textContent ?? '');
  const keys = rows.filter((t) => /(^|[.:])[0-9a-f]{8}$/.test(t));
  if (keys.length) throw new Error(`the Inspector of ${where} shows the pin keys ${keys.join(', ')}`);
  return `pin names ${where}: ${rows.length} rows`;
}

/** Throws on a wire through a body that is not one of its ends, or on a route conflict mark (15.9). */
function wiresClear(where: string) {
  const bad: string[] = [];
  let seen = 0;
  const nodes = [...document.querySelectorAll<HTMLElement>('.react-flow__node')].map((n) => [n.dataset.id ?? '', n.getBoundingClientRect()] as const);
  for (const e of document.querySelectorAll('.react-flow__edge')) {
    const id = e.getAttribute('data-id') ?? '';
    const ends = id.split('>').map((pin) => pin.slice(0, pin.lastIndexOf('.')));
    const path = e.querySelector<SVGPathElement>('.wire-hit');
    const m = path?.getScreenCTM();
    if (!path || !m) continue;
    seen++;
    // The automatic route keeps 6 px from each body, so a point 3 px inside a body is wrong.
    const inset = 3 * m.a;
    const through = new Set<string>();
    for (let s = 0, len = path.getTotalLength(); s <= len; s += 4) {
      const p = path.getPointAtLength(s);
      const x = m.a * p.x + m.c * p.y + m.e;
      const y = m.b * p.x + m.d * p.y + m.f;
      for (const [n, r] of nodes) if (!ends.includes(n) && x > r.left + inset && x < r.right - inset && y > r.top + inset && y < r.bottom - inset) through.add(n);
    }
    for (const n of through) bad.push(`${id} through ${n}`);
  }
  for (const c of document.querySelectorAll('.route-conflict')) bad.push(`a conflict mark at ${c.getAttribute('style')}`);
  // A badge keeps room for a value of 6 characters (15.3).
  for (const b of document.querySelectorAll<HTMLElement>('.wire-value'))
    if ((b.textContent ?? '').length <= 6 && b.scrollWidth > b.clientWidth + 1) bad.push(`the badge "${b.textContent}" cuts its text`);
  // A bus label drops parts to fit, so it never cuts its text (15.9).
  for (const l of document.querySelectorAll<HTMLElement>('.wire-type'))
    if (/ \/ \d+$|^\/ \d+$/.test(l.textContent ?? '') && l.scrollWidth > l.clientWidth + 1) bad.push(`the bus label "${l.textContent}" cuts its text`);
  // A badge that is not selected is under the nodes, so a pin or a node body must not cover it (15.9).
  const solid = [...document.querySelectorAll('.patina-node:not(.frame), .react-flow__handle:not(.frame-pin)')].map((n) => n.getBoundingClientRect()).filter((r) => r.width > 0);
  const joints = [...document.querySelectorAll('.wire-joint')].map((j) => j.getBoundingClientRect());
  for (const b of document.querySelectorAll('.wire-value:not(.selected)')) {
    const r = b.getBoundingClientRect();
    const on = (s: DOMRect) => Math.min(r.right, s.right) - Math.max(r.left, s.left) > 0.5 && Math.min(r.bottom, s.bottom) - Math.max(r.top, s.top) > 0.5;
    if (solid.some(on)) bad.push(`a pin or a node covers the badge "${b.textContent}"`);
    // A badge on a junction dot hides where the wires part.
    if (joints.some(on)) bad.push(`the badge "${b.textContent}" covers a junction dot`);
  }
  if (bad.length) throw new Error(`the wires of ${where} are wrong: ${bad.join(', ')}`);
  return `wires ${where}: ${seen} wires clear of the bodies`;
}

/** Throws on a node out of the canvas, after the graph opens with no saved pan and zoom (15.2). */
function openFit(where: string) {
  const c = document.querySelector('.react-flow')!.getBoundingClientRect();
  const nodes = [...document.querySelectorAll<HTMLElement>('.react-flow__node')];
  const bad = nodes.filter((n) => {
    const r = n.getBoundingClientRect();
    return r.left < c.left - 1 || r.top < c.top - 1 || r.right > c.right + 1 || r.bottom > c.bottom + 1;
  });
  if (bad.length) throw new Error(`the graph ${where} does not fit at open: ${bad.map((n) => n.dataset.id).join(', ')}`);
  return `open fit ${where}: ${nodes.length} nodes`;
}

/** Selects each node, and throws on an Inspector that scrolls to the side, a widget out of it, a label that cuts its text, a GUI that cannot draw or a node with an error (15.7). */
async function inspectNodes(where: string, ids = [...document.querySelectorAll<HTMLElement>('.react-flow__node')].map((n) => n.dataset.id ?? '')) {
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

/** The outputs of the examples with a `Viewer` thumbnail, and the image labels of the Inspector: the frame of the computer GPU with its screen widget, and the image of the gpu gradient. */
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

/** The views of the examples: an input on the viewport, and the text of the view after it. gpu: a wheel turn up sets the light to 60 %. computer: the monitor shows the frame of the run. */
const VIEWS: Record<string, { input?: () => Event; shows: RegExp }> = {
  scene: { input: () => new WheelEvent('wheel', { deltaY: -100, bubbles: true, cancelable: true }), shows: /light: 60 %/ },
  monitor: { shows: /Click the monitor/ },
};

/** Opens view `name` from the Views menu, gives it its input of `VIEWS`, and throws on a view that does not build, a side scroll, a widget out of the panel or a label that cuts its text (15.8). Captures `view-<name>`. */
async function openView(name: string) {
  const panel = () => document.querySelector<HTMLElement>('aside[aria-label="View"]');
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
      // The zoom step leaves the view on one node, so Fit shows the inputs on all nodes.
      button('Fit')?.click();
      await sleep(500);
      await api.capture('main-poke');
      steps.push('main-poke');
      if (placesOnCanvas()) steps.push('placed widgets on their canvas');
      steps.push(guiFit('main'));
      steps.push(wiresClear('main'));
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
        steps.push(wiresClear(n.subgraph));
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
    await api.checkDone({ ok: true, steps });
  } catch (e) {
    await api.checkDone({ ok: false, steps, error: String(e) });
  } finally {
    stop();
  }
}
