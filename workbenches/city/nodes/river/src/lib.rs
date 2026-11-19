use city::Terrain;
use geometry::Seed;
use patina_sdk::node;

/// Cuts a river of full width `width` along the routes of the water, from the edge opposite the
/// lowest edge to the sea. The bed is `depth` meters below the sea level. A width of 0 gives no
/// river.
/// The river cuts a valley with sides of `valley` meters. 0 gives no valley.
#[node(color = "#3f4a3a")]
pub fn river(terrain: &Terrain, seed: &Seed, width: f64, depth: f64, valley: f64) -> Result<Terrain, String> {
    city::river(terrain, *seed, width, depth, valley)
}
