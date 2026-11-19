use geometry::Sketch;
use patina_sdk::node;

/// A rectangle with its center at `x y`.
#[node]
pub fn rectangle(width: f64, height: f64, x: f64, y: f64) -> Sketch {
    Sketch::rectangle(width, height).translate([x, y])
}
