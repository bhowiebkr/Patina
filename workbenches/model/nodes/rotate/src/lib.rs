use geometry::Solid;
use patina_sdk::node;

/// Turns the solid around the X axis, then the Y axis, then the Z axis, in degrees.
#[node]
pub fn rotate(solid: &Solid, x: f64, y: f64, z: f64) -> Solid {
    solid.rotate([x, y, z])
}
