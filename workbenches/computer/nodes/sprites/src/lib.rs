use frame::{Frame, PALETTE, SIZE};
use parts::{BOARD, EDGE, LIT, SILK, TEXT, UNLIT, digits, edges, package, round_rect, window};
use patina_sdk::{Ui, node};
use std::fmt::Write;

/// The width of the drawing of the chip.
const WIDTH: f32 = 252.0;

/// The number of sprites.
const COUNT: usize = 8;

/// One sprite: a pattern of 8 x 8 pixels in one color.
#[derive(Clone, Copy)]
struct Sprite {
    x: u8,
    y: u8,
    color: u8,
    rows: [u8; 8],
}

/// A sprite chip on the IO ports of the CPU. It draws 8 sprites of 8 x 8 pixels on the frame of
/// the gpu, and gives the new frame to the screen. When `wr` is on, the CPU writes `data` to the
/// register at `port`:
///
/// - `address`: select. The next writes go to sprite `data`, 0 to 7, and to row 0 of its pattern.
/// - `address + 1`: X of the left column. At 32 or more, the sprite is off the screen.
/// - `address + 2`: Y of the top row.
/// - `address + 3`: color, 1 to 15. Color 0 hides the sprite.
/// - `address + 4`: pattern. The next row gets `data`, and bit 7 is the left pixel. After row 7,
///   row 0 is next.
///
/// A new sprite has one pixel at its top left corner, so a program moves a dot with two writes:
/// X and Y. Sprite 0 is on top. The sprites do not change the frame of the gpu, so a read of a
/// pixel at the gpu does not see them.
#[node(color = "#59366f", free_layout)]
pub struct Sprites {
    sprites: [Sprite; COUNT],
    selected: usize,
    row: usize,
}

impl Sprites {
    pub fn init() -> Self {
        let dot = Sprite { x: 0, y: 0, color: 0, rows: [0x80, 0, 0, 0, 0, 0, 0, 0] };
        Sprites { sprites: [dot; COUNT], selected: 0, row: 0 }
    }

    pub fn process(&mut self, frame: &Frame, data: &u8, port: &u8, wr: &bool, address: u8) -> Frame {
        if *wr {
            let sprite = &mut self.sprites[self.selected];
            match port.wrapping_sub(address) {
                0 => (self.selected, self.row) = (usize::from(*data) % COUNT, 0),
                1 => sprite.x = *data,
                2 => sprite.y = *data,
                3 => sprite.color = data % 16,
                4 => {
                    sprite.rows[self.row] = *data;
                    self.row = (self.row + 1) % 8;
                }
                _ => {}
            }
        }
        let mut out = frame.clone();
        for sprite in self.sprites.iter().rev().filter(|s| s.color != 0) {
            for (dy, bits) in sprite.rows.iter().enumerate() {
                let y = usize::from(sprite.y) + dy;
                for dx in 0..8 {
                    let x = usize::from(sprite.x) + dx;
                    if bits << dx & 0x80 != 0 && x < SIZE && y < SIZE {
                        out.0[y * SIZE + x] = sprite.color;
                    }
                }
            }
        }
        out
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let shown = self.sprites.iter().filter(|s| s.color != 0).count();
        let e = edges(&[0, 8, 8, 0, 8], &[0]);
        let mut c = ui.canvas(&format!("Sprite {}, {shown} of {COUNT} shown", self.selected), [WIDTH, e.height]);
        let [x0, _] = package(&mut c, &e, WIDTH);
        c.text([x0 + 18.0, 24.0], TEXT, "SPRITES");
        c.text([x0 + 96.0, 24.0], SILK, "8 x 8 x 8");
        // The patterns of the sprites in their colors. A hidden sprite is dark, and the selected one has a frame.
        window(&mut c, [x0 + 14.0, 34.0], [168.0, 86.0]);
        let mut d = String::new();
        for (i, sprite) in self.sprites.iter().enumerate() {
            let at = [x0 + 21.0 + (i % 4) as f32 * 41.0, 40.0 + (i / 4) as f32 * 41.0];
            c.rect([at[0] - 1.0, at[1] - 1.0], [34.0, 34.0], BOARD, true);
            pattern(&mut d, &sprite.rows, at, 4.0);
            c.path(&d, if sprite.color == 0 { UNLIT } else { PALETTE[usize::from(sprite.color % 16)] }, true);
            if i == self.selected {
                c.rect([at[0] - 3.0, at[1] - 3.0], [38.0, 38.0], LIT, false);
            }
        }
        // The selected sprite: its place, its pattern with the next row, and its color.
        let s = self.sprites[self.selected];
        window(&mut c, [x0 + 14.0, 134.0], [168.0, 36.0]);
        c.text([x0 + 22.0, 157.0], SILK, "#");
        digits(&mut c, [x0 + 34.0, 140.0], 24.0, 1, &self.selected.to_string());
        c.text([x0 + 62.0, 157.0], SILK, "X");
        digits(&mut c, [x0 + 74.0, 140.0], 24.0, 2, &format!("{:02X}", s.x));
        c.text([x0 + 118.0, 157.0], SILK, "Y");
        digits(&mut c, [x0 + 130.0, 140.0], 24.0, 2, &format!("{:02X}", s.y));
        window(&mut c, [x0 + 14.0, 184.0], [88.0, 88.0]);
        pattern(&mut d, &s.rows.map(|r| !r), [x0 + 18.0, 188.0], 10.0);
        c.path(&d, UNLIT, true);
        pattern(&mut d, &s.rows, [x0 + 18.0, 188.0], 10.0);
        c.path(&d, if s.color == 0 { SILK } else { PALETTE[usize::from(s.color % 16)] }, true);
        let y = 192.0 + self.row as f32 * 10.0;
        c.path(&format!("M {} {y} l 7 -5 v 10 Z", x0 + 106.0), LIT, true);
        round_rect(&mut c, [x0 + 116.0, 184.0], [66.0, 28.0], 3.0, PALETTE[usize::from(s.color % 16)]);
        c.rect([x0 + 116.0, 184.0], [66.0, 28.0], EDGE, false);
        c.text([x0 + 116.0, 232.0], SILK, format_args!("COLOR {}", s.color));
        c.text([x0 + 116.0, 250.0], SILK, format_args!("ROW {}", self.row));
        c.text([x0 + 116.0, 268.0], SILK, format_args!("{shown}/{COUNT} ON"));
        c.text([x0 + 14.0, 302.0], SILK, "REGISTERS");
        for (i, name) in ["SEL", "X", "Y", "COLOR", "ROW"].into_iter().enumerate() {
            let at = [x0 + 14.0 + (i / 3) as f32 * 90.0, 322.0 + (i % 3) as f32 * 18.0];
            c.text(at, SILK, format_args!("+{i}"));
            c.text([at[0] + 26.0, at[1]], TEXT, name);
        }
    }
}

/// Writes the path of the pixels of a pattern, bit 7 at the left, at `at` with a pitch of `dot`.
fn pattern(d: &mut String, rows: &[u8; 8], [x, y]: [f32; 2], dot: f32) {
    d.clear();
    let size = dot - 1.0;
    for (r, bits) in rows.iter().enumerate() {
        for col in 0..8 {
            if bits << col & 0x80 != 0 {
                let _ = write!(d, "M {} {} h {size} v {size} h -{size} Z ", x + col as f32 * dot, y + r as f32 * dot);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(chip: &mut Sprites, port: u8, data: u8) -> Frame {
        chip.process(&Frame::default(), &data, &port, &true, 9)
    }

    #[test]
    fn a_sprite_moves_with_two_writes_and_sprite_0_is_on_top() {
        let mut chip = Sprites::init();
        // A new sprite is hidden. With a color, it is one dot at X and Y.
        assert_eq!(write(&mut chip, 0, 0), Frame::default());
        write(&mut chip, 12, 14);
        write(&mut chip, 10, 5);
        let frame = write(&mut chip, 11, 3);
        assert_eq!(frame.0[3 * SIZE + 5], 14);
        assert_eq!(frame.0.iter().filter(|&&c| c != 0).count(), 1);
        // Sprite 1 gets a pattern of two rows at the same place. Sprite 0 stays on top.
        write(&mut chip, 9, 1);
        write(&mut chip, 13, 0b1100_0000);
        write(&mut chip, 13, 0b0100_0000);
        write(&mut chip, 10, 5);
        write(&mut chip, 11, 3);
        let frame = write(&mut chip, 12, 4);
        assert_eq!([frame.0[3 * SIZE + 5], frame.0[3 * SIZE + 6], frame.0[4 * SIZE + 6], frame.0[4 * SIZE + 5]], [14, 4, 4, 0]);
        // Pixels past the right edge do not show. X of 32 or more hides the sprite.
        write(&mut chip, 10, 31);
        assert_eq!(write(&mut chip, 0, 0).0.iter().filter(|&&c| c == 4).count(), 1);
        let frame = write(&mut chip, 10, 32);
        assert_eq!(frame.0.iter().filter(|&&c| c == 4).count(), 0);
        // The input frame shows under the sprites.
        let mut under = Frame::default();
        under.0[0] = 2;
        assert_eq!(chip.process(&under, &0, &0, &false, 9).0[0], 2);
    }
}
