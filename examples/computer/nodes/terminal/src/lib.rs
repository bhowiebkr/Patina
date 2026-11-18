use parts::{BODY, LIT, PIN_STEP, SILK, board, glass, pin_names, round_rect};
use patina_sdk::{Ui, node};

const COLS: usize = 20;
const ROWS: usize = 6;
const CELL: [f32; 2] = [8.0, 16.0];
/// The size of the board.
const SIZE: [f32; 2] = [228.0, 140.0];
/// The top left corner of the glass, and the margin of the text on the glass.
const SCREEN: [f32; 2] = [44.0, 10.0];
const MARGIN: f32 = 6.0;
/// The inputs on the left edge, in order.
const PINS: [&str; 4] = ["data", "port", "wr", "address"];

/// A text screen of 6 lines of 20 characters on an IO port of the CPU. When `wr` is on and `port`
/// is `address`, it writes the ASCII character `data` at the cursor. Enter (13) or line feed (10)
/// starts a new line, and backspace (8) removes the last character. After the last line, the lines
/// move up. After each write, it writes the lines to the standard output.
#[node(color = "#59366f", free_layout)]
pub struct Terminal {
    cells: [u8; COLS * ROWS],
    cursor: usize,
}

impl Terminal {
    pub fn init() -> Self {
        Terminal { cells: [b' '; COLS * ROWS], cursor: 0 }
    }

    pub fn process(&mut self, data: &u8, port: &u8, wr: &bool, address: u8) {
        if *wr && *port == address {
            self.put(*data);
            let lines: Vec<&str> = self.cells.chunks(COLS).map(|row| std::str::from_utf8(row).unwrap_or("")).collect();
            println!("term |{}|", lines.join("|"));
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut c = ui.canvas("Terminal", SIZE);
            board(&mut c, SIZE);
            pin_names(&mut c, 16.0, &["data", "port", "wr", "adr"]);
            let size = [2.0 * MARGIN + COLS as f32 * CELL[0], 2.0 * MARGIN + ROWS as f32 * CELL[1]];
            round_rect(&mut c, [SCREEN[0] - 4.0, SCREEN[1] - 4.0], [size[0] + 8.0, size[1] + 8.0], 5.0, BODY);
            glass(&mut c, SCREEN, size);
            let cell = |i: usize, dy: f32| [SCREEN[0] + MARGIN + (i % COLS) as f32 * CELL[0], SCREEN[1] + MARGIN + (i / COLS + 1) as f32 * CELL[1] - dy];
            // One text for each character keeps the columns straight in any font.
            let mut one = [0u8; 1];
            for (i, &byte) in self.cells.iter().enumerate().filter(|&(_, &b)| b != b' ') {
                one[0] = byte;
                c.text(cell(i, 4.0), LIT, std::str::from_utf8(&one).unwrap_or("?"));
            }
            c.rect(cell(self.cursor, 3.0), [CELL[0] - 1.0, 2.0], LIT, true);
            c.text([SCREEN[0], 134.0], SILK, "TTY 20 x 6");
            c.text([144.0, 134.0], SILK, &format!("row {} col {}", self.cursor / COLS, self.cursor % COLS));
        }
        for (i, key) in PINS.iter().enumerate() {
            ui.pin(key, [0.0, 16.0 + i as f32 * PIN_STEP]);
        }
    }

    fn put(&mut self, byte: u8) {
        match byte {
            8 => {
                if !self.cursor.is_multiple_of(COLS) {
                    self.cursor -= 1;
                    self.cells[self.cursor] = b' ';
                }
            }
            10 | 13 => self.cursor = (self.cursor / COLS + 1) * COLS,
            _ => {
                self.scroll();
                self.cells[self.cursor] = if (32..127).contains(&byte) { byte } else { b'?' };
                self.cursor += 1;
            }
        }
        // The cursor stays on the screen, so a new line at the bottom moves the lines up at once.
        self.scroll();
    }

    /// Moves the lines up one line when the cursor is after the last cell.
    fn scroll(&mut self) {
        if self.cursor == self.cells.len() {
            self.cells.copy_within(COLS.., 0);
            self.cells[COLS * (ROWS - 1)..].fill(b' ');
            self.cursor = COLS * (ROWS - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(term: &mut Terminal, text: &[u8]) {
        for byte in text {
            term.process(byte, &8, &true, 8);
        }
    }

    #[test]
    fn enter_backspace_and_the_scroll_after_the_last_line() {
        let mut term = Terminal::init();
        write(&mut term, b"ABX\x08\rC\x01");
        // A write to another port does nothing.
        term.process(&b'Z', &9, &true, 8);
        assert_eq!(&term.cells[..3], b"AB ");
        assert_eq!(&term.cells[COLS..COLS + 3], b"C? ");
        write(&mut term, b"\r\r\r\r\rD");
        assert_eq!(&term.cells[..2], b"C?");
        assert_eq!(&term.cells[COLS * (ROWS - 1)..COLS * (ROWS - 1) + 2], b"D ");
        assert_eq!(term.cursor, COLS * (ROWS - 1) + 1);
    }
}
