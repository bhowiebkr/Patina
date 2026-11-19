//! Shared material node panels. All edits use Patina parameters and normal graph undo.
use material_lab::{Operator, ParamType, catalog};
use patina_sdk::{Mode, Pointer, Ui};
pub mod workflow;

pub const INK: [u8; 3] = [225; 3];
pub const MUTED: [u8; 3] = [166; 3];
pub const COPPER: [u8; 3] = [207, 143, 98];
pub const TEAL: [u8; 3] = [108, 193, 180];

#[derive(Default)]
pub struct Panel {
    stop: usize,
}

impl Panel {
    pub fn show(&mut self, ui: &mut Ui, op: &mut Operator) {
        let Some(def) = catalog().get(&op.node.kind) else {
            ui.label("Missing material definition");
            return;
        };
        let layer = op.node.kind == "layer";
        if ui.mode() == Mode::Large {
            ui.label(&def.description);
            if !op.error.is_empty() {
                ui.label(&op.error);
            }
            if !op.thumbnail.error.is_empty() {
                ui.label(&op.thumbnail.error);
            }
            if layer {
                self.precise(ui, op);
            }
            if ui.button("Restore kernel defaults").clicked() {
                self.defaults(ui, op);
            }
            return;
        }
        if op.revision != 0
            && let Ok(gpu) = patina_gpu::gpu()
        {
            op.thumbnail.poll(gpu);
        }
        ui.pin("target", [0.0, 46.0]);
        let mask = op.node.kind == "mask";
        let width = if layer { 352.0 } else { 304.0 };
        let accent = if layer || op.node.kind == "finish" {
            COPPER
        } else {
            TEAL
        };
        let preview_height = (width - 32.0) * 0.5;
        let pins_y = 80.0 + preview_height;
        let inputs_height = (def.inputs.len().max(def.outputs.len()).max(1) as f32) * 22.0;
        let chart_y = pins_y + inputs_height + 10.0;
        let mut controls_y = chart_y + if mask { 92.0 } else { 0.0 };
        if layer {
            controls_y += 116.0;
        }
        let count = def
            .params
            .iter()
            .filter(|p| !matches!(p.ty, ParamType::Gradient))
            .count();
        let height = controls_y + count as f32 * 32.0 + 34.0;
        {
            let title = if op.node.title.is_empty() {
                def.name.as_str()
            } else {
                op.node.title.as_str()
            };
            let mut c = ui.canvas(&format!("{} material node", title), [width, height]);
            c.rect([0.0, 0.0], [width, height], [38; 3], true);
            c.rect([12.0, 17.0], [3.0, 30.0], accent, true);
            c.text([23.0, 28.0], INK, title);
            c.text(
                [23.0, 46.0],
                MUTED,
                &format!("{} / BAKE TARGET", def.name.to_uppercase()),
            );
            c.line([16.0, 57.0], [width - 16.0, 57.0], [67; 3]);
            c.rect(
                [15.0, 63.0],
                [width - 30.0, preview_height + 2.0],
                [20; 3],
                true,
            );
            for (i, input) in def.inputs.iter().enumerate() {
                c.text([12.0, pins_y + i as f32 * 22.0 + 4.0], MUTED, &input.label);
            }
            c.text(
                [width - 77.0, pins_y + 4.0],
                accent,
                if layer || op.node.kind == "finish" {
                    "MATERIAL"
                } else {
                    "MASK"
                },
            );
            if op.node.kind == "cells" {
                c.text([width - 77.0, pins_y + 26.0], accent, "CELL ID");
            }
            c.line(
                [16.0, height - 28.0],
                [width - 16.0, height - 28.0],
                [62; 3],
            );
            c.text(
                [16.0, height - 10.0],
                MUTED,
                if op.error.is_empty() {
                    "MILLWRIGHT / NATIVE GPU"
                } else {
                    "ERROR / SEE INSPECTOR"
                },
            );
            if mask {
                let r = [16.0, chart_y, width - 32.0, 70.0];
                c.rect([r[0], r[1]], [r[2], r[3]], [24; 3], true);
                for i in 1..4 {
                    let x = r[0] + r[2] * i as f32 / 4.0;
                    c.line([x, r[1]], [x, r[1] + r[3]], [48; 3]);
                }
                let from = op.value("from");
                let to = op.value("to");
                let low = op.value("low");
                let high = op.value("high");
                let span = if (to - from).abs() < 0.00001 {
                    0.00001
                } else {
                    to - from
                };
                let point = |i: usize| {
                    let x = i as f32 / 64.0;
                    let t = ((x - from) / span).clamp(0.0, 1.0);
                    let value = low + (high - low) * t * t * (3.0 - 2.0 * t);
                    [r[0] + x * r[2], r[1] + (1.0 - value) * r[3]]
                };
                for i in 1..=64 {
                    c.line(point(i - 1), point(i), accent);
                }
                c.text([20.0, chart_y + 84.0], MUTED, "DRIVER TO COVERAGE");
            }
        }
        if !op.thumbnail.pixels.is_empty() {
            ui.place([16.0, 64.0]);
            ui.image("", [256, 128], &op.thumbnail.pixels);
        }
        ui.place([width * 0.5 + 10.0, 64.0]);
        {
            let mut c = ui.canvas(
                "Material preview tones",
                [width * 0.5 - 26.0, preview_height],
            );
            c.text(
                [0.0, 23.0],
                accent,
                if layer {
                    "PBR MATERIAL"
                } else {
                    "PREVIEW TONES"
                },
            );
            c.text(
                [0.0, 45.0],
                INK,
                &if op.revision == 0 {
                    "Run to evaluate".to_string()
                } else {
                    format!("{} x {}", op.size, op.size)
                },
            );
            c.text(
                [0.0, 63.0],
                MUTED,
                if layer {
                    "3 float planes"
                } else {
                    "Full float precision"
                },
            );
            let max = op
                .thumbnail
                .histogram
                .iter()
                .copied()
                .max()
                .unwrap_or(1)
                .max(1) as f32;
            let dx = (width * 0.5 - 28.0) / 32.0;
            for (i, count) in op.thumbnail.histogram.iter().enumerate() {
                let h = *count as f32 / max * 34.0;
                c.rect(
                    [i as f32 * dx, 114.0 - h],
                    [dx.max(1.0) - 0.5, h],
                    accent,
                    true,
                );
            }
        }
        if layer {
            self.ramp(ui, op, width, chart_y);
        }
        for p in &def.params {
            if matches!(p.ty, ParamType::Gradient) {
                continue;
            }
            ui.pin(&p.id, [0.0, controls_y + 12.0]);
            ui.place([16.0, controls_y]);
            let mut v = op.value(&p.id);
            let changed = match &p.ty {
                ParamType::Enum(names) => {
                    let mut index = v as usize;
                    let labels: Vec<&str> = names.iter().map(String::as_str).collect();
                    let changed = ui.dropdown(&p.label, &mut index, &labels).changed();
                    v = index as f32;
                    changed
                }
                ParamType::Float { min, max } => ui.slider(&p.label, &mut v, *min..=*max).changed(),
                ParamType::Int { min, max } => {
                    let mut integer = v as i32;
                    let changed = ui.slider(&p.label, &mut integer, *min..=*max).changed();
                    v = integer as f32;
                    changed
                }
                _ => false,
            };
            if changed {
                ui.set_param(&p.id, v);
                op.node.params.insert(p.id.clone(), vec![v]);
            }
            controls_y += 32.0;
        }
        ui.pin("title", [0.0, 28.0]);
        if layer {
            ui.pin("color", [0.0, chart_y + 12.0]);
        }
        for (i, input) in def.inputs.iter().enumerate() {
            ui.pin(&input.id, [0.0, pins_y + i as f32 * 22.0]);
        }
        if op.node.kind == "cells" {
            ui.pin("out0", [width, pins_y]);
            ui.pin("out1", [width, pins_y + 22.0]);
        } else {
            ui.pin("out", [width, pins_y]);
        }
    }

    fn ramp(&mut self, ui: &mut Ui, op: &mut Operator, width: f32, top: f32) {
        let mut values = op.node.value(catalog(), "color");
        let count = (values[0] as usize).clamp(1, 256);
        values.truncate(1 + count * 5);
        self.stop = self.stop.min(count - 1);
        ui.place([16.0, top]);
        {
            let mut c = ui.canvas(
                "Coat color ramp. Click a stop to select it.",
                [width - 32.0, 45.0],
            );
            for p in c.pointer() {
                if let Pointer::Down([x, _]) = p {
                    self.stop = (0..count)
                        .min_by(|&a, &b| {
                            ((values[1 + a * 5] * (width - 32.0)) - x)
                                .abs()
                                .total_cmp(&((values[1 + b * 5] * (width - 32.0)) - x).abs())
                        })
                        .unwrap_or(0);
                }
            }
            for i in 0..128 {
                let t = i as f32 / 127.0;
                let color =
                    ramp_color(&values, t).map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
                c.rect(
                    [i as f32 * (width - 32.0) / 128.0, 0.0],
                    [(width - 32.0) / 128.0 + 0.1, 24.0],
                    color,
                    true,
                );
            }
            for i in 0..count {
                c.circle(
                    [values[1 + i * 5] * (width - 32.0), 30.0],
                    if i == self.stop { 4.0 } else { 2.5 },
                    INK,
                    true,
                );
            }
        }
        let at = 1 + self.stop * 5;
        let mut rgb =
            std::array::from_fn(|i| (values[at + i + 1].clamp(0.0, 1.0) * 255.0).round() as u8);
        ui.place([16.0, top + 48.0]);
        if ui.color("Selected stop", &mut rgb).changed() {
            for i in 0..3 {
                values[at + i + 1] = f32::from(rgb[i]) / 255.0;
            }
            save_ramp(ui, op, values);
        }
        ui.place([16.0, top + 80.0]);
        ui.label("Exact RGBA and stop positions in Inspector");
    }
    fn precise(&mut self, ui: &mut Ui, op: &mut Operator) {
        let mut values = op.node.value(catalog(), "color");
        let count = (values[0] as usize).clamp(1, 256);
        values.truncate(1 + count * 5);
        self.stop = self.stop.min(count - 1);
        let mut selected = self.stop as u32;
        ui.slider("Stop", &mut selected, 0..=count as u32 - 1);
        self.stop = selected as usize;
        let at = 1 + self.stop * 5;
        let mut changed = false;
        for (i, name) in ["Position", "Red", "Green", "Blue", "Alpha"]
            .iter()
            .enumerate()
        {
            changed |= ui
                .number_unit(name, &mut values[at + i], 0.001, "")
                .changed();
        }
        if changed {
            values[at] = values[at].clamp(
                if self.stop == 0 { 0.0 } else { values[at - 5] },
                if self.stop + 1 == count {
                    1.0
                } else {
                    values[at + 5]
                },
            );
            save_ramp(ui, op, values);
        }
    }
    fn defaults(&mut self, ui: &mut Ui, op: &mut Operator) {
        let Some(def) = catalog().get(&op.node.kind) else {
            return;
        };
        self.stop = 0;
        for param in &def.params {
            if matches!(param.ty, ParamType::Gradient) {
                save_ramp(ui, op, param.default.clone());
            } else if let Some(&value) = param.default.first() {
                ui.set_param(&param.id, value);
                op.node.params.insert(param.id.clone(), vec![value]);
            }
        }
    }
}

fn save_ramp(ui: &mut Ui, op: &mut Operator, values: Vec<f32>) {
    if values.iter().any(|v| !v.is_finite()) {
        return;
    }
    if let Ok(text) = serde_json::to_string(&values) {
        ui.set_param("color", text);
        op.node.params.insert("color".into(), values);
    }
}
fn ramp_color(values: &[f32], t: f32) -> [f32; 3] {
    let count = values[0] as usize;
    let stops = values[1..1 + count * 5].as_chunks::<5>().0;
    let a = stops.iter().rfind(|s| s[0] <= t).unwrap_or(&stops[0]);
    let b = stops
        .iter()
        .find(|s| s[0] >= t)
        .unwrap_or(&stops[stops.len() - 1]);
    let f = if b[0] > a[0] {
        (t - a[0]) / (b[0] - a[0])
    } else {
        0.0
    };
    std::array::from_fn(|i| a[i + 1] + (b[i + 1] - a[i + 1]) * f)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_interpolates_without_changing_the_float_stops() {
        let v = [2.0, 0.0, 0.1, 0.2, 0.3, 0.4, 1.0, 0.3, 0.4, 0.5, 0.6];
        let c = ramp_color(&v, 0.5);
        assert!((c[0] - 0.2).abs() < 0.00001);
        assert_eq!(v[5], 0.4);
    }
    #[test]
    fn defaults_restore_scalar_values_and_the_full_precision_ramp() {
        let mut panel = Panel::default();
        let mut ui = Ui::new(1, "");
        let mut noise = Operator::new("noise");
        noise.node.params.insert("scale".into(), vec![200.0]);
        panel.defaults(&mut ui, &mut noise);
        assert_eq!(noise.value("scale"), 8.0);
        let mut layer = Operator::new("layer");
        panel.defaults(&mut ui, &mut layer);
        assert_eq!(
            layer.node.params["color"],
            catalog()
                .get("layer")
                .unwrap()
                .param("color")
                .unwrap()
                .default
        );
    }
}
