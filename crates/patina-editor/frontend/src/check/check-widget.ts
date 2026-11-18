// The widget folder step of the self-check (check.ts): a node shows the start widget of the
// made-widget folder that the Palette step made, and a drag on the widget sets its value (15.4).

import { api, type View } from '../api';
import { BUILD_MS, RUN_MS, entry, leftTab, rightClick, type CheckContext } from './check-fixtures';

const SLIDE_TOML = `[package]\nname = "slide"\nversion = "0.1.0"\nedition = "2024"\n
[dependencies]\npatina-sdk = { workspace = true }\nmade-widget = { path = "../../widgets/made-widget" }\n`;

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
  await api.checkWrite('nodes/slide/Cargo.toml', SLIDE_TOML);
  await api.checkWrite('nodes/slide/src/lib.rs', SLIDE_NODE);
  const slideItem = () => [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')].find((b) => b.textContent?.startsWith('slide'));
  leftTab('Project').click();
  await until('the slide node folder', BUILD_MS, () => !!slideItem());
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
  v = await step('remove the node with the widget', () => api.undo());
  if (v.graph.node?.some((n) => n.id === S)) throw new Error('the undo did not remove the node with the widget');
  await api.checkWrite('nodes/slide', null);
  await until('no slide node folder', BUILD_MS, () => !slideItem());
  return v;
}
