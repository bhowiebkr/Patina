// The viewer step of the self-check (check.ts): a type crate with a `Viewer` that gives a text, a
// thumbnail and a waveform, on the node body and in the Inspector of a selected wire (5.3, 15.7).

import { api, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { openPanel } from '../panels/layout';
import { BUILD_MS, RUN_MS, entry, nodeToml, rightClick, type CheckContext } from './check-fixtures';

const TYPE_CODE = `use std::fmt::{self, Write};

/// Samples that the self-check shows.
pub struct Samples(pub Vec<f32>);

impl patina_sdk::Viewer for Samples {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        write!(out, "{} samples", self.0.len())
    }

    fn thumbnail(&self, out: &mut patina_sdk::Thumbnail<'_>) {
        for pixel in out.pixels(8, 8).chunks_mut(4) {
            pixel.copy_from_slice(&[200, 120, 40, 255]);
        }
    }

    fn show(&self, ui: &mut patina_sdk::Ui) {
        ui.waveform("Samples", &self.0, None);
    }
}
`;

const SAMPLER_NODE = `/// Gives four samples.
#[patina_sdk::node]
pub fn sampler() -> samples::Samples {
    samples::Samples(vec![0.0, 0.5, -0.5, 1.0])
}
`;

const COUNT_NODE = `/// Prints the number of samples.
#[patina_sdk::node]
pub fn count(values: &samples::Samples) {
    println!("count {}", values.0.len());
}
`;

const S = 'f4ee000000000003';
const C = 'f4ee000000000004';
const withSamples = (name: string) => `${nodeToml(name)}samples = { path = "../../types/samples" }\n`;

/** Adds a sampler and a count, selects their wire, reads the Viewer in the Inspector and removes them. */
export async function checkViewer({ step, until, result, mark, steps }: CheckContext, v: View) {
  await api.checkWrite('types/samples/Cargo.toml', nodeToml('samples'));
  await api.checkWrite('types/samples/src/lib.rs', TYPE_CODE);
  await api.checkWrite('nodes/sampler/Cargo.toml', withSamples('sampler'));
  await api.checkWrite('nodes/sampler/src/lib.rs', SAMPLER_NODE);
  await api.checkWrite('nodes/count/Cargo.toml', withSamples('count'));
  await api.checkWrite('nodes/count/src/lib.rs', COUNT_NODE);
  const item = (name: string) => [...document.querySelectorAll('[aria-label=Project] .item')].find((b) => b.textContent?.startsWith(name));
  await until('the sampler and count node folders', BUILD_MS, () => !!item('sampler') && !!item('count'));
  v = await step('add a sampler and a count', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddNode', id: S, folder: 'nodes/sampler', pos: [0, 640] },
        { kind: 'AddNode', id: C, folder: 'nodes/count', pos: [320, 640] },
        { kind: 'Connect', from: `${S}.out`, to: `${C}.values` },
      ],
    }),
  );
  const from = mark();
  v = await step('run the sampler', () => api.run());
  await result(v, from, 'count 4', BUILD_MS);
  rightClick(document.querySelector('.react-flow__pane')!);
  await until('the canvas menu for the sampler', RUN_MS, () => !!entry('Canvas', 'Fit view'));
  entry('Canvas', 'Fit view')!.click();
  // The node body shows the thumbnail under the pins (5.3).
  await until('the thumbnail on the sampler', RUN_MS, () => !!document.querySelector(`.react-flow__node[data-id="${S}"] canvas.thumb`));

  // A click on the middle of the wire selects it, and the Inspector shows its value.
  changeLayout((l) => openPanel(l, 'inspector'));
  const edge = `.react-flow__edge[data-id="${S}.out>${C}.values"]`;
  await until('the wire of the samples', RUN_MS, () => !!document.querySelector(`${edge} .wire-hit`));
  const hit = document.querySelector<SVGPathElement>(`${edge} .wire-hit`)!;
  const p = hit.getPointAtLength(hit.getTotalLength() / 2);
  const m = hit.getScreenCTM()!;
  hit.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: m.a * p.x + m.c * p.y + m.e, clientY: m.b * p.x + m.d * p.y + m.f }));
  await until('the selected wire of the samples', RUN_MS, () => !!document.querySelector(`${edge}.selected`));
  const inspector = 'aside[aria-label="Inspector"]';
  const shown = () => ({
    text: document.querySelector(`${inspector} .value-text`)?.textContent,
    wave: !!document.querySelector(`${inspector} svg[aria-label="Samples"] polygon.wave`),
    thumb: !!document.querySelector(`${inspector} canvas.thumb[aria-label="Value: 8 x 8 pixels"]`),
  });
  await until('the text, the waveform and the thumbnail in the Inspector', RUN_MS, () => {
    const s = shown();
    return s.text === '4 samples' && s.wave && s.thumb;
  }).catch((e: Error) => {
    throw new Error(`${e.message} (${JSON.stringify(shown())})`);
  });
  steps.push('a Viewer shows its text, thumbnail and widgets in the Inspector');

  v = await step('remove the sampler and the count', () => api.undo());
  if (v.graph.node?.some((n) => n.id === S || n.id === C)) throw new Error('the undo did not remove the sampler and the count');
  for (const folder of ['nodes/sampler', 'nodes/count', 'types/samples']) await api.checkWrite(folder, null);
  await until('no sampler and count node folders', BUILD_MS, () => !item('sampler') && !item('count'));
  return v;
}
