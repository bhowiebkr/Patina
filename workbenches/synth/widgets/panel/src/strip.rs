//! The live panel of the audio nodes: a trace, a stereo peak meter and small controls (spec 4.3).
use crate::{AMBER, DARK, EDGE, INK, MUTED, button, inside, panel, rounded, screen, trace};
use patina_sdk::{Canvas, Key, Pointer, Ui};
use std::f32::consts::PI;
use std::fmt::Write;

/// The display value of an audio node: 32 points of the last block, the peaks and up to five
/// values of its inputs.
#[derive(Clone, Copy, Default)]
pub struct Live {
    pub wave: [f32; 32],
    pub peak: [f32; 2],
    pub values: [f32; 5],
    pub ready: bool,
}

impl Live {
    /// Keeps a short view of the last block. The audio thread calls it, so it does not allocate.
    pub fn keep(&mut self, left: &[f32], right: &[f32], values: [f32; 5]) {
        let clean = |s: f32| if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 };
        let step = (left.len() / 32).max(1);
        for (i, w) in self.wave.iter_mut().enumerate() {
            *w = clean(left.get(i * step).copied().unwrap_or(0.0));
        }
        for (peak, side) in self.peak.iter_mut().zip([left, right]) {
            let block = side.iter().fold(0.0_f32, |m, &s| m.max(clean(s).abs()));
            *peak = block.max(*peak * 0.97);
        }
        self.values = values.map(|v| if v.is_finite() { v } else { 0.0 });
        self.ready = true;
    }
}

#[derive(Clone, Copy)]
enum Kind<'a> {
    Dial,
    Whole,
    Pick(&'a [&'a str]),
    Switch,
}

/// One control of a strip. The key is an input of the node, and the control sets it as a
/// parameter. The value of control `i` is `Live::values[i]`.
#[derive(Clone, Copy)]
pub struct Ctl<'a> {
    key: &'a str,
    label: &'a str,
    range: [f32; 2],
    unit: &'a str,
    kind: Kind<'a>,
}

impl<'a> Ctl<'a> {
    /// A rotary control. A range with a positive start and an end of 100 x the start or more
    /// turns as a logarithm.
    pub const fn dial(key: &'a str, label: &'a str, range: [f32; 2], unit: &'a str) -> Self {
        Self { key, label, range, unit, kind: Kind::Dial }
    }

    /// A rotary control for a whole number, for example a bit depth.
    pub const fn whole(key: &'a str, label: &'a str, range: [f32; 2]) -> Self {
        Self { key, label, range, unit: "", kind: Kind::Whole }
    }

    /// A button that selects the next name at each click. The value is the index of the name.
    pub const fn pick(key: &'a str, label: &'a str, names: &'a [&'a str]) -> Self {
        Self { key, label, range: [0.0, 1.0], unit: "", kind: Kind::Pick(names) }
    }

    /// A button that sets a true or false input.
    pub const fn switch(key: &'a str, label: &'a str) -> Self {
        Self { key, label, range: [0.0, 1.0], unit: "", kind: Kind::Switch }
    }

    fn log(&self) -> bool {
        self.range[0] > 0.0 && self.range[1] >= self.range[0] * 100.0
    }

    fn norm(&self, v: f32) -> f32 {
        let [lo, hi] = self.range;
        let t = if self.log() { (v.max(lo) / lo).ln() / (hi / lo).ln() } else { (v - lo) / (hi - lo) };
        if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 }
    }

    fn scale(&self, t: f32) -> f32 {
        let [lo, hi] = self.range;
        let v = if self.log() { lo * (hi / lo).powf(t) } else { lo + (hi - lo) * t };
        if matches!(self.kind, Kind::Whole) { v.round() } else { v }
    }

    fn text(&self, out: &mut String, v: f32) {
        out.clear();
        let unit = self.unit;
        let _ = match self.kind {
            Kind::Whole => write!(out, "{v:.0}"),
            _ if unit == "s" && v.abs() < 1.0 => write!(out, "{:.0} ms", v * 1000.0),
            _ if v.abs() >= 1000.0 => write!(out, "{:.1}k {unit}", v / 1000.0),
            _ if v.abs() >= 10.0 => write!(out, "{v:.0} {unit}"),
            _ => write!(out, "{v:.2} {unit}"),
        };
        out.truncate(out.trim_end().len());
    }
}

/// The fixed text and color of a strip.
pub struct Look<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    pub color: [u8; 3],
}

const TOP: f32 = 116.0;
const COLUMN: f32 = 60.0;

/// The GUI state of a strip. The editor does not save it.
#[derive(Default)]
pub struct Strip {
    drag: Option<(usize, f32, f32)>,
    focus: usize,
    /// The value that a control sent, and the live value at that time, until the live value changes.
    sent: [Option<(f32, f32)>; 5],
}

impl Strip {
    /// Draws the strip as the whole node, and sends each control change as a parameter.
    pub fn show(&mut self, ui: &mut Ui, live: &Live, look: Look, ctls: &[Ctl]) {
        let ctls = &ctls[..ctls.len().min(5)];
        let w = (24.0 + COLUMN * ctls.len() as f32).max(240.0);
        let h = if ctls.is_empty() { 120.0 } else { 208.0 };
        let mut values = [0.0; 5];
        for (i, value) in values.iter_mut().enumerate().take(ctls.len()) {
            *value = match self.sent[i] {
                Some((sent, before)) if live.values[i] == before => sent,
                _ => {
                    self.sent[i] = None;
                    live.values[i]
                }
            };
        }
        let mut text = String::with_capacity(160);
        let _ = write!(text, "{} {}", look.title, look.subtitle);
        let mut buf = String::with_capacity(16);
        for (ctl, &v) in ctls.iter().zip(&values) {
            ctl.text(&mut buf, v);
            let _ = write!(text, ", {} {buf}", ctl.label);
        }
        let mut send = [None; 5];
        {
            let mut c = ui.canvas(&text, [w, h]);
            let rect = |i: usize| [12.0 + i as f32 * COLUMN, TOP, COLUMN, 84.0];
            for i in 0..ctls.len() {
                let [x, y, cw, ch] = rect(i);
                c.grab_rect([x, y], [cw, ch]);
            }
            for p in c.pointer() {
                match *p {
                    Pointer::Down(at) => {
                        let Some(i) = (0..ctls.len()).find(|&i| inside(at, rect(i))) else { continue };
                        self.focus = i;
                        let v = match ctls[i].kind {
                            Kind::Dial | Kind::Whole => {
                                self.drag = Some((i, at[1], ctls[i].norm(values[i])));
                                continue;
                            }
                            Kind::Pick(names) => ((values[i] as usize + 1) % names.len().max(1)) as f32,
                            Kind::Switch => f32::from(values[i] < 0.5),
                        };
                        (values[i], send[i]) = (v, Some(v));
                    }
                    Pointer::Move([_, y]) => {
                        let Some((i, start, base)) = self.drag else { continue };
                        let v = ctls[i].scale((base + (start - y) / 140.0).clamp(0.0, 1.0));
                        if v != values[i] {
                            (values[i], send[i]) = (v, Some(v));
                        }
                    }
                    Pointer::Up(_) => self.drag = None,
                }
            }
            if ctls.iter().any(|c| matches!(c.kind, Kind::Dial | Kind::Whole)) {
                for key in c.keys() {
                    let Key::Down(key) = key else { continue };
                    let step = match key.as_str() {
                        "ArrowUp" | "ArrowRight" => 1.0,
                        "ArrowDown" | "ArrowLeft" => -1.0,
                        _ => continue,
                    };
                    let i = self.focus.min(ctls.len() - 1);
                    let ctl = &ctls[i];
                    let v = match ctl.kind {
                        Kind::Whole => (values[i] + step).clamp(ctl.range[0], ctl.range[1]),
                        Kind::Dial => ctl.scale((ctl.norm(values[i]) + step * 0.01).clamp(0.0, 1.0)),
                        _ => continue,
                    };
                    if v != values[i] {
                        (values[i], send[i]) = (v, Some(v));
                    }
                }
            }
            draw(&mut c, [w, h], live, &look);
            for (i, ctl) in ctls.iter().enumerate() {
                let x = 12.0 + (i as f32 + 0.5) * COLUMN;
                let v = values[i];
                centered(&mut c, [x, TOP + 60.0], MUTED, ctl.label);
                match ctl.kind {
                    Kind::Dial | Kind::Whole => {
                        knob(&mut c, [x, TOP + 24.0], ctl.norm(v), live.ready, look.color);
                        ctl.text(&mut buf, v);
                        centered(&mut c, [x, TOP + 76.0], INK, if live.ready { &buf } else { "--" });
                    }
                    Kind::Pick(names) => {
                        let name = if live.ready { names.get(v as usize).copied().unwrap_or("?") } else { "--" };
                        button(&mut c, [x - 27.0, TOP + 12.0, 54.0, 24.0], name, live.ready, look.color);
                    }
                    Kind::Switch => {
                        let name = if !live.ready { "--" } else if v >= 0.5 { "ON" } else { "OFF" };
                        button(&mut c, [x - 27.0, TOP + 12.0, 54.0, 24.0], name, v >= 0.5, look.color);
                    }
                }
            }
        }
        for (i, ctl) in ctls.iter().enumerate() {
            let Some(v) = send[i] else { continue };
            self.sent[i] = Some((v, live.values[i]));
            match ctl.kind {
                Kind::Dial => ui.set_param(ctl.key, v),
                Kind::Switch => ui.set_param(ctl.key, v >= 0.5),
                Kind::Whole | Kind::Pick(_) => ui.set_param(ctl.key, v as i64),
            }
        }
    }
}

/// The panel, the trace of the last block and the stereo peak meter.
fn draw(c: &mut Canvas, [w, h]: [f32; 2], live: &Live, look: &Look) {
    panel(c, [w, h], look.title, look.subtitle, look.color);
    let db = |peak: f32| 20.0 * peak.max(0.00001).log10();
    let loud = db(live.peak[0].max(live.peak[1]));
    if live.ready && loud > -90.0 {
        c.text([w - 66.0, 47.0], MUTED, format_args!("{loud:>4.0} dB"));
    } else {
        c.text([w - 52.0, 47.0], MUTED, if live.ready { "QUIET" } else { "IDLE" });
    }
    let sw = w - 54.0;
    screen(c, [12.0, 68.0, sw, 40.0]);
    trace(c, [14.0, 70.0, sw - 4.0, 36.0], &live.wave, look.color);
    for (k, &peak) in live.peak.iter().enumerate() {
        let x = w - 36.0 + k as f32 * 13.0;
        rounded(c, [x, 68.0, 10.0, 40.0], 2.0, DARK);
        let t = ((db(peak) + 48.0) / 48.0).clamp(0.0, 1.0);
        if live.ready && t > 0.0 {
            c.rect([x + 2.0, 106.0 - t * 36.0], [6.0, t * 36.0], if peak > 0.9 { AMBER } else { look.color }, true);
        }
    }
}

fn centered(c: &mut Canvas, [x, y]: [f32; 2], color: [u8; 3], text: &str) {
    c.text([x - text.len() as f32 * 3.3, y], color, text);
}

fn knob(c: &mut Canvas, [x, y]: [f32; 2], t: f32, ready: bool, color: [u8; 3]) {
    let arc = |c: &mut Canvas, to: f32, color: [u8; 3]| {
        let mut path = String::with_capacity(320);
        for k in 0..=16 {
            let a = PI * 0.75 + to * PI * 1.5 * k as f32 / 16.0;
            let _ = write!(path, "{} {:.1} {:.1} ", if k == 0 { "M" } else { "L" }, x + a.cos() * 20.0, y + a.sin() * 20.0);
        }
        c.path(&path, color, false);
    };
    arc(c, 1.0, EDGE);
    if ready && t > 0.0 {
        arc(c, t, color);
    }
    c.circle([x, y + 2.0], 16.0, [14, 14, 14], true);
    c.circle([x, y], 15.0, [74, 74, 72], true);
    c.circle([x, y], 13.0, [48, 48, 48], true);
    let a = PI * 0.75 + t * PI * 1.5;
    c.line([x + a.cos() * 5.0, y + a.sin() * 5.0], [x + a.cos() * 12.0, y + a.sin() * 12.0], if ready { color } else { MUTED });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dial_maps_its_range_both_ways_and_rounds_whole_numbers() {
        let cutoff = Ctl::dial("cutoff", "CUTOFF", [20.0, 20_000.0], "Hz");
        assert!(cutoff.log());
        assert!((cutoff.scale(cutoff.norm(850.0)) - 850.0).abs() < 0.1);
        assert_eq!(cutoff.norm(f32::NAN), 0.0);
        let bits = Ctl::whole("bits", "BITS", [2.0, 16.0]);
        assert_eq!(bits.scale(0.5), 9.0);
        let mut text = String::new();
        cutoff.text(&mut text, 1600.0);
        assert_eq!(text, "1.6k Hz");
        Ctl::dial("attack", "ATTACK", [0.001, 2.0], "s").text(&mut text, 0.008);
        assert_eq!(text, "8 ms");
    }

    #[test]
    fn live_keeps_finite_points_and_falling_peaks() {
        let mut live = Live::default();
        let mut block = [0.5_f32; 256];
        block[0] = f32::NAN;
        live.keep(&block, &block, [1.0, f32::INFINITY, 0.0, 0.0, 0.0]);
        assert!(live.ready && live.wave[0] == 0.0 && live.wave[1] == 0.5);
        assert_eq!(live.peak, [0.5, 0.5]);
        assert_eq!(live.values[1], 0.0);
        live.keep(&[0.0; 256], &[0.0; 256], [0.0; 5]);
        assert!(live.peak[0] < 0.5 && live.peak[0] > 0.4);
    }
}
