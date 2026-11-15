//! A node that uses values. It uses version 2 of `units`.

use geometry::{Point, Probe};
use units::Meters;

pub fn greeting_len(text: &str) -> usize {
    text.len()
}

/// Returns the number of points and the sums of their coordinates.
pub fn sum_points(points: Vec<Point>) -> (usize, f64, f64) {
    let x = points.iter().map(|p| f64::from(p.x)).sum();
    let y = points.iter().map(|p| f64::from(p.y)).sum();
    (points.len(), x, y)
}

/// Drops the probe here, in cluster B.
pub fn take_probe(probe: Probe) -> usize {
    probe.label.len()
}

pub fn distance_m(distance: Meters) -> f64 {
    distance.0
}
