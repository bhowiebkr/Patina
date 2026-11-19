//! The street network: a grid with random offsets of the crossings (task E1).

use geometry::Seed;

use crate::{Edge, Streets};

/// The most cells on one side of the grid, so that a small spacing cannot fill the memory.
pub const MAX_CELLS: usize = 200;

/// The input of `grid`.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    /// The size of the area along X and Y.
    pub width: f64,
    pub depth: f64,
    /// The distance between two streets.
    pub spacing: f64,
    /// The largest random offset of a crossing, as a part of the cell from 0 to 0.45.
    pub jitter: f64,
    /// The part of the inner streets that drop out, from 0 to 1.
    pub drop: f64,
    /// Each `avenue`th line is an avenue. 0 gives no avenues.
    pub avenue: u32,
    /// The widths of a street and of an avenue.
    pub street: f64,
    pub avenue_width: f64,
}

/// A grid of streets in the area, with the center at the origin. An avenue stays straight and
/// does not drop out. A street drops out only when its two crossings have all four streets, so
/// each crossing loses one street at most and the graph stays connected.
pub fn grid(seed: Seed, g: &Grid) -> Result<Streets, String> {
    let positive = |v: f64| v.is_finite() && v > 0.0;
    if ![g.width, g.depth, g.spacing, g.street, g.avenue_width].into_iter().all(positive) {
        return Err("width, depth, spacing, street and avenue width must be more than 0".into());
    }
    if !(0.0..=0.45).contains(&g.jitter) || !(0.0..=1.0).contains(&g.drop) {
        return Err("jitter must be from 0 to 0.45 and drop from 0 to 1".into());
    }
    if g.street.max(g.avenue_width) >= g.spacing {
        return Err("a street must be narrower than the spacing".into());
    }
    let cells = |size: f64| (size / g.spacing).round().max(1.0);
    let (nx, ny) = (cells(g.width), cells(g.depth));
    if nx > MAX_CELLS as f64 || ny > MAX_CELLS as f64 {
        return Err(format!("the grid can have {MAX_CELLS} cells on a side, not {nx} x {ny}"));
    }
    let (nx, ny) = (nx as usize, ny as usize);
    let avenue = |k: usize| g.avenue > 0 && k.is_multiple_of(g.avenue as usize);
    let (dx, dy) = (g.width / nx as f64, g.depth / ny as f64);
    let mut random = seed.child("streets").random();
    let mut points = Vec::with_capacity((nx + 1) * (ny + 1));
    for j in 0..=ny {
        for i in 0..=nx {
            let mut p = [i as f64 * dx - g.width / 2.0, j as f64 * dy - g.depth / 2.0];
            let (ox, oy) = (random.range(-g.jitter, g.jitter), random.range(-g.jitter, g.jitter));
            if i > 0 && i < nx && j > 0 && j < ny {
                p[0] += if avenue(i) { 0.0 } else { ox * dx };
                p[1] += if avenue(j) { 0.0 } else { oy * dy };
            }
            points.push(p);
        }
    }
    let at = |i: usize, j: usize| j * (nx + 1) + i;
    let width = |on_avenue: bool| if on_avenue { g.avenue_width } else { g.street };
    let mut edges = Vec::with_capacity(2 * (nx + 1) * (ny + 1));
    // An inner street is not on the outer ring, so it can drop out.
    let mut inner = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            if i < nx {
                inner.push(j > 0 && j < ny && !avenue(j));
                edges.push(Edge { a: at(i, j), b: at(i + 1, j), width: width(avenue(j)) });
            }
            if j < ny {
                inner.push(i > 0 && i < nx && !avenue(i));
                edges.push(Edge { a: at(i, j), b: at(i, j + 1), width: width(avenue(i)) });
            }
        }
    }
    let mut degree = vec![0_u8; points.len()];
    for e in &edges {
        degree[e.a] += 1;
        degree[e.b] += 1;
    }
    let mut keep = vec![true; edges.len()];
    for (k, e) in edges.iter().enumerate() {
        // Each candidate takes a number, so a change of `drop` keeps the other streets.
        let r = random.unit();
        if inner[k] && r < g.drop && degree[e.a] == 4 && degree[e.b] == 4 {
            keep[k] = false;
            degree[e.a] -= 1;
            degree[e.b] -= 1;
        }
    }
    let edges = edges.into_iter().zip(keep).filter_map(|(e, k)| k.then_some(e)).collect();
    Ok(Streets { points, edges })
}
