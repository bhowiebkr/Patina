use patina_sdk::{Mode, Ui, node};

const TEXT: [u8; 3] = [178, 178, 173];
const DIM: [u8; 3] = [102, 102, 97];
const ON: [u8; 3] = [223, 166, 75];
const BACK: [u8; 3] = [24, 24, 23];
const MARK: [u8; 3] = [70, 54, 26];
const CELL: [f32; 2] = [20.0, 15.0];

/// A RAM chip of 256 bytes. It gives the byte at `address`. When `write` is on, the byte at
/// `address` gets `data` after the read, as at the clock edge of a real chip. The full size shows
/// all bytes in hexadecimal, with the address of the last access.
#[node(color = "#655219", full_gui)]
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
        ui.label(&format!("{at:02X}: {:02X}", self.bytes[usize::from(at)]));
        if ui.mode() != Mode::Full {
            return;
        }
        let size = [CELL[0] * 17.0 + 8.0, CELL[1] * 17.0 + 4.0];
        let mut canvas = ui.canvas("Bytes", size);
        canvas.rect([0.0, 0.0], size, BACK, true);
        for i in 0..16_u8 {
            let x = 8.0 + CELL[0] * f32::from(i + 1);
            let y = 2.0 + CELL[1] * f32::from(i + 1);
            canvas.text([x, CELL[1] - 2.0], DIM, &format!("{i:X}"));
            canvas.text([4.0, y + CELL[1] - 4.0], DIM, &format!("{:X}0", i));
        }
        for (address, &byte) in (0..=255_u8).zip(&self.bytes) {
            let x = 8.0 + CELL[0] * f32::from(address % 16 + 1);
            let y = 2.0 + CELL[1] * f32::from(address / 16 + 1);
            if address == at {
                canvas.rect([x - 3.0, y], [CELL[0], CELL[1]], MARK, true);
            }
            let color = if self.written == Some(address) { ON } else if byte == 0 { DIM } else { TEXT };
            canvas.text([x, y + CELL[1] - 4.0], color, &format!("{byte:02X}"));
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
    fn the_full_size_shows_each_byte() {
        let mut ram = Ram::init();
        ram.process(&0x2A, &0xBE, &true);
        let mut ui = Ui::new(0, "");
        ram.ui(&mut ui);
        assert!(ui.tree().starts_with("label\t2A: BE"));
        assert!(!ui.tree().contains("canvas"), "the node body shows only the label");
    }
}
