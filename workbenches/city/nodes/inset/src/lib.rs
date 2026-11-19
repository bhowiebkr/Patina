use city::Plots;
use patina_sdk::node;

/// Moves each edge of each plot in by `distance`, for a sidewalk or a setback. A plot that the
/// inset closes is not in the output.
#[node]
pub fn inset(plots: &Plots, distance: f64) -> Result<Plots, String> {
    city::inset(plots, distance)
}
