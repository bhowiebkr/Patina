use patina_sdk::{Ui, node};

const COLS: usize = 20;
const ROWS: usize = 6;
const BACK: [u8; 3] = [24, 24, 23];
const TEXT: [u8; 3] = [223, 166, 75];
const CELL: [f32; 2] = [8.0, 16.0];
const MARGIN: f32 = 6.0;

/// A text screen of 6 lines of 20 characters on an IO port of the CPU. When `wr` is on and `port`
/// is `address`, it writes the ASCII character `data` at the cursor. Enter (13) or line feed (10)
/// starts a new line, and backspace (8) removes the last character. After the last line, the lines
/// move up. After each write, it writes the lines to the standard output.
#[node(color = "#59366f")]
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
        let size = [2.0 * MARGIN + COLS as f32 * CELL[0], 2.0 * MARGIN + ROWS as f32 * CELL[1]];
        let mut canvas = ui.canvas("Terminal", size);
        canvas.rect([0.0, 0.0], size, BACK, true);
        // One text for each character keeps the columns straight in any font.
        let mut one = [0u8; 1];
        for (i, &byte) in self.cells.iter().enumerate().filter(|&(_, &b)| b != b' ') {
            let at = [MARGIN + (i % COLS) as f32 * CELL[0], MARGIN + (i / COLS + 1) as f32 * CELL[1] - 4.0];
            one[0] = byte;
            canvas.text(at, TEXT, std::str::from_utf8(&one).unwrap_or("?"));
        }
        let cursor = [MARGIN + (self.cursor % COLS) as f32 * CELL[0], MARGIN + (self.cursor / COLS + 1) as f32 * CELL[1] - 3.0];
        canvas.rect(cursor, [CELL[0] - 1.0, 2.0], TEXT, true);
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
