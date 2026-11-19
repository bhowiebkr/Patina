//! `Seed` and `Random`: random values that are the same on each platform for the same seed.

use std::fmt;

use patina_sdk::Param;

/// The start value of the random values of a model. The same seed gives the same model.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Seed(pub u64);

impl Seed {
    /// A sub-seed for one part, for example `seed.child("barrel")`. A change to the random
    /// values of one part does not change the other parts.
    pub fn child(self, name: &str) -> Seed {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in name.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
        Seed(mix(self.0 ^ mix(hash)))
    }

    /// A generator of random values that starts at this seed.
    pub fn random(self) -> Random {
        Random(self.0)
    }
}

/// A seed input with no wire is a whole number in the **Inspector**.
impl Param for Seed {
    fn default_value() -> Seed {
        Seed(0)
    }

    fn from_text(text: &str) -> Result<Seed, String> {
        text.trim().parse().map(Seed).map_err(|_| format!("{text:?} is not a seed: write a whole number from 0"))
    }

    fn to_text(&self) -> String {
        self.0.to_string()
    }
}

impl fmt::Debug for Seed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "seed {}", self.0)
    }
}

/// The SplitMix64 generator. It uses only integer operations, so each platform gives the same
/// values.
#[derive(Clone, Debug)]
pub struct Random(u64);

impl Random {
    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.0)
    }

    /// A value from 0 to 1, without 1.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// A value from `low` to `high`, without `high`.
    pub fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }

    /// An index from 0 to `count`, without `count`. `count` 0 gives 0.
    pub fn index(&mut self, count: usize) -> usize {
        ((self.next_u64() as u128 * count as u128) >> 64) as usize
    }

    /// An index into `weights`, with a chance in proportion to each weight. Weights that are
    /// not positive never win. No positive weight gives 0.
    pub fn pick(&mut self, weights: &[f64]) -> usize {
        let total: f64 = weights.iter().filter(|w| **w > 0.0).sum();
        let mut left = self.unit() * total;
        for (i, &w) in weights.iter().enumerate() {
            if w > 0.0 {
                if left < w {
                    return i;
                }
                left -= w;
            }
        }
        weights.iter().rposition(|w| *w > 0.0).unwrap_or(0)
    }
}

/// The output function of SplitMix64.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
