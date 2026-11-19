use geometry::{Sketch, Solid};
use patina_sdk::node;

/// A ring around Z: a circle with the radius `minor` turns around Z at the distance `major`.
/// `segments` 0 lets the kernel choose.
#[node]
pub fn torus(major: f64, minor: f64, segments: i32) -> Result<Solid, String> {
    let circle = Sketch::circle(minor, segments).translate([major, 0.0]);
    Solid::revolve(&circle, 360.0, segments).checked()
}
