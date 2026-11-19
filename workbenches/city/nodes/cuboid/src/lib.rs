use geometry::Solid;
use patina_sdk::node;

/// A box with the sizes `x`, `y` and `z`, with its center at the origin.
#[node]
pub fn cuboid(
    #[param(default = 1.0, min = 0)] x: f64,
    #[param(default = 1.0, min = 0)] y: f64,
    #[param(default = 1.0, min = 0)] z: f64,
) -> Solid {
    Solid::cube(x, y, z)
}
