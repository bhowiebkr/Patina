use patina_sdk::node;

/// Splits a byte into 8 bits. Output pin `out0` is bit 0, the least significant bit.
#[node(color = "#175c59")]
pub fn split(value: &u8) -> (bool, bool, bool, bool, bool, bool, bool, bool) {
    let bit = |i: u8| value >> i & 1 == 1;
    (bit(0), bit(1), bit(2), bit(3), bit(4), bit(5), bit(6), bit(7))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_bit_0_first() {
        assert_eq!(split(&0b1000_0101), (true, false, true, false, false, false, false, true));
    }
}
