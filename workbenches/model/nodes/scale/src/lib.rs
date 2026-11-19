use geometry::Solid;
use patina_sdk::node;

/// Multiplies the sizes of the solid by `x y z`. 1 keeps the size on that axis, and 0 makes the
/// solid empty.
#[node]
pub fn scale(solid: &Solid, x: f64, y: f64, z: f64) -> Solid {
    solid.scale([x, y, z])
}
