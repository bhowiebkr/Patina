// The GUI writes in a subgraph (4.3, 15.9): a switch drives a NAND gate, and the gate drives an
// LED. A click on the switch writes its parameter, in a subgraph that runs alone, in a subgraph
// open through its use and in the Test mode of that use. The LED must follow each click. Then a
// face node with Rust code draws each use of the subgraph and shows the output of the gate.

import { api, type View } from '../api';
import { bufferOf } from '../code/code';
import { BUILD_MS, RUN_MS, nodeToml, statusText, topButton, type CheckContext } from './check-fixtures';

const FLIP_NODE = `use patina_sdk::{Pointer, Ui, node};

/// A switch: a click on the canvas flips the parameter "on".
#[node(free_layout)]
pub struct Flip {
    on: bool,
}

impl Flip {
    pub fn init() -> Self {
        Flip { on: false }
    }

    pub fn process(&mut self, on: bool) -> bool {
        self.on = on;
        on
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let flip = {
            let mut c = ui.canvas(if self.on { "Switch on" } else { "Switch off" }, [76.0, 32.0]);
            c.rect([0.0, 0.0], [76.0, 32.0], [40, 40, 40], true);
            c.circle([if self.on { 52.0 } else { 24.0 }, 16.0], 10.0, [200, 200, 200], true);
            c.pointer().iter().any(|p| matches!(p, Pointer::Down(_)))
        };
        if flip {
            ui.set_param("on", !self.on);
        }
        ui.pin("on", [0.0, 16.0]);
        ui.pin("out", [76.0, 16.0]);
    }
}
`;

const NAND_NODE = `#[patina_sdk::node]
pub fn notand(a: &bool, b: &bool) -> bool {
    !(*a && *b)
}
`;

const LIGHT_NODE = `use patina_sdk::{Ui, node};

/// An LED that is on when its input is true.
#[node(free_layout)]
pub struct Light {
    on: bool,
}

impl Light {
    pub fn init() -> Self {
        Light { on: false }
    }

    pub fn process(&mut self, on: &bool) {
        self.on = *on;
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut c = ui.canvas(if self.on { "LED on" } else { "LED off" }, [32.0, 32.0]);
            c.circle([16.0, 16.0], 8.0, if self.on { [255, 64, 64] } else { [64, 16, 16] }, true);
        }
        ui.pin("on", [0.0, 16.0]);
    }
}
`;

const FACE_NODE = `use patina_sdk::{Ui, node};

/// The look of each use of gate_check: a lamp that shows the output of the NAND gate.
#[node(free_layout)]
pub struct GateCheckLook {
    on: bool,
}

impl GateCheckLook {
    pub fn init() -> Self {
        GateCheckLook { on: false }
    }

    pub fn process(&mut self, on: &bool) {
        self.on = *on;
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let mut c = ui.canvas(if self.on { "Face on" } else { "Face off" }, [96.0, 48.0]);
        c.rect([0.0, 0.0], [96.0, 48.0], [38, 38, 38], true);
        c.circle([24.0, 24.0], 10.0, if self.on { [255, 200, 64] } else { [64, 48, 16] }, true);
        c.text([44.0, 28.0], [238, 238, 235], if self.on { "on" } else { "off" });
    }
}
`;

const NODES: [string, string][] = [
  ['flip', FLIP_NODE],
  ['notand', NAND_NODE],
  ['light', LIGHT_NODE],
];
const FLIP = 'd000000000000001';
const GATE = 'd000000000000002';
const LIGHT = 'd000000000000003';
const USE = 'd000000000000004';
const FACE_LIB = 'nodes/gate_check_look/src/lib.rs';
const INSPECTOR = 'aside[aria-label="Inspector"]';

const canvas = (label: string) => document.querySelector<SVGSVGElement>(`svg.gui-canvas[aria-label="${label}"]`);
const pathButton = (text: string) => [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === text);
const written = /can not change|Open that subgraph|Open the subgraph/;
const useNode = () => document.querySelector(`.react-flow__node[data-id="${USE}"]`);
const useCanvas = (label: string) => useNode()?.querySelector(`svg.gui-canvas[aria-label="${label}"]`);
const shapeList = () => document.querySelector<HTMLSelectElement>(`${INSPECTOR} select:has(option[value="code"])`);

/** A click on the canvas of the switch, as the mouse gives it: a press and a release at one point. */
function click(svg: SVGSVGElement) {
  const box = svg.getBoundingClientRect();
  const at = { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: box.left + box.width / 2, clientY: box.top + box.height / 2 };
  svg.dispatchEvent(new PointerEvent('pointerdown', at));
  svg.dispatchEvent(new PointerEvent('pointerup', at));
}

/** Two clicks on the switch: the LED goes off, then on again, and the editor shows no error. */
async function flipTwice({ until }: CheckContext, where: string) {
  await until(`the switch off and the LED on ${where}`, BUILD_MS, () => !!canvas('Switch off') && !!canvas('LED on')).catch((e: Error) => {
    throw new Error(`${e.message} (status ${statusText()})`);
  });
  for (const [from, to, led] of [
    ['off', 'on', 'off'],
    ['on', 'off', 'on'],
  ]) {
    click(canvas(`Switch ${from}`)!);
    await until(`the LED ${led} after a click on the switch ${where}`, RUN_MS, () => {
      if (written.test(statusText())) throw new Error(`the switch did not write ${where}: ${statusText()}`);
      return !!canvas(`Switch ${to}`) && !!canvas(`LED ${led}`);
    });
  }
}

async function openUse({ until }: CheckContext, what: string) {
  useNode()!.querySelector<HTMLButtonElement>('.open-subgraph')!.click();
  await until(what, BUILD_MS, () => document.querySelector('.graph-path [aria-current="page"]')?.textContent === '/ gate_check');
}

async function toMain({ until }: CheckContext, what: string) {
  pathButton('main')!.click();
  await until(what, BUILD_MS, () => document.querySelector('.graph-path') === null && !!useNode());
}

async function pickShape({ until }: CheckContext, shape: string) {
  await until('the Shape list of the gate subgraph', RUN_MS, () => !!shapeList());
  shapeList()!.value = shape;
  shapeList()!.dispatchEvent(new Event('change', { bubbles: true }));
}

/** The code shape (4.3): a face node in the subgraph draws each use. Its Rust code is in the Look section. */
async function checkFace(c: CheckContext): Promise<View> {
  const { step, until, steps } = c;
  await step('use the gate subgraph for its face', () => api.edit({ kind: 'AddNode', id: USE, folder: 'graphs/gate_check', pos: [0, 400] }));
  await until('the use for the face', BUILD_MS, () => !!useNode()?.querySelector('.open-subgraph'));
  await openUse(c, 'the gate subgraph for its face');
  await pickShape(c, 'code');
  await until('the face node in the gate subgraph', BUILD_MS, () => document.querySelectorAll('.react-flow__node').length === 4);
  topButton('Fit')!.click();
  // The new face node draws the symbol that the use had, and its code opens in the Look code section.
  await until('the face node and its code', BUILD_MS, () => !!canvas('gate_check') && !!document.querySelector(`${INSPECTOR} .look-code .monaco-editor`) && bufferOf(FACE_LIB) !== undefined);
  const face = canvas('gate_check')!.closest<HTMLElement>('.react-flow__node')!.dataset.id!;
  bufferOf(FACE_LIB)!.setValue(FACE_NODE);
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true }));
  await until('the new code of the face node', BUILD_MS, () => !!canvas('Face off'));
  if ((await api.readCode(FACE_LIB)) !== FACE_NODE) throw new Error('Save did not write the code of the face node');
  await step('wire the NAND gate to the face node', () => api.edit({ kind: 'Connect', from: `${GATE}.out`, to: `${face}.on` }));
  await until('the face of the NAND gate in the subgraph', RUN_MS, () => !!canvas('Face on') && !!canvas('LED on'));
  await toMain(c, 'the main graph with the face');
  await until('the face on the use', BUILD_MS, () => !!useCanvas('Face on') && !useNode()?.querySelector('.patina-node.symbol'));
  // A click on the switch in the subgraph changes the face of the use on the main graph.
  await openUse(c, 'the gate subgraph to switch the face');
  await until('the switch off for the face', BUILD_MS, () => !!canvas('Switch off'));
  click(canvas('Switch off')!);
  await until('the face off after a click on the switch', RUN_MS, () => !!canvas('LED off') && !!canvas('Face off'));
  await toMain(c, 'the main graph with the face off');
  await until('the face off on the use', BUILD_MS, () => !!useCanvas('Face off'));
  steps.push('a face node with Rust code draws each use of a subgraph and shows a value from inside it');
  // Another shape removes the face node. Its folder and its code stay.
  await openUse(c, 'the gate subgraph to remove the face');
  await pickShape(c, 'chip');
  await until('the chip without the face node', RUN_MS, () => shapeList()?.value === 'chip' && !document.querySelector(`.react-flow__node[data-id="${face}"]`));
  if ((await api.readCode(FACE_LIB)) !== FACE_NODE) throw new Error('another shape removed the code of the face node');
  await toMain(c, 'the main graph with the chip');
  await until('the use as a chip', BUILD_MS, () => !!useNode()?.querySelector('.patina-node.symbol'));
  steps.push('another shape removes the face node and keeps its code');
  return step('remove the use of the gate subgraph', () => api.edit({ kind: 'RemoveNode', id: USE }));
}

export async function checkGate(c: CheckContext, v: View): Promise<View> {
  const { step, until, steps } = c;
  for (const [name, code] of NODES) {
    await api.checkWrite(`nodes/${name}/Cargo.toml`, nodeToml(name));
    await api.checkWrite(`nodes/${name}/src/lib.rs`, code);
  }
  v = await step('make a subgraph for the gate', () => api.newGraph('gate_check'));
  v = await step('open the gate subgraph', () => api.openGraph('gate_check'));
  v = await step('add a switch, a NAND gate and an LED to the subgraph', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddNode', id: FLIP, folder: 'nodes/flip', pos: [-300, 0] },
        { kind: 'AddNode', id: GATE, folder: 'nodes/notand', pos: [0, 0] },
        { kind: 'AddNode', id: LIGHT, folder: 'nodes/light', pos: [300, 0] },
        { kind: 'Connect', from: `${FLIP}.out`, to: `${GATE}.a` },
        { kind: 'Connect', from: `${FLIP}.out`, to: `${GATE}.b` },
        { kind: 'Connect', from: `${GATE}.out`, to: `${LIGHT}.on` },
      ],
    }),
  );
  await flipTwice(c, 'in the subgraph that runs alone');
  steps.push('a switch in a subgraph that runs alone sets a NAND gate and an LED');

  v = await step('open the main graph for the gate', () => api.openGraph('main'));
  v = await step('use the gate subgraph', () => api.edit({ kind: 'AddNode', id: USE, folder: 'graphs/gate_check', pos: [0, 400] }));
  const open = () => document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${USE}"] .open-subgraph`);
  await until('the use of the gate subgraph', BUILD_MS, () => document.querySelector('.graph-path') === null && !!open());
  open()!.click();
  await until('the gate subgraph through its use', BUILD_MS, () => document.querySelector('.graph-path [aria-current="page"]')?.textContent === '/ gate_check');
  await flipTwice(c, 'in the subgraph through its use');
  pathButton('Test')!.click();
  await until('the test of the gate subgraph', BUILD_MS, () => !!pathButton('End test'));
  await flipTwice(c, 'in the Test mode of the use');
  pathButton('End test')!.click();
  await until('the end of the gate test', RUN_MS, () => !!pathButton('Test'));
  pathButton('main')!.click();
  await until('the main graph after the gate', BUILD_MS, () => document.querySelector('.graph-path') === null);
  steps.push('a switch in a subgraph use and in its Test mode sets a NAND gate and an LED');

  v = await step('undo the use of the gate subgraph', () => api.undo());
  if (v.graph.node?.some((n) => n.id === USE)) throw new Error('undo did not remove the use of the gate subgraph');
  await checkFace(c);
  // The node folders go first, so the view of the delete does not list them for the next checks.
  for (const name of [...NODES.map(([n]) => n), 'gate_check_look']) await api.checkWrite(`nodes/${name}`, null);
  return step('delete the gate subgraph', () => api.deleteFolder('graphs/gate_check'));
}
