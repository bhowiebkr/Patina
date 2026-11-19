use city::{Plots, Streets};
use patina_sdk::node;

/// The blocks between the streets: each closed face of the street graph, inset by half the width
/// of its streets. All the edges of a block are on a street.
#[node(color = "#3a3c40")]
pub fn blocks(streets: &Streets) -> Plots {
    city::blocks(streets)
}
