// The dock steps of the self-check (check.ts): the tab menu, the splitter, the layout and the pan
// and zoom after an open, the rail, the run times, the Debug tab and the far zoom (15.1, 15.2).

import { api, type EngineEvent, type View } from '../api';
import { flushLayout, savedViewport } from '../panels/Dock';
import { layoutText, parseLayout } from '../panels/layout';
import { FAR_ZOOM } from '../wires/gl';
import { A, B, BUILD_MS, RUN_MS, badge, mouseWheel, scale, tab, transform, type CheckContext } from './check-fixtures';

/** Saves, closes and opens the project again. It returns the last view and the middle of the canvas. */
export async function checkDock({ step, until, result, mark, events, steps }: CheckContext, v: View) {
  v = await step('save', () => api.save());
  if (v.dirty) throw new Error('the project is still dirty after a save');
  const saved = JSON.stringify(v.graph);
  await step('pause', () => api.pause());

  // The dock (15.2): a drag and the menu of a tab move it, the keys and a drag move a splitter, and
  // the layout and the pan and zoom of the graph come back when the project opens again.
  const data = new DataTransfer();
  const drag = (el: Element, type: string) => el.dispatchEvent(new DragEvent(type, { dataTransfer: data, bubbles: true, cancelable: true }));
  const logs = tab('Logs')!;
  drag(logs, 'dragstart');
  await new Promise((r) => setTimeout(r));
  drag(tab('Project')!, 'dragover');
  drag(tab('Project')!, 'drop');
  drag(logs, 'dragend');
  await until('Logs before Project in the left area after a drag', RUN_MS, () => tab('Logs')?.closest('.area.left') !== null && tab('Logs')?.nextElementSibling === tab('Project'));
  const menuItem = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('.menu button')].find((b) => b.textContent === label);
  tab('Logs')!.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  await until('the menu of the tab', RUN_MS, () => menuItem('Move to the right area') !== undefined);
  menuItem('Move to the right area')!.click();
  await until('Logs in the right area', RUN_MS, () => !!tab('Logs')?.closest('.area.right'));
  const splitter = () => document.querySelector<HTMLElement>('[aria-label="Size of the left area"]')!;
  splitter().dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', shiftKey: true, bubbles: true }));
  await until('a wider left area', RUN_MS, () => splitter().getAttribute('aria-valuenow') === '324');
  // Chromium gives page pointer events no capture, so the moves go to the splitter itself.
  const line = splitter().getBoundingClientRect();
  const press = (type: string, dx: number) =>
    splitter().dispatchEvent(new PointerEvent(type, { clientX: line.left + dx, clientY: line.top + 40, bubbles: true, pointerId: 1, isPrimary: true, button: 0 }));
  press('pointerdown', 0);
  press('pointermove', 40);
  await until('a left area 40 px wider during a splitter drag', RUN_MS, () => splitter().getAttribute('aria-valuenow') === '364');
  press('pointermove', 0);
  press('pointerup', 0);
  await until('the left area back to 324 px after the drag', RUN_MS, () => splitter().getAttribute('aria-valuenow') === '324');
  steps.push('drag a tab and a splitter');
  const pane = document.querySelector('.react-flow__pane')!.getBoundingClientRect();
  const was = savedViewport('main');
  const at = { clientX: pane.left + pane.width / 2, clientY: pane.top + pane.height / 2 };
  document.querySelector('.react-flow__pane')!.dispatchEvent(mouseWheel(-120, at.clientX, at.clientY));
  await until('the saved pan and zoom', RUN_MS, () => savedViewport('main') !== was);
  // A two-finger touchpad scroll pans on the two axes and keeps the zoom (15.2).
  const prev = savedViewport('main')!;
  document.querySelector('.react-flow__pane')!.dispatchEvent(new WheelEvent('wheel', { deltaX: 30, deltaY: 20, ...at, bubbles: true, cancelable: true }));
  await until('the touchpad pan', RUN_MS, () => {
    const v = savedViewport('main');
    return !!v && v.zoom === prev.zoom && Math.abs(v.x - prev.x + 30) < 0.5 && Math.abs(v.y - prev.y + 20) < 0.5;
  });
  steps.push('touchpad pan');
  const zoomed = transform();
  await flushLayout();
  const file = parseLayout(await api.layout(v.id));
  if (!file.areas.right.tabs.includes('logs') || file.areas.left.size !== 324 || !file.viewports.main) {
    throw new Error(`the saved layout is not the layout of the window: ${layoutText(file)}`);
  }
  steps.push('dock layout saved');

  await api.closeProject();
  const start = await api.startInfo();
  if (start.recent[0]?.path !== v.root) throw new Error(`the recent list starts with ${start.recent[0]?.path}`);
  steps.push('close');

  let from = mark();
  v = await step('open again', () => api.openProject(v.root));
  if (JSON.stringify(v.graph) !== saved) throw new Error('the graph after open is not the saved graph');
  if (!v.trusted || v.live) throw new Error('an open project is trusted and paused');
  // An open loads the last build, so the node GUIs show while the graph is paused (15.10).
  const opened = (state: EngineEvent['state']) => events.slice(from).some((e) => e.serial === v.serial && e.state === state);
  await until('the load of the last build', BUILD_MS, () => opened('loaded') || opened('failed'));
  if (!opened('loaded') || opened('ran')) throw new Error('the open did not load the last build, or it ran the graph');
  await api.closeProject();
  from = mark();
  v = await step('open again with no change', () => api.openProject(v.root));
  await until('the second load', BUILD_MS, () => opened('loaded') || opened('failed'));
  if (opened('building') || opened('ran')) throw new Error('an open with no change built or ran the graph');
  await until('Logs in the right area after open', RUN_MS, () => !!tab('Logs')?.closest('.area.right'));
  if (transform() !== zoomed) throw new Error(`the pan and zoom after open are ${transform()}, not ${zoomed}`);
  const views = [...document.querySelectorAll<HTMLButtonElement>('.topbar button')].find((b) => b.textContent === 'Views')!;
  views.click();
  await until('the Views menu', RUN_MS, () => menuItem('Reset layout') !== undefined);
  menuItem('Reset layout')!.click();
  await until('Logs in the bottom area after Reset layout', RUN_MS, () => !!tab('Logs')?.closest('.area.bottom'));
  if (transform() !== zoomed) throw new Error('Reset layout changed the pan and zoom');
  steps.push('dock layout restored and reset');
  // The rail icon of the active left panel folds the left area, and a second click unfolds it.
  const rail = (name: string) => document.querySelector<HTMLButtonElement>(`nav.strip button[aria-label="${name}"]`)!;
  rail('Project').click();
  await until('a folded left area', RUN_MS, () => document.querySelector('.area.left') === null && rail('Project').getAttribute('aria-pressed') === 'false');
  rail('Project').click();
  await until('the left area again', RUN_MS, () => document.querySelector('.area.left') !== null && rail('Project').getAttribute('aria-pressed') === 'true');
  steps.push('rail folds the left area');

  from = mark();
  v = await step('run again', () => api.run());
  await result(v, from, '2.5', BUILD_MS);
  await until('two ready nodes on the canvas', RUN_MS, () => document.querySelectorAll('.patina-node.ready').length === 2);
  steps.push('ready on the canvas');
  const timeOf = (id: string) => document.querySelector(`.react-flow__node[data-id="${id}"] .patina-node-time`)?.textContent ?? '';
  await until('the run time on each node', RUN_MS, () => [A, B].every((id) => /^Run \d+(\.\d+)? (us|ms|s)$/.test(timeOf(id))));
  steps.push(`run time on the nodes: ${timeOf(A)}, ${timeOf(B)}`);
  // The Debug tab shows a bar for each node call on the timeline of its thread (15.7).
  tab('Debug')!.click();
  await until('a bar for each node in the Debug tab', RUN_MS, () => document.querySelectorAll('.debug-call:not(:disabled)').length >= 2);
  steps.push(`Debug tab: ${document.querySelector('.debug-lane header')?.textContent}`);
  tab('Output')!.click();
  await until('the live value on the wire', RUN_MS, () => badge()?.textContent === '2.5');
  if (badge()!.classList.contains('stale')) throw new Error('the value of the last run is stale');
  const label = document.querySelector('.wire-type')?.textContent;
  if (label !== 'f64') throw new Error(`the wire type label is ${label}, not f64`);
  steps.push('live value and type label on the wire');

  // Below FAR_ZOOM, the nodes have no row text and the wires have no badge (15.1).
  const nodeElements = () => document.querySelectorAll('.react-flow__node *').length;
  const wheel = (deltaY: number) =>
    document.querySelector('.react-flow__pane')!.dispatchEvent(mouseWheel(deltaY, at.clientX, at.clientY));
  const near = nodeElements();
  const heights = () => [...document.querySelectorAll<HTMLElement>('.react-flow__node')].map((n) => n.offsetHeight).join(' ');
  const boxes = heights();
  let outs = 0;
  while (scale() >= FAR_ZOOM) {
    const z = scale();
    wheel(240);
    outs++;
    await until('a smaller zoom', RUN_MS, () => scale() < z);
  }
  await until('the simple node form', RUN_MS, () =>
    document.querySelectorAll('.patina-row').length === 0 && !document.querySelector('.wire-value') && !document.querySelector('.wire-type'));
  if (document.querySelectorAll('.patina-node .title').length !== 2) throw new Error('a node at a far zoom has no title');
  if (heights() !== boxes) throw new Error(`the node heights are ${heights()} at a far zoom, not ${boxes}`);
  steps.push(`simple nodes at a far zoom: ${near} node elements near, ${nodeElements()} far`);
  for (; outs > 0; outs--) {
    const z = scale();
    wheel(-240);
    await until('a larger zoom', RUN_MS, () => scale() > z);
  }
  await until('the badge after the far zoom', RUN_MS, () => badge()?.textContent === '2.5' && document.querySelectorAll('.patina-row').length > 0);
  return { v, at };
}
