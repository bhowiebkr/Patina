// The fixtures of the self-check (check.ts): its node IDs, the code that it writes, and the pixel
// reads of the wire layer.

import { api, type EngineEvent, type View } from '../api';
import { saveCode, undoRedo } from '../code/codeState';

export const BUILD_MS = 150_000;
export const RUN_MS = 20_000;
export const A = 'a000000000000001';
export const B = 'b000000000000002';
export const T = 'c000000000000003';

/** Undoes a code change. The text goes into the Code tab and writes no file, so a save builds it (15.10). */
export async function undoCode(): Promise<View> {
  const v = await undoRedo(api.undo);
  if (!v.codeText) throw new Error('the undo gave no text to the Code tab');
  if ((await api.readCode(v.codeText.path)) === v.codeText.text) throw new Error(`the undo wrote ${v.codeText.path}`);
  return (await saveCode()) ?? v;
}

/** The screen point of a mouse event. */
export type At = { clientX: number; clientY: number };

/** The shared state and waits of the self-check steps. */
export type CheckContext = {
  /** Runs a job, shows its view and records the step name. */
  step: (name: string, job: () => Promise<View>) => Promise<View>;
  /** Waits until the test is true. It throws after `ms`. */
  until: (what: string, ms: number, test: () => boolean) => Promise<void>;
  /** Waits for the result of the engine after `from`, and for an output line. */
  result: (v: View, from: number, line: string, ms: number) => Promise<Extract<EngineEvent, { state: 'ran' }>>;
  /** Marks the start of the events and the output lines to look at, and gives the event index. */
  mark: () => number;
  events: EngineEvent[];
  /** The output lines after the last mark. */
  fresh: () => string[];
  steps: string[];
};

/** A mouse wheel event at a point. A mouse wheel step has a `wheelDeltaY` of 120, a touchpad other values (15.2). */
export const mouseWheel = (deltaY: number, clientX: number, clientY: number) =>
  new WheelEvent('wheel', { deltaY, clientX, clientY, wheelDeltaY: deltaY < 0 ? 120 : -120, bubbles: true, cancelable: true } as WheelEventInit);

/** An event node (9.4): it gives the number plus the length of the text of the event. */
export const EVENT_NODE = `use patina_sdk::node;

#[node(domain = event)]
pub fn number(value: f64, text: &String) -> f64 {
    while text == "hang" {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    if text == "crash" {
        unsafe { (8usize as *mut u8).write_volatile(1) }
    }
    value + text.len() as f64
}
`;

/** A number node with a bool input: the switch doubles the value. */
export const SWITCH_NODE = `use patina_sdk::node;

#[node]
pub fn number(value: f64, double: bool) -> f64 {
    if double { value * 2.0 } else { value }
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
        // At 33 the view does not return, so the second Stop must end it (16.1).
        while graph.param("${A}", "value") == Some("33") {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
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

#[node(full_gui)]
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
    ramp: patina_sdk::ColorRamp,
    stop: usize,
    layers: Vec<patina_sdk::Layer>,
    layer: usize,
    path: String,
    source: String,
    dot: [f32; 2],
    code: String,
}

impl Number {
    pub fn init() -> Self {
        Number { added: 0.0, row: None, tint: [223, 166, 75], points: vec![[0.0, 0.2], [0.5, 0.8], [1.0, 0.4]], gain: 1.5, page: 0, more: false, trim: [0.0, 1.0], turns: 0, ramp: patina_sdk::ColorRamp { stops: vec![(0.0, [24, 32, 96]), (0.62, [232, 163, 61]), (1.0, [250, 240, 220])], blend: patina_sdk::Blend::Linear }, stop: 1, layers: vec![
            patina_sdk::Layer { name: "Edge polish".into(), color: [232, 176, 128], visible: true, amount: Some(0.35) },
            patina_sdk::Layer { name: "Verdigris".into(), color: [52, 190, 160], visible: true, amount: Some(0.62) },
            patina_sdk::Layer { name: "Copper".into(), color: [180, 100, 56], visible: true, amount: None },
        ], layer: 1, path: "out/frame_####.png".into(), source: String::new(), dot: [20.0, 24.0], code: "LDI 1\\nOUT".into() }
    }

    pub fn process(&mut self, value: f64) -> f64 {
        value + self.added
    }

    pub fn ui(&mut self, ui: &mut patina_sdk::Ui) {
        if ui.button("Add 1").clicked() {
            self.added += 1.0;
        }
        if ui.button("Set 4").clicked() {
            ui.set_param("value", 4);
        }
        if ui.icon_button("rotate-right", "Rotate right").clicked() {
            self.turns = (self.turns + 1) % 4;
        }
        ui.progress("Turns", self.turns as f32 / 4.0);
        ui.color("Tint", &mut self.tint);
        ui.number_unit("Gain", &mut self.gain, 0.5, "dB");
        ui.color_ramp("Ramp", &mut self.ramp, &mut self.stop);
        ui.layer_stack("Layers", &mut self.layers, &mut self.layer);
        ui.file_path("Path", &mut self.path, patina_sdk::PathKind::Save);
        ui.asset_picker("Source", &mut self.source, &["toml"]);
        ui.code_field("Code", &mut self.code);
        let mut pad = ui.canvas("Pad", [160.0, 48.0]);
        if pad.keys().contains(&patina_sdk::Key::Down("KeyK".into())) {
            self.dot[0] += 10.0;
        }
        if let Some(patina_sdk::Pointer::Down(p) | patina_sdk::Pointer::Move(p)) = pad.pointer().last().copied() {
            self.dot = p;
        }
        pad.circle(self.dot, 6.0, [232, 163, 61], true);
        if ui.mode() == patina_sdk::Mode::Full {
            ui.label("Full size");
        }
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

/** Types a text into a field as a person does, and presses Enter. */
export function enterText(field: HTMLInputElement, text: string) {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(field, text);
  field.dispatchEvent(new Event('input', { bubbles: true }));
  field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
}

/** True if a pixel of the RGBA column passes the test. */
export function anyPixel(column: Uint8Array, test: (r: number, g: number, b: number) => boolean): boolean {
  for (let i = 0; i < column.length; i += 4) if (column[i + 3] > 128 && test(column[i], column[i + 1], column[i + 2])) return true;
  return false;
}

/** The fire button of trigger T in the open graph (9.4). */
export const fireButton = () => document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${T}"] button[aria-label="Fire the trigger"]`);
/** True when the node `id` has an element of the class `cls`, for example an icon. */
export const icon = (id: string, cls: string) => document.querySelector(`.react-flow__node[data-id="${id}"] .${cls}`) !== null;
export const amber = (r: number, _g: number, b: number) => r - b > 80;
export const gray = (r: number, g: number, b: number) => Math.abs(r - g) < 12 && Math.abs(r - b) < 20;

/** The first button with the text in the window. */
export const button = (text: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent === text);
export const tab = (name: string) => [...document.querySelectorAll<HTMLElement>('[role=tab]')].find((t) => t.textContent === name);
export const leftTab = (name: string) => [...document.querySelectorAll<HTMLElement>('.area.left [role=tab]')].find((t) => t.textContent === name)!;
export const topButton = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('.topbar button')].find((b) => b.textContent === label);
export const statusText = () => document.querySelector('.statusbar')?.textContent ?? '';
/** The value badge of the first wire. */
export const badge = () => document.querySelector('.wire-value');
/** The CSS transform of the canvas, and its zoom. */
export const transform = () => document.querySelector<HTMLElement>('.react-flow__viewport')?.style.transform;
export const scale = () => Number(/scale\(([^)]+)\)/.exec(transform() ?? '')?.[1]);

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
    pane.dispatchEvent(mouseWheel(-1, x, y));
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
