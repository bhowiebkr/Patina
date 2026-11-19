use city::{Density, Streets, Terrain};
use geometry::Seed;
use patina_sdk::node;

/// Grows arterials and local streets out from the highways. Each new segment is about `block`
/// long. It follows the contours, and it turns along the edge, the water and steep slopes.
/// It joins a near crossing or stops at a street that it meets. A high density gives more
/// branches: `branch` is the chance of a branch at full density. No street grows where the
/// density is below `threshold`. The widths are full widths.
#[node(color = "#3a3c40")]
#[allow(clippy::too_many_arguments)]
pub fn grow(highways: &Streets, terrain: &Terrain, density: &Density, seed: &Seed, block: f64, arterial: f64, street: f64, branch: f64, threshold: f64) -> Result<Streets, String> {
    city::grow(highways, terrain, density, *seed, block, arterial, street, branch, threshold)
}
