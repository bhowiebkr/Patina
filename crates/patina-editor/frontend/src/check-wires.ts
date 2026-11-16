// The wire steps of the self-check: selection, route points, the look of a wire, the routing
// conflict mark and the cross-domain dash (15.1, 15.3, 15.9).

import { api, type View } from './api';
import { A, B, RUN_MS, amber, anyPixel, gray, icon, wireColumn } from './check-fixtures';

type Step = (name: string, job: () => Promise<View>) => Promise<View>;
type Until = (what: string, ms: number, test: () => boolean) => Promise<void>;

/** Runs the wire steps on the graph of A to B, and returns the last view. */
export async function checkWires(step: Step, until: Until, steps: string[], v: View): Promise<View> {
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
  if ((await api.value(`${A}.out`)) !== '2.5') throw new Error('the engine has no full text of the value');
  steps.push('value in the inspector');

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
  // The look of the wire (15.9): the Route field of the Inspector sets the style, and Reset route
  // removes the route points. Both are edits with undo.
  const field = (name: string) =>
    [...document.querySelectorAll('.param')].find((p) => p.querySelector('.label')?.textContent === name)?.querySelector('select');
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
  // A delay is in the frame domain (9.1). Each node shows its kind icon and its domain icon (15.3).
  const D = 'd000000000000004';
  v = await step('add a delay', () => api.edit({ kind: 'AddNode', id: D, folder: 'delay', builtin: true, pos: [300, 300] }));
  await until('the kind icons', RUN_MS, () => icon(A, 'kind-icon.function') && icon(D, 'kind-icon.stateful'));
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
  await until('a dashed cross-domain wire', RUN_MS, () => document.querySelectorAll('.wire-line.cross').length === 1);
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
