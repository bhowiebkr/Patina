use patina_sdk::{Lanes, connection};
use segments::Segments;

/// Draws the segments of a digit as a copper ribbon of 7 lanes, named a to g.
#[connection(lanes = ["a", "b", "c", "d", "e", "f", "g"], label = "seg", color = "copper")]
pub fn segment_bus(value: &Segments) -> Lanes {
    Lanes::bits(7, u64::from(value.0)).label("seg")
}
