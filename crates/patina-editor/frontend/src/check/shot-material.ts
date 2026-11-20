// The native material workbench, checked on the hidden screenshot desktop.
import { api, onEngine, onOutput, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { closePanel, openView, resizeArea, VIEW_SLOTS } from '../panels/layout';
import { entry, rightClick } from './check-fixtures';

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const panel = () => document.querySelector<HTMLElement>('aside[data-view="material-preview"]');
async function capture(name: string) {
  const r = panel()?.querySelector('.gui-viewport')?.getBoundingClientRect();
  return api.capture(name, r && [r.left, r.top, r.width, r.height].map((v) => v * devicePixelRatio));
}
async function until(test: () => boolean, message: string, fault: () => string) {
  const end = Date.now() + 150_000;
  while (!test()) {
    if (fault()) throw new Error(fault());
    const error = panel()?.querySelector('.error-text')?.textContent;
    if (error) throw new Error(error);
    if (Date.now() > end) throw new Error(message);
    await sleep(100);
  }
}

export async function materialWorkbench(view: View, show: (view: View) => void, fit: () => string): Promise<string[]> {
  if (!view.views.includes('material-preview')) return [];
  let stage = 'open';
  let failure = '';
  let runs = 0;
  const errors: string[] = [];
  const off = await onEngine((event) => {
    if (event.state === 'lost' || event.state === 'failed') { failure = event.text; errors.push(event.text); }
    if (event.state === 'ran') {
      runs++;
      if (!event.ok) failure = JSON.stringify(event.reports);
    }
  });
  const offOutput = await onOutput((event) => { if (event.err) errors.push(event.text); });
  try {
    const steps: string[] = [];
    changeLayout((layout) => VIEW_SLOTS.reduce((next, slot) => closePanel(next, slot), layout));
    await sleep(500);
    show(await api.stop());
    show(await api.openGraph('materials'));
    stage = 'first material run';
    const beforeRun = runs;
    const first = await api.run(); show(first);
    await until(() => runs > beforeRun, 'The copper graph did not finish', () => failure);
    await until(() => document.querySelectorAll('.react-flow__node[data-id^="c0ffee"]').length === 28, 'The copper graph did not open', () => failure);
    changeLayout((layout) => openView(VIEW_SLOTS.reduce((next, slot) => closePanel(next, slot), layout), 'material-preview', 'right'));
    await until(() => !!panel()?.querySelector('.node-gui'), 'The material view did not build', () => failure);
    await sleep(500);
    stage = 'capture material value';
    const beforeCapture = runs;
    show(await api.run());
    await until(() => runs > beforeCapture, 'The material capture run did not finish', () => failure);
    await until(() => !!panel()?.querySelector('.gui-viewport'), 'The material view did not receive its GPU value', () => failure);
    await sleep(1000);
    steps.push(fit());
    await capture('materials-graph');
    stage = 'preview layout';
    for (const width of [240, 360]) {
      changeLayout((layout) => resizeArea(layout, 'right', width));
      await sleep(400);
      const body = panel()!.querySelector<HTMLElement>('.view-controls')!;
      if (body.scrollWidth > body.clientWidth + 1) throw new Error(`The material preview needs horizontal scrolling at ${width}`);
      const area = panel()!.querySelector('.gui-viewport')!.getBoundingClientRect();
      const drawn = await api.capture(`materials-preview-${width}`, [area.left, area.top, area.width, area.height].map((v) => v * devicePixelRatio));
      if (drawn !== null && drawn < 0.9) throw new Error(`Material preview drew ${drawn} of its viewport`);
    }
    steps.push('the native material preview draws in 240 and 360 px side docks');
    const select = panel()!.querySelector<HTMLSelectElement>('select')!;
    for (const index of [1, 2, 3, 4, 5, 6, 0]) {
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value')!.set!.call(select, String(index));
      select.dispatchEvent(new Event('change', { bubbles: true }));
      await sleep(180);
    }
    steps.push('seven material display channels accept GUI selection');
    stage = 'object bake and export';
    const objectId = 'd00d000000000001';
    const bakeId = 'd00d000000000002';
    const exportId = 'd00d000000000003';
    for (const id of [objectId, bakeId, exportId]) {
      const node = document.querySelector<HTMLElement>(`.react-flow__node[data-id="${id}"]`)!;
      rightClick(node);
      await until(() => !!entry('Node', 'Fit node'), 'No workflow Fit node command', () => failure);
      entry('Node', 'Fit node')!.click();
      await sleep(400);
      steps.push(fit());
    }
    const bakeNode = document.querySelector<HTMLElement>(`.react-flow__node[data-id="${bakeId}"]`)!;
    if (!bakeNode.textContent?.includes('2048 maps / saved bake')) throw new Error('The graph did not use the original 2048 sphere bake');
    show(await api.edit({ kind: 'SetParam', id: exportId, key: 'path', text: `build/check-material-passes-${Date.now()}.zip` }));
    await sleep(500);
    const exportNode = document.querySelector<HTMLElement>(`.react-flow__node[data-id="${exportId}"]`)!;
    const exportButton = [...exportNode.querySelectorAll('button')].find((b) => b.textContent === 'Export passes');
    if (!exportButton) throw new Error('No Export passes button');
    exportButton.click();
    await until(() => exportNode.textContent?.includes('Saved 8 PBR passes.') ?? false, 'The export button did not write PBR passes', () => failure);
    await capture('materials-object-export');
    const bakeButton = [...bakeNode.querySelectorAll('button')].find((b) => b.textContent === 'Bake maps again');
    if (!bakeButton) throw new Error('No Bake maps again button');
    bakeButton.click();
    await until(() => bakeNode.textContent?.includes('2048 maps / ray traced') ?? false, 'The sphere did not rebake at its original atlas resolution', () => failure);
    await capture('materials-sphere-bake');
    show(await api.edit({ kind: 'SetParam', id: bakeId, key: 'rebake', text: '0' }));
    const beforeRestore = runs;
    show(await api.run());
    await until(() => runs > beforeRestore, 'The reference object did not restore', () => failure);
    steps.push('the original sphere loads its 2048 maps, rebakes at 2048, restores its saved bake, and exports eight PBR passes');
    stage = 'node controls';
    for (const suffix of ['01', '0b', '18', '1b']) {
      const node = document.querySelector<HTMLElement>(`.react-flow__node[data-id="c0ffee00000000${suffix}"]`)!;
      rightClick(node);
      await until(() => !!entry('Node', 'Fit node'), 'No Fit node command', () => failure);
      entry('Node', 'Fit node')!.click();
      await sleep(800);
      steps.push(fit());
      await capture(`materials-node-${suffix}`);
    }
    const id = 'c0ffee000000001b';
    const before = await api.pause(); show(before);
    const depth = before.graph.node?.find((n) => n.id === id)?.params?.depth;
    const control = document.querySelector<HTMLInputElement>(`.react-flow__node[data-id="${id}"] input[type="range"]`)!;
    const drag = (value: string) => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(control, value);
      control.dispatchEvent(new Event('input', { bubbles: true }));
    };
    const depthNow = async () => { const v = await api.pause(); show(v); return v.graph.node?.find((n) => n.id === id)?.params?.depth; };
    // The node gets the values during the drag, before the pointer goes up, and one undo removes all of the drag (9.1).
    control.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, pointerId: 1 }));
    drag('0.02');
    const end = Date.now() + 10_000;
    while (Number(await depthNow()) !== 0.02) {
      if (Date.now() > end) throw new Error('The Finish slider did not change depth during the drag');
      await sleep(100);
    }
    drag('0.025');
    await sleep(500);
    control.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, pointerId: 1 }));
    await sleep(300);
    if (Number(await depthNow()) !== 0.025) throw new Error('The Finish slider did not change depth');
    const undo = await api.undo(); show(undo);
    if (undo.graph.node?.find((n) => n.id === id)?.params?.depth !== depth) throw new Error('One undo did not restore Finish depth after the drag');
    show(await api.save());
    const reopened = await api.openGraph('main'); show(reopened);
    const saved = await api.openGraph('materials'); show(saved);
    if (saved.graph.node?.find((n) => n.id === id)?.params?.depth !== depth) throw new Error('Material parameters did not survive graph switching');
    steps.push('a Finish slider drag changes depth before the release, one undo restores it, and save and top graph switching keep it');
    show(await api.stop());
    show(await api.openGraph('main'));
    changeLayout((layout) => VIEW_SLOTS.reduce((next, slot) => closePanel(next, slot), layout));
    return steps;
  } catch (error) {
    await sleep(600);
    await api.capture('materials-failed');
    throw new Error(`${stage}: ${error}\n${errors.slice(-15).join('\n')}`);
  } finally {
    off(); offOutput();
  }
}
