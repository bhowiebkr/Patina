use parts::{BODY, GLASS, LIT, SILK, TEXT, digits, edges, package, round_rect, window};
use patina_sdk::{Ui, node};
use std::collections::VecDeque;

/// The number of keys that the buffer keeps. A real keyboard chip has a small buffer too.
const BUFFER: usize = 64;
/// The ASCII code of the Enter key.
const ENTER: u8 = 13;
/// The width of the drawing of the chip.
const WIDTH: f32 = 260.0;
/// The rows of keys on the face of the chip, with the indent of each row.
const KEYS: [(&str, f32); 4] = [("1234567890", 0.0), ("QWERTYUIOP", 0.0), ("ASDFGHJKL", 9.0), ("ZXCVBNM", 18.0)];
/// The number of waiting keys that the glass shows.
const SHOWN: usize = 19;

/// A keyboard on an IO port of the CPU, with a key buffer. Type a text and press Enter: the
/// characters and an Enter key go into the buffer. `key` is the ASCII code of the first key in the
/// buffer, or 0 when the buffer is empty. When `rd` is on and `port` is `address`, the CPU reads
/// the key, and the buffer removes it.
#[node(color = "#59366f", free_layout)]
pub struct Keyboard {
    text: String,
    keys: VecDeque<u8>,
}

impl Keyboard {
    pub fn init() -> Self {
        Keyboard { text: String::new(), keys: VecDeque::with_capacity(BUFFER) }
    }

    pub fn process(&mut self, port: &u8, rd: &bool, address: u8) -> u8 {
        let key = self.keys.front().copied().unwrap_or(0);
        if *rd && *port == address {
            self.keys.pop_front();
        }
        key
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let waiting: String = self.keys.iter().map(|&k| if k == ENTER { '|' } else { char::from(k) }).collect();
        let e = edges(&[8, 0, 8], &[8]);
        let x0 = {
            let mut c = ui.canvas(&format!("Buffer {waiting}"), [WIDTH, e.height]);
            let [x0, _] = package(&mut c, &e, WIDTH);
            c.text([x0 + 18.0, 24.0], TEXT, "KBD");
            c.text([x0 + 56.0, 24.0], SILK, "ASCII  FIFO 64");
            // The key buffer on the glass, the next key for the CPU first and lit.
            window(&mut c, [x0 + 12.0, 62.0], [180.0, 22.0]);
            let mut one = [0u8; 4];
            for (i, key) in waiting.chars().take(SHOWN).enumerate() {
                let color = if i == 0 { LIT } else { TEXT };
                c.text([x0 + 18.0 + i as f32 * 9.0, 77.0], color, key.encode_utf8(&mut one));
            }
            // The keys, with the key of the next code for the CPU lit.
            let next = self.keys.front().copied().unwrap_or(0);
            let lit = next.to_ascii_uppercase();
            for (row, (keys, indent)) in KEYS.iter().enumerate() {
                for (i, key) in keys.bytes().enumerate() {
                    let at = [x0 + 12.0 + indent + i as f32 * 18.0, 94.0 + row as f32 * 18.0];
                    let on = key == lit;
                    round_rect(&mut c, at, [16.0, 16.0], 3.0, if on { LIT } else { BODY });
                    c.text([at[0] + 4.0, at[1] + 12.5], if on { GLASS } else { SILK }, char::from(key).encode_utf8(&mut one));
                }
            }
            round_rect(&mut c, [x0 + 48.0, 166.0], [108.0, 16.0], 3.0, if next == b' ' { LIT } else { BODY });
            let enter = next == ENTER;
            round_rect(&mut c, [x0 + 160.0, 166.0], [32.0, 16.0], 3.0, if enter { LIT } else { BODY });
            c.text([x0 + 165.0, 178.5], if enter { GLASS } else { SILK }, "RET");
            c.text([x0 + 12.0, 200.0], SILK, &format!("FIFO {}/{BUFFER}", self.keys.len()));
            // The code that the CPU reads next, as hex digits and as a name.
            window(&mut c, [x0 + 12.0, 208.0], [180.0, 40.0]);
            c.text([x0 + 22.0, 233.0], SILK, "NEXT");
            digits(&mut c, [x0 + 70.0, 214.0], 28.0, 2, &format!("{next:02X}"));
            let name = match next {
                0 => "EMPTY",
                ENTER => "RET",
                b' ' => "SPACE",
                k => &*char::from(k).encode_utf8(&mut one),
            };
            c.text([x0 + 130.0, 233.0], TEXT, name);
            x0
        };
        ui.place([x0 + 12.0, 30.0]);
        if ui.text_field("Type", &mut self.text).changed() && !self.text.is_empty() {
            self.send();
        }
    }

    /// Puts the ASCII characters of the text and an Enter key into the buffer, while it has space.
    fn send(&mut self) {
        for key in self.text.bytes().filter(|k| (32..127).contains(k)).chain([ENTER]) {
            if self.keys.len() < BUFFER {
                self.keys.push_back(key);
            }
        }
        self.text.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cpu_reads_each_key_once_and_then_0() {
        let mut keyboard = Keyboard::init();
        keyboard.text = "Hi\u{e9}".into();
        keyboard.send();
        assert_eq!(keyboard.process(&8, &false, 8), b'H');
        // A read on another port does not remove the key.
        assert_eq!(keyboard.process(&9, &true, 8), b'H');
        let read: Vec<u8> = (0..4).map(|_| keyboard.process(&8, &true, 8)).collect();
        assert_eq!(read, [b'H', b'i', ENTER, 0]);
    }
}
