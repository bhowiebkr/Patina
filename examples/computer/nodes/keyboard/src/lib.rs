use parts::{LIT, PIN_STEP, SILK, TEXT, board, glass, pin_names};
use patina_sdk::{Ui, node};
use std::collections::VecDeque;

/// The number of keys that the buffer keeps. A real keyboard chip has a small buffer too.
const BUFFER: usize = 64;
/// The ASCII code of the Enter key.
const ENTER: u8 = 13;
/// The size of the board.
const SIZE: [f32; 2] = [232.0, 84.0];
/// The inputs on the left edge, in order.
const PINS: [&str; 3] = ["port", "rd", "address"];
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
        {
            let mut c = ui.canvas(&format!("Buffer {waiting}"), SIZE);
            board(&mut c, SIZE);
            pin_names(&mut c, 20.0, &["port", "rd", "adr"]);
            // The key buffer on the glass, the next key for the CPU first and lit.
            glass(&mut c, [44.0, 40.0], [180.0, 24.0]);
            let mut one = [0u8; 4];
            for (i, key) in waiting.chars().take(SHOWN).enumerate() {
                let color = if i == 0 { LIT } else { TEXT };
                c.text([50.0 + i as f32 * 9.0, 57.0], color, key.encode_utf8(&mut one));
            }
            c.text([44.0, 78.0], SILK, &format!("FIFO {}/{BUFFER}", self.keys.len()));
        }
        for (i, key) in PINS.iter().enumerate() {
            ui.pin(key, [0.0, 20.0 + i as f32 * PIN_STEP]);
        }
        ui.pin("out", [SIZE[0], 52.0]);
        ui.place([44.0, 8.0]);
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
