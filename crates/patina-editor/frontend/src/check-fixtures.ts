// The fixtures of the self-check (check.ts): its node IDs, the code that it writes, and the pixel
// reads of the wire layer.

export const BUILD_MS = 150_000;
export const RUN_MS = 20_000;
export const A = 'a000000000000001';
export const B = 'b000000000000002';
export const T = 'c000000000000003';

/** An event node (9.4): it gives the number plus the length of the text of the event. */
export const EVENT_NODE = `use patina_sdk::node;

#[node(domain = event)]
pub fn number(value: f64, text: &String) -> f64 {
    value + text.len() as f64
}
`;

/** A stateful number node with a GUI (4.2, 4.3): its button adds 1 to the output. It also shows each value widget. */
// A view (15.8) that shows a parameter of node A, and sets it with a button. Its other button
// fires the trigger node T with the text "abcd" (9.4).
export const VIEW_CODE = `use patina_sdk::{Graph, Ui, view};

#[view]
pub struct Panel;

impl Panel {
    pub fn init() -> Self {
        Panel
    }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        ui.label(&format!("value {}", graph.param("${A}", "value").unwrap_or_default()));
        if ui.button("Set").clicked() {
            graph.set_param("${A}", "value", 5.0);
        }
        if ui.button("Fire").clicked() {
            graph.fire("${T}", "abcd");
        }
    }
}
`;

export const GUI_NODE = `use patina_sdk::node;

#[node]
pub struct Number {
    added: f64,
    row: Option<usize>,
    tint: [u8; 3],
    points: Vec<[f32; 2]>,
    gain: f32,
    page: usize,
    more: bool,
    trim: [f32; 2],
    turns: u32,
}

impl Number {
    pub fn init() -> Self {
        Number { added: 0.0, row: None, tint: [223, 166, 75], points: vec![[0.0, 0.2], [0.5, 0.8], [1.0, 0.4]], gain: 1.5, page: 0, more: false, trim: [0.0, 1.0], turns: 0 }
    }

    pub fn process(&mut self, value: f64) -> f64 {
        value + self.added
    }

    pub fn ui(&mut self, ui: &mut patina_sdk::Ui) {
        if ui.button("Add 1").clicked() {
            self.added += 1.0;
        }
        if ui.icon_button("rotate-right", "Rotate right").clicked() {
            self.turns = (self.turns + 1) % 4;
        }
        ui.progress("Turns", self.turns as f32 / 4.0);
        ui.color("Tint", &mut self.tint);
        ui.number_unit("Gain", &mut self.gain, 0.5, "dB");
        ui.tabs("Pages", &mut self.page, &["Values", "Shape"]);
        ui.section("More", &mut self.more, |ui| ui.label("Hidden detail"));
        let wave: Vec<f32> = (0..400).map(|i| (i as f32 * 0.2).sin() * (1.0 - i as f32 / 400.0)).collect();
        ui.plot_axes("Wave plot", &wave[..100], -1.0..=1.0, 0.0..=99.0, ["ms", ""]);
        let dots: Vec<[f32; 2]> = (0..40).map(|i| [i as f32, (i as f32 * 0.7).sin()]).collect();
        ui.scatter("Scatter", &dots, 0.0..=39.0, -1.0..=1.0, ["s", "V"]);
        ui.waveform_trim("Waveform", &wave, Some(0.3), &mut self.trim);
        let added = self.added.to_string();
        ui.table("Values", &["Name", "Value"], &["added", added.as_str(), "points", "3"], &mut self.row);
        let mut levels = [0.0_f32; 32];
        for v in &wave {
            levels[((v.abs() * 31.0) as usize).min(31)] += 1.0;
        }
        ui.curve_histogram("Curve", &mut self.points, &levels);
        let pixels: Vec<u8> = (0..256_u32).flat_map(|i| [(i % 32 * 8) as u8, 96, (255 - i % 32 * 8) as u8, 255]).collect();
        ui.image("Image", [32, 8], &pixels);
    }
}
`;

/** True if a pixel of the RGBA column passes the test. */
export function anyPixel(column: Uint8Array, test: (r: number, g: number, b: number) => boolean): boolean {
  for (let i = 0; i < column.length; i += 4) if (column[i + 3] > 128 && test(column[i], column[i + 1], column[i + 2])) return true;
  return false;
}

/** True when the node `id` has an element of the class `cls`, for example an icon. */
export const icon = (id: string, cls: string) => document.querySelector(`.react-flow__node[data-id="${id}"] .${cls}`) !== null;
export const amber = (r: number, _g: number, b: number) => r - b > 80;
export const gray = (r: number, g: number, b: number) => Math.abs(r - g) < 12 && Math.abs(r - b) < 20;

/** Reads a column of 16 px of the wire canvas at a screen point, by default the middle of the first wire. */
export async function wireColumn(at?: { x: number; y: number }): Promise<{ x: number; y: number; column: Uint8Array }> {
  const canvas = document.querySelector<HTMLCanvasElement>('.wire-layer');
  const path = document.querySelector<SVGPathElement>('.wire-hit');
  const pane = document.querySelector('.react-flow__pane');
  if (!canvas || !path || !pane || document.querySelector('.svg-wires')) throw new Error('the WebGL wire layer is not active');
  const gl = canvas.getContext('webgl2')!;
  for (let tries = 0; tries < 5; tries++) {
    const box = path.getBoundingClientRect();
    const x = at?.x ?? box.left + box.width / 2;
    const y = at?.y ?? box.top + box.height / 2;
    // A small zoom at the middle of the wire makes the layer draw in the next frame. This callback
    // runs after that draw and before the frame shows, so the drawing buffer still has the wires.
    pane.dispatchEvent(new WheelEvent('wheel', { deltaY: -1, clientX: x, clientY: y, bubbles: true, cancelable: true }));
    const column = await new Promise<Uint8Array>((resolve) =>
      requestAnimationFrame(() => {
        const rect = canvas.getBoundingClientRect();
        const scale = canvas.width / rect.width;
        const h = Math.round(16 * scale);
        const top = Math.round((y - 8 - rect.top) * scale);
        const out = new Uint8Array(h * 4);
        gl.readPixels(Math.round((x - rect.left) * scale), canvas.height - top - h, 1, h, gl.RGBA, gl.UNSIGNED_BYTE, out);
        resolve(out);
      }),
    );
    if (at || anyPixel(column, () => true)) return { x, y, column };
  }
  throw new Error('the wire layer drew nothing at the middle of the wire');
}
