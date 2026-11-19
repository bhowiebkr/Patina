use geometry::Sketch;
use patina_sdk::node;

/// A circle with its center at `x y`. `segments` 0 lets the kernel choose.
#[node]
pub fn circle(radius: f64, x: f64, y: f64, segments: i32) -> Sketch {
    Sketch::circle(radius, segments).translate([x, y])
}
