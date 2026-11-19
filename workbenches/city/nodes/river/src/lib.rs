use city::Terrain;
use geometry::Seed;
use patina_sdk::node;

/// Cuts a winding river of full width `width` from the highest edge of the terrain to the lowest
/// edge. The bed is `depth` meters below the sea level. A width of 0 gives no river.
#[node(color = "#3f4a3a")]
pub fn river(terrain: &Terrain, seed: &Seed, width: f64, depth: f64) -> Result<Terrain, String> {
    city::river(terrain, *seed, width, depth)
}
