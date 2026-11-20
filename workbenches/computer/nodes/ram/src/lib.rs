use parts::{LIT, SILK, TEXT as WHITE, UNLIT, digits, edges, mix, package, window};
use patina_sdk::{Mode, Ui, node};
use std::fmt::Write;

const TEXT: [u8; 3] = [178, 178, 173];
const DIM: [u8; 3] = [102, 102, 97];
const ON: [u8; 3] = [223, 166, 75];
const BACK: [u8; 3] = [24, 24, 23];
const MARK: [u8; 3] = [70, 54, 26];
const CELL: [f32; 2] = [20.0, 15.0];
/// The width of the drawing of the chip.
const WIDTH: f32 = 252.0;

/// A RAM chip of 256 bytes. It gives the byte at `address`. When `write` is on, the byte at
/// `address` gets `data` after the read, as at the clock edge of a real chip. The node shows the
/// chip, and the full size shows all bytes in hexadecimal, with the address of the last access.
#[node(color = "#655219", full_gui, free_layout)]
pub struct Ram {
    bytes: [u8; 256],
    address: u8,
    written: Option<u8>,
}

impl Ram {
    pub fn init() -> Self {
        Ram { bytes: [0; 256], address: 0, written: None }
    }

    pub fn process(&mut self, address: &u8, data: &u8, write: &bool) -> u8 {
        let at = usize::from(*address);
        let value = self.bytes[at];
        if *write {
            self.bytes[at] = *data;
            self.written = Some(*address);
        }
        self.address = *address;
        value
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let at = self.address;
        let text = format!("{at:02X}: {:02X}", self.bytes[usize::from(at)]);
        if ui.mode() == Mode::Compact {
            let e = edges(&[8, 8, 0], &[8]);
            let mut c = ui.canvas(&format!("RAM {text}"), [WIDTH, e.height]);
            let [x0, _] = package(&mut c, &e, WIDTH);
            c.text([x0 + 18.0, 24.0], WHITE, "RAM");
            c.text([x0 + 52.0, 24.0], SILK, "256 x 8");
            // The memory map: one cell for each byte, brighter for a larger byte. One path for each
            // row keeps each path under the limit of the editor (4.3).
            window(&mut c, [x0 + 14.0, 34.0], [168.0, 168.0]);
            let mut levels: [String; 5] = Default::default();
            for (row, bytes) in self.bytes.chunks(16).enumerate() {
                let y = 38.0 + row as f32 * 10.0;
                for (col, &byte) in bytes.iter().enumerate() {
                    let level = if byte == 0 { 0 } else { 1 + usize::from(byte >> 6) };
                    let _ = write!(levels[level], "M {} {y} h 8 v 8 h -8 Z ", x0 + 18.0 + col as f32 * 10.0);
                }
                for (level, d) in levels.iter_mut().enumerate().filter(|(_, d)| !d.is_empty()) {
                    let t = if level == 0 { 0.0 } else { 0.2 + 0.2 * level as f32 };
                    c.path(d, mix(UNLIT, LIT, t), true);
                    d.clear();
                }
            }
            let [x, y] = [x0 + 18.0 + f32::from(at % 16) * 10.0, 38.0 + f32::from(at / 16) * 10.0];
            c.rect([x - 1.5, y - 1.5], [11.0, 11.0], WHITE, false);
            // The address and the byte of the last access.
            window(&mut c, [x0 + 14.0, 212.0], [168.0, 36.0]);
            c.text([x0 + 24.0, 235.0], SILK, "A");
            digits(&mut c, [x0 + 38.0, 218.0], 24.0, 2, &format!("{at:02X}"));
            c.text([x0 + 98.0, 235.0], SILK, "D");
            digits(&mut c, [x0 + 112.0, 218.0], 24.0, 2, &format!("{:02X}", self.bytes[usize::from(at)]));
            return;
        }
        ui.label(&text);
        if ui.mode() != Mode::Full {
            return;
        }
        let size = [CELL[0] * 17.0 + 8.0, CELL[1] * 17.0 + 4.0];
        let mut canvas = ui.canvas("Bytes", size);
        canvas.rect([0.0, 0.0], size, BACK, true);
        for i in 0..16_u8 {
            let x = 8.0 + CELL[0] * f32::from(i + 1);
            let y = 2.0 + CELL[1] * f32::from(i + 1);
            canvas.text([x, CELL[1] - 2.0], DIM, format_args!("{i:X}"));
            canvas.text([4.0, y + CELL[1] - 4.0], DIM, format_args!("{i:X}0"));
        }
        for (address, &byte) in (0..=255_u8).zip(&self.bytes) {
            let x = 8.0 + CELL[0] * f32::from(address % 16 + 1);
            let y = 2.0 + CELL[1] * f32::from(address / 16 + 1);
            if address == at {
                canvas.rect([x - 3.0, y], [CELL[0], CELL[1]], MARK, true);
            }
            let color = if self.written == Some(address) { ON } else if byte == 0 { DIM } else { TEXT };
            canvas.text([x, y + CELL[1] - 4.0], color, format_args!("{byte:02X}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_comes_after_the_read() {
        let mut ram = Ram::init();
        assert_eq!(ram.process(&7, &42, &true), 0, "the read gives the old byte");
        assert_eq!(ram.process(&7, &9, &false), 42);
        assert_eq!(ram.process(&255, &1, &true), 0);
        assert_eq!((ram.bytes[255], ram.written), (1, Some(255)));
    }

    #[test]
    fn the_node_shows_the_chip_and_the_last_access() {
        let mut ram = Ram::init();
        ram.process(&0x2A, &0xBE, &true);
        let mut ui = Ui::new(0, "");
        ram.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tRAM 2A: BE\t"), "{}", ui.tree());
        // The editor skips a path of more than 4096 characters (4.3), also when all bytes are set.
        ram.bytes = [0xFF; 256];
        let mut full = Ui::new(0, "");
        ram.ui(&mut full);
        assert!(full.tree().split('\t').all(|field| field.len() <= 4096));
    }
}
