// The bus step of the self-check (check.ts): a wire of the kind that New connection made
// (check-palette.ts), with its lanes before and after a run, the Lanes row of the Inspector, the
// lane tooltip and the one line below the far zoom (6.3, 15.9).

import { api, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { openPanel } from '../panels/layout';
import { FAR_ZOOM } from '../wires/gl';
import { BUILD_MS, RUN_MS, anyPixel, entry, mouseWheel, nodeToml, rightClick, scale, wireColumn, type CheckContext } from './check-fixtures';

const WORD_NODE = `/// Gives a word for the bus check.
#[patina_sdk::node]
pub fn word16() -> u16 {
    0xA5C3
}
`;

const TAKE_NODE = `/// Prints the word.
#[patina_sdk::node]
pub fn take16(value: u16) {
    println!("take {value}");
}
`;

const W = 'f4ee000000000005';
const K = 'f4ee000000000006';
const BITS = '1010010111000011';

/** Wires a word to the 16 lanes of the made-bus kind, reads them on the canvas and in the Inspector, and removes the nodes. */
export async function checkBus({ step, until, result, mark, steps }: CheckContext, v: View) {
  await api.checkWrite('nodes/word16/Cargo.toml', nodeToml('word16'));
  await api.checkWrite('nodes/word16/src/lib.rs', WORD_NODE);
  await api.checkWrite('nodes/take16/Cargo.toml', nodeToml('take16'));
  await api.checkWrite('nodes/take16/src/lib.rs', TAKE_NODE);
  const item = (name: string) => [...document.querySelectorAll('[aria-label=Project] .item')].find((b) => b.textContent?.startsWith(name));
  await until('the word16 and take16 node folders', BUILD_MS, () => !!item('word16') && !!item('take16'));
  if (!v.kinds.some((k) => k.ty === 'u16' && k.lanes.length === 16)) throw new Error('the made-bus kind of New connection is missing');
  v = await step('add a word16 and a take16', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddNode', id: W, folder: 'nodes/word16', pos: [0, 800] },
        { kind: 'AddNode', id: K, folder: 'nodes/take16', pos: [320, 800] },
        { kind: 'Connect', from: `${W}.out`, to: `${K}.value` },
      ],
    }),
  );
  const fit = () => {
    rightClick(document.querySelector('.react-flow__pane')!);
    return until('the Fit view entry', RUN_MS, () => !!entry('Canvas', 'Fit view')).then(() => entry('Canvas', 'Fit view')!.click());
  };
  // A new panel can change the size of the canvas, so open the Inspector before the zoom.
  changeLayout((l) => openPanel(l, 'inspector'));
  await fit();
  const edge = `.react-flow__edge[data-id="${W}.out>${K}.value"]`;
  await until('the wire of the word', RUN_MS, () => !!document.querySelector(`${edge} .wire-hit`));
  // The fit waits for the size of the new nodes, so wait for a zoom that stays the same.
  let last = NaN;
  let same = 0;
  await until('a still zoom after Fit view', RUN_MS, () => {
    same = scale() === last ? same + 1 : 0;
    last = scale();
    return same >= 10;
  });
  // The middle of the wire on the screen.
  const mid = () => {
    const hit = document.querySelector<SVGPathElement>(`${edge} .wire-hit`)!;
    const p = hit.getPointAtLength(hit.getTotalLength() / 2);
    const m = hit.getScreenCTM()!;
    return { hit, clientX: m.a * p.x + m.c * p.y + m.e, clientY: m.b * p.x + m.d * p.y + m.f };
  };
  const wheel = async (deltaY: number) => {
    const z = scale();
    const { clientX, clientY } = mid();
    document.querySelector('.react-flow__pane')!.dispatchEvent(mouseWheel(deltaY, clientX, clientY));
    await until('a zoom change at the wire', RUN_MS, () => scale() !== z);
  };
  while (scale() < 0.6) await wheel(-240);

  const { hit, clientX, clientY } = mid();
  hit.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX, clientY }));
  await until('the selected wire of the word', RUN_MS, () => !!document.querySelector(`${edge}.selected`));
  if (scale() < 0.6) throw new Error(`the zoom went back to ${scale()} after the click on the wire`);
  // The label of the selected wire is the input name, the lane range and the width (15.9).
  const label = () => document.querySelector('.wire-type.selected')?.textContent;
  const labels = () => [...document.querySelectorAll('.wire-type')].map((l) => `${l.className}: ${l.textContent}`).join(', ');
  await until('the bus label before a run', RUN_MS, () => label() === 'value[15:0] / 16').catch((e: Error) => {
    throw new Error(`${e.message} (${labels()})`);
  });
  steps.push('a kind with lanes shows its bus label before a run');

  const from = mark();
  v = await step('run the word', () => api.run());
  await result(v, from, 'take 42435', BUILD_MS);
  const lanesRow = () =>
    [...document.querySelectorAll('aside[aria-label="Inspector"] .param')].find((p) => p.querySelector('.label')?.textContent === 'Lanes')?.querySelector('span[title]')?.textContent;
  await until('the Lanes row in the Inspector', RUN_MS, () => lanesRow() === `value[15:0] / 16: ${BITS}`).catch((e: Error) => {
    throw new Error(`${e.message} (${lanesRow()})`);
  });
  steps.push('the Inspector shows the bus name and the live lanes');
  // The Name field of the wire renames the bus label and the Lanes row. React reads its edit at focusout.
  const nameField = () => document.querySelector<HTMLInputElement>('aside[aria-label="Inspector"] input[placeholder="Default"][maxlength="32"]');
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(nameField()!, 'data');
  nameField()!.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
  await until('the bus label of the wire name', RUN_MS, () => label() === 'data[15:0] / 16' && lanesRow() === `data[15:0] / 16: ${BITS}`);
  v = await step('undo the wire name', () => api.undo());
  await until('the bus label of the input name after the undo', RUN_MS, () => label() === 'value[15:0] / 16' && nameField()?.value === '');
  steps.push('the Name field of a wire renames its bus label');

  // The pointer on the middle of the ribbon shows the tooltip of one lane with its live state.
  const at = mid();
  at.hit.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, clientX: at.clientX, clientY: at.clientY }));
  const tip = () => document.querySelector('.lane-tip')?.textContent ?? '';
  await until('the lane tooltip', RUN_MS, () => /^value\[\d+\] = [01]$/.test(tip()));
  const [, bit, state] = /^value\[(\d+)\] = ([01])$/.exec(tip())!;
  if (BITS[15 - Number(bit)] !== state) throw new Error(`the lane tooltip "${tip()}" does not agree with the bits ${BITS}`);
  at.hit.dispatchEvent(new PointerEvent('pointerout', { bubbles: true, relatedTarget: document.querySelector('.react-flow__pane') }));
  await until('no lane tooltip', RUN_MS, () => !document.querySelector('.lane-tip'));
  steps.push('a lane tooltip shows the bit and its live state');

  // Below the far zoom, the bus is one line and its label is the width only.
  while (scale() >= FAR_ZOOM * 0.8) await wheel(240);
  await until('the far bus label', RUN_MS, () => label() === '/ 16').catch((e: Error) => {
    throw new Error(`${e.message} (${labels()})`);
  });
  const far = mid();
  const { column } = await wireColumn({ x: far.clientX, y: far.clientY });
  if (!anyPixel(column, () => true)) throw new Error('the wire layer drew no bus at the far zoom');
  steps.push(`a bus at a zoom of ${scale().toFixed(2)} is one line with the label "/ 16"`);
  await fit();

  v = await step('remove the word16 and the take16', () => api.undo());
  if (v.graph.node?.some((n) => n.id === W || n.id === K)) throw new Error('the undo did not remove the word16 and the take16');
  for (const folder of ['nodes/word16', 'nodes/take16']) await api.checkWrite(folder, null);
  await until('no word16 and take16 node folders', BUILD_MS, () => !item('word16') && !item('take16'));
  return v;
}
