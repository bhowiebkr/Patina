use geometry::Solid;
use patina_sdk::node;

/// A sphere with its center at the origin. `segments` 0 lets the kernel choose.
#[node]
pub fn sphere(
    #[param(default = 0.5, min = 0)] radius: f64,
    segments: i32,
) -> Result<Solid, String> {
    Solid::sphere(radius, segments).checked()
}
