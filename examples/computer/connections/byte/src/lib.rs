use patina_sdk::{Lanes, connection};

/// Draws a byte wire as a green ribbon of 8 lanes, the most significant bit at the top.
#[connection(lanes = 8, label = "bus", color = "green")]
pub fn byte(value: &u8) -> Lanes {
    Lanes::bits(8, u64::from(*value)).label("bus")
}
