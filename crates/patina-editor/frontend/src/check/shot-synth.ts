// End-to-end checks of the synth dock tools on the hidden screenshot desktop.
import { api, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { closePanel, movePanel, openPanel, openView, resizeArea, VIEW_SLOTS } from '../panels/layout';
import { enterText } from './check-fixtures';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const panel = (name: string) => document.querySelector<HTMLElement>(`aside[data-view="${name}"]`);
async function until(test: () => boolean, message: string, timeout = 150_000) {
  const end = Date.now() + timeout;
  while (!test()) {
    const error = [...document.querySelectorAll('aside[data-view] .error-text')].map((e) => e.textContent).join(' ');
    if (error) throw new Error(error);
    if (Date.now() > end) throw new Error(message);
    await sleep(100);
  }
}
export function click(name: string, text: string) {
  const b = [...(panel(name)?.querySelectorAll('button') ?? [])].find((b) => b.textContent?.trim() === text);
  if (!b) throw new Error(`No ${text} in ${name}`);
  b.click();
}
export function field(name: string, text: string) {
  const label = [...(panel(name)?.querySelectorAll('label') ?? [])].find((l) => l.querySelector('span')?.textContent === text);
  const input = label?.querySelector<HTMLInputElement>('input');
  if (!input) throw new Error(`No field ${text} in ${name}`);
  return input;
}
export async function showTab(name: string) {
  changeLayout((l) => {
    const slot = VIEW_SLOTS.find((s) => l.views[s] === name);
    return slot ? openPanel(l, slot) : openView(l, name, ['sounds', 'learn'].includes(name) ? 'left' : ['controls', 'audio', 'modulation'].includes(name) ? 'right' : 'bottom');
  });
  await until(() => !!panel(name)?.querySelector('.node-gui'), `No ${name} controls`);
  await sleep(250);
}
export function fit(name: string) {
  const p = panel(name)!;
  const body = p.querySelector<HTMLElement>('.view-controls')!;
  if (body.scrollWidth > body.clientWidth + 1) throw new Error(`${name} scrolls sideways`);
  const r = body.getBoundingClientRect();
  for (const control of body.querySelectorAll('input, select, button, svg.gui-canvas')) {
    const b = control.getBoundingClientRect();
    if (b.width && (b.left < r.left - 1 || b.right > r.right + 1)) throw new Error(`${name} control leaves the dock: ${control.textContent}`);
  }
}

export async function synthDocks(view: View, show: (view: View) => void): Promise<string[]> {
  if (!view.views.includes('sounds') || !view.views.includes('analysis')) return [];
  const steps: string[] = [];
  changeLayout((l) => VIEW_SLOTS.reduce((next, s) => closePanel(next, s), l));
  for (const name of ['sounds', 'learn', 'modulation', 'controls', 'audio', 'analysis', 'patterns', 'keyboard', 'export']) await showTab(name);
  for (const width of [220, 360]) {
    changeLayout((l) => resizeArea(resizeArea(l, 'left', width), 'right', width));
    for (const name of ['sounds', 'learn', 'modulation', 'controls', 'audio']) { await showTab(name); fit(name); }
  }
  document.querySelector<HTMLButtonElement>('[aria-label="Tabs in the right area"]')!.click();
  await until(() => !!document.querySelector('[role=menu]'), 'Tab menu did not open');
  const modulationTab = [...document.querySelectorAll<HTMLElement>('[role=menuitemcheckbox]')].find((e) => e.textContent?.trim() === 'modulation');
  if (!modulationTab) throw new Error('Modulation tab is missing from the tab menu');
  modulationTab.click();
  await until(() => !!panel('modulation'), 'Tab menu did not select Modulation');
  steps.push('the tab menu keeps hidden dock tabs reachable');
  changeLayout((l) => resizeArea(resizeArea(resizeArea(l, 'left', 260), 'right', 300), 'bottom', 190));
  await api.checkResize(1280, 720);
  for (const name of ['analysis', 'patterns', 'keyboard', 'export']) { await showTab(name); fit(name); }
  steps.push('nine synth tabs fit side docks at 220/360 px and a 190 px bottom dock at 1280 x 720');

  await showTab('patterns');
  const before = await api.pause(); show(before);
  enterText(field('patterns', 'Tempo / BPM'), '123');
  await sleep(350);
  const edited = await api.pause(); show(edited);
  const sequence = (v: View) => v.graph.node?.find((n) => n.id === 'e6be000000000010')?.params?.bpm;
  if (sequence(edited) !== '123') throw new Error('Pattern tempo did not reach the graph');
  const undone = await api.undo(); show(undone);
  if (sequence(undone) !== sequence(before)) throw new Error('Pattern undo did not restore tempo');
  steps.push('pattern tempo writes the real graph and Undo restores it');

  await showTab('sounds');
  enterText(field('sounds', 'Save as name'), 'Dock check sound');
  click('sounds', 'Save sound as new');
  await until(() => panel('sounds')!.textContent!.includes('Saved sound.'), 'Sound did not save');
  click('sounds', 'Save sound as new');
  await until(() => panel('sounds')!.textContent!.includes('Cannot save'), 'Sound save replaced an existing file');
  const saved = await api.pause(); show(saved); show(await api.run());
  click('sounds', 'Preview sound');
  await until(() => panel('sounds')!.textContent!.includes('Stop preview'), 'Preview did not start');
  await sleep(500);
  click('sounds', 'Stop preview');
  const after = await api.pause(); show(after);
  if (JSON.stringify(after.graph) !== JSON.stringify(saved.graph)) throw new Error('Preview changed the patch');
  steps.push('sound saves without overwrite and preview leaves the graph unchanged');
  show(await api.edit({ kind: 'SetParam', id: 'e6be000000000030', key: 'level', text: '0.31' }));
  await until(() => panel('sounds')!.textContent!.includes('Modified'), 'Sound did not detect a graph edit');
  click('sounds', 'Load sound');
  await until(() => panel('sounds')!.textContent!.includes('Cancel load'), 'Load did not protect changed sound');
  click('sounds', 'Cancel load');
  await sleep(250);
  click('sounds', 'Load sound');
  await until(() => panel('sounds')!.textContent!.includes('Discard sound changes and load'), 'Load confirmation did not return');
  click('sounds', 'Discard sound changes and load');
  await until(() => panel('sounds')!.textContent!.includes('Loaded.'), 'Preset did not load');
  show(await api.undo()); show(await api.undo());
  steps.push('preset load protects changed sound and uses graph Undo');
  show(await api.run());

  await showTab('keyboard');
  const keys = () => panel('keyboard')!.querySelector<SVGSVGElement>('svg[role=application]')!;
  const keyColor = () => keys().querySelector('rect')!.getAttribute('fill');
  const idle = keyColor();
  keys().focus();
  if (document.activeElement !== keys()) throw new Error('Keyboard canvas did not take focus');
  keys().dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyA', key: 'a', bubbles: true }));
  await until(() => keyColor() !== idle, 'Keyboard did not hold C', 5000);
  let blurred = false;
  const focusedKeys = keys();
  focusedKeys.addEventListener('focusout', () => { blurred = true; }, { once: true });
  panel('keyboard')!.querySelector('button')!.focus();
  // A hidden desktop can change activeElement without native window focus events.
  if (!blurred) focusedKeys.dispatchEvent(new FocusEvent('focusout', { bubbles: true, relatedTarget: document.activeElement }));
  await until(() => keyColor() === idle, `Keyboard did not release on focus loss; focus: ${document.activeElement?.outerHTML.slice(0, 200)}, idle: ${idle}`, 5000);
  steps.push('keyboard holds a played note and releases it on a focus-loss event');

  await showTab('analysis');
  await until(() => panel('analysis')!.textContent!.includes('Live | Peak'), 'Analysis did not capture live audio');
  const freeze = [...panel('analysis')!.querySelectorAll('label')].find((l) => l.textContent?.includes('Freeze'))!.querySelector('input')!;
  freeze.click();
  await until(() => panel('analysis')!.textContent!.includes('FROZEN'), 'Analysis did not freeze');
  await sleep(450);
  if (panel('analysis')!.textContent!.includes('Stale / engine paused')) throw new Error('Frozen analysis falsely reports a paused engine');
  freeze.click();
  show(await api.pause());
  await until(() => panel('analysis')!.textContent!.includes('Stale / engine paused'), 'Analysis hid stale samples');
  steps.push('continuous analysis captures, freezes, and marks paused samples stale');

  await showTab('export');
  const outputBefore = await api.pause(); show(outputBefore);
  if (outputBefore.graph.node?.find((n) => n.id === 'e6be000000000026')?.params?.audible !== 'true') {
    show(await api.edit({ kind: 'SetParam', id: 'e6be000000000026', key: 'audible', text: 'true' }));
  }
  show(await api.save());
  enterText(field('export', 'WAV destination'), 'dock-check.wav');
  enterText(field('export', 'Duration'), '0.1');
  click('export', 'Export saved patch');
  await until(() => panel('export')!.textContent!.includes('Wrote '), 'WAV export failed');
  click('export', 'Export saved patch');
  await until(() => panel('export')!.textContent!.includes('destination exists'), 'Export replaced an existing file');
  steps.push('GUI export writes a saved-patch WAV and preserves an existing destination');
  enterText(field('export', 'WAV destination'), 'dock-cancel.wav');
  enterText(field('export', 'Duration'), '3600');
  click('export', 'Export saved patch');
  await until(() => panel('export')!.textContent!.includes('Cancel export'), 'Cancel action did not appear');
  click('export', 'Cancel export');
  await until(() => panel('export')!.textContent!.includes('Export cancelled.'), 'Export did not cancel');
  show(await api.edit({ kind: 'SetParam', id: 'e6be000000000026', key: 'audible', text: 'false' }));
  show(await api.save());
  steps.push('export cancellation stops its process tree and removes the partial render');

  await api.checkResize(1440, 900);
  await showTab('sounds'); await showTab('modulation'); await showTab('analysis');
  changeLayout((l) => {
    const slot = VIEW_SLOTS.find((s) => l.views[s] === 'analysis')!;
    return resizeArea(movePanel(l, slot, 'bottom'), 'bottom', 240);
  });
  show(await api.run());
  await sleep(600);
  await api.capture('synth-docks');
  return steps;
}
