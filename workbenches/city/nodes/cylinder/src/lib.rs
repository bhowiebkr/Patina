use geometry::Solid;
use patina_sdk::node;

/// A cylinder along Z, with its center at the origin. `segments` 0 lets the kernel choose the
/// number of sides.
#[node]
pub fn cylinder(
    #[param(default = 1.0, min = 0)] height: f64,
    #[param(default = 0.5, min = 0)] radius: f64,
    segments: i32,
) -> Result<Solid, String> {
    Solid::cylinder(height, radius, radius, segments).checked()
}
