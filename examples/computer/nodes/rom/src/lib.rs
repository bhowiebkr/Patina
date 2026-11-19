use parts::{LIT, SILK, TEXT, UNLIT, digits, edges, mix, package, window};
use patina_sdk::{Ui, node};
use std::fmt::Write;

const WORDS: usize = 256;
/// The width of the drawing of the chip and the distance from one word cell to the next.
const WIDTH: f32 = 236.0;
const CELL: f32 = 9.0;

/// A ROM of 256 words of 16 bits, as the ROM 256x16 chip of Digital Logic Sim 2. It gives the word
/// at `address`: the high byte on `out0` and the low byte on `out1`. `words` is the text of the
/// words from address 0, with spaces, commas or lines between them. A word is decimal, hexadecimal
/// with `0x` or binary with `0b`, and `_` can split its digits. An incorrect word is an error.
/// The node shows the chip with a map of the words and the address and word of the last read.
#[node(color = "#655219", free_layout)]
pub struct Rom {
    text: String,
    words: Result<[u16; WORDS], String>,
    address: u8,
}

impl Rom {
    pub fn init() -> Self {
        Rom { text: String::new(), words: Ok([0; WORDS]), address: 0 }
    }

    pub fn process(&mut self, address: &u8, words: &str) -> Result<(u8, u8), String> {
        if self.text != words {
            words.clone_into(&mut self.text);
            self.words = parse(words);
        }
        self.address = *address;
        let [high, low] = self.words.as_ref().map_err(String::clone)?[usize::from(*address)].to_be_bytes();
        Ok((high, low))
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let at = self.address;
        let word = self.words.as_ref().ok().map(|w| w[usize::from(at)]);
        let text = word.map_or("----".to_owned(), |w| format!("{w:04X}"));
        let e = edges(&[8, 0], &[8, 8]);
        let mut c = ui.canvas(&format!("ROM {at:02X}: {text}"), [WIDTH, e.height]);
        let [x0, _] = package(&mut c, &e, WIDTH);
        c.text([x0 + 18.0, 24.0], TEXT, "ROM");
        c.text([x0 + 52.0, 24.0], SILK, "256 x 16");
        // The word map: one cell for each word, brighter for a larger word, as on the RAM chip.
        let [left, top] = [x0 + 18.0, 34.0];
        window(&mut c, [left - 4.0, top - 4.0], [16.0 * CELL + 6.0; 2]);
        let mut levels: [String; 5] = Default::default();
        for (row, words) in self.words.as_ref().unwrap_or(&[0; WORDS]).chunks(16).enumerate() {
            let y = top + row as f32 * CELL;
            for (col, &w) in words.iter().enumerate() {
                let level = if w == 0 { 0 } else { 1 + usize::from(w >> 14) };
                let _ = write!(levels[level], "M {} {y} h 7 v 7 h -7 Z ", left + col as f32 * CELL);
            }
            for (level, d) in levels.iter_mut().enumerate().filter(|(_, d)| !d.is_empty()) {
                let t = if level == 0 { 0.0 } else { 0.2 + 0.2 * level as f32 };
                c.path(d, mix(UNLIT, LIT, t), true);
                d.clear();
            }
        }
        let [x, y] = [left + f32::from(at % 16) * CELL, top + f32::from(at / 16) * CELL];
        c.rect([x - 1.5, y - 1.5], [10.0, 10.0], TEXT, false);
        // The address and the word of the last read. An error shows dashes.
        window(&mut c, [left - 4.0, 194.0], [16.0 * CELL + 6.0, 28.0]);
        c.text([left + 2.0, 213.0], SILK, "A");
        digits(&mut c, [left + 14.0, 198.0], 20.0, 2, &format!("{at:02X}"));
        c.text([left + 58.0, 213.0], SILK, "D");
        digits(&mut c, [left + 70.0, 198.0], 20.0, 4, &text);
    }
}

/// Reads the words of `text`, from address 0. The other words are 0.
fn parse(text: &str) -> Result<[u16; WORDS], String> {
    let mut words = [0; WORDS];
    for (n, part) in text.split(|c: char| c.is_whitespace() || c == ',').filter(|p| !p.is_empty()).enumerate() {
        let digits = part.replace('_', "").to_ascii_lowercase();
        let word = match (digits.strip_prefix("0x"), digits.strip_prefix("0b")) {
            (Some(hex), _) => u16::from_str_radix(hex, 16),
            (_, Some(bits)) => u16::from_str_radix(bits, 2),
            _ => digits.parse(),
        };
        let word = word.map_err(|_| format!("The word {n} \"{part}\" is not a 16-bit number"))?;
        *words.get_mut(n).ok_or(format!("The ROM has {WORDS} words, and the text has more"))? = word;
    }
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_decimal_hex_and_binary_words() {
        let mut rom = Rom::init();
        let words = "258, 0x1F_02\n0b1_0000_0011";
        assert_eq!([0, 1, 2, 3].map(|a| rom.process(&a, words)), [Ok((1, 2)), Ok((0x1F, 2)), Ok((1, 3)), Ok((0, 0))]);
        assert_eq!(rom.process(&0, "1 0x10000"), Err("The word 1 \"0x10000\" is not a 16-bit number".to_owned()));
        assert!(parse(&"1 ".repeat(257)).is_err());
    }

    #[test]
    fn the_node_shows_the_chip_and_the_last_read() {
        let mut rom = Rom::init();
        let _ = rom.process(&1, "0x1234 0xABCD");
        let mut ui = Ui::new(0, "");
        rom.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tROM 01: ABCD\t"), "{}", ui.tree());
        // The editor skips a path of more than 4096 characters (4.3), also when all words are set.
        let _ = rom.process(&0, &"0xFFFF ".repeat(WORDS));
        let mut full = Ui::new(0, "");
        rom.ui(&mut full);
        assert!(full.tree().split('\t').all(|field| field.len() <= 4096));
        let _ = rom.process(&0, "x");
        let mut bad = Ui::new(0, "");
        rom.ui(&mut bad);
        assert!(bad.tree().starts_with("canvas\tROM 00: ----\t"), "{}", bad.tree());
    }
}
