use city::{Plots, Streets, Terrain};
use geometry::Scene;
use patina_sdk::node;

/// The roads on the terrain: an asphalt ribbon of its width for each street, road lines for each
/// road class, and a sidewalk ring of `sidewalk` with a curb in each block. A highway over water
/// becomes a bridge with piers.
#[node(color = "#3a3c40")]
pub fn roads(streets: &Streets, blocks: &Plots, terrain: &Terrain, sidewalk: f64) -> Result<Scene, String> {
    city::roads(streets, blocks, terrain, sidewalk)
}
