use city::Plots;
use geometry::{Scene, Seed, Solid};
use patina_sdk::node;

/// Copies of the solid along the edges of each plot, `inset` in from the edges and `spacing`
/// apart, for example trees or street lamps. Each copy has a random turn and size. Write the
/// color as "#rrggbb".
#[node]
pub fn scatter(plots: &Plots, solid: &Solid, seed: &Seed, spacing: f64, inset: f64, color: &str) -> Result<Scene, String> {
    city::scatter(plots, solid, *seed, spacing, inset, city::color(color)?)
}
