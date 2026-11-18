// The subgraph steps of the self-check (check.ts): a trigger in a subgraph use, the Subgraph
// interface and the frame of the inner nodes (6.4, 15.3, 15.8, 15.9).

import { api, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { openPanel } from '../panels/layout';
import { A, BUILD_MS, EVENT_NODE, RUN_MS, T, fireButton, leftTab, topButton, type CheckContext } from './check-fixtures';

/** Runs the subgraph steps with the code file `lib` of node A. They are last, because the change of graph saves. */
export async function checkSubgraph({ step, until, mark, events, steps, fresh }: CheckContext, v: View, lib: string) {
  let from = 0;
  const ofRun = () => events.slice(from).filter((e) => e.serial === v.serial);
  const eventRuns = () => ofRun().flatMap((e) => (e.state === 'fired' ? [e] : []));
  // Fire in a subgraph use (15.8): Open subgraph opens the subgraph through its use, and Fire
  // sends the flat ID <use ID>/<node ID>. This step is last, because the change of graph saves.
  const use = 'f12e000000000001';
  v = await step('make an event node for the subgraph', () => api.edit({ kind: 'SetCode', path: lib, text: EVENT_NODE }));
  v = await step('add a trigger for the subgraph', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddNode', id: T, folder: 'trigger', builtin: true, pos: [-300, 0] },
        { kind: 'Connect', from: `${T}.out`, to: `${A}.text` },
      ],
    }),
  );
  v = await step('move the trigger into a subgraph', () => api.collapse('fire_check', [T], use));
  from = mark();
  v = await step('run the trigger in the subgraph', () => api.run());
  await until('a run with the trigger in the subgraph', BUILD_MS, () => {
    const failed = ofRun().find((e) => e.state === 'failed' || (e.state === 'ran' && !e.ok));
    if (failed) throw new Error(`the run failed: ${JSON.stringify(failed)}`);
    return ofRun().some((e) => e.state === 'ran' && e.events);
  });
  await until('the use of the subgraph', RUN_MS, () => !!document.querySelector(`.react-flow__node[data-id="${use}"]`));
  document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${use}"] .open-subgraph`)!.click();
  await until('the use in the path bar', BUILD_MS, () => document.querySelector('.graph-path [aria-current="page"]')?.textContent === '/ fire_check');
  await until('the fire button in the subgraph', BUILD_MS, () => fireButton()?.disabled === false);
  fireButton()!.click();
  await until('the event run of the trigger in the use', RUN_MS, () => eventRuns().some((e) => e.trigger === `${use}/${T}`) && fresh().includes('2.5'));
  topButton('Stop')!.click();
  await until('the end of the event runs in the subgraph', RUN_MS, () => ofRun().some((e) => e.state === 'ended'));
  // The Subgraph interface (mockup 11): the Inspector lists the public pins in pin order (6.4).
  const rows = () => [...document.querySelectorAll<HTMLElement>('aside[aria-label="Inspector"] .pin-row')];
  // A view tab can cover the Inspector. The click on Fire selected the trigger, and a click on the
  // pane clears the selection. With selection on drag, the pane takes a pointer down and up.
  changeLayout((l) => openPanel(l, 'inspector'));
  const flow = document.querySelector('.react-flow__pane')!;
  const corner = flow.getBoundingClientRect();
  for (const type of ['pointerdown', 'pointerup']) flow.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: corner.left + 5, clientY: corner.top + 5 }));
  await until('the subgraph interface', RUN_MS, () => rows().length === 1);
  const firstPin = rows()[0].dataset.pin;
  [...document.querySelectorAll<HTMLButtonElement>('aside[aria-label="Inspector"] .btn')].find((b) => b.textContent === 'Add output')!.click();
  await until('a new public output', BUILD_MS, () => rows().length === 2);
  const added = rows()[1].dataset.pin;
  rows()[1].querySelector<HTMLButtonElement>('button[title="Move up"]')!.click();
  await until('the new output first in the pin order', BUILD_MS, () => rows()[0].dataset.pin === added);
  const field = rows()[0].querySelector('input')!;
  field.value = 'sum';
  field.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
  await until('the new name in the preview', BUILD_MS, () => document.querySelector('.pin-preview-body > :last-child > :first-child')?.textContent === 'sum');
  for (const what of ['name', 'order', 'output']) v = await step(`undo the ${what} of the public output`, () => api.undo());
  await until('the old public pins', BUILD_MS, () => rows().length === 1 && rows()[0].dataset.pin === firstPin);
  steps.push('subgraph interface');
  // A removed public pin keeps the wire of the use. The use shows a lost pin for it, so the user can delete the wire (15.9).
  rows()[0].querySelector<HTMLButtonElement>('button[title="Remove"]')!.click();
  await until('no public output', BUILD_MS, () => rows().length === 0);
  [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === 'main')!.click();
  await until('the main graph from the path bar', BUILD_MS, () => document.querySelector('.graph-path') === null);
  const lostPin = `.react-flow__node[data-id="${use}"] .react-flow__handle.lost-pin[data-handleid="${firstPin}"]`;
  await until('the lost pin of the use and its wire', BUILD_MS, () => !!document.querySelector(lostPin) && !!document.querySelector(`.react-flow__edge[data-id="${use}.${firstPin}>${A}.text"]`));
  // The only use of a subgraph gives its values when it opens from the Project panel (15.9).
  leftTab('Project').click();
  [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')].find((b) => b.textContent === 'fire_check')!.click();
  await until('the subgraph from the Project panel', BUILD_MS, () => document.querySelector('.graph-path') !== null);
  v = await step('undo the removed public output', () => api.undo());
  if (v.graph.output?.[0]?.key !== firstPin) throw new Error(`the undo did not give back the public output ${firstPin}`);
  await until('the domain of the only use', BUILD_MS, () => !!document.querySelector(`.react-flow__node[data-id="${T}"] .domain-icon`));
  [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === 'main')!.click();
  await until('the main graph again', BUILD_MS, () => document.querySelector('.graph-path') === null);
  steps.push('open a subgraph use and fire its trigger');

  // A node folder that comes or goes from outside the editor shows in the Project panel (15.10).
  const outside = () => [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')].find((b) => b.textContent?.startsWith('outside'));
  await api.checkWrite('nodes/outside/src/lib.rs', '#[patina_sdk::node]\npub fn outside(a: f64) -> f64 {\n    a\n}\n');
  await api.checkWrite('nodes/outside/Cargo.toml', '[package]\nname = "outside"\nversion = "0.1.0"\nedition = "2024"\n\n[dependencies]\npatina-sdk = { workspace = true }\n');
  leftTab('Project').click();
  await until('the node from outside the editor', BUILD_MS, () => !!outside());
  await api.checkWrite('nodes/outside', null);
  await until('no node from outside the editor', BUILD_MS, () => !outside());
  steps.push('a node folder from outside the editor');

  // Show inner nodes puts the nodes of the use in place, in a frame, with the wire to the frame pin (15.3).
  const frameButton = () => document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${use}"] .frame-toggle`);
  const innerNode = () => document.querySelector(`.react-flow__node[data-id="${use}/${T}"]`);
  await until('the frame button of the use', BUILD_MS, () => frameButton()?.getAttribute('aria-label') === 'Show inner nodes');
  // The open frame moves the other nodes aside, so it covers none of them. They come back when it closes.
  const others = () => [...document.querySelectorAll<HTMLElement>('.react-flow__node')].filter((n) => n.dataset.id !== use && !n.dataset.id?.startsWith(`${use}/`));
  const places = () => others().map((n) => `${n.dataset.id} ${n.style.transform}`).join('\n');
  const covered = () => {
    const f = document.querySelector(`.react-flow__node[data-id="${use}"]`)!.getBoundingClientRect();
    return others().some((n) => {
      const r = n.getBoundingClientRect();
      return r.left < f.right - 1 && r.right > f.left + 1 && r.top < f.bottom - 1 && r.bottom > f.top + 1;
    });
  };
  const before = places();
  frameButton()!.click();
  await until('the inner node and its wire to the frame pin', BUILD_MS, () => !!innerNode() && !!document.querySelector(`.react-flow__edge[data-id^="${use}/${T}.out>${use}.out:"]`));
  await until('an open frame that covers no node', BUILD_MS, () => !covered());
  if (innerNode()!.classList.contains('draggable') || innerNode()!.querySelector('.frame-toggle, .open-subgraph')) throw new Error('an inner node has edits');
  frameButton()!.click();
  await until('no inner nodes', RUN_MS, () => !innerNode() && frameButton()?.getAttribute('aria-expanded') === 'false');
  await until('the nodes in their places again', BUILD_MS, () => places() === before);
  steps.push('show the inner nodes of a use in a frame');
}
