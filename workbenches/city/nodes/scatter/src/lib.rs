use city::{Plots, Terrain};
use geometry::{Scene, Seed, Solid};
use patina_sdk::node;

/// Copies of the solid along the edges of each plot, `inset` in from the edges and `spacing`
/// apart, on the terrain, for example trees or street lamps. A copy over water is left out.
/// Each copy has a random turn and size. Write the color as "#rrggbb".
#[node]
#[allow(clippy::too_many_arguments)]
pub fn scatter(plots: &Plots, terrain: &Terrain, solid: &Solid, seed: &Seed, spacing: f64, inset: f64, color: &str) -> Result<Scene, String> {
    city::scatter(plots, terrain, solid, *seed, spacing, inset, city::color(color)?)
}
