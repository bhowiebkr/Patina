use city::{Density, Streets, Terrain};
use geometry::Seed;
use patina_sdk::node;

/// Highways from `gates` points on the edge of the area to the main center. Each highway takes
/// the path of least cost: a steep slope costs more, and water costs `bridge` times more. The
/// highways share their common parts. `width` is the full width.
#[node(color = "#3a3c40")]
pub fn highways(terrain: &Terrain, density: &Density, seed: &Seed, gates: u32, width: f64, bridge: f64) -> Result<Streets, String> {
    city::highways(terrain, density, *seed, gates, width, bridge)
}
