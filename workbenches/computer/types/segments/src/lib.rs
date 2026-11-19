//! The segments a to g of a 7-segment digit in one value, for one bus between two nodes.

use std::fmt;

/// Segments a to g of a digit. Bit 6 is segment a and bit 0 is segment g.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Segments(pub u8);

impl Segments {
    /// True when segment `i` is on: 0 is a and 6 is g.
    pub fn on(self, i: usize) -> bool {
        self.0 >> (6 - i) & 1 == 1
    }
}

/// Shows the segments a to g as bits, for example 1111110 for the digit 0.
impl fmt::Debug for Segments {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:07b}", self.0 & 0x7f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_a_is_bit_6() {
        let seg = Segments(0b100_0001);
        assert_eq!((0..7).map(|i| seg.on(i)).collect::<Vec<_>>(), [true, false, false, false, false, false, true]);
        assert_eq!(format!("{seg:?}"), "1000001");
    }
}
