use city::{Density, Terrain};
use geometry::Seed;
use patina_sdk::node;

/// The density of the population on the terrain, from 0 to 1. It is 1 at the main center, on
/// flat land near the middle, and goes down over `radius`. `centers` counts the main center and
/// the smaller sub-centers. Steep slopes get less, and the water gets nothing.
#[node(color = "#6a4f7a")]
pub fn density(terrain: &Terrain, seed: &Seed, centers: u32, radius: f64) -> Result<Density, String> {
    city::density(terrain, *seed, centers, radius)
}
