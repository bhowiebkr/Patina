// The free layout step of the self-check (check.ts): a node that draws all of its body on a
// canvas, its pins at the places of the node, a drag on the canvas and its undo (4.3).

import { api, type View } from '../api';
import { BUILD_MS, RUN_MS, entry, leftTab, nodeToml, rightClick, type CheckContext } from './check-fixtures';

/** A knob with a free layout. A drag to the right turns it up by 0.01 for each pixel, and a run prints its value. */
const KNOB_NODE = `use patina_sdk::{Pointer, Ui, node};

/// A knob that the self-check turns.
#[node(free_layout)]
pub struct Knob {
    value: f64,
    drag: Option<(f32, f64)>,
}

impl Knob {
    pub fn init() -> Self {
        Knob { value: 0.0, drag: None }
    }

    pub fn process(&mut self, value: f64) -> f64 {
        self.value = value;
        println!("knob {value}");
        value
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let before = self.value;
        {
            let mut c = ui.canvas(&format!("Knob {:.2}", self.value), [100.0, 60.0]);
            c.grab();
            for p in c.pointer() {
                match (*p, self.drag) {
                    (Pointer::Down([x, _]), _) => self.drag = Some((x, self.value)),
                    (Pointer::Move([x, _]), Some((x0, start))) => self.value = ((start + f64::from(x - x0) / 100.0) * 100.0).round() / 100.0,
                    (Pointer::Up(_), _) => self.drag = None,
                    _ => {}
                }
            }
            c.rect([0.0, 0.0], [100.0, 60.0], [40, 40, 40], true);
        }
        if self.value != before {
            ui.set_param("value", self.value);
        }
        ui.pin("value", [0.0, 20.0]);
        ui.pin("out", [100.0, 40.0]);
    }
}
`;

const K = 'f4ee000000000001';
const tick = () => new Promise<void>((r) => setTimeout(r));

/** Adds a free knob, checks its pins, drags it in two moves and undoes the drag in one step. */
export async function checkFree({ step, until, result, mark, steps }: CheckContext, v: View) {
  await api.checkWrite('nodes/knob/Cargo.toml', nodeToml('knob'));
  await api.checkWrite('nodes/knob/src/lib.rs', KNOB_NODE);
  const knobItem = () => [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')].find((b) => b.textContent?.startsWith('knob'));
  leftTab('Project').click();
  await until('the knob node folder', BUILD_MS, () => !!knobItem());
  v = await step('add a free knob', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddNode', id: K, folder: 'nodes/knob', pos: [0, 320] },
        { kind: 'SetParam', id: K, key: 'value', text: '0.25' },
        // The knob has no input `gone`, so the parameter is lost.
        { kind: 'SetParam', id: K, key: 'gone', text: '1' },
      ],
    }),
  );
  let from = mark();
  v = await step('run the free knob', () => api.run());
  await result(v, from, 'knob 0.25', BUILD_MS);
  rightClick(document.querySelector('.react-flow__pane')!);
  await until('the canvas menu for the knob', RUN_MS, () => !!entry('Canvas', 'Fit view'));
  entry('Canvas', 'Fit view')!.click();

  const node = `.react-flow__node[data-id="${K}"]`;
  const pad = () => document.querySelector<SVGSVGElement>(`${node} .patina-node.free svg.gui-canvas`);
  const label = () => pad()?.getAttribute('aria-label');
  await until('the free knob with its canvas', BUILD_MS, () => label() === 'Knob 0.25');
  const caption = document.querySelector(`${node} .free-caption .title`)?.textContent;
  if (caption !== 'knob') throw new Error(`the caption of the free node is ${caption}`);
  // The doc comment of the node is its tooltip on the canvas and in the Project panel.
  const doc = 'A knob that the self-check turns.';
  const tips = [document.querySelector(`${node} .free-caption`)?.getAttribute('title'), knobItem()?.title.split('\n')[0]];
  if (tips.some((t) => t !== doc)) throw new Error(`the tooltips of the knob are ${JSON.stringify(tips)}`);
  steps.push('the doc comment of a node is its tooltip');
  // A lost parameter does not block the run. Its x deletes it.
  const lostX = () => document.querySelector<HTMLButtonElement>(`${node} [aria-label="Delete the lost parameter gone"]`);
  if (!lostX()) throw new Error('the free knob does not show its lost parameter');
  lostX()!.click();
  await until('no lost parameter on the knob', RUN_MS, () => !lostX());
  // The pins are at the places of `ui.pin`: value at (0, 20) and out at (100, 40) of the 100 x 60 canvas.
  await tick();
  const box = pad()!.getBoundingClientRect();
  const s = box.width / 100;
  const pinAt = (key: string) => {
    const r = document.querySelector(`${node} .react-flow__handle[data-handleid="${key}"]`)!.getBoundingClientRect();
    return [(r.left + r.width / 2 - box.left) / s, (r.top + r.height / 2 - box.top) / s];
  };
  for (const [key, x, y] of [['value', 0, 20], ['out', 100, 40]] as const) {
    const [px, py] = pinAt(key);
    if (Math.abs(px - x) > 12 || Math.abs(py - y) > 3) throw new Error(`the pin ${key} of the free node is at (${px.toFixed(1)}, ${py.toFixed(1)}), not at (${x}, ${y})`);
  }
  steps.push('the pins of a free node are at their places');

  // A drag on a canvas that grabs turns the knob and does not move the node. Each move writes the parameter.
  const place = document.querySelector<HTMLElement>(node)!.style.transform;
  const at = (x: number) => ({ bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: box.left + x * s, clientY: box.top + 30 * s });
  from = mark();
  pad()!.dispatchEvent(new PointerEvent('pointerdown', at(50)));
  await tick();
  pad()!.dispatchEvent(new PointerEvent('pointermove', at(65)));
  await result(v, from, 'knob 0.4', RUN_MS);
  from = mark();
  pad()!.dispatchEvent(new PointerEvent('pointermove', at(80)));
  await tick();
  pad()!.dispatchEvent(new PointerEvent('pointerup', at(80)));
  await result(v, from, 'knob 0.55', RUN_MS);
  await until('the knob label after the drag', RUN_MS, () => label() === 'Knob 0.55');
  if (document.querySelector<HTMLElement>(node)!.style.transform !== place) throw new Error('a drag on a canvas that grabs moved the node');
  // The two writes of the drag are one step of undo (4.3).
  from = mark();
  v = await step('undo the drag of the free knob', () => api.undo());
  const value = v.graph.node?.find((n) => n.id === K)?.params?.value;
  if (value !== '0.25') throw new Error(`one undo after a drag gave the value ${value}, not 0.25`);
  await result(v, from, 'knob 0.25', RUN_MS);
  steps.push('drag a free node and undo the drag in one step');

  v = await step('undo the delete of the lost parameter', () => api.undo());
  if (v.graph.node?.find((n) => n.id === K)?.params?.gone !== '1') throw new Error('the undo did not give back the lost parameter');
  await until('the lost parameter after the undo', RUN_MS, () => !!lostX());
  steps.push('delete a lost parameter and undo it');
  v = await step('remove the free knob', () => api.undo());
  if (v.graph.node?.some((n) => n.id === K)) throw new Error('the undo did not remove the free knob');
  await api.checkWrite('nodes/knob', null);
  await until('no knob node folder', BUILD_MS, () => !knobItem());
  return v;
}
