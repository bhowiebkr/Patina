use patina_sdk::{Lanes, connection};

/// Draws a byte wire as a ribbon of 8 lanes, the most significant bit at the top.
#[connection]
pub fn byte(value: &u8) -> Lanes {
    Lanes::bits(8, u64::from(*value)).label("bus")
}
