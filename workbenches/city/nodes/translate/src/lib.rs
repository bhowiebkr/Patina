use geometry::Solid;
use patina_sdk::node;

/// Moves the solid by `x y z`.
#[node]
pub fn translate(solid: &Solid, x: f64, y: f64, z: f64) -> Solid {
    solid.translate([x, y, z])
}
