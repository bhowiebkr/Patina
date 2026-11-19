use geometry::Solid;
use patina_sdk::node;

/// Multiplies the sizes of the solid by `x y z`. 1 keeps the size on that axis, and 0 makes the
/// solid empty.
#[node]
pub fn scale(
    solid: &Solid,
    #[param(default = 1.0)] x: f64,
    #[param(default = 1.0)] y: f64,
    #[param(default = 1.0)] z: f64,
) -> Solid {
    solid.scale([x, y, z])
}
