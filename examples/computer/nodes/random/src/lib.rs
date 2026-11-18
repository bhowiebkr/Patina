use parts::{PIN_STEP, SILK, board, digits, glass, pin_names};
use patina_sdk::{Ui, node};

/// The inputs on the left edge, in order.
const PINS: [&str; 4] = ["bus", "port", "rd", "address"];

/// A random number chip on an IO port of the CPU. When `port` is `address`, it gives a random byte,
/// and each LD (`rd` on) gives a new byte. At the other ports, the byte on `bus` goes through, so
/// the device before it on the **io** wire of the CPU still gives its values.
#[node(color = "#59366f", free_layout)]
pub struct Random {
    state: u32,
    last: u8,
}

impl Random {
    pub fn init() -> Self {
        // The time seeds the generator, so each run gives other numbers.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        Random::seeded(now.map_or(1, |d| d.subsec_nanos()))
    }

    fn seeded(seed: u32) -> Self {
        // A xorshift state must not be 0.
        Random { state: seed | 1, last: 0 }
    }

    pub fn process(&mut self, bus: &u8, port: &u8, rd: &bool, address: u8) -> u8 {
        if *port != address {
            return *bus;
        }
        let byte = self.state.to_be_bytes()[0];
        if *rd {
            self.last = byte;
            // The xorshift32 generator of Marsaglia (2003).
            self.state ^= self.state << 13;
            self.state ^= self.state >> 17;
            self.state ^= self.state << 5;
        }
        byte
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let size = [136.0, 80.0];
        let mut c = ui.canvas(&format!("Last read {}", self.last), size);
        board(&mut c, size);
        pin_names(&mut c, 16.0, &["bus", "port", "rd", "adr"]);
        c.text([72.0, 19.0], SILK, "RND");
        glass(&mut c, [72.0, 26.0], [56.0, 44.0]);
        digits(&mut c, [81.0, 34.0], 28.0, 2, &format!("{:02X}", self.last));
        drop(c);
        for (i, key) in PINS.iter().enumerate() {
            ui.pin(key, [0.0, 16.0 + i as f32 * PIN_STEP]);
        }
        ui.pin("out", [size[0], 48.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_read_gives_a_new_byte_and_other_ports_pass_the_bus() {
        let mut random = Random::seeded(0x1234_5678);
        let first = random.process(&0, &13, &false, 13);
        assert_eq!(random.process(&0, &13, &false, 13), first, "no read keeps the byte");
        assert_eq!(random.process(&0, &13, &true, 13), first, "a read gives the byte, then moves on");
        let bytes: Vec<u8> = (0..64).map(|_| random.process(&0, &13, &true, 13)).collect();
        assert!(bytes.iter().filter(|&&b| b == bytes[0]).count() < 8, "the bytes change: {bytes:?}");
        assert_eq!(random.process(&65, &8, &true, 13), 65, "port 8 passes the key on the bus");
    }
}
