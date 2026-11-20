//! The Ember instrument panels, rotary controls and display traces.
use patina_sdk::{Canvas, Key, Mode, Pointer, Ui};
use std::f32::consts::PI;
use std::fmt::Write;

mod xy;
pub use xy::XyPad;
mod strip;
pub use strip::{Ctl, Live, Look, Strip};

pub const INK: [u8; 3] = [238, 238, 235];
pub const MUTED: [u8; 3] = [178, 178, 173];
pub const DARK: [u8; 3] = [20, 20, 20];
pub const EDGE: [u8; 3] = [69, 69, 69];
pub const AMBER: [u8; 3] = [227, 184, 105];
pub const TEAL: [u8; 3] = [121, 212, 200];
pub const PINK: [u8; 3] = [222, 143, 186];
pub const VIOLET: [u8; 3] = [185, 155, 223];

pub fn rounded(c: &mut Canvas, [x, y, w, h]: [f32; 4], r: f32, color: [u8; 3]) {
    c.path(&format!("M {} {y} H {} Q {} {y} {} {} V {} Q {} {} {} {} H {} Q {x} {} {x} {} V {} Q {x} {y} {} {y} Z",
        x+r, x+w-r, x+w, x+w, y+r, y+h-r, x+w, y+h, x+w-r, y+h, x+r, y+h, y+h-r, y+r, x+r), color, true);
}

pub fn panel(c: &mut Canvas, [w, h]: [f32; 2], title: &str, subtitle: &str, color: [u8; 3]) {
    rounded(c, [0.0, 0.0, w, h], 8.0, EDGE);
    rounded(c, [1.0, 1.0, w - 2.0, h - 2.0], 7.0, [38, 38, 38]);
    rounded(c, [10.0, 10.0, 4.0, 28.0], 2.0, color);
    c.text([23.0, 27.0], INK, title);
    c.text([23.0, 47.0], MUTED, subtitle);
    c.line([16.0, 59.0], [w - 16.0, 59.0], EDGE);
    for x in [9.0, w - 9.0] {
        c.circle([x, h - 9.0], 2.5, [100, 100, 98], false);
        c.line([x - 1.0, h - 10.0], [x + 1.0, h - 8.0], MUTED);
    }
}

pub fn screen(c: &mut Canvas, rect: [f32; 4]) {
    let [x, y, w, h] = rect;
    rounded(c, rect, 4.0, DARK);
    for i in 1..8 {
        let at = x + w * i as f32 / 8.0;
        c.line([at, y + 1.0], [at, y + h - 1.0], [37, 37, 37]);
    }
    for i in 1..4 {
        let at = y + h * i as f32 / 4.0;
        c.line([x + 1.0, at], [x + w - 1.0, at], [37, 37, 37]);
    }
}

pub fn trace(c: &mut Canvas, [x, y, w, h]: [f32; 4], values: &[f32], color: [u8; 3]) {
    let mut path = String::with_capacity(3000);
    for (i, &value) in values.iter().enumerate() {
        let px = x + w * i as f32 / (values.len().saturating_sub(1).max(1)) as f32;
        let py = y + h * (0.5 - value.clamp(-1.0, 1.0) * 0.45);
        let _ = write!(
            path,
            "{} {:.1} {:.1} ",
            if i == 0 { "M" } else { "L" },
            px,
            py
        );
    }
    c.path(&path, color, false);
}

pub fn button(c: &mut Canvas, [x, y, w, h]: [f32; 4], label: &str, on: bool, color: [u8; 3]) {
    rounded(c, [x, y, w, h], 4.0, if on { color } else { EDGE });
    if !on {
        rounded(c, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], 3.0, [48, 48, 48]);
    }
    c.text(
        [x + (w - label.len() as f32 * 6.6) * 0.5, y + h * 0.5 + 4.0],
        if on { DARK } else { INK },
        label,
    );
}

pub fn inside([x, y]: [f32; 2], [left, top, w, h]: [f32; 4]) -> bool {
    x >= left && x < left + w && y >= top && y < top + h
}

/// The fixed labels, range and position of one rotary control.
pub struct Knob<'a> {
    pub at: [f32; 2],
    pub key: &'a str,
    pub label: &'a str,
    pub range: [f32; 2],
    pub color: [u8; 3],
    pub logarithmic: bool,
    pub unit: &'a str,
}

/// The Inspector already shows editable parameters above the custom GUI.
pub fn inspector(ui: &mut Ui, lines: &[&str]) -> bool {
    if ui.mode() != Mode::Large {
        return false;
    }
    for line in lines {
        ui.label(line);
    }
    true
}

/// Drag vertically, or use the arrow keys on a focused dial. Values save as node parameters.
#[derive(Default)]
pub struct Dial {
    drag: Option<(f32, f32)>,
}
impl Dial {
    pub fn show(&mut self, ui: &mut Ui, value: &mut f32, knob: Knob) {
        let key = knob.key;
        if self.edit(ui, value, knob) {
            ui.set_param(key, *value);
        }
    }

    /// A view uses the same dial, then sends its change through `Graph::set_param`.
    pub fn edit(&mut self, ui: &mut Ui, value: &mut f32, knob: Knob) -> bool {
        let Knob {
            at,
            key: _,
            label,
            range,
            color,
            logarithmic,
            unit,
        } = knob;
        let [lo, hi] = range;
        let norm = |v: f32| {
            if logarithmic {
                (v / lo).ln() / (hi / lo).ln()
            } else {
                (v - lo) / (hi - lo)
            }
        };
        let scale = |t: f32| {
            if logarithmic {
                lo * (hi / lo).powf(t)
            } else {
                lo + (hi - lo) * t
            }
        };
        let before = *value;
        let original = norm(value.clamp(lo, hi));
        let mut t = original;
        ui.place(at);
        {
            let mut c = ui.canvas(&format!("{label} {:.2} {unit}", *value), [88.0, 112.0]);
            c.grab();
            for p in c.pointer() {
                match *p {
                    Pointer::Down([_, y]) => self.drag = Some((y, t)),
                    Pointer::Move([_, y]) => {
                        if let Some((start, base)) = self.drag {
                            t = (base + (start - y) / 140.0).clamp(0.0, 1.0);
                        }
                    }
                    Pointer::Up(_) => self.drag = None,
                }
            }
            for key in c.keys() {
                if let Key::Down(key) = key {
                    match key.as_str() {
                        "ArrowUp" | "ArrowRight" => t = (t + 0.01).min(1.0),
                        "ArrowDown" | "ArrowLeft" => t = (t - 0.01).max(0.0),
                        _ => {}
                    }
                }
            }
            if t != original {
                *value = scale(t);
            }
            let center = [44.0, 39.0];
            for i in 0..=24 {
                let a = PI * 0.75 + i as f32 / 24.0 * PI * 1.5;
                let p = |r: f32| [center[0] + a.cos() * r, center[1] + a.sin() * r];
                c.line(
                    p(29.0),
                    p(32.0),
                    if i as f32 / 24.0 <= t { color } else { EDGE },
                );
            }
            c.circle([44.0, 41.0], 25.0, [14, 14, 14], true);
            c.circle(center, 24.0, [74, 74, 72], true);
            c.circle(center, 21.5, [48, 48, 48], true);
            c.circle(center, 18.0, [42, 42, 42], false);
            let a = PI * 0.75 + t * PI * 1.5;
            c.line(
                [44.0 + a.cos() * 8.0, 39.0 + a.sin() * 8.0],
                [44.0 + a.cos() * 20.0, 39.0 + a.sin() * 20.0],
                color,
            );
            c.text([44.0 - label.len() as f32 * 3.3, 83.0], MUTED, label);
            let text = if unit == "s" && *value < 1.0 {
                format!("{:.0} ms", *value * 1000.0)
            } else if *value >= 1000.0 {
                format!("{:.1}k {unit}", *value / 1000.0)
            } else if hi > 10.0 {
                format!("{:.0} {unit}", *value)
            } else {
                format!("{:.2} {unit}", *value)
            };
            c.text([44.0 - text.len() as f32 * 3.3, 102.0], INK, &text);
        }
        (*value - before).abs() > 0.000001
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hit_test_excludes_the_next_pad_and_accepts_its_own_edges() {
        assert!(inside([1.0, 2.0], [1.0, 2.0, 3.0, 4.0]));
        assert!(!inside([4.0, 2.0], [1.0, 2.0, 3.0, 4.0]));
        assert!(!inside([1.0, 6.0], [1.0, 2.0, 3.0, 4.0]));
    }
}
