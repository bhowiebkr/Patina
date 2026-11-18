//! The drawing parts of the hello example, so that its node GUIs have one style (spec 4.3): the
//! case of a pocket calculator with a gray-green LCD and slanted 7-segment digits.
use patina_sdk::Canvas;
use std::fmt::Write;

/// The plastic case.
pub const CASE: [u8; 3] = [44, 45, 48];
/// The edge line of the case.
pub const RIM: [u8; 3] = [92, 94, 99];
/// The printed text on the case.
pub const PRINT: [u8; 3] = [170, 172, 178];
/// The glass of the LCD.
pub const LCD: [u8; 3] = [168, 182, 150];
/// The frame around the LCD.
pub const BEZEL: [u8; 3] = [24, 25, 27];
/// A dark segment of the LCD.
pub const INK: [u8; 3] = [28, 34, 26];
/// A segment that is off. It shows a little, as on a real LCD.
const GHOST: [u8; 3] = [154, 168, 137];
/// The cells of the solar panel.
const SOLAR: [u8; 3] = [58, 40, 34];
/// The cap of a key.
pub const KEY: [u8; 3] = [74, 76, 82];
/// The cap of an orange key, for clear.
pub const ORANGE: [u8; 3] = [200, 106, 46];
/// The dark edge under a key, so that the key stands up from the case.
const KEY_EDGE: [u8; 3] = [18, 19, 21];
/// The text on a key.
const KEY_TEXT: [u8; 3] = [236, 237, 240];

/// The segments a to g of the digits 0 to 9, with segment a in bit 0.
const DIGITS: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];
/// The decimal point bit of a cell.
const POINT: u8 = 0x80;
/// The slant of the digits: the move to the right for each pixel up.
const SLANT: f32 = 0.12;

/// Fills a rectangle with round corners.
pub fn round_rect(c: &mut Canvas, [x, y]: [f32; 2], [w, h]: [f32; 2], r: f32, color: [u8; 3]) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let (x2, y2) = (x + w, y + h);
    let mut d = String::new();
    let _ = write!(d, "M {} {y} H {} Q {x2} {y} {x2} {} V {} Q {x2} {y2} {} {y2} ", x + r, x2 - r, y + r, y2 - r, x2 - r);
    let _ = write!(d, "H {} Q {x} {y2} {x} {} V {} Q {x} {y} {} {y} Z", x + r, y2 - r, y + r, x + r);
    c.path(&d, color, true);
}

/// The case of a calculator with an edge line and round corners, and its brand name.
pub fn case(c: &mut Canvas, [w, h]: [f32; 2], name: &str) {
    round_rect(c, [0.0, 0.0], [w, h], 10.0, RIM);
    round_rect(c, [1.0, 1.0], [w - 2.0, h - 2.0], 9.0, CASE);
    c.text([12.0, 18.0], PRINT, name);
}

/// A solar panel of four cells, with its top right corner at `at`.
pub fn solar(c: &mut Canvas, [x, y]: [f32; 2]) {
    round_rect(c, [x - 50.0, y], [50.0, 12.0], 2.0, BEZEL);
    for i in 0..4 {
        c.rect([x - 48.0 + i as f32 * 12.0, y + 2.0], [10.0, 8.0], SOLAR, true);
    }
}

/// An LCD with a bezel that shows `value` on `count` digits, at the right end.
pub fn lcd(c: &mut Canvas, at: [f32; 2], count: usize, value: f64) {
    lcd_text(c, at, count, &fit(value, count));
}

/// An LCD with a bezel that shows `text` on `count` digits, at the right end, for example a
/// number that the user types.
pub fn lcd_text(c: &mut Canvas, [x, y]: [f32; 2], count: usize, text: &str) {
    let h = 22.0;
    let size = [count as f32 * step(h) + 16.0, h + 14.0];
    round_rect(c, [x - 3.0, y - 3.0], [size[0] + 6.0, size[1] + 6.0], 5.0, BEZEL);
    round_rect(c, [x, y], size, 3.0, LCD);
    for (i, bits) in cells(text, count).into_iter().enumerate() {
        digit(c, [x + 9.0 + i as f32 * step(h), y + 7.0], h, bits);
    }
}

/// A key at `at` of `size`, with `label` in the middle. A key that is down sits lower, so a
/// selected key shows as held.
pub fn key(c: &mut Canvas, [x, y]: [f32; 2], [w, h]: [f32; 2], label: &str, cap: [u8; 3], down: bool) {
    round_rect(c, [x, y + 2.0], [w, h - 2.0], 4.0, KEY_EDGE);
    let top = if down { y + 2.0 } else { y };
    round_rect(c, [x, top], [w, h - 2.0], 4.0, cap);
    // A label character is about 6.6 pixels wide at the 12 pixel font of the canvas.
    c.text([x + w / 2.0 - label.len() as f32 * 3.3, top + h / 2.0 + 3.0], KEY_TEXT, label);
}

/// The distance from one digit of the height `h` to the next.
fn step(h: f32) -> f32 {
    h * 0.7
}

/// A slanted 7-segment digit of the height `h`, with segment a in bit 0 and the point in [`POINT`].
fn digit(c: &mut Canvas, [x, y]: [f32; 2], h: f32, bits: u8) {
    let (w, t) = (h * 0.42, h * 0.1);
    // Each segment as a line from one corner of the digit to another, in units of the digit.
    let lines = [([0.0, 0.0], [1.0, 0.0]), ([1.0, 0.0], [1.0, 0.5]), ([1.0, 0.5], [1.0, 1.0]), ([0.0, 1.0], [1.0, 1.0]), ([0.0, 0.5], [0.0, 1.0]), ([0.0, 0.0], [0.0, 0.5]), ([0.0, 0.5], [1.0, 0.5])];
    let at = |[u, v]: [f32; 2], dx: f32, dy: f32| {
        let py = y + v * h + dy;
        (x + u * w + dx + (y + h - py) * SLANT, py)
    };
    let mut d = String::new();
    for (i, (a, b)) in lines.into_iter().enumerate() {
        let (gap, k) = (t * 0.8, t / 2.0);
        let ((ax, ay), (bx, by), (cx, cy), (ex, ey)) = if a[1] == b[1] {
            (at(a, gap, -k), at(b, -gap, -k), at(b, -gap, k), at(a, gap, k))
        } else {
            (at(a, -k, gap), at(a, k, gap), at(b, k, -gap), at(b, -k, -gap))
        };
        d.clear();
        let _ = write!(d, "M {ax:.1} {ay:.1} L {bx:.1} {by:.1} L {cx:.1} {cy:.1} L {ex:.1} {ey:.1} Z");
        c.path(&d, if bits >> i & 1 == 1 { INK } else { GHOST }, true);
    }
    // The point sits in the middle of the gap to the next digit, so it does not touch a segment.
    let (px, py) = at([1.0, 1.0], (step(h) - w) / 2.0, 0.0);
    c.circle([px, py - t / 2.0], t * 0.7, if bits & POINT != 0 { INK } else { GHOST }, true);
}

/// The text of `value` for `count` digits: as Rust writes it, else rounded, else in exponent form.
/// A value that is not finite, or that does not fit, shows `Err` as on a calculator.
pub fn fit(value: f64, count: usize) -> String {
    let long = |s: &str| s.chars().filter(|&ch| ch != '.').count() > count;
    if !value.is_finite() {
        return "Err".to_owned();
    }
    let plain = format!("{value}");
    if !long(&plain) {
        return plain;
    }
    for places in (0..count).rev() {
        let fixed = format!("{value:.places$}");
        let fixed = if places > 0 { fixed.trim_end_matches('0').trim_end_matches('.') } else { &fixed };
        if !long(fixed) && !fixed.trim_start_matches(['-', '0', '.']).is_empty() {
            return fixed.to_owned();
        }
    }
    let exponent = |places: usize| {
        let text = format!("{value:.places$e}");
        let (mantissa, power) = text.split_once('e').unwrap_or((&text, "0"));
        let mantissa = if places > 0 { mantissa.trim_end_matches('0').trim_end_matches('.') } else { mantissa };
        format!("{mantissa}e{power}")
    };
    (0..count).rev().map(exponent).find(|s| !long(s)).unwrap_or_else(|| "Err".to_owned())
}

/// The segments of each of `count` cells that show `text` at the right end. A point lights the
/// point of the cell before it.
fn cells(text: &str, count: usize) -> Vec<u8> {
    let mut cells = vec![0; count];
    let (mut n, mut point) = (0, false);
    for ch in text.chars().rev() {
        if n == count {
            break;
        }
        if ch == '.' {
            point = true;
            continue;
        }
        let bits = match ch {
            '-' => 0x40,
            'E' | 'e' => 0x79,
            'r' => 0x50,
            _ => ch.to_digit(10).map_or(0, |d| DIGITS[d as usize]),
        };
        cells[count - 1 - n] = bits | if point { POINT } else { 0 };
        (n, point) = (n + 1, false);
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_fits_the_digits_of_the_lcd() {
        assert_eq!(fit(6.5, 8), "6.5");
        assert_eq!(fit(0.1 + 0.2, 8), "0.3");
        assert_eq!(fit(-1234.5678, 6), "-1234.6");
        assert_eq!(fit(1e21, 8), "1e21");
        assert_eq!(fit(1e-9, 8), "1e-9");
        assert_eq!(fit(f64::NAN, 8), "Err");
        assert_eq!(cells("-2.5", 4), [0, 0x40, DIGITS[2] | POINT, DIGITS[5]]);
        assert_eq!(cells("Err", 3), [0x79, 0x50, 0x50]);
    }
}
