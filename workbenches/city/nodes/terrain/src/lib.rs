use city::{Land, Terrain};
use geometry::Seed;
use patina_sdk::node;

/// A height map of the area, with the center at the origin. Hills of size `hills` rise up to
/// `relief` meters. On one side, which the seed picks, the land goes down into the sea over the
/// `coast` distance. The ground below `sea` is water.
#[node(color = "#3f4a3a")]
#[allow(clippy::too_many_arguments)]
pub fn terrain(seed: &Seed, width: f64, depth: f64, cell: f64, relief: f64, hills: f64, coast: f64, sea: f64) -> Result<Terrain, String> {
    city::terrain(*seed, &Land { width, depth, cell, relief, hills, coast, sea })
}
