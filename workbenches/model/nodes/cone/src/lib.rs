use geometry::Solid;
use patina_sdk::node;

/// A cone along Z, with its center at the origin. `radius_top` 0 makes a point. `segments` 0
/// lets the kernel choose the number of sides.
#[node]
pub fn cone(
    #[param(default = 1.0, min = 0)] height: f64,
    #[param(default = 0.5, min = 0)] radius_bottom: f64,
    #[param(min = 0)] radius_top: f64,
    segments: i32,
) -> Result<Solid, String> {
    Solid::cylinder(height, radius_bottom, radius_top, segments).checked()
}
