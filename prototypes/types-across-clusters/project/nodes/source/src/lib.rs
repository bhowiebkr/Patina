//! A node that makes values. It uses version 1 of `units`.

use geometry::{Point, Probe};
use units::Meters;

pub fn greeting() -> String {
    "hello from cluster A".to_owned()
}

pub fn points() -> Vec<Point> {
    (1..=4).map(|i| Point::from_ints(i, i * 10)).collect()
}

pub fn probe() -> Probe {
    Probe::new("made in cluster A")
}

pub fn distance() -> Meters {
    Meters(12.5)
}
