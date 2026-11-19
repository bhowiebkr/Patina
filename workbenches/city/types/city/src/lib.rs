//! The types of the city workbench: a street graph and plots. The algorithms are here, so the
//! nodes stay thin. Units are meters, X is east, Y is north and Z is up.

mod buildings;
mod plots;
mod streets;

use std::fmt::{self, Write};

use patina_sdk::{Text, Viewer};

pub use buildings::{MAX_PLACES, color, facades, heights, masses, roads, scatter};
pub use plots::{blocks, inset, lots};
pub use streets::{Grid, MAX_CELLS, grid};

/// A street from point `a` to point `b`, with its full width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub width: f64,
}

/// A street graph on the ground.
#[derive(Clone, Default, PartialEq)]
pub struct Streets {
    pub points: Vec<[f64; 2]>,
    pub edges: Vec<Edge>,
}

/// A polygon on the ground with its corners counterclockwise. `fronts[i]` is true when the edge
/// from corner `i` to the next corner is on a street. A park has no floors.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plot {
    pub corners: Vec<[f64; 2]>,
    pub fronts: Vec<bool>,
    pub park: bool,
    pub floors: u32,
    pub height: f64,
}

/// A list of plots: blocks, lots or the footprints of buildings.
#[derive(Clone, Default, PartialEq)]
pub struct Plots(pub Vec<Plot>);

/// Shows "120 points, 220 streets".
impl fmt::Debug for Streets {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} points, {} streets", self.points.len(), self.edges.len())
    }
}

/// Shows "400 plots, 30 parks, 2100 floors".
impl fmt::Debug for Plots {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let parks = self.0.iter().filter(|p| p.park).count();
        let floors: u64 = self.0.iter().map(|p| u64::from(p.floors)).sum();
        write!(f, "{} plots, {parks} parks, {floors} floors", self.0.len())
    }
}

impl Viewer for Streets {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }
}

impl Viewer for Plots {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }
}

pub(crate) fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

pub(crate) fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

pub(crate) fn scale(a: [f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}

pub(crate) fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

pub(crate) fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

pub(crate) fn length(a: [f64; 2]) -> f64 {
    dot(a, a).sqrt()
}

/// The area of a polygon, positive when the corners are counterclockwise.
pub(crate) fn area(corners: &[[f64; 2]]) -> f64 {
    let n = corners.len();
    (0..n).map(|i| cross(corners[i], corners[(i + 1) % n])).sum::<f64>() / 2.0
}

/// The center of the area of a polygon, or the first corner for a polygon with no area.
pub(crate) fn centroid(corners: &[[f64; 2]]) -> [f64; 2] {
    let (n, a) = (corners.len(), area(corners));
    if a.abs() < 1e-12 {
        return corners.first().copied().unwrap_or_default();
    }
    let mut c = [0.0; 2];
    for i in 0..n {
        let (p, q) = (corners[i], corners[(i + 1) % n]);
        c = add(c, scale(add(p, q), cross(p, q)));
    }
    scale(c, 1.0 / (6.0 * a))
}

/// The direction of the longest edge and the direction to its left.
pub(crate) fn axes(corners: &[[f64; 2]]) -> ([f64; 2], [f64; 2]) {
    let n = corners.len();
    let edge = (0..n).map(|i| sub(corners[(i + 1) % n], corners[i])).max_by(|a, b| length(*a).total_cmp(&length(*b)));
    let u = edge.filter(|e| length(*e) > 0.0).map_or([1.0, 0.0], |e| scale(e, 1.0 / length(e)));
    (u, [-u[1], u[0]])
}

/// The lowest and the highest value of the corners along `axis`.
pub(crate) fn extent(corners: &[[f64; 2]], axis: [f64; 2]) -> (f64, f64) {
    corners.iter().map(|&p| dot(p, axis)).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

#[cfg(test)]
mod tests;
