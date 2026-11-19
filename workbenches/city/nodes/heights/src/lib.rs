use city::Plots;
use geometry::Seed;
use patina_sdk::node;

/// Gives each lot that is not a park a height in full floors of `floor`. The height is near
/// `high` at the center of the city and goes down to `low` at `radius` and farther, with a random
/// part from the seed.
#[node]
pub fn heights(plots: &Plots, seed: &Seed, low: f64, high: f64, radius: f64, floor: f64) -> Result<Plots, String> {
    city::heights(plots, *seed, low, high, radius, floor)
}
