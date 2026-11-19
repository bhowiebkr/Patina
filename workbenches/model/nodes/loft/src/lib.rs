use geometry::{Sketch, Solid};
use patina_sdk::node;

/// The solid from the outline of `bottom` at z = 0 to the outline of `top` at z = `height`. Each
/// sketch must have one outline and no hole.
#[node]
pub fn loft(bottom: &Sketch, top: &Sketch, #[param(default = 1.0, min = 0)] height: f64) -> Result<Solid, String> {
    Solid::loft(bottom, top, height)
}
