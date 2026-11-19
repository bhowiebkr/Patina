//! The drawing parts of the gpu example, so that its node GUIs have one style (spec 4.3): the
//! front panel of a lab instrument, with rack screws and a dark screen with a phosphor trace.
use patina_sdk::Canvas;
use std::fmt::Write;

/// The metal of the front panel.
pub const PANEL: [u8; 3] = [38, 46, 56];
/// The edge line of the panel and the frame of a screen.
pub const RIM: [u8; 3] = [86, 100, 116];
/// The printed text on the panel.
pub const LABEL: [u8; 3] = [160, 176, 192];
/// The glass of a screen.
pub const SCREEN: [u8; 3] = [8, 16, 18];
/// A line or a segment that the phosphor lights.
pub const TRACE: [u8; 3] = [96, 232, 214];
/// The lines of the grid on a screen.
const GRID: [u8; 3] = [24, 50, 52];
/// A segment of a bar that is off.
const DIM: [u8; 3] = [20, 56, 54];
/// The frame of a screen, in the panel.
const BEZEL: [u8; 3] = [18, 22, 28];
/// The head of a screw and its slot.
const SCREW: [u8; 3] = [120, 130, 142];
const SLOT: [u8; 3] = [50, 58, 68];
/// The number of segments of a bar.
const SEGMENTS: usize = 20;

/// Fills a rectangle with round corners.
pub fn round_rect(c: &mut Canvas, [x, y]: [f32; 2], [w, h]: [f32; 2], r: f32, color: [u8; 3]) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let (x2, y2) = (x + w, y + h);
    let mut d = String::new();
    let _ = write!(d, "M {} {y} H {} Q {x2} {y} {x2} {} V {} Q {x2} {y2} {} {y2} ", x + r, x2 - r, y + r, y2 - r, x2 - r);
    let _ = write!(d, "H {} Q {x} {y2} {x} {} V {} Q {x} {y} {} {y} Z", x + r, y2 - r, y + r, x + r);
    c.path(&d, color, true);
}

/// The front panel with an edge line, a screw in each corner and the name of the instrument.
pub fn panel(c: &mut Canvas, [w, h]: [f32; 2], name: &str) {
    round_rect(c, [0.0, 0.0], [w, h], 6.0, RIM);
    round_rect(c, [1.0, 1.0], [w - 2.0, h - 2.0], 5.0, PANEL);
    for at in [[8.0, 8.0], [w - 8.0, 8.0], [8.0, h - 8.0], [w - 8.0, h - 8.0]] {
        screw(c, at);
    }
    c.text([18.0, 16.0], LABEL, name);
}

/// A screw head with a slot.
fn screw(c: &mut Canvas, [x, y]: [f32; 2]) {
    c.circle([x, y], 3.5, SCREW, true);
    c.line([x - 2.5, y + 1.5], [x + 2.5, y - 1.5], SLOT);
}

/// A screen in a frame, with a grid of 8 x 4 divisions and brighter center lines.
pub fn screen(c: &mut Canvas, [x, y]: [f32; 2], [w, h]: [f32; 2]) {
    round_rect(c, [x - 3.0, y - 3.0], [w + 6.0, h + 6.0], 5.0, BEZEL);
    round_rect(c, [x, y], [w, h], 3.0, SCREEN);
    for i in 1..8 {
        let gx = x + w * i as f32 / 8.0;
        c.line([gx, y + 1.0], [gx, y + h - 1.0], if i == 4 { DIM } else { GRID });
    }
    for i in 1..4 {
        let gy = y + h * i as f32 / 4.0;
        c.line([x + 1.0, gy], [x + w - 1.0, gy], if i == 2 { DIM } else { GRID });
    }
}

/// A small screen with a value, at the right end of the top edge of a panel of the width `w`.
pub fn readout(c: &mut Canvas, w: f32, text: &str) {
    // A character of the font is about 6.6 pixels wide.
    let tw = text.len() as f32 * 6.6 + 10.0;
    round_rect(c, [w - 16.0 - tw, 6.0], [tw, 15.0], 3.0, SCREEN);
    c.text([w - 11.0 - tw, 17.0], TRACE, text);
}

/// A bar of segments that shows `level` from 0 to 1, with marks at 0, 0.5 and 1 under it.
pub fn bar(c: &mut Canvas, [x, y]: [f32; 2], w: f32, level: f64) {
    let (gap, step) = (2.0, w / SEGMENTS as f32);
    let lit = lit(level);
    for i in 0..SEGMENTS {
        c.rect([x + i as f32 * step, y], [step - gap, 9.0], if i < lit { TRACE } else { DIM }, true);
    }
    for (i, mark) in ["0", ".5", "1"].into_iter().enumerate() {
        let mx = x + (w - gap) * i as f32 / 2.0;
        c.line([mx, y + 11.0], [mx, y + 14.0], LABEL);
        c.text([mx - 3.0 * mark.len() as f32, y + 26.0], LABEL, mark);
    }
}

/// The number of lit segments for `level`. A level that is not a number lights none.
fn lit(level: f64) -> usize {
    if level.is_nan() { 0 } else { (level.clamp(0.0, 1.0) * SEGMENTS as f64).round() as usize }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_lights_a_segment_for_each_twentieth() {
        assert_eq!([0.0, 0.336, 0.5, 1.0].map(lit), [0, 7, 10, 20]);
        assert_eq!([-2.0, 9.0, f64::NAN].map(lit), [0, 20, 0]);
    }
}
