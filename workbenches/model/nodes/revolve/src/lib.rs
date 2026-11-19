use geometry::{Sketch, Solid};
use patina_sdk::node;

/// Turns the sketch around its Y axis, and makes that axis the Z axis. Only the part of the
/// sketch at x > 0 counts. `gap` is the angle that stays open, in degrees: 0 makes a full turn.
/// `segments` 0 lets the kernel choose.
#[node]
pub fn revolve(
    sketch: &Sketch,
    #[param(min = 0, max = 360)] gap: f64,
    segments: i32,
) -> Result<Solid, String> {
    let degrees = 360.0 - gap.clamp(0.0, 360.0);
    if degrees.is_nan() || degrees <= 0.0 {
        return Ok(Solid::empty());
    }
    Solid::revolve(sketch, degrees, segments).checked()
}
