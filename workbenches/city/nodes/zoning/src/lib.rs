use city::{Density, Plots, Terrain};
use geometry::Seed;
use patina_sdk::node;

/// Gives each lot a zone: residential, commercial, industrial or park. A high density, or an
/// arterial in a mid density, gives shops. An industrial district covers about the part
/// `industry` of the flat land. A lot with no street, on a steep slope, and a random part
/// `parks` of the other lots, become parks.
#[node(color = "#6a4f7a")]
pub fn zoning(plots: &Plots, terrain: &Terrain, density: &Density, seed: &Seed, parks: f64, industry: f64) -> Result<Plots, String> {
    city::zoning(plots, terrain, density, *seed, parks, industry)
}
