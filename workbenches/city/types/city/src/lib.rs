//! The types of the city workbench: a terrain, a street graph and plots. The algorithms are here, so the
//! nodes stay thin. Units are meters, X is east, Y is north and Z is up.

mod buildings;
mod density;
mod facades;
mod grow;
mod highways;
mod plots;
mod props;
mod ribbons;
mod roofs;
mod shapes;
mod streets;
mod terrain;
mod walls;
mod zoning;

use std::fmt::{self, Write};

use patina_sdk::{Text, Viewer};

pub use buildings::{Building, Buildings, Kind, Roof, Style, Tier, buildings};
pub use density::{Density, MAX_CENTERS, density};
pub use facades::facades;
pub use grow::{MAX_ROADS, grow, prune};
pub use highways::{MAX_GATES, highways};
pub use plots::{blocks, inset, lots};
pub use props::{MAX_PLACES, color, scatter};
pub use ribbons::roads;
pub use roofs::roofs;
pub use streets::{Grid, MAX_CELLS, grid};
pub use terrain::{Land, MAX_SAMPLES, Terrain, ground, river, terrain};
pub use walls::walls;
pub use zoning::{MAX_FLOORS, heights, zoning};

/// The road classes of `Edge::class`.
pub const HIGHWAY: u8 = 0;
pub const ARTERIAL: u8 = 1;
pub const LOCAL: u8 = 2;

/// A street from point `a` to point `b`, with its full width and its road class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub width: f64,
    pub class: u8,
}

/// A street graph on the ground.
#[derive(Clone, Default, PartialEq)]
pub struct Streets {
    pub points: Vec<[f64; 2]>,
    pub edges: Vec<Edge>,
}

/// The use of a plot (task E11).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Zone {
    #[default]
    Residential,
    Commercial,
    Industrial,
    Park,
}

/// A polygon on the ground with its corners counterclockwise. `fronts[i]` is the road class of
/// the street at the edge from corner `i` to the next corner, or `None` when no street is there.
/// A park has no floors.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plot {
    pub corners: Vec<[f64; 2]>,
    pub fronts: Vec<Option<u8>>,
    pub zone: Zone,
    pub floors: u32,
}

impl Plot {
    /// The road class of the largest street at the plot, or `None` when no street is there.
    pub fn front(&self) -> Option<u8> {
        self.fronts.iter().flatten().min().copied()
    }
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

/// Shows "400 plots: 300 residential, 50 commercial, 20 industrial, 30 parks, 2100 floors".
impl fmt::Debug for Plots {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let count = |z: Zone| self.0.iter().filter(|p| p.zone == z).count();
        let floors: u64 = self.0.iter().map(|p| u64::from(p.floors)).sum();
        let (r, c, i, p) = (count(Zone::Residential), count(Zone::Commercial), count(Zone::Industrial), count(Zone::Park));
        write!(f, "{} plots: {r} residential, {c} commercial, {i} industrial, {p} parks, {floors} floors", self.0.len())
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
mod road_tests;
#[cfg(test)]
mod tests;
