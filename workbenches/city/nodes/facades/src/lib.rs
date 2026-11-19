use city::Plots;
use geometry::{Scene, Solid};
use patina_sdk::node;

/// A window on each floor of each `bay` of each wall, and a door on the longest street edge.
/// Model the window and the door with the wall along X, the outside at +Y and the base at z = 0.
/// The scene has one mesh of each, at many places.
#[node]
pub fn facades(plots: &Plots, window: &Solid, door: &Solid, bay: f64) -> Result<Scene, String> {
    city::facades(plots, window, door, bay)
}
