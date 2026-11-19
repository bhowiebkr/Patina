// The layout checks of the shot (15.2, 15.9): the texts, the node GUIs, the chrome, the menus and the
// wires of the open graph must fit and must not cover each other.

import { type GraphFile } from '../api';
import { button, closeMenu, rightClick } from './check-fixtures';
import { lowContrast } from './contrast';
import { noFocusLook } from './focus';
import { textsMeet } from './boxes';
import { GAP } from '../wires/route';
import { pathOf } from '../wires/wires';
import { busPinFaults } from './busPins';

export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Waits up to `ms` for `test`, or throws the text of `what`. */
export async function wait(test: () => boolean, what: () => string, ms = 30_000) {
  const end = Date.now() + ms;
  while (!test()) {
    if (Date.now() > end) throw new Error(what());
    await sleep(100);
  }
}

/** Each placed widget is on its canvas (4.3), on the graph and in the Inspector. Returns false when no GUI has a placed widget. */
export function placesOnCanvas() {
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
export function textsClear(root: ParentNode) {
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
export function guiFit(where: string) {
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
  const nodes = [...document.querySelectorAll('.react-flow__node')].filter((n) => !n.getAttribute('data-id')?.startsWith('group:')).map((n) => [n.getAttribute('data-id'), n.getBoundingClientRect()] as const);
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
  // The pin names, the parameter fields and the bus socket heads of a node must not cover each other (15.3).
  for (const node of document.querySelectorAll('.react-flow__node')) {
    const items = [...node.querySelectorAll('.patina-row > span, .patina-row .param-text, .patina-row .pin-switch, .bus-socket-head')].filter((e) => e.getBoundingClientRect().width > 0);
    for (const [i, a] of items.entries())
      for (const b of items.slice(i + 1)) {
        const [r, q] = [a.getBoundingClientRect(), b.getBoundingClientRect()];
        if (!a.contains(b) && !b.contains(a) && Math.min(r.right, q.right) - Math.max(r.left, q.left) > 0.5 && Math.min(r.bottom, q.bottom) - Math.max(r.top, q.top) > 0.5)
          bad.push(`"${a.textContent}" covers "${b.textContent}" in ${node.getAttribute('data-id')}`);
      }
  }
  // A public pin shows its name in its header only, not again in its bus socket (15.9).
  for (const head of document.querySelectorAll('.port .bus-socket-head'))
    if (!head.textContent?.startsWith('[')) bad.push(`the bus socket of ${head.closest('.react-flow__node')?.getAttribute('data-id')} repeats the name in "${head.textContent}"`);
  // A free layout with a bus pin keeps the contacts of each lane in columns beside its drawing (4.3).
  for (const node of document.querySelectorAll('.patina-node.socketed')) {
    const id = node.closest('.react-flow__node')?.getAttribute('data-id');
    const drawing = node.querySelector('.socketed-gui')?.getBoundingClientRect();
    if (!drawing?.width) bad.push(`the socketed node ${id} shows no drawing`);
    for (const socket of node.querySelectorAll('.bus-socket'))
      if (!socket.querySelector('.bus-contact')) bad.push(`a bus socket of ${id} has no contacts`);
    for (const e of node.querySelectorAll('.bus-contact, .bus-socket-head, .patina-row > span')) {
      const r = e.getBoundingClientRect();
      if (drawing && r.width && Math.min(r.right, drawing.right) - Math.max(r.left, drawing.left) > 0.5 && Math.min(r.bottom, drawing.bottom) - Math.max(r.top, drawing.top) > 0.5) bad.push(`the drawing of ${id} covers "${e.textContent}"`);
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
  return `fit ${where}: ${seen} widgets, ${document.querySelectorAll('svg.gui-canvas text, .patina-node.symbol > svg text').length} drawn texts, ${nodes.length} nodes and ${document.querySelectorAll('.patina-node.socketed').length} socketed nodes`;
}

/** The buttons, fields and tabs in `root` that have no accessible name, for a screen reader (15.3). */
export function unnamed(root: ParentNode) {
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
export function chromeFit(where: string) {
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
export function contrastOk(where: string) {
  const { bad, seen } = lowContrast(document);
  if (bad.length) throw new Error(`the contrast of ${where} is too low: ${bad.join(', ')}`);
  return `contrast ${where}: ${seen} texts`;
}

/** Throws on a button, a field or a tab of the page that a keyboard focus does not mark (WCAG 2.4.7). */
export function focusOk(where: string) {
  const { bad, seen } = noFocusLook(document);
  const name = (w: Element) => `${w.tagName.toLowerCase()}.${w.className} "${(w.getAttribute('aria-label') ?? w.textContent ?? '').slice(0, 24)}" in ${w.closest('[data-id]')?.getAttribute('data-id') ?? w.closest('[aria-label]')?.getAttribute('aria-label') ?? 'the page'}`;
  if (bad.length) throw new Error(`no focus look in ${where}: ${bad.map(name).join(', ')}`);
  return `focus ${where}: ${seen} controls`;
}

/** Opens the menus of the canvas at its right bottom corner, a node, a wire, the Views button, a dock tab and a Project item, and throws on a menu out of the window or an item that cuts its text (15.2). */
export async function menusFit(where: string) {
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

/** The automatic wires of two outputs that meet at a bend or on top of each other, so the crossing has no gap and reads as a junction (15.9). */
function bendMeets(): string[] {
  const flows = [...document.querySelectorAll('.react-flow')];
  const wires: { id: string; net: string; pts: Float64Array }[] = [];
  const cells = new Map<string, number[]>();
  for (const e of document.querySelectorAll('.react-flow__edge')) {
    const path = e.querySelector<SVGPathElement>('.wire-hit[data-net]');
    if (!path?.getScreenCTM()) continue;
    const w = wires.length;
    const n = Math.floor(path.getTotalLength()) + 1;
    const pts = new Float64Array(n * 2);
    const f = flows.indexOf(e.closest('.react-flow')!);
    for (let k = 0; k < n; k++) {
      const p = path.getPointAtLength(k);
      [pts[2 * k], pts[2 * k + 1]] = [p.x, p.y];
      const key = `${f} ${Math.floor(p.x / 2)} ${Math.floor(p.y / 2)}`;
      const cell = cells.get(key);
      if (cell) cell.push(w, k);
      else cells.set(key, [w, k]);
    }
    wires.push({ id: e.getAttribute('data-id') ?? '', net: path.getAttribute('data-net') ?? '', pts });
  }
  // 'h' or 'v' when the wire is straight 3 px to each side of the point, else ''.
  const axis = (pts: Float64Array, k: number) => {
    const [a, b] = [Math.max(0, k - 3), Math.min(pts.length / 2 - 1, k + 3)];
    const flat = (o: number) => Math.abs(pts[2 * a + o] - pts[2 * k + o]) <= 0.3 && Math.abs(pts[2 * b + o] - pts[2 * k + o]) <= 0.3;
    return flat(1) ? 'h' : flat(0) ? 'v' : '';
  };
  const bad = new Map<string, string>();
  for (const [key, cell] of cells) {
    const [f, cx, cy] = key.split(' ').map(Number);
    for (let i = 0; i < cell.length; i += 2) {
      const [a, ka] = [cell[i], cell[i + 1]];
      const A = wires[a];
      for (let dx = -1; dx <= 1; dx++)
        for (let dy = -1; dy <= 1; dy++)
          for (const [b, kb] of pairs(cells.get(`${f} ${cx + dx} ${cy + dy}`))) {
            const B = wires[b];
            const pair = `${a} ${b}`;
            if (b <= a || A.net === B.net || bad.has(pair)) continue;
            // The nearest samples of a crossing are 0.71 px apart or less, at a step of 1 px.
            if (Math.hypot(A.pts[2 * ka] - B.pts[2 * kb], A.pts[2 * ka + 1] - B.pts[2 * kb + 1]) > 0.75) continue;
            const [ha, hb] = [axis(A.pts, ka), axis(B.pts, kb)];
            const at = `at ${Math.round(A.pts[2 * ka])} ${Math.round(A.pts[2 * ka + 1])}`;
            const [bend, line, kl] = ha ? [B, A, ka] : [A, B, kb];
            const y = line.pts[2 * kl + 1];
            // shortcut: the pin rows of two outputs nearer than the gap cross at a bend, which the nudge cannot fix (STATUS); fix it in the A* router.
            if ((ha || hb) === 'h' && ha !== hb && pinRow(line.pts, y, 0.5) && pinRow(bend.pts, y, 2 * GAP)) continue;
            if (!ha || !hb) bad.set(pair, `${A.id} and ${B.id} cross at a bend ${at}`);
            else if (ha === hb) bad.set(pair, `${A.id} and ${B.id} overlap ${at}`);
          }
    }
  }
  return [...bad.values()];
}

/** True when the first or the last point of the path, where its pin segments are, is nearer than `near` to the row `y`. */
const pinRow = (pts: Float64Array, y: number, near: number) => Math.abs(pts[1] - y) < near || Math.abs(pts[pts.length - 1] - y) < near;

function* pairs(cell: number[] | undefined) {
  for (let i = 0; cell && i < cell.length; i += 2) yield [cell[i], cell[i + 1]] as const;
}

/** Throws when an Inspector row shows the random key of a public pin of a subgraph, not its name (6.4). */
export function pinNames(where: string) {
  const rows = [...document.querySelectorAll('aside[aria-label="Inspector"] .param > span')].map((s) => s.textContent ?? '');
  const keys = rows.filter((t) => /(^|[.:])[0-9a-f]{8}$/.test(t));
  if (keys.length) throw new Error(`the Inspector of ${where} shows the pin keys ${keys.join(', ')}`);
  return `pin names ${where}: ${rows.length} rows`;
}

/** Throws on a wire through a body that is not one of its ends, a crossing at a bend or a route conflict mark (15.9). */
export function wiresClear(where: string, graph: GraphFile) {
  const bad: string[] = [];
  let seen = 0;
  // Only the automatic route goes around the bodies and the labels. A curved, orthogonal or polyline wire keeps its path (15.9).
  const manual = new Set((graph.wire ?? []).filter((w) => pathOf(w.style, graph) !== undefined).map((w) => `${w.from}>${w.to}`));
  const nodes = [...document.querySelectorAll<HTMLElement>('.react-flow__node')].map((n) => [n.dataset.id ?? '', n.getBoundingClientRect()] as const);
  // A label that is not selected hides where another wire goes through it (15.9).
  const labels = [...document.querySelectorAll<HTMLElement>('.wire-type:not(.selected)')].map((l) => [l, l.getBoundingClientRect()] as const);
  // A badge that is not selected hides where a routed wire of another output goes through it (15.9).
  const badges = [...document.querySelectorAll<HTMLElement>('.wire-value:not(.selected)')].map((b) => [b, b.getBoundingClientRect()] as const);
  const crossed = new Set<string>();
  for (const e of document.querySelectorAll('.react-flow__edge')) {
    const id = e.getAttribute('data-id') ?? '';
    if (manual.has(id)) continue;
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
      for (const [l, r] of labels) if (l.dataset.wire !== id && x > r.left + 1 && x < r.right - 1 && y > r.top + 1 && y < r.bottom - 1) crossed.add(`${id} through the label "${l.textContent}"`);
      if (path.dataset.net !== undefined)
        for (const [b, r] of badges) if (b.dataset.net !== path.dataset.net && x > r.left + 1 && x < r.right - 1 && y > r.top + 1 && y < r.bottom - 1) crossed.add(`${id} through the badge "${b.textContent}"`);
    }
    for (const n of through) bad.push(`${id} through ${n}`);
  }
  // Two shown labels keep a space, so that their texts do not run together (15.9).
  for (let i = 0; i < labels.length; i++)
    for (let j = i + 1; j < labels.length; j++) {
      const [[la, a], [lb, b]] = [labels[i], labels[j]];
      if (a.left < b.right + 0.5 && b.left < a.right + 0.5 && a.top < b.bottom + 0.5 && b.top < a.bottom + 0.5) bad.push(`the labels "${la.textContent}" and "${lb.textContent}" run together`);
    }
  bad.push(...crossed, ...bendMeets(), ...busPinFaults());
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
    if (joints.some(on)) bad.push(`the badge "${b.textContent}" covers a junction dot at ${Math.round(r.left)} ${Math.round(r.top)}`);
  }
  if (bad.length) throw new Error(`the wires of ${where} are wrong: ${bad.join(', ')}`);
  return `wires ${where}: ${seen} wires clear of the bodies`;
}

/** Throws on a node out of the canvas, after the graph opens with no saved pan and zoom (15.2). */
export function openFit(where: string) {
  const c = document.querySelector('.react-flow')!.getBoundingClientRect();
  const nodes = [...document.querySelectorAll<HTMLElement>('.react-flow__node')];
  const bad = nodes.filter((n) => {
    const r = n.getBoundingClientRect();
    return r.left < c.left - 1 || r.top < c.top - 1 || r.right > c.right + 1 || r.bottom > c.bottom + 1;
  });
  if (bad.length) throw new Error(`the graph ${where} does not fit at open: ${bad.map((n) => n.dataset.id).join(', ')}`);
  return `open fit ${where}: ${nodes.length} nodes`;
}
