//! The drawing parts of the example devices, so that each node GUI has one style (spec 4.3): a
//! board with silk-screen text, LEDs, 7-segment digits and the glass of a display.

use patina_sdk::Canvas;
use std::fmt::Write;

/// The board of a device.
pub const BOARD: [u8; 3] = [36, 36, 34];
/// The edge line of a board.
pub const EDGE: [u8; 3] = [88, 88, 83];
/// The glass of a display.
pub const GLASS: [u8; 3] = [16, 16, 15];
/// The text of a value on a board.
pub const TEXT: [u8; 3] = [238, 238, 235];
/// The silk-screen text and marks on a board.
pub const SILK: [u8; 3] = [178, 178, 173];
/// A lit LED, segment or dot, in the accent color of the theme.
pub const LIT: [u8; 3] = [223, 166, 75];
/// An LED that is off.
pub const DARK: [u8; 3] = [72, 57, 33];
/// A segment or dot that is off. It shows a little through the glass, as on a real display.
pub const UNLIT: [u8; 3] = [42, 35, 23];
/// The metal of a switch, a button or a key.
pub const METAL: [u8; 3] = [168, 168, 162];
/// The plastic body of a switch or a key.
pub const BODY: [u8; 3] = [60, 60, 57];
/// The black body of a chip.
pub const CHIP: [u8; 3] = [24, 24, 23];
/// The halo of a lit LED.
const HALO: [u8; 3] = [96, 75, 41];
/// The highlight of a lit LED.
const SHINE: [u8; 3] = [252, 228, 176];
/// The gold of a finger on the edge of a board.
pub const GOLD: [u8; 3] = [186, 150, 84];
/// The copper of a trace on a board.
pub const COPPER: [u8; 3] = [112, 92, 58];
/// The edge line of a chip body.
const LID: [u8; 3] = [62, 62, 58];
/// The rim of a window on a chip body.
const RIM: [u8; 3] = [70, 70, 66];

// The rows of the socket columns beside a drawing, as the editor shows them (NodeView.tsx, BusSocket.tsx).
const ROW: f32 = 24.0;
const SOCKET: f32 = 22.0;
const FIRST: f32 = 24.0;
const CONTACT: f32 = 12.0;
/// The space from an edge with pins to the chip body: the finger, the trace and the leg.
const LEGS: f32 = 28.0;

/// The distance between two pins on the edge of a board.
pub const PIN_STEP: f32 = 16.0;

/// The segments a to g of the hex digits 0 to F, with segment a in bit 0.
pub const HEX: [u8; 16] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F, 0x77, 0x7C, 0x39, 0x5E, 0x79, 0x71];
/// The decimal point bit of a digit.
pub const POINT: u8 = 0x80;

/// Fills a rectangle with round corners.
pub fn round_rect(c: &mut Canvas, [x, y]: [f32; 2], [w, h]: [f32; 2], r: f32, color: [u8; 3]) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let (x2, y2) = (x + w, y + h);
    let d = format!(
        "M {} {y} H {} Q {x2} {y} {x2} {} V {} Q {x2} {y2} {} {y2} H {} Q {x} {y2} {x} {} V {} Q {x} {y} {} {y} Z",
        x + r,
        x2 - r,
        y + r,
        y2 - r,
        x2 - r,
        x + r,
        y2 - r,
        y + r,
        x + r
    );
    c.path(&d, color, true);
}

/// A device board with an edge line and round corners.
pub fn board(c: &mut Canvas, [w, h]: [f32; 2]) {
    round_rect(c, [0.0, 0.0], [w, h], 6.0, EDGE);
    round_rect(c, [1.0, 1.0], [w - 2.0, h - 2.0], 5.0, BOARD);
}

/// The dark glass of a display.
pub fn glass(c: &mut Canvas, at: [f32; 2], size: [f32; 2]) {
    round_rect(c, at, size, 3.0, GLASS);
}

/// The color at `t` of the way from `a` to `b`, with `t` from 0 to 1.
pub fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    std::array::from_fn(|i| (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t).round() as u8)
}

/// A window with a rim on a chip body, for digits, pixels or text.
pub fn window(c: &mut Canvas, [x, y]: [f32; 2], [w, h]: [f32; 2]) {
    round_rect(c, [x - 2.0, y - 2.0], [w + 4.0, h + 4.0], 5.0, RIM);
    round_rect(c, [x, y], [w, h], 3.0, GLASS);
}

/// The y of each contact on the left and the right edge of a drawing between socket columns, and
/// the height of the rows.
pub struct Edges {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub height: f32,
}

/// The edges of a drawing between socket columns (4.3). `left` and `right` give the contacts of
/// each pin in order: the lanes of a bus pin, or 0 for a pin with a label and no lanes.
pub fn edges(left: &[usize], right: &[usize]) -> Edges {
    let mut e = Edges { left: Vec::new(), right: Vec::new(), height: 0.0 };
    for i in 0..left.len().max(right.len()) {
        let (top, mut row) = (e.height, ROW);
        for (pins, ys) in [(left, &mut e.left), (right, &mut e.right)] {
            match pins.get(i) {
                Some(0) => ys.push(top + ROW / 2.0),
                Some(&n) => {
                    ys.extend((0..n).map(|j| top + FIRST + j as f32 * CONTACT));
                    row = row.max(SOCKET + n as f32 * CONTACT);
                }
                None => {}
            }
        }
        e.height += row;
    }
    e
}

/// A chip package as large as a drawing between socket columns, `w` wide and as tall as the rows.
/// Each contact of the edges gets a gold finger, a copper trace and a leg into the body, so a
/// lead of a socket goes on into the chip. It gives the left and the right end of the body.
pub fn package(c: &mut Canvas, e: &Edges, w: f32) -> [f32; 2] {
    let h = e.height;
    board(c, [w, h]);
    let x0 = if e.left.is_empty() { 4.0 } else { LEGS };
    let x1 = if e.right.is_empty() { w - 4.0 } else { w - LEGS };
    for &y in &e.left {
        c.rect([0.0, y - 3.0], [8.0, 6.0], GOLD, true);
        c.line([8.0, y], [x0 - 6.0, y], COPPER);
        c.rect([x0 - 6.0, y - 2.5], [6.0, 5.0], METAL, true);
    }
    for &y in &e.right {
        c.rect([w - 8.0, y - 3.0], [8.0, 6.0], GOLD, true);
        c.line([x1 + 6.0, y], [w - 8.0, y], COPPER);
        c.rect([x1, y - 2.5], [6.0, 5.0], METAL, true);
    }
    round_rect(c, [x0, 4.0], [x1 - x0, h - 8.0], 4.0, LID);
    round_rect(c, [x0 + 1.5, 5.5], [x1 - x0 - 3.0, h - 11.0], 3.0, CHIP);
    // The notch at the top end and the dot at pin 1.
    let mid = (x0 + x1) / 2.0;
    c.path(&format!("M {} 4 A 7 7 0 0 0 {} 4 Z", mid - 7.0, mid + 7.0), BOARD, true);
    c.circle([x0 + 10.0, 15.0], 2.5, SILK, true);
    [x0, x1]
}

/// The names of the pins on the left edge of a board, from `top` down in steps of [`PIN_STEP`].
pub fn pin_names(c: &mut Canvas, top: f32, names: &[&str]) {
    for (i, name) in names.iter().enumerate() {
        c.text([7.0, top + i as f32 * PIN_STEP + 4.0], SILK, name);
    }
}

/// A round LED. A lit LED has a halo and a highlight.
pub fn led(c: &mut Canvas, [x, y]: [f32; 2], r: f32, on: bool) {
    if on {
        c.circle([x, y], r + 2.5, HALO, true);
        c.circle([x, y], r, LIT, true);
        c.circle([x - r * 0.35, y - r * 0.35], r * 0.3, SHINE, true);
    } else {
        c.circle([x, y], r, DARK, true);
    }
}

/// The distance from one digit of the height `h` to the next.
pub fn digit_step(h: f32) -> f32 {
    h * 0.75
}

/// A 7-segment digit of the height `h`, with segment a in bit 0 of `bits` and the decimal point in
/// [`POINT`]. The segments are hexagons, as on a real display.
pub fn digit(c: &mut Canvas, [x, y]: [f32; 2], h: f32, bits: u8) {
    let (w, t) = (h * 0.55, h * 0.13);
    let (left, right) = (x + t / 2.0, x + w - t / 2.0);
    let (top, mid, bottom) = (y + t / 2.0, y + h / 2.0, y + h - t / 2.0);
    // Each segment: flat or not, the start and the end along the segment, and the line it is on.
    let segments = [
        (true, left, right, top),
        (false, top, mid, right),
        (false, mid, bottom, right),
        (true, left, right, bottom),
        (false, mid, bottom, left),
        (false, top, mid, left),
        (true, left, right, mid),
    ];
    let (gap, k) = (t * 0.3, t / 2.0);
    let mut d = String::new();
    for (i, &(flat, a, b, at)) in segments.iter().enumerate() {
        let (a, b) = (a + gap, b - gap);
        d.clear();
        let _ = if flat {
            write!(d, "M {a:.1} {at:.1} L {:.1} {:.1} H {:.1} L {b:.1} {at:.1} L {:.1} {:.1} H {:.1} Z", a + k, at - k, b - k, b - k, at + k, a + k)
        } else {
            write!(d, "M {at:.1} {a:.1} L {:.1} {:.1} V {:.1} L {at:.1} {b:.1} L {:.1} {:.1} V {:.1} Z", at + k, a + k, b - k, at - k, b - k, a + k)
        };
        c.path(&d, if bits >> i & 1 == 1 { LIT } else { UNLIT }, true);
    }
    c.circle([x + w + t * 0.7, bottom], t * 0.55, if bits & POINT != 0 { LIT } else { UNLIT }, true);
}

/// Shows `text` on `count` digits, at the right end. A point lights the decimal point of the
/// digit before it. A text that does not fit shows dashes.
pub fn digits(c: &mut Canvas, [x, y]: [f32; 2], h: f32, count: usize, text: &str) {
    for (i, bits) in segments(text, count).into_iter().enumerate() {
        digit(c, [x + i as f32 * digit_step(h), y], h, bits);
    }
}

/// The segments of each of `count` digits that show `text` at the right end, as [`digits`] draws them.
pub fn segments(text: &str, count: usize) -> Vec<u8> {
    let mut cells = vec![0; count];
    let (mut n, mut point) = (0, false);
    for ch in text.chars().rev() {
        if ch == '.' {
            point = true;
            continue;
        }
        if n == count {
            cells.fill(0x40);
            break;
        }
        cells[count - 1 - n] = glyph(ch) | if point { POINT } else { 0 };
        (n, point) = (n + 1, false);
    }
    cells
}

/// The segments of a character: the hex digits, a minus sign and the letters of `inf` and `NaN`.
/// Another character is blank.
fn glyph(ch: char) -> u8 {
    match ch {
        '-' => 0x40,
        'n' | 'N' => 0x54,
        'i' | 'I' => 0x04,
        _ => ch.to_digit(16).map_or(0, |d| HEX[d as usize]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_goes_to_the_right_digits() {
        assert_eq!(segments("A5", 2), [HEX[10], HEX[5]]);
        assert_eq!(segments("-2.5", 4), [0, 0x40, HEX[2] | POINT, HEX[5]]);
        assert_eq!(segments("inf", 3), [0x04, 0x54, HEX[15]]);
        assert_eq!(segments("12345", 4), [0x40; 4], "a long text shows dashes");
        assert_eq!(segments("", 1), [0]);
    }

    #[test]
    fn edges_follow_the_rows_of_the_socket_columns() {
        // The RAM: address, data and write on the left, out on the right.
        let e = edges(&[8, 8, 0], &[8]);
        assert_eq!(e.height, 260.0);
        assert_eq!((e.left.len(), e.left[0], e.left[8], e.left[16]), (17, 24.0, 142.0, 248.0));
        assert_eq!((e.right.len(), e.right[7]), (8, 108.0));
    }
}
