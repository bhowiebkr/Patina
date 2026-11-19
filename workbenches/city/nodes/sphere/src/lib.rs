use geometry::Solid;
use patina_sdk::node;

/// A sphere with its center at the origin. `segments` 0 lets the kernel choose.
#[node]
pub fn sphere(radius: f64, segments: i32) -> Result<Solid, String> {
    Solid::sphere(radius, segments).checked()
}
