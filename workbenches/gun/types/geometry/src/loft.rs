//! `Solid::loft`: a solid between two outlines at two heights.

use crate::sketch::triangulate;
use crate::{Sketch, Solid};

/// Two parts of the length of an outline that are nearer than this are one point of the loft.
const SAME: f64 = 1e-9;

impl Solid {
    /// The solid from the outline of `bottom` at z = 0 to the outline of `top` at z = `height`.
    /// Each sketch must have one outline and no hole. A straight line joins the points at the same
    /// part of the length of the two outlines. Each outline starts where a line from its center
    /// to +X leaves it, so the sides do not twist. Very different outlines can give sides that
    /// cross.
    pub fn loft(bottom: &Sketch, top: &Sketch, height: f64) -> Result<Solid, String> {
        if !(height.is_finite() && height > 0.0) {
            return Err(format!("the height must be more than 0, not {height}"));
        }
        let (a, b) = (start(outline(bottom, "bottom")?), start(outline(top, "top")?));
        let (ta, tb) = (parts(&a), parts(&b));
        // Each corner of each outline is a point of both rings, so the loft keeps all corners.
        let mut ts: Vec<f64> = ta.iter().chain(&tb).copied().collect();
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|t, kept| *t - *kept < SAME);
        let ring_a: Vec<[f64; 2]> = ts.iter().map(|&t| at(&a, &ta, t)).collect();
        let ring_b: Vec<[f64; 2]> = ts.iter().map(|&t| at(&b, &tb, t)).collect();
        let level = |ring: &[[f64; 2]], z: f64| ring.iter().map(|&[x, y]| [x as f32, y as f32, z as f32]).collect::<Vec<_>>();
        let corners = [level(&ring_a, 0.0), level(&ring_b, height)].concat();
        let n = ts.len() as u32;
        let mut triangles = Vec::with_capacity(ts.len() * 4);
        for i in 0..n {
            let j = (i + 1) % n;
            triangles.extend([[i, j, n + j], [i, n + j, n + i]]);
        }
        // The bottom faces down, so its triangles turn the other way.
        triangles.extend(triangulate(&ring_a).into_iter().map(|[x, y, z]| [x, z, y]));
        triangles.extend(triangulate(&ring_b).into_iter().map(|t| t.map(|i| i + n)));
        Solid::of_triangles(&corners, &triangles).checked()
    }
}

/// The one outline of a sketch, counterclockwise.
fn outline(sketch: &Sketch, name: &str) -> Result<Vec<[f64; 2]>, String> {
    let mut outlines = sketch.outlines();
    match (outlines.pop(), outlines.is_empty()) {
        (Some(outline), true) if outline.len() >= 3 => Ok(outline),
        _ => Err(format!("the {name} sketch must have one outline and no hole")),
    }
}

/// The outline from the point where a line from its center to +X leaves it.
fn start(ring: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    let [cx, cy] = center(&ring);
    let n = ring.len();
    let mut exit: Option<(f64, usize)> = None;
    for i in 0..n {
        let (p, q) = (ring[i], ring[(i + 1) % n]);
        if (p[1] <= cy) != (q[1] <= cy) {
            let x = p[0] + (cy - p[1]) / (q[1] - p[1]) * (q[0] - p[0]);
            if x >= cx && exit.is_none_or(|(far, _)| x > far) {
                exit = Some((x, i));
            }
        }
    }
    let Some((x, i)) = exit else { return ring };
    let mut out = vec![[x, cy]];
    out.extend(ring[i + 1..].iter().chain(&ring[..=i]).filter(|p| (p[0] - x).hypot(p[1] - cy) > SAME));
    out
}

/// The center of the area of a ring, or the mean of its corners for a ring with no area.
fn center(ring: &[[f64; 2]]) -> [f64; 2] {
    let (mut area, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for (i, p) in ring.iter().enumerate() {
        let q = ring[(i + 1) % ring.len()];
        let cross = p[0] * q[1] - q[0] * p[1];
        area += cross;
        cx += (p[0] + q[0]) * cross;
        cy += (p[1] + q[1]) * cross;
    }
    if area.abs() < SAME {
        let n = ring.len() as f64;
        return [ring.iter().map(|p| p[0]).sum::<f64>() / n, ring.iter().map(|p| p[1]).sum::<f64>() / n];
    }
    [cx / (3.0 * area), cy / (3.0 * area)]
}

/// The part of the length of the ring at each corner, from 0 at the first corner to below 1.
fn parts(ring: &[[f64; 2]]) -> Vec<f64> {
    let mut sums = Vec::with_capacity(ring.len());
    let mut sum = 0.0;
    for (i, p) in ring.iter().enumerate() {
        sums.push(sum);
        let q = ring[(i + 1) % ring.len()];
        sum += (q[0] - p[0]).hypot(q[1] - p[1]);
    }
    sums.iter().map(|s| if sum > 0.0 { s / sum } else { 0.0 }).collect()
}

/// The point at the part `t` of the length of the ring. `ts` is the part at each corner.
fn at(ring: &[[f64; 2]], ts: &[f64], t: f64) -> [f64; 2] {
    let k = ts.partition_point(|&s| s <= t).saturating_sub(1);
    let (p, q) = (ring[k], ring[(k + 1) % ring.len()]);
    let span = ts.get(k + 1).copied().unwrap_or(1.0) - ts[k];
    let u = if span > 0.0 { (t - ts[k]) / span } else { 0.0 };
    [p[0] + (q[0] - p[0]) * u, p[1] + (q[1] - p[1]) * u]
}
