// The wire steps of the self-check: a node drag, selection, route points, the look of a wire, the routing
// conflict mark, the cross-domain dash and the switch of a bool input (15.1, 15.3, 15.9).

import { api, type View } from '../api';
import { A, B, BUILD_MS, RUN_MS, SWITCH_NODE, amber, anyPixel, gray, icon, undoCode, wireColumn, type CheckContext } from './check-fixtures';

/** The select of an Inspector field, by its label. */
const field = (name: string) =>
    [...document.querySelectorAll('.param')].find((p) => p.querySelector('.label')?.textContent === name)?.querySelector('select');
/** The texts of the live value badges. */
const values = () => [...document.querySelectorAll('.wire-value:not(.stale)')].map((b) => b.textContent);

/** Runs the wire steps on the graph of A to B, and returns the last view. */
export async function checkWires(c: CheckContext, v: View): Promise<View> {
  v = await checkDrag(c, v);
  v = await checkRoutePoints(c, v);
  v = await checkLook(c, v);
  v = await checkSegments(c, v);
  v = await checkDelay(c, v);
  return checkSwitch(c, v);
}

/** A node drag, the selection of a wire, the Inspector value and a lost WebGL context. */
async function checkDrag({ step, until, steps }: CheckContext, v: View): Promise<View> {
  // A mouse drag moves a node (15.2). The drop is one edit with undo, and the wire follows the node.
  const nodeB = () => document.querySelector<HTMLElement>(`.react-flow__node[data-id="${B}"]`)!;
  const posB = (view: View) => JSON.stringify(view.graph.node?.find((n) => n.id === B)?.pos);
  const [startPos, topB, startRoute] = [posB(v), nodeB().getBoundingClientRect().top, document.querySelector('.wire-hit')?.getAttribute('d')];
  if (document.querySelector<HTMLButtonElement>('button[aria-label="Undo"]')?.disabled !== true) throw new Error('the drag check needs a project with no undo history');
  const grip = nodeB().querySelector('.title')!.getBoundingClientRect();
  const mouse = (dy: number) => ({ bubbles: true, cancelable: true, view: window, button: 0, clientX: grip.left + 10, clientY: grip.top + grip.height / 2 + dy });
  nodeB().querySelector('.title')!.dispatchEvent(new MouseEvent('mousedown', mouse(0)));
  for (const dy of [10, 40, 80]) document.dispatchEvent(new MouseEvent('mousemove', mouse(dy)));
  document.dispatchEvent(new MouseEvent('mouseup', mouse(80)));
  await until('the node at the drop point', RUN_MS, () => nodeB().getBoundingClientRect().top - topB > 40 && !nodeB().classList.contains('dragging'));
  await until('the wire to the moved node', RUN_MS, () => document.querySelector('.wire-hit')?.getAttribute('d') !== startRoute);
  // The open project has no undo history, so the Undo button turns on when the move edit arrives.
  await until('the move edit', RUN_MS, () => document.querySelector<HTMLButtonElement>('button[aria-label="Undo"]')?.disabled === false);
  v = await step('undo the node drag', () => api.undo());
  if (posB(v) !== startPos) throw new Error(`undo did not give the node its place: ${posB(v)}, not ${startPos}`);
  steps.push('node dragged');
  // The wire draws in WebGL (15.1). Its hit area is wider than the line, and a click selects it (15.9).
  const wire = await wireColumn();
  if (!anyPixel(wire.column, gray) || anyPixel(wire.column, amber)) throw new Error('the wire is not a neutral line');
  const hit = document.elementFromPoint(wire.x, wire.y + 4);
  if (!hit?.closest('.react-flow__edge')) throw new Error(`no wire 4 px from the line: ${hit?.getAttribute('class')}`);
  hit.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: wire.x, clientY: wire.y + 4 }));
  await until('the selected wire', RUN_MS, () => document.querySelector('.react-flow__edge.selected') !== null);
  if (!anyPixel((await wireColumn()).column, amber)) throw new Error('the selected wire has no accent mark');
  steps.push('wire drawn and selected');
  // The Inspector shows the full text of the selected wire (15.7), from the engine.
  await until('the value in the Inspector', RUN_MS, () => document.querySelector('.value-text')?.textContent === '2.5');
  if ((await api.value(`${A}.out`))?.text !== '2.5') throw new Error('the engine has no full text of the value');
  steps.push('value in the inspector');
  // A lost WebGL context shows the wires as SVG lines, and the restore draws them in WebGL again (15.1).
  const lose = document.querySelector<HTMLCanvasElement>('canvas.wire-layer')?.getContext('webgl2')?.getExtension('WEBGL_lose_context');
  if (!lose) throw new Error('the wire layer has no WebGL context to lose');
  lose.loseContext();
  await until('the SVG wires after a lost WebGL context', RUN_MS, () => {
    const line = document.querySelector('.react-flow.svg-wires .wire-line');
    return !!line && getComputedStyle(line).display !== 'none';
  });
  lose.restoreContext();
  await until('the WebGL wires after the restore', RUN_MS, () => !document.querySelector('.react-flow.svg-wires'));
  if (!anyPixel((await wireColumn()).column, amber)) throw new Error('the restored WebGL wires do not draw');
  steps.push('the wires show as SVG while the WebGL context is lost');
  return v;
}

/** The route points of a wire: add, drag, Esc, an arrow key, Delete and the keyboard. */
async function checkRoutePoints({ step, until, steps }: CheckContext, v: View): Promise<View> {
  // Route points (15.9): a double-click adds one, a drag moves it, and Delete on its handle
  // removes the point but keeps the wire. Undo and redo go through the engine.
  const hitPath = document.querySelector<SVGPathElement>('.wire-hit')!;
  const line = hitPath.getBoundingClientRect();
  hitPath.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, clientX: line.left + line.width / 2, clientY: line.top + line.height / 2 }));
  await until('a route point', RUN_MS, () => document.querySelectorAll('.route-point').length === 1);
  steps.push('route point added');
  const handle = () => document.querySelector<HTMLElement>('.route-point')!;
  const center = (b = handle().getBoundingClientRect()) => [b.left + b.width / 2, b.top + b.height / 2];
  const [cx, cy] = center();
  for (const [type, y] of [['pointerdown', cy], ['pointermove', cy + 60], ['pointerup', cy + 60]] as const) {
    handle().dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: cx, clientY: y }));
  }
  await until('the route point under the pointer after the edit', RUN_MS, () =>
    Math.abs(center()[1] - (cy + 60)) < 2 && !handle().classList.contains('dragging'),
  );
  // The automatic route goes through the point, and before the drag no wire was 60 px below the line.
  if (!anyPixel((await wireColumn({ x: cx, y: cy + 60 })).column, () => true)) throw new Error('the wire does not draw at the new place of the route point');
  steps.push('route point dragged');
  const added = (await step('undo the drag', () => api.undo())).graph.wire?.[0]?.points;
  const moved = (await step('redo the drag', () => api.redo())).graph.wire?.[0]?.points;
  if (added?.length !== 1 || moved?.length !== 1 || !(moved[0][1] > added[0][1])) throw new Error(`the drag did not move the point: ${JSON.stringify([added, moved])}`);
  // Esc during a drag puts the point back and sends no edit. An arrow key moves the point.
  await until('the route point after the redo', RUN_MS, () => Math.abs(center()[1] - (cy + 60)) < 2);
  const [ex, ey] = center();
  const pointer = (type: string, y: number) => new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: ex, clientY: y });
  handle().focus();
  handle().dispatchEvent(pointer('pointerdown', ey));
  handle().dispatchEvent(pointer('pointermove', ey + 60));
  await until('the route point under the pointer during the drag', RUN_MS, () => Math.abs(center()[1] - (ey + 60)) < 2);
  handle().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
  await until('the route point back after Esc', RUN_MS, () => Math.abs(center()[1] - ey) < 2 && !handle().classList.contains('dragging'));
  handle().dispatchEvent(pointer('pointerup', ey + 60));
  handle().dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
  await until('the route point below after the arrow key', RUN_MS, () => center()[1] > ey + 2);
  v = await step('undo the arrow key on the route point', () => api.undo());
  if (JSON.stringify(v.graph.wire?.[0]?.points) !== JSON.stringify(moved)) throw new Error(`Esc or the arrow key did not give one edit: ${JSON.stringify(v.graph.wire)}`);
  steps.push('route point drag stopped with Esc');
  handle().focus();
  handle().dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true, cancelable: true }));
  await until('no route point', RUN_MS, () => document.querySelectorAll('.route-point').length === 0);
  v = await step('undo the removal', () => api.undo());
  if (v.graph.wire?.length !== 1 || v.graph.wire[0].points?.length !== 1) throw new Error('Delete on a route point changed more than the point');
  // The keyboard adds a route point too: + on the focused wire adds one at its middle.
  await until('the route point after the undo', RUN_MS, () => document.querySelectorAll('.route-point').length === 1);
  const edge = document.querySelector<SVGGElement>('.react-flow__edge-wire')!;
  edge.focus();
  edge.dispatchEvent(new KeyboardEvent('keydown', { key: '+', bubbles: true, cancelable: true }));
  await until('a second route point from the keyboard', RUN_MS, () => document.querySelectorAll('.route-point').length === 2);
  v = await step('undo the route point of the keyboard', () => api.undo());
  if (v.graph.wire?.[0]?.points?.length !== 1) throw new Error('undo did not remove the route point of the keyboard');
  steps.push('route point added from the keyboard');
  return v;
}

/** The look of a wire: the Route field, Reset route, the graph default route and Lock bends. */
async function checkLook({ step, until, steps }: CheckContext, v: View): Promise<View> {
  // The look of the wire (15.9): the Route field of the Inspector sets the style, and Reset route
  // removes the route points. Both are edits with undo.
  const rounded = () => (document.querySelector('.wire-hit')?.getAttribute('d')?.match(/Q/g) ?? []).length;
  await until('the Route field and the corners of the automatic route', RUN_MS, () => !!field('Route') && rounded() >= 2);
  field('Route')!.value = 'curved';
  field('Route')!.dispatchEvent(new Event('change', { bubbles: true }));
  await until('the curved wire', RUN_MS, () => rounded() === 0 && !!document.querySelector('.wire-hit')?.getAttribute('d')?.includes('C'));
  [...document.querySelectorAll<HTMLButtonElement>('.panel button')].find((b) => b.textContent === 'Reset route')!.click();
  await until('no route point after Reset route', RUN_MS, () => document.querySelectorAll('.route-point').length === 0);
  v = await step('undo Reset route', () => api.undo());
  if (v.graph.wire?.[0]?.style !== 'curved' || v.graph.wire[0].points?.length !== 1) throw new Error(`the route style or Reset route failed: ${JSON.stringify(v.graph.wire)}`);
  v = await step('undo the route style', () => api.undo());
  if (v.graph.wire?.[0]?.style !== undefined) throw new Error('undo did not give the graph default route');
  // The graph default route draws each wire with no style, and the wire field names it.
  v = await step('the graph default route', () => api.edit({ kind: 'SetGraphLook', style: 'curved' }));
  if (v.graph.style !== 'curved') throw new Error('SetGraphLook did not set the graph default route');
  await until('the curved wire of the graph default', RUN_MS, () => rounded() === 0 && field('Route')?.options[0]?.textContent === 'Default (Curved)');
  v = await step('undo the graph default route', () => api.undo());
  if (v.graph.style !== undefined) throw new Error('undo did not give the automatic route');
  // Lock bends saves the bends of the automatic route as route points, with undo.
  const lock = () => [...document.querySelectorAll<HTMLButtonElement>('.panel button')].find((b) => b.textContent === 'Lock bends');
  await until('Lock bends on for the automatic wire', RUN_MS, () => document.querySelectorAll('.route-point').length === 1 && lock()?.disabled === false);
  lock()!.click();
  const handles = () => document.querySelectorAll('.route-point').length;
  await until('the locked bends', RUN_MS, () => handles() >= 2).catch((e: Error) => {
    throw new Error(`${e.message}: ${handles()} route points, ${document.querySelectorAll('.wire-hit').length} wires, path ${document.querySelector('.wire-hit')?.getAttribute('d')}`);
  });
  v = await step('undo Lock bends', () => api.undo());
  if (v.graph.wire?.[0]?.points?.length !== 1) throw new Error(`undo did not remove the locked bends: ${JSON.stringify(v.graph.wire)}`);
  steps.push('bends locked');
  return v;
}

/** The segment bars of an orthogonal wire, and the conflict mark of a blocked route. */
async function checkSegments({ step, until, steps }: CheckContext, v: View): Promise<View> {
  // An orthogonal wire shows a bar on each middle segment. A drag on the bar moves the segment
  // sideways, and an arrow key moves it by 10 px. Both are edits with undo.
  const before = JSON.stringify(v.graph.wire?.[0]?.points);
  await until('the Route field for the orthogonal wire', RUN_MS, () => !!field('Route'));
  field('Route')!.value = 'orthogonal';
  field('Route')!.dispatchEvent(new Event('change', { bubbles: true }));
  const bar = () => document.querySelector<HTMLElement>('.segment-handle')!;
  await until('a segment bar of the orthogonal wire', RUN_MS, () => !!document.querySelector('.segment-handle'));
  const across = bar().classList.contains('vertical') ? 0 : 1;
  const [sx0, sy0] = (({ left, top, width, height }) => [left + width / 2, top + height / 2])(bar().getBoundingClientRect());
  const to = across === 0 ? [sx0 + 40, sy0] : [sx0, sy0 + 40];
  for (const [type, [x, y]] of [['pointerdown', [sx0, sy0]], ['pointermove', to], ['pointerup', to]] as const) {
    bar().dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: x, clientY: y }));
  }
  const barAt = () => (({ left, top, width, height }) => [left + width / 2, top + height / 2])(bar().getBoundingClientRect())[across];
  await until('the segment bar under the pointer after the edit', RUN_MS, () => Math.abs(barAt() - to[across]) < 2);
  v = await step('undo the segment move', () => api.undo());
  if (JSON.stringify(v.graph.wire?.[0]?.points) !== before) throw new Error('undo did not give the route before the segment move');
  await until('the segment bar after the undo', RUN_MS, () => Math.abs(barAt() - [sx0, sy0][across]) < 2);
  bar().focus();
  bar().dispatchEvent(new KeyboardEvent('keydown', { key: across === 0 ? 'ArrowRight' : 'ArrowDown', bubbles: true, cancelable: true }));
  await until('the segment bar after the arrow key', RUN_MS, () => barAt() > [sx0, sy0][across] + 2);
  // Esc during a drag puts the segment back and sends no edit, so the next undo takes back the arrow key.
  const from = (({ left, top, width, height }) => [left + width / 2, top + height / 2])(bar().getBoundingClientRect());
  const away = across === 0 ? [from[0] + 40, from[1]] : [from[0], from[1] + 40];
  for (const [type, [x, y]] of [['pointerdown', from], ['pointermove', away]] as const) {
    bar().dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: x, clientY: y }));
  }
  await until('the segment bar under the pointer during the drag', RUN_MS, () => Math.abs(barAt() - away[across]) < 2);
  bar().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
  await until('the segment bar back after Esc', RUN_MS, () => Math.abs(barAt() - from[across]) < 2);
  bar().dispatchEvent(new PointerEvent('pointerup', { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: away[0], clientY: away[1] }));
  v = await step('undo the arrow key on the segment', () => api.undo());
  v = await step('undo the orthogonal style', () => api.undo());
  if (v.graph.wire?.[0]?.style !== undefined || JSON.stringify(v.graph.wire?.[0]?.points) !== before) throw new Error('undo did not give the wire before the segment moves');
  steps.push('orthogonal segment moved sideways');
  // A route point in a node body gives the automatic route no clear path. The wire shows a
  // conflict mark (15.9), and the mark goes after the undo.
  const [nx, ny] = v.graph.node!.find((n) => n.id === B)!.pos!;
  v = await step('a route point in a node body', () => api.edit({ kind: 'SetRoute', to: `${B}.value`, points: [[nx + 40, ny + 40]] }));
  await until('the conflict mark of the wire', RUN_MS, () => document.querySelector('.route-conflict') !== null);
  v = await step('undo the route point in the body', () => api.undo());
  await until('no conflict mark after the undo', RUN_MS, () => document.querySelector('.route-conflict') === null);
  steps.push('routing conflict marked');
  return v;
}

/** A delay: the kind and domain icons, a moved wire end, a dashed wire and a junction dot. */
async function checkDelay({ step, until, steps }: CheckContext, v: View): Promise<View> {
  // A delay is in the frame domain (9.1). Each node shows its kind icon and its domain icon (15.3).
  const D = 'd000000000000004';
  v = await step('add a delay', () => api.edit({ kind: 'AddNode', id: D, folder: 'delay', builtin: true, pos: [300, 300] }));
  await until('the kind icons', RUN_MS, () => icon(A, 'kind-icon.function') && icon(D, 'kind-icon.stateful'));
  // No wire gives the type of the delay (4.5), so it is unresolved, with gray pins (15.2).
  const unresolved = () => document.querySelector(`.react-flow__node[data-id="${D}"] .patina-node.unresolved`) !== null;
  await until('the unresolved delay', RUN_MS, unresolved);
  // A drag on the end of the wire moves it to the delay (15.9). During the drag, the sockets that
  // fit get an accent ring and the order sockets dim. The batch value to the delay goes on a dashed
  // wire (15.3). One undo gives the wire to B again.
  const mid = (el: Element, b = el.getBoundingClientRect()) => ({ clientX: b.left + b.width / 2, clientY: b.top + b.height / 2 });
  const end = document.querySelector(`.react-flow__edge[data-id="${A}.out>${B}.value"] .react-flow__edgeupdater-target`)!;
  const socket = document.querySelector(`.react-flow__node[data-id="${D}"] .react-flow__handle.target[data-handleid="value"]`)!;
  end.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, ...mid(end) }));
  const order = document.querySelector(`.react-flow__node[data-id="${D}"] .react-flow__handle.target.order-pin`)!;
  const look = (el: Element) => getComputedStyle(el);
  document.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, ...mid(socket) }));
  await until('a ring on the fit socket and a dim order socket', RUN_MS, () => look(socket).boxShadow.includes('223, 166, 75') && look(socket).opacity === '1' && look(order).opacity === '0.3');
  socket.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, ...mid(socket) }));
  await until('no dim socket after the drop', RUN_MS, () => look(order).opacity === '1');
  await until('the wire moved to the delay', RUN_MS, () => !!document.querySelector(`.react-flow__edge[data-id="${A}.out>${D}.value"]`) && !document.querySelector(`.react-flow__edge[data-id="${A}.out>${B}.value"]`));
  await until('the domain icons', RUN_MS, () => icon(A, 'domain-icon.batch') && icon(D, 'domain-icon.frame'));
  await until('the delay resolved by its wire', RUN_MS, () => !unresolved());
  await until('a dashed cross-domain wire', RUN_MS, () => document.querySelectorAll('.wire-line.cross').length === 1);
  // The delay is generic (4.5), and the wire from A gives it f64. The live graph runs A through the
  // delay to B, one frame late, so both wires show 2.5.
  v = await step('wire the delay to B', () => api.edit({ kind: 'Connect', from: `${D}.out`, to: `${B}.value` }));
  await until('the value of A through the generic delay', BUILD_MS, () => values().join() === '2.5,2.5');
  v = await step('undo the wire from the delay', () => api.undo());
  v = await step('undo the move of the wire end', () => api.undo());
  if (v.graph.wire?.length !== 1 || v.graph.wire[0].to !== `${B}.value`) throw new Error(`undo did not give the wire to B: ${JSON.stringify(v.graph.wire)}`);
  await until('no dashed wire after the undo', RUN_MS, () => document.querySelector('.wire-line.cross') === null);
  // A second wire from the output shares the trunk, and a junction dot marks where the two part (15.9).
  // The route point of the first wire can make the two wires part at a second place, so remove it first.
  v = await step('remove the route point of the first wire', () => api.edit({ kind: 'SetRoute', to: `${B}.value`, points: [] }));
  v = await step('a second wire from A', () => api.edit({ kind: 'Connect', from: `${A}.out`, to: `${D}.value` }));
  let joints = 0;
  await until('one junction dot', RUN_MS, () => (joints = document.querySelectorAll('.wire-joint').length) === 1).catch((e: Error) => {
    throw new Error(`${e.message} (${joints} dots, wires ${JSON.stringify(v.graph.wire)})`);
  });
  v = await step('undo the second wire', () => api.undo());
  await until('no junction dot after the undo', RUN_MS, () => document.querySelector('.wire-joint') === null);
  v = await step('undo the removal of the route point', () => api.undo());
  v = await step('undo the delay', () => api.undo());
  steps.push('domain icons, a wire end moved to a ringed socket, a dashed cross-domain wire and a junction dot');
  return v;
}

/** The switch of a bool input: in the node, in the Inspector and in the Test mode of a subgraph. */
async function checkSwitch({ step, until, steps }: CheckContext, v: View): Promise<View> {
  // An unwired bool input is a switch in the node row (15.3) and a check box in the Inspector (15.7).
  // A click on the switch also selects the node. Each click is an edit, and the live graph runs again.
  v = await step('make a node with a switch', () => api.edit({ kind: 'SetCode', path: 'nodes/number/src/lib.rs', text: SWITCH_NODE }));
  const pinSwitch = () => document.querySelector<HTMLInputElement>(`.react-flow__node[data-id="${A}"] input.pin-switch`);
  v = await step('run the switch node', () => api.run());
  await until('the switch of the bool input and the value of the build', BUILD_MS, () => pinSwitch()?.disabled === false && values().join() === '2.5');
  pinSwitch()!.click();
  await until('the double value after the switch', RUN_MS, () => values().join() === '5.0').catch((e: Error) => {
    throw new Error(`${e.message} (values ${values().join()}, switch ${pinSwitch()?.checked})`);
  });
  // An earlier step can leave the Inspector on its Code tab.
  const inspector = 'aside[aria-label="Inspector"]';
  [...document.querySelectorAll<HTMLElement>(`${inspector} [role=tab]`)].find((t) => t.textContent === 'Parameters')?.click();
  const box = () =>
    [...document.querySelectorAll(`${inspector} .param`)]
      .find((p) => p.querySelector('.label')?.textContent === 'double')
      ?.querySelector<HTMLInputElement>('input[type="checkbox"]');
  await until('the set check box in the Inspector', RUN_MS, () => box()?.checked === true);
  box()!.click();
  await until('the value after the Inspector check box', BUILD_MS, () => values().join() === '2.5' && pinSwitch()?.checked === false);
  v = await step('undo the Inspector check box', () => api.undo());
  v = await step('undo the switch', () => api.undo());
  if (v.graph.node?.find((n) => n.id === A)?.params?.double !== undefined) throw new Error('undo did not remove the switch value');
  // The Test mode runs a subgraph alone. A click on the switch of a bool public input sets it (15.9).
  const use = 'f12e000000000002';
  v = await step('move the switch node into a subgraph', () => api.collapse('switch_check', [A], use));
  const pathButton = (text: string) => [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === text);
  await until('the use of the switch subgraph', RUN_MS, () => !!document.querySelector(`.react-flow__node[data-id="${use}"] .open-subgraph`));
  document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${use}"] .open-subgraph`)!.click();
  await until('the switch subgraph in the path bar', BUILD_MS, () => document.querySelector('.graph-path [aria-current="page"]')?.textContent === '/ switch_check');
  v = await step('add a bool input to the subgraph', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddPin', key: '0000000d', name: 'double', input: true, pos: [-300, 0] },
        { kind: 'ConnectPin', key: '0000000d', end: `${A}.double` },
      ],
    }),
  );
  const portSwitch = () => document.querySelector<HTMLButtonElement>('.port-switch[aria-label="Switch double"]');
  const portValue = () => [...document.querySelectorAll('.patina-node.port .port-value')].map((p) => p.textContent).join();
  pathButton('Test')!.click();
  await until('the test run with the input off', BUILD_MS, () => portSwitch()?.getAttribute('aria-pressed') === 'false' && portValue() === '2.5').catch((e: Error) => {
    throw new Error(`${e.message} (ports ${portValue()}, switch ${portSwitch()?.getAttribute('aria-pressed')})`);
  });
  portSwitch()!.click();
  await until('the test run with the input on', BUILD_MS, () => portSwitch()?.getAttribute('aria-pressed') === 'true' && portValue() === '5.0');
  pathButton('End test')!.click();
  await until('the end of the test', RUN_MS, () => !!pathButton('Test') && !portSwitch());
  v = await step('undo the bool input of the subgraph', () => api.undo());
  pathButton('main')!.click();
  await until('the main graph after the test', BUILD_MS, () => document.querySelector('.graph-path') === null);
  v = await step('undo the switch subgraph', () => api.undo());
  if (!v.graph.node?.some((n) => n.id === A)) throw new Error('undo did not put the switch node back');
  await until('the main run after the test', BUILD_MS, () => values().join() === '2.5').catch((e: Error) => {
    throw new Error(`${e.message} (values ${values().join()}, problems ${v.problems.join(' | ')})`);
  });
  steps.push('a subgraph run alone in the Test mode, with a click on the switch of its bool input');
  v = await step('undo the switch node', undoCode);
  steps.push('a bool input set with the switch in the node and the Inspector check box');
  return v;
}
