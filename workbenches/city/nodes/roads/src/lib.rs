use city::{Plots, Streets};
use geometry::Scene;
use patina_sdk::node;

/// The asphalt under the streets, with the top at z = 0, and a sidewalk slab on each block.
#[node(color = "#3a3c40")]
pub fn roads(streets: &Streets, blocks: &Plots) -> Scene {
    city::roads(streets, blocks)
}
