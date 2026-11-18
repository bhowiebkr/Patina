// The widget folder step of the self-check (check.ts): a node shows the start widget of the
// made-widget folder that the Palette step made, and a drag on the widget sets its value (15.4).
// Add crate gives the node the widget folder and the `log` crate. The Logs tabs show its log line (15.7).

import { api, type View } from '../api';
import { bufferOf, unsavedFiles } from '../code/code';
import { BUILD_MS, RUN_MS, enterText, entry, leftTab, nodeToml, rightClick, type CheckContext } from './check-fixtures';

/** A node that keeps the start widget of made-widget in its state and shows it. */
const SLIDE_NODE = `use made_widget::MadeWidget;
use patina_sdk::{Ui, node};

/// Shows the widget of the made-widget folder.
#[node]
pub struct Slide {
    bar: MadeWidget,
}

impl Slide {
    pub fn init() -> Self {
        Slide { bar: MadeWidget { value: 0.0 } }
    }

    pub fn process(&mut self) -> f64 {
        log::info!("slide {}", self.bar.value);
        f64::from(self.bar.value)
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.add(&mut self.bar);
    }
}
`;

const S = 'f4ee000000000002';

/** Builds a node that shows a widget folder, drags on the widget, and removes the node again. */
export async function checkWidget({ step, until, steps }: CheckContext, v: View) {
  await api.checkWrite('nodes/slide/Cargo.toml', nodeToml('slide'));
  await api.checkWrite('nodes/slide/src/lib.rs', SLIDE_NODE);
  const slideItem = () => [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')].find((b) => b.textContent?.startsWith('slide'));
  leftTab('Project').click();
  await until('the slide node folder', BUILD_MS, () => !!slideItem());
  // Add crate (15.4) adds a widget folder as a path and a crate of crates.io, and Save of the Code
  // tab writes Cargo.toml. The local check runs offline with the log crate in the cargo cache.
  const toml = 'nodes/slide/Cargo.toml';
  const codeButton = (text: string) => [...document.querySelectorAll<HTMLButtonElement>('.code-bar .btn')].find((b) => b.textContent === text);
  const crateField = () => document.querySelector<HTMLInputElement>('[aria-label="Crate, widget or type name"]');
  slideItem()!.click();
  for (const [spec, added] of [['made-widget', /^made-widget = .*widgets[\\/]+made-widget/m], ['log', /^log = "0\.4/m]] as const) {
    await until(`Add crate in the Code tab of slide for ${spec}`, RUN_MS, () => !!codeButton('Add crate') && !crateField());
    codeButton('Add crate')!.click();
    await until('the crate field', RUN_MS, () => !!crateField());
    enterText(crateField()!, spec);
    await until(`${spec} in the Cargo.toml of slide`, BUILD_MS, () => added.test(bufferOf(toml)?.getValue() ?? ''));
    codeButton('Save')!.click();
    await until(`the saved Cargo.toml with ${spec}`, RUN_MS, () => !unsavedFiles().includes(toml));
  }
  steps.push('Add crate adds a widget folder and a crate of crates.io, and Save writes Cargo.toml');
  v = await step('add a node that shows a widget folder', () => api.edit({ kind: 'AddNode', id: S, folder: 'nodes/slide', pos: [0, 520] }));
  v = await step('run the node with the widget', () => api.run());
  rightClick(document.querySelector('.react-flow__pane')!);
  await until('the canvas menu for the widget node', RUN_MS, () => !!entry('Canvas', 'Fit view'));
  entry('Canvas', 'Fit view')!.click();
  const pad = () => document.querySelector<SVGSVGElement>(`.react-flow__node[data-id="${S}"] svg.gui-canvas`);
  const bar = () => pad()?.querySelector('rect')?.getAttribute('width');
  await until('the widget of the made-widget folder on the node', BUILD_MS, () => bar() === '0').catch((e: Error) => {
    throw new Error(`${e.message} (bar ${bar()}, problems ${v.problems.join('; ')})`);
  });
  // The widget grabs the press, so a press at the middle of the 120 px bar sets the value 0.5.
  const box = pad()!.getBoundingClientRect();
  const at = { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: box.left + box.width / 2, clientY: box.top + box.height / 2 };
  pad()!.dispatchEvent(new PointerEvent('pointerdown', at));
  await new Promise<void>((r) => setTimeout(r));
  pad()!.dispatchEvent(new PointerEvent('pointerup', at));
  await until('the half bar after a press on the widget', RUN_MS, () => Math.abs(Number(bar()) - 60) < 1);
  steps.push('a node shows the start widget of a widget folder and takes a press');
  // A click selects the node, so the Logs tab of the Inspector shows its log line.
  const inspectorTab = (name: string) => [...document.querySelectorAll<HTMLElement>('aside[aria-label="Inspector"] [role=tab]')].find((t) => t.textContent === name);
  document.querySelector<HTMLElement>(`.react-flow__node[data-id="${S}"]`)!.click();
  await until('the Inspector of the widget node', RUN_MS, () => !!inspectorTab('Logs'));
  inspectorTab('Logs')!.click();
  const line = 'INFO slide 0';
  await until('the log line in the Logs tab of the Inspector', RUN_MS, () => !!document.querySelector('aside[aria-label="Inspector"] [role=log]')?.textContent?.includes(line));
  inspectorTab('Parameters')!.click();
  // The Inspector has a Logs tab too, so the step looks only in the tab rows of the dock.
  const dockLogs = [...document.querySelectorAll<HTMLElement>('[role=tablist][aria-label^="The "] [role=tab]')].find((t) => t.textContent === 'Logs')!;
  const shown = dockLogs.parentElement?.querySelector<HTMLElement>('[role=tab][aria-selected=true]');
  dockLogs.click();
  await until('the log line in the Logs panel', RUN_MS, () => !!document.querySelector('[role=log][aria-label=Logs]')?.textContent?.includes(`(${S}): ${line}`));
  shown?.click();
  steps.push('the Logs tab of the Inspector and the Logs panel show a line of the log crate');
  v = await step('remove the node with the widget', () => api.undo());
  if (v.graph.node?.some((n) => n.id === S)) throw new Error('the undo did not remove the node with the widget');
  await api.checkWrite('nodes/slide', null);
  await until('no slide node folder', BUILD_MS, () => !slideItem());
  return v;
}
