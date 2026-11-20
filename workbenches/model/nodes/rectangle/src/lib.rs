use geometry::Sketch;
use patina_sdk::node;

/// A rectangle with its center at `x y`.
#[node]
pub fn rectangle(
    #[param(default = 1.0, min = 0)] width: f64,
    #[param(default = 1.0, min = 0)] height: f64,
    x: f64,
    y: f64,
) -> Result<Sketch, String> {
    Sketch::rectangle(width, height).translate([x, y]).checked()
}
