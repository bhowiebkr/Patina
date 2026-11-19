use geometry::{Sketch, Solid};
use patina_sdk::node;

/// Makes the sketch thick along Z, from 0 to `height`. The top turns by `twist` degrees.
/// `taper` makes the top smaller: 0 keeps the size, and 1 makes a point. `slices` 0 lets the
/// kernel choose.
#[node]
pub fn extrude(
    sketch: &Sketch,
    #[param(default = 1.0, min = 0)] height: f64,
    twist: f64,
    #[param(min = 0, max = 1)] taper: f64,
    slices: i32,
) -> Result<Solid, String> {
    let top = 1.0 - taper;
    Solid::extrude(sketch, height, twist, [top, top], slices).checked()
}
