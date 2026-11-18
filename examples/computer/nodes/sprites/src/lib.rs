use frame::{Frame, SIZE};
use parts::{PIN_STEP, SILK, board, chip, pin_names};
use patina_sdk::{Ui, node};

/// The size of the board.
const BOARD: [f32; 2] = [212.0, 96.0];
/// The inputs on the left edge, in order.
const PINS: [&str; 5] = ["frame", "data", "port", "wr", "address"];

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
        {
            let mut c = ui.canvas(&format!("Sprite {}, {shown} of {COUNT} shown", self.selected), BOARD);
            board(&mut c, BOARD);
            pin_names(&mut c, 16.0, &["frame", "data", "port", "wr", "adr"]);
            chip(&mut c, [48.0, 14.0], [120.0, 68.0], "SPRITES");
            c.text([64.0, 58.0], SILK, &format!("#{}  {shown}/{COUNT} on", self.selected));
            c.text([176.0, 52.0], SILK, "frame");
        }
        for (i, key) in PINS.iter().enumerate() {
            ui.pin(key, [0.0, 16.0 + i as f32 * PIN_STEP]);
        }
        ui.pin("out", [BOARD[0], 48.0]);
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
