use city::{Buildings, Plots, Terrain};
use geometry::Seed;
use patina_sdk::node;

/// The plan of a building on each lot that is not a park. The zone and the floors give the kind:
/// a house, an apartment, a shop, a tower or a warehouse. A house stands `setback` back from
/// its street. Each floor is `floor` high. The ground level comes from the terrain.
#[node(color = "#5a4a3c")]
pub fn buildings(plots: &Plots, terrain: &Terrain, seed: &Seed, floor: f64, setback: f64) -> Result<Buildings, String> {
    city::buildings(plots, terrain, *seed, floor, setback)
}
