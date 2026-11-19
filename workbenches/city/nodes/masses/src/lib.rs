use city::Plots;
use geometry::{Scene, Seed};
use patina_sdk::node;

/// The body of each building to its height, with a flat, gable or hip roof, and a lawn on each
/// park. `pitched` is the part of the roofs with a slope, from 0 to 1, and `rise` is the height
/// of the slope. Each building is a group for `export-buildings`.
#[node]
pub fn masses(plots: &Plots, seed: &Seed, pitched: f64, rise: f64) -> Result<Scene, String> {
    city::masses(plots, *seed, pitched, rise)
}
