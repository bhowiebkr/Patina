// The GUI steps of the self-check (check.ts): a node GUI, a view, a view that does not return, a
// view that fires a trigger, a second instance and a float window (4.3, 15.8, 16.1).

import { emit, listen } from '@tauri-apps/api/event';
import { api, type View } from '../api';
import { changeLayout, flushLayout } from '../panels/Dock';
import { showGraph } from '../panels/layout';
import { checkGuard, checkGuiZoom, checkWidgets } from './check-gui';
import { A, BUILD_MS, EVENT_NODE, GUI_NODE, RUN_MS, T, VIEW_CODE, button, enterText, leftTab, mouseWheel, scale, statusText, tab, topButton, undoCode, type At, type CheckContext } from './check-fixtures';

/** Runs the node GUI and view steps with the code file `lib` of node A, and returns the last view. */
export async function checkViews({ step, until, result, mark, events, steps, fresh }: CheckContext, v: View, lib: string, at: At) {
  let from = 0;
  const ofRun = () => events.slice(from).filter((e) => e.serial === v.serial);
  // A node GUI (4.3): the node body shows the button of the `ui`. A click in a live graph runs
  // the graph again with the new state, and does not build again.
  v = await step('make a node with a GUI', () => api.edit({ kind: 'SetCode', path: lib, text: GUI_NODE }));
  from = mark();
  v = await step('run the node GUI', () => api.run());
  await result(v, from, '2.5', BUILD_MS);
  const guiButton = (text: string) =>
    [...document.querySelectorAll<HTMLButtonElement>(`.react-flow__node[data-id="${A}"] .node-gui button`)].find((b) => b.textContent === text);
  await until('the button of the node GUI', RUN_MS, () => guiButton('Add 1')?.disabled === false);
  from = mark();
  guiButton('Add 1')!.click();
  const clicked = await result(v, from, '3.5', RUN_MS);
  if (clicked.builtMs !== null) throw new Error('a click in a node GUI built the graph again');
  // A node GUI writes its own parameter through the edit path, so undo removes the write.
  from = mark();
  guiButton('Set 4')!.click();
  await result(v, from, '5', RUN_MS);
  from = mark();
  v = await step('undo the parameter write of the node GUI', () => api.undo());
  await result(v, from, '3.5', RUN_MS);
  if (v.graph.node?.find((n) => n.id === A)?.params?.value !== '2.5') throw new Error('undo did not remove the parameter write of the node GUI');
  // Bake fuses node A back into the base, and the base takes the state of A from the slot (7.2).
  await until('an enabled Bake button', RUN_MS, () => topButton('Bake')?.disabled === false);
  from = mark();
  topButton('Bake')!.click();
  await until('the run of the fused clusters', BUILD_MS, () => {
    const e = ofRun().find((e) => e.state === 'failed' || (e.state === 'ran' && e.fused));
    if (e?.state === 'failed') throw new Error(`the fuse failed: ${e.text}`);
    return e !== undefined;
  });
  await until('the value of the moved state', RUN_MS, () => fresh().includes('3.5'));
  if (fresh().includes('2.5')) throw new Error('the fuse did not move the state of the node GUI');
  steps.push('the Bake button fuses the node, and the fuse moves the state of the node GUI');
  const gui = `.react-flow__node[data-id="${A}"] .node-gui`;
  await checkWidgets(until, gui);
  await checkGuard(until);
  // The wheel over the node GUI zooms the canvas (15.2).
  const wheel = (deltaY: number) => document.querySelector(gui)!.dispatchEvent(mouseWheel(deltaY, at.clientX, at.clientY));
  await checkGuiZoom(until, gui, wheel, scale);
  steps.push('node GUI at a far zoom');
  await api.capture('node-gui');
  tab('Parameters')!.click();
  await until('the large node GUI in the Inspector', RUN_MS, () => document.querySelector('aside[aria-label="Inspector"] .node-gui .gui-image') !== null);
  await api.capture('node-gui-large');
  // The expand button opens the full size, the node draws it in the full mode, and Esc closes it.
  document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${A}"] button[aria-label="Open the full size"]`)!.click();
  const full = () => document.querySelector<HTMLElement>('.full-gui');
  await until('the full size of the node GUI', RUN_MS, () => full()?.textContent?.includes('Full size') === true);
  if (document.querySelector(gui)?.textContent?.includes('Full size')) throw new Error('the node body shows the full size');
  await api.capture('node-gui-full');
  full()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await until('the closed full size', RUN_MS, () => !full());
  // Back in the top bar closes the full size too.
  document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${A}"] button[aria-label="Open the full size"]`)!.click();
  await until('the full size and the Back button', RUN_MS, () => !!full() && !!topButton('Back'));
  topButton('Back')!.click();
  await until('the closed full size after Back', RUN_MS, () => !full() && !topButton('Back'));
  // A new wire builds again, and the new build takes the GUI state of A from the fused base (7.2).
  const delay = 'd000000000000004';
  const moveX = () => document.querySelector<HTMLInputElement>(`${gui} .gui-transform-fields input`)?.value;
  from = mark();
  v = await step('wire a delay after the baked node GUI', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddNode', id: delay, folder: 'delay', builtin: true, pos: [300, 300] },
        { kind: 'Connect', from: `${A}.out`, to: `${delay}.value` },
      ],
    }),
  );
  const wired = await result(v, from, '3.5', BUILD_MS);
  if (wired.builtMs === null) throw new Error('a new wire did not build the graph again');
  if (fresh().includes('2.5')) throw new Error(`the build after a new wire did not move the GUI state: ${fresh().join(', ')}`);
  await until('the transform X of the moved GUI state', RUN_MS, () => moveX() === '48.78');
  v = await step('undo the delay', () => api.undo());
  steps.push('a new wire moves the state of the node GUI');
  await step('pause', () => api.pause());
  v = await step('undo the node GUI', undoCode);
  steps.push('click a node GUI');

  // A view (15.8): the Views menu opens it in the View panel. Its button writes a parameter
  // through the edit path, so undo removes the write.
  leftTab('Project').click();
  const viewName = () => document.querySelector<HTMLInputElement>('[aria-label="View name"]');
  await until('the New view button', RUN_MS, () => !!document.querySelector('[aria-label="New view"]'));
  document.querySelector<HTMLButtonElement>('[aria-label="New view"]')!.click();
  await until('the view name field', RUN_MS, () => !!viewName());
  enterText(viewName()!, 'panel');
  await until('the new view folder', RUN_MS, () => !viewName() && [...document.querySelectorAll('[aria-label=Project] .item')].some((b) => b.textContent === 'panel'));
  steps.push('make a view');
  v = await step('write the view code', () => api.edit({ kind: 'SetCode', path: 'views/panel/src/lib.rs', text: VIEW_CODE }));
  button('Views')!.click();
  const item = () => [...document.querySelectorAll<HTMLElement>('[role="menuitemcheckbox"]')].find((e) => e.textContent?.trim() === 'panel');
  await until('the item of the view in the Views menu', RUN_MS, () => !!item());
  item()!.click();
  const panel = 'aside[aria-label="View"]';
  const viewText = () => {
    const failed = document.querySelector(`${panel} .error-text`)?.textContent;
    if (failed) throw new Error(`the view failed: ${failed}`);
    return document.querySelector(`${panel} .node-gui`)?.textContent ?? '';
  };
  await until('the widgets of the view', BUILD_MS, () => viewText().startsWith('value '));
  if (!viewText().startsWith(`value ${v.graph.node?.find((n) => n.id === A)?.params?.value ?? ''}Set`)) throw new Error(`the view shows ${viewText()}`);
  const viewButton = (text: string) => [...document.querySelectorAll<HTMLButtonElement>(`${panel} .node-gui button`)].find((b) => b.textContent === text);
  viewButton('Set')!.click();
  await until('the write of the view', RUN_MS, () => viewText().startsWith('value 5Set'));
  await api.capture('view');
  v = await step('undo the view write', () => api.undo());
  if (v.graph.node?.find((n) => n.id === A)?.params?.value === '5') throw new Error('undo did not remove the write of the view');
  steps.push('a view sets a parameter');

  // A view call that does not return names no node. The second Stop ends it too (16.1).
  v = await step('a value that the view does not return at', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '33' }));
  v = await step('run with the view that does not return', () => api.run());
  // Stop before the hang ends the watch of the status bar, so wait for the view call.
  for (const end = Date.now() + RUN_MS; (await api.running()) !== ''; ) {
    if (Date.now() > end) throw new Error(`no result after ${RUN_MS} ms: the view call that does not return`);
    await new Promise((r) => setTimeout(r, 100));
  }
  await until('the Stop button for the view', RUN_MS, () => topButton('Stop')?.disabled === false);
  topButton('Stop')!.click();
  await until('the view that does not stop in the status bar', RUN_MS, () => statusText().includes('or a view does not stop'));
  topButton('Stop')!.click();
  const viewEnded = 'Patina ended the node process, because a node `Default`, a node `Drop` or a view did not stop.';
  await until('the end of the node process for the view', RUN_MS, () => statusText().includes(viewEnded));
  v = await step('undo the value that the view does not return at', () => api.undo());
  v = await step('run in a new node process for the view', () => api.run());
  const again = () => document.querySelector<HTMLButtonElement>(`${panel} .btn`);
  await until('the Try again button of the view', RUN_MS, () => !!again());
  again()!.click();
  // The old error shows until the page draws again after the click.
  await until('the widgets of the view in a new node process', BUILD_MS, () => !document.querySelector(`${panel} .error-text`) && viewText().startsWith('value '));
  steps.push('the second Stop ends a view that does not return');

  // A view fires a `trigger` node of the live run with a text (9.4, 15.8).
  v = await step('make an event node for the view', () => api.edit({ kind: 'SetCode', path: lib, text: EVENT_NODE }));
  v = await step('add a trigger for the view', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddNode', id: T, folder: 'trigger', builtin: true, pos: [-300, 0] },
        { kind: 'Connect', from: `${T}.out`, to: `${A}.text` },
      ],
    }),
  );
  from = mark();
  v = await step('run the triggers for the view', () => api.run());
  await until('a run with triggers that listen for the view', BUILD_MS, () => {
    const failed = ofRun().find((e) => e.state === 'failed' || (e.state === 'ran' && !e.ok));
    if (failed) throw new Error(`the run failed: ${JSON.stringify(failed)}`);
    return ofRun().some((e) => e.state === 'ran' && e.events);
  });
  viewButton('Fire')!.click();
  await until('the event run of the view that prints 6.5', RUN_MS, () => viewText() !== '' && fresh().includes('6.5'));
  topButton('Stop')!.click();
  await until('the end of the event runs of the view', RUN_MS, () => ofRun().some((e) => e.state === 'ended'));
  v = await step('undo the trigger of the view', () => api.undo());
  v = await step('undo the event node of the view', undoCode);
  steps.push('a view fires a trigger node');
  button('Views')!.click();
  await until('the Views menu again', RUN_MS, () => !!item());
  item()!.click();
  const second = () => [...document.querySelectorAll('[role="tab"]')].find((t) => t.textContent === 'panel 2');
  await until('the tab of a second instance', RUN_MS, () => !!second());
  await until('the widgets of the second instance', BUILD_MS, () => viewText().startsWith('value '));
  steps.push('a second instance of a view');

  // A float window (15.8): Float in a window of the tab menu opens the second instance in its own
  // window, and Dock brings it back. The window of the self-check stays hidden.
  let ready = false;
  const stopReady = await listen<string>('float-ready', (e) => (ready ||= e.payload === 'view2'));
  const floatItem = () => [...document.querySelectorAll<HTMLButtonElement>('.menu button')].find((b) => b.textContent === 'Float in a window');
  try {
    second()!.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await until('Float in a window in the tab menu', RUN_MS, () => !!floatItem());
    floatItem()!.click();
    await until('the tab of the float view to go', RUN_MS, () => !second());
    await until('the float window', BUILD_MS, () => ready);
  } finally {
    stopReady();
  }
  await emit('float-dock', 'view2');
  await until('the docked tab of the float view', RUN_MS, () => !!second());
  await until('the widgets of the docked view', BUILD_MS, () => viewText().startsWith('value '));
  // A drag of the view tab that ends out of the window, with no drop, floats the view (15.2).
  ready = false;
  const stopOut = await listen<string>('float-ready', (e) => (ready ||= e.payload === 'view2'));
  try {
    const data = new DataTransfer();
    second()!.dispatchEvent(new DragEvent('dragstart', { dataTransfer: data, bubbles: true }));
    await new Promise((r) => setTimeout(r, 50));
    second()!.dispatchEvent(new DragEvent('dragend', { dataTransfer: data, bubbles: true, clientX: -20, clientY: 40, screenX: 200, screenY: 200 }));
    await until('the tab of the view that dropped out of the window to go', RUN_MS, () => !second());
    await until('the float window of the dropped view', BUILD_MS, () => ready);
  } finally {
    stopOut();
  }
  await emit('float-dock', 'view2');
  await until('the docked tab of the dropped view', RUN_MS, () => !!second());
  // Move to the right area and Move to the center of the tab menu move the view (15.2).
  const menuOf = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('.menu button')].find((b) => b.textContent === label);
  for (const [label, area] of [['Move to the right area', 'right'], ['Move to the center', 'center']]) {
    second()!.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await until(`${label} in the tab menu`, RUN_MS, () => !!menuOf(label));
    menuOf(label)!.click();
    await until(`the view tab in the ${area} area`, RUN_MS, () => !!second()?.closest(`.area.${area}`));
  }
  // The views open in the center (15.2). The Graph tab shows the graph under them again.
  const graphTab = () => [...document.querySelectorAll<HTMLElement>('[role="tab"]')].find((t) => t.textContent === 'Graph');
  if (!document.querySelector('.center-view') || !graphTab()) throw new Error('the view did not open in the center');
  graphTab()!.click();
  await until('the graph under the views', RUN_MS, () => !document.querySelector('.center-view') && graphTab()?.getAttribute('aria-selected') === 'true');
  changeLayout(showGraph);
  steps.push('Float in a window and a drop out of the window float a view, and the tab menu moves it to the right area and the center');
  // Move to the split pane shows the view next to the graph. The keys move its splitter, Stack the
  // panes puts it under the graph, and Split center moves it back (15.2).
  second()!.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  await until('Move to the split pane in the tab menu', RUN_MS, () => !!menuOf('Move to the split pane'));
  menuOf('Move to the split pane')!.click();
  // The display scale can give a width such as 480.00003, so the width is rounded.
  const width = (sel: string) => Math.round(document.querySelector(sel)?.getBoundingClientRect().width ?? 0);
  await until('the view in the split pane next to the graph', RUN_MS, () => !!second()?.closest('.panes > .area.split') && !document.querySelector('.center-view') && width('.react-flow__pane') > 200 && width('.area.split') === 480);
  const paneSplitter = () => document.querySelector<HTMLElement>('[aria-label="Size of the split area"]')!;
  paneSplitter().dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', shiftKey: true, bubbles: true }));
  await until('a wider split pane', RUN_MS, () => width('.area.split') === 544);
  // The split pane and its size come back when the project opens again (15.2).
  await flushLayout();
  const live = v.live;
  if (v.dirty) v = await step('save before the open with a split', () => api.save());
  v = await step('open again with a split', () => api.openProject(v.root));
  await until('the view in the split pane at 544 px after the open', BUILD_MS, () => !!second()?.closest('.panes > .area.split') && width('.area.split') === 544);
  if (live) v = await step('run after the open with a split', () => api.run());
  steps.push('the split pane and its size after an open');
  const fromViews = async (label: string) => {
    button('Views')!.click();
    await until(`${label} in the Views menu`, RUN_MS, () => !!menuOf(label));
    menuOf(label)!.click();
  };
  await fromViews('Stack the panes');
  await until('the split pane under the graph', RUN_MS, () => !!document.querySelector('.panes.stacked > .area.split') && paneSplitter().getAttribute('aria-orientation') === 'horizontal');
  await fromViews('Stack the panes');
  await fromViews('Split center');
  await until('the view back in the center', RUN_MS, () => !!second()?.closest('.area.center') && !document.querySelector('.area.split') && !document.querySelector('.panes.stacked'));
  changeLayout(showGraph);
  steps.push('Move to the split pane, the split splitter, Stack the panes and Split center');
  return v;
}
