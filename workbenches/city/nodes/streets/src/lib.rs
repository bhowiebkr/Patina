use city::{Grid, Streets, grid};
use geometry::Seed;
use patina_sdk::node;

/// A grid of streets in the area, with the center at the origin. Each `avenue`th line is a wide
/// and straight avenue. `jitter` moves the crossings at random, and `drop` removes a part of the
/// inner streets. The street widths are full widths.
#[node(color = "#3a3c40")]
#[allow(clippy::too_many_arguments)]
pub fn streets(seed: &Seed, width: f64, depth: f64, spacing: f64, jitter: f64, drop: f64, avenue: u32, street: f64, avenue_width: f64) -> Result<Streets, String> {
    grid(*seed, &Grid { width, depth, spacing, jitter, drop, avenue, street, avenue_width })
}
