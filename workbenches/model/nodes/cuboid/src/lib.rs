use geometry::Solid;
use patina_sdk::node;

/// A box with the sizes `x`, `y` and `z`, with its center at the origin.
#[node]
pub fn cuboid(x: f64, y: f64, z: f64) -> Solid {
    Solid::cube(x, y, z)
}
