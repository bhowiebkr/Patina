use city::{Density, Plots};
use geometry::Seed;
use patina_sdk::node;

/// Gives each lot its floors from the density and the zone, up to `high`. A residential lot has
/// houses at a low density and apartments at a high density. A commercial lot has shops, and
/// towers near `high` at the peak of the density. An industrial lot has one or two floors.
#[node(color = "#6a4f7a")]
pub fn heights(plots: &Plots, density: &Density, seed: &Seed, high: u32) -> Result<Plots, String> {
    city::heights(plots, density, *seed, high)
}
