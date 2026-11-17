// The palette steps of the self-check (check.ts): the stale value, the Add node palette, the order
// wires, the new folders and Add crate (9.2, 15.2, 15.4).

import { api, type View } from '../api';
import { A, B, RUN_MS, badge, enterText, type At, type CheckContext } from './check-fixtures';

/** Runs the palette steps with the point `at` on the canvas, and returns the last view. */
export async function checkPalette({ step, until, steps }: CheckContext, v: View, at: At, edited: (v: View) => void) {
  // While the graph is paused, an edit makes the value of the last run stale (15.3, 15.9).
  await step('pause', () => api.pause());
  v = await step('edit while paused', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '3' }));
  edited(v);
  await until('the stale value on the wire', RUN_MS, () => badge()?.classList.contains('stale') === true && badge()?.textContent === '2.5').catch((e: Error) => {
    throw new Error(`${e.message} (badges ${[...document.querySelectorAll('.wire-value')].map((b) => `${b.textContent}:${b.className}`).join()})`);
  });
  v = await step('undo the edit', () => api.undo());

  // The Add node palette (15.2): Shift+A opens it and Esc closes it. A double-click on an empty
  // area opens it, and Enter adds the first match at the point of the double-click.
  const search = () => document.querySelector<HTMLInputElement>('.palette input');
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'A', shiftKey: true, bubbles: true }));
  await until('the palette after Shift+A', RUN_MS, () => !!search());
  search()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await until('no palette after Esc', RUN_MS, () => !search());
  document.querySelector('.react-flow__pane')!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, ...at }));
  await until('the palette after a double-click', RUN_MS, () => !!search());
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(search(), 'SHO');
  search()!.dispatchEvent(new Event('input', { bubbles: true }));
  const options = () => [...document.querySelectorAll('.palette [role=option]')].map((o) => o.textContent);
  await until('two matches for SHO', RUN_MS, () => options().join() === 'show,showchecklib');
  search()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  const newNode = () => [...document.querySelectorAll('.react-flow__node')].find((n) => ![A, B].includes(n.getAttribute('data-id')!));
  await until('the added node', RUN_MS, () => !search() && newNode()?.querySelector('.title')?.textContent === 'show');
  const box = newNode()!.querySelector('.patina-node')!.getBoundingClientRect();
  if (Math.abs(box.left - at.clientX) > 4 || Math.abs(box.top - at.clientY) > 4) throw new Error('the added node is not at the point of the double-click');
  v = await step('undo the added node', () => api.undo());
  if (v.graph.node?.length !== 2) throw new Error('undo did not remove the added node');
  steps.push('add a node with the palette');

  // A drag from a pin into an empty area opens the palette with only the nodes that fit the pin
  // (15.2). The new node and its wire are one undo step.
  const dragFromPin = async (what: string) => {
    const pinEl = document.querySelector(`.react-flow__node[data-id="${B}"] .react-flow__handle[data-handleid="value"]`)!;
    const pinBox = pinEl.getBoundingClientRect();
    pinEl.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, clientX: pinBox.left + pinBox.width / 2, clientY: pinBox.top + pinBox.height / 2 }));
    document.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, ...at }));
    document.querySelector('.react-flow__pane')!.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, ...at }));
    await until(`the palette after ${what}`, RUN_MS, () => !!search());
  };
  const fedFromPin = async (what: string) => {
    await until(`the ${what} node from the pin`, RUN_MS, () => !search() && newNode()?.querySelector('.title')?.textContent === 'number');
    const fed = newNode()!.getAttribute('data-id');
    v = await step(`undo the ${what} node and its wire`, () => api.undo());
    if (v.graph.node?.length !== 2 || v.graph.wire?.[0]?.from !== `${A}.out`) throw new Error(`one undo did not remove the node and its wire: ${JSON.stringify(v.graph.wire)}`);
    v = await step(`redo the ${what} node and its wire`, () => api.redo());
    if (v.graph.wire?.length !== 1 || v.graph.wire[0].from !== `${fed}.out` || v.graph.wire[0].to !== `${B}.value`) {
      throw new Error(`the new node does not feed the pin: ${JSON.stringify(v.graph.wire)}`);
    }
    v = await step(`undo the ${what} node from a pin`, () => api.undo());
  };
  await dragFromPin('a drag from a pin');
  // The show node has no output. The output of the built-in delay is generic, so it fits. The
  // subgraph of the Test mode step and the number node of the library have an f64 output.
  await until('only the nodes with an f64 output', RUN_MS, () => options().join() === 'number,numberchecklib,switch_checksubgraph,delaybuilt-in').catch((e: Error) => {
    throw new Error(`${e.message} (options ${options().join()})`);
  });
  search()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await fedFromPin('added');
  // A library node from the palette of a drag gets the wire in the same undo step (6.2).
  await dragFromPin('a second drag from a pin');
  const libOption = () => [...document.querySelectorAll<HTMLElement>('.palette [role=option]')].find((o) => o.textContent === 'numberchecklib');
  await until('the library node in the palette of the drag', RUN_MS, () => !!libOption());
  libOption()!.click();
  await fedFromPin('library');
  steps.push('add a node and a library node from a pin');

  // Order wires (9.2): a drag from the order socket of A to the order socket of B connects them,
  // and Delete on the selected order wire removes it. Both are edits with undo.
  const orderPin = (id: string, side: string) => document.querySelector(`.react-flow__node[data-id="${id}"] .react-flow__handle.${side}[data-handleid="@order"]`)!;
  const mid = (el: Element, b = el.getBoundingClientRect()) => ({ clientX: b.left + b.width / 2, clientY: b.top + b.height / 2 });
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  orderPin(A, 'source').dispatchEvent(new MouseEvent('mousedown', { bubbles: true, ...mid(orderPin(A, 'source')) }));
  document.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, ...mid(orderPin(B, 'target')) }));
  orderPin(B, 'target').dispatchEvent(new MouseEvent('mouseup', { bubbles: true, ...mid(orderPin(B, 'target')) }));
  await until('the order wire after a drag between the order sockets', RUN_MS, () => document.querySelector('.react-flow__edge-order') !== null);
  const orderHit = document.querySelector('.react-flow__edge-order .wire-hit')!;
  orderHit.dispatchEvent(new MouseEvent('click', { bubbles: true, ...mid(orderHit) }));
  await until('the selected order wire', RUN_MS, () => document.querySelector('.react-flow__edge-order.selected') !== null);
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true }));
  await until('no order wire after Delete', RUN_MS, () => document.querySelector('.react-flow__edge-order') === null);
  v = await step('undo the removal of the order wire', () => api.undo());
  if (v.graph.order?.length !== 1 || v.graph.order[0].before !== A || v.graph.order[0].after !== B || v.graph.wire?.length !== 1) {
    throw new Error(`Delete changed more than the order wire: ${JSON.stringify([v.graph.order, v.graph.wire])}`);
  }
  v = await step('undo the order wire', () => api.undo());
  if (v.graph.order?.length) throw new Error('undo did not remove the order wire');
  steps.push('order wire connected and removed');

  // New node (15.2): the + of the Nodes group asks for a name. A used name shows its problem. A
  // new name makes the folder, and the Code tab shows its code.
  const nameField = () => document.querySelector<HTMLInputElement>('[aria-label="Node name"]');
  const enterName = (text: string) => enterText(nameField()!, text);
  document.querySelector<HTMLButtonElement>('[aria-label="New node"]')!.click();
  await until('the name field', RUN_MS, () => !!nameField());
  enterName('Number');
  await until('the problem of a used name', RUN_MS, () => !!document.querySelector('.new-item [role=alert]'));
  enterName('made-node');
  const listed = () => [...document.querySelectorAll('[aria-label=Project] .item')].some((b) => b.textContent === 'made-node');
  const codeTab = () => document.querySelector('[aria-label=Inspector] [aria-selected=true]')?.textContent === 'Code';
  await until('the new node folder and its code', RUN_MS, () => !nameField() && listed() && codeTab());
  steps.push('make a node folder');
  // New widget, New connection and New type (15.4): the same + makes a widget, a connection kind and a type crate.
  for (const [one, name] of [['widget', 'made-widget'], ['connection', 'made-bus'], ['type', 'made-type']]) {
    const field = () => document.querySelector<HTMLInputElement>(`[aria-label="${one[0].toUpperCase()}${one.slice(1)} name"]`);
    document.querySelector<HTMLButtonElement>(`[aria-label="New ${one}"]`)!.click();
    await until(`the ${one} name field`, RUN_MS, () => !!field());
    enterText(field()!, name);
    const shown = () => [...document.querySelectorAll('[aria-label=Project] .item')].some((b) => b.textContent === name);
    await until(`the new ${one} folder and its code`, RUN_MS, () => !field() && shown() && codeTab());
  }
  steps.push('make a widget, a connection and a type folder');

  // Add crate (15.4): a bad crate name shows its problem before cargo runs, so no network is used.
  const crateField = () => document.querySelector<HTMLInputElement>('[aria-label="Crate, widget or type name"]');
  [...document.querySelectorAll<HTMLButtonElement>('.code-bar .btn')].find((b) => b.textContent === 'Add crate')!.click();
  await until('the crate field', RUN_MS, () => !!crateField());
  enterText(crateField()!, '-x');
  const crateProblem = () => document.querySelector('.new-item [role=alert]')?.textContent ?? '';
  await until('the problem of a bad crate name', RUN_MS, () => crateProblem().includes('name@version'));
  crateField()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await until('the closed crate field', RUN_MS, () => !crateField());
  steps.push('refuse a bad crate name');
  // Undo removes a new folder that has only its start files, and redo makes it again (15.2).
  const typeListed = () => [...document.querySelectorAll('[aria-label=Project] .item')].some((b) => b.textContent === 'made-type');
  const typeShown = () => document.querySelector('[aria-label=Inspector] .panel-title')?.textContent === 'made-type';
  v = await step('undo the new type folder', () => api.undo());
  await until('the type folder that undo removes', RUN_MS, () => !typeListed() && !typeShown());
  v = await step('redo the new type folder', () => api.redo());
  await until('the type folder that redo makes again', RUN_MS, () => typeListed() && typeShown() && codeTab());
  steps.push('undo and redo a new folder');
  v = await step('save the route', () => api.save());
  if (v.dirty) throw new Error('the project is still dirty after a save');
  return v;
}
