use geometry::Solid;
use patina_sdk::node;

/// A cone along Z, with its center at the origin. `radius_top` 0 makes a point. `segments` 0
/// lets the kernel choose the number of sides.
#[node]
pub fn cone(height: f64, radius_bottom: f64, radius_top: f64, segments: i32) -> Result<Solid, String> {
    Solid::cylinder(height, radius_bottom, radius_top, segments).checked()
}
