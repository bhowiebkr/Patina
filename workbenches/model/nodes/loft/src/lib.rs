use geometry::{Sketch, Solid};
use patina_sdk::node;

/// The solid from the outlines of `bottom` at z = 0 to the outlines of `top` at z = `height`.
/// Each outer outline joins the outer outline of the other sketch with the nearest center and
/// size. Two joined outer outlines must have the same number of holes.
#[node]
pub fn loft(bottom: &Sketch, top: &Sketch, #[param(default = 1.0, min = 0)] height: f64) -> Result<Solid, String> {
    Solid::loft(bottom, top, height)
}
