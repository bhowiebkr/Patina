use parts::{BODY, GLASS, LIT, METAL, PIN_STEP, SILK, UNLIT, board, glass, pin_names, round_rect};
use patina_sdk::{Ui, node};

const COLS: usize = 16;
const PITCH: f32 = 2.5;
/// The size of the board. It is for all 16 x 2 cells, so the node keeps its size (4.3).
const SIZE: [f32; 2] = [316.0, 176.0];
/// The top left corner of the first cell.
const FIRST: [f32; 2] = [51.0, 66.0];
/// The inputs on the left edge, in order, and their names on the board.
const PINS: [&str; 10] = ["d0", "d1", "d2", "d3", "d4", "d5", "d6", "d7", "write", "clear"];
const NAMES: [&str; 10] = ["d0", "d1", "d2", "d3", "d4", "d5", "d6", "d7", "wr", "clr"];

/// The rows of a 5 x 7 character, top first. Bit 4 is the left column.
const FONT: &[(char, [u8; 7])] = &[
    ('0', [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E]),
    ('1', [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    ('2', [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F]),
    ('3', [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E]),
    ('4', [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02]),
    ('5', [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E]),
    ('6', [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E]),
    ('7', [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08]),
    ('8', [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E]),
    ('9', [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C]),
    ('A', [0x0E, 0x11, 0x11, 0x11, 0x1F, 0x11, 0x11]),
    ('B', [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E]),
    ('C', [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E]),
    ('D', [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C]),
    ('E', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F]),
    ('F', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10]),
    ('G', [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F]),
    ('H', [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
    ('I', [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    ('J', [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C]),
    ('K', [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
    ('L', [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F]),
    ('M', [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11]),
    ('N', [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11]),
    ('O', [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    ('P', [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10]),
    ('Q', [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D]),
    ('R', [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11]),
    ('S', [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E]),
    ('T', [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
    ('U', [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    ('V', [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04]),
    ('W', [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A]),
    ('X', [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11]),
    ('Y', [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04]),
    ('Z', [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F]),
    (' ', [0; 7]),
    ('!', [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04]),
    ('+', [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00]),
    ('-', [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00]),
    ('.', [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C]),
    (':', [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00]),
    (';', [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x04, 0x08]),
    ('<', [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02]),
    ('=', [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00]),
    ('>', [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08]),
    ('?', [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04]),
];

/// A box for a character that the font does not have.
const UNKNOWN: [u8; 7] = [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F];

/// A display of 2 lines of 16 characters, as a character LCD. When `write` is on, it puts the
/// ASCII character of bits d0 to d7 at the cursor, with d0 as the low bit. After the last cell, the
/// lines move up one line. `clear` empties the display first. A change writes the two lines to the
/// standard output.
#[node(color = "#59366f", free_layout)]
pub struct Lcd {
    cells: [u8; 2 * COLS],
    cursor: usize,
}

impl Lcd {
    pub fn init() -> Self {
        Lcd { cells: [b' '; 2 * COLS], cursor: 0 }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process(&mut self, d0: &bool, d1: &bool, d2: &bool, d3: &bool, d4: &bool, d5: &bool, d6: &bool, d7: &bool, write: &bool, clear: &bool) {
        let old = self.cells;
        if *clear {
            *self = Lcd::init();
        }
        if *write {
            let byte = [d7, d6, d5, d4, d3, d2, d1, d0].iter().fold(0u8, |n, &&b| n << 1 | u8::from(b));
            self.put(byte);
        }
        if self.cells != old {
            println!("lcd |{}|{}|", self.line(0), self.line(1));
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut c = ui.canvas(&format!("LCD {} / {}", self.line(0).trim_end(), self.line(1).trim_end()), SIZE);
            board(&mut c, SIZE);
            pin_names(&mut c, 16.0, &NAMES);
            // A bracket marks the 8 data pins.
            c.line([27.0, 10.0], [27.0, 134.0], SILK);
            // The module: a body with 4 mounting holes, and the glass with a 5 x 7 dot matrix.
            round_rect(&mut c, [34.0, 10.0], [274.0, 156.0], 4.0, BODY);
            for hole in [[43.0, 19.0], [299.0, 19.0], [43.0, 157.0], [299.0, 157.0]] {
                c.circle(hole, 4.0, METAL, true);
                c.circle(hole, 2.5, GLASS, true);
            }
            c.text([56.0, 42.0], SILK, "LCD 16 x 2");
            glass(&mut c, [44.0, 56.0], [254.0, 64.0]);
            for (i, &byte) in self.cells.iter().enumerate() {
                let x = FIRST[0] + (i % COLS) as f32 * 6.0 * PITCH;
                let y = FIRST[1] + (i / COLS) as f32 * 9.0 * PITCH;
                c.rect([x, y], [5.0 * PITCH, 8.0 * PITCH], UNLIT, true);
                for (r, bits) in glyph(byte).iter().enumerate() {
                    for col in (0..5).filter(|col| bits & (0x10 >> col) != 0) {
                        c.rect([x + col as f32 * PITCH, y + r as f32 * PITCH], [PITCH - 0.4, PITCH - 0.4], LIT, true);
                    }
                }
            }
            c.text([56.0, 142.0], SILK, &format!("cursor {}:{}", self.cursor / COLS, self.cursor % COLS));
        }
        for (i, key) in PINS.iter().enumerate() {
            ui.pin(key, [0.0, 16.0 + i as f32 * PIN_STEP]);
        }
    }

    fn put(&mut self, byte: u8) {
        if self.cursor == self.cells.len() {
            self.cells.copy_within(COLS.., 0);
            self.cells[COLS..].fill(b' ');
            self.cursor = COLS;
        }
        self.cells[self.cursor] = byte;
        self.cursor += 1;
    }

    fn line(&self, row: usize) -> String {
        self.cells[row * COLS..(row + 1) * COLS].iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '?' }).collect()
    }
}

/// The dots of a character. A lowercase letter shows as its uppercase letter.
fn glyph(byte: u8) -> [u8; 7] {
    let c = byte.to_ascii_uppercase() as char;
    FONT.iter().find(|(f, _)| *f == c).map_or(UNKNOWN, |(_, rows)| *rows)
}
