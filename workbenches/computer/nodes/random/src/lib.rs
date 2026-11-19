use parts::{SILK, TEXT, digits, edges, led, package, window};
use patina_sdk::{Ui, node};

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
        let (e, w) = (edges(&[8, 8, 0, 8], &[8]), 232.0);
        let mut c = ui.canvas(&format!("Last read {}", self.last), [w, e.height]);
        let [x0, _] = package(&mut c, &e, w);
        c.text([x0 + 18.0, 24.0], TEXT, "RNG");
        c.text([x0 + 56.0, 24.0], SILK, "XORSHIFT 32");
        window(&mut c, [x0 + 14.0, 36.0], [148.0, 80.0]);
        digits(&mut c, [x0 + 44.0, 46.0], 60.0, 2, &format!("{:02X}", self.last));
        c.text([x0 + 14.0, 136.0], SILK, "LAST READ");
        // The 32 bits of the state, the most significant bit first.
        c.text([x0 + 14.0, 162.0], SILK, "STATE");
        window(&mut c, [x0 + 14.0, 170.0], [148.0, 76.0]);
        for bit in 0..32 {
            let at = [x0 + 25.0 + (bit % 8) as f32 * 18.0, 183.0 + (bit / 8) as f32 * 17.0];
            led(&mut c, at, 4.0, self.state >> (31 - bit) & 1 == 1);
        }
        for (i, line) in ["x ^= x << 13", "x ^= x >> 17", "x ^= x << 5", "SEED: CLOCK"].into_iter().enumerate() {
            c.text([x0 + 14.0, 276.0 + i as f32 * 18.0], SILK, line);
        }
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
