//! `Solid::loft`: a solid between two outlines at two heights.

use crate::sketch::triangulate;
use crate::{Sketch, Solid};

/// Two parts of the length of an outline that are nearer than this are one point of the loft.
const SAME: f64 = 1e-9;

type Ring = Vec<[f64; 2]>;

impl Solid {
    /// The solid from the outlines of `bottom` at z = 0 to the outlines of `top` at z = `height`.
    /// Each sketch must have one outer outline, and the two sketches must have the same number of
    /// holes. Each hole of `bottom` joins the hole of `top` with the nearest center. A straight
    /// line joins the points at the same part of the length of two joined outlines. Each outline
    /// starts where a line from its center to +X leaves it, so the sides do not twist. Very
    /// different outlines can give sides that cross.
    pub fn loft(bottom: &Sketch, top: &Sketch, height: f64) -> Result<Solid, String> {
        if !(height.is_finite() && height > 0.0) {
            return Err(format!("the height must be more than 0, not {height}"));
        }
        let (mut a, mut b) = (rings(bottom, "bottom")?, rings(top, "top")?);
        if a.len() != b.len() {
            return Err(format!("the two sketches must have the same number of holes, not {} and {}", a.len() - 1, b.len() - 1));
        }
        let mut joined = vec![join(a.remove(0), b.remove(0))];
        for hole in a {
            let c = center(&hole);
            let far = |ring: &Ring| center(ring).iter().zip(c).map(|(p, q)| (p - q).powi(2)).sum::<f64>();
            if let Some(k) = (0..b.len()).min_by(|&i, &j| far(&b[i]).total_cmp(&far(&b[j]))) {
                joined.push(join(hole, b.swap_remove(k)));
            }
        }
        let (bottoms, tops): (Vec<Ring>, Vec<Ring>) = joined.into_iter().unzip();
        let level = |rings: &[Ring], z: f64| rings.iter().flatten().map(|&[x, y]| [x as f32, y as f32, z as f32]).collect::<Vec<_>>();
        let corners = [level(&bottoms, 0.0), level(&tops, height)].concat();
        let n = (corners.len() / 2) as u32;
        let mut triangles = Vec::with_capacity(corners.len() * 4);
        let mut first = 0;
        for ring in &bottoms {
            let m = ring.len() as u32;
            for i in 0..m {
                let (i, j) = (first + i, first + (i + 1) % m);
                triangles.extend([[i, j, n + j], [i, n + j, n + i]]);
            }
            first += m;
        }
        // The bottom faces down, so its triangles turn the other way.
        triangles.extend(triangulate(&bottoms).into_iter().map(|[x, y, z]| [x, z, y]));
        triangles.extend(triangulate(&tops).into_iter().map(|t| t.map(|i| i + n)));
        Solid::of_triangles(&corners, &triangles).checked()
    }
}

/// The outlines of a sketch: the one outer outline, counterclockwise, and then the holes,
/// clockwise.
fn rings(sketch: &Sketch, name: &str) -> Result<Vec<Ring>, String> {
    let (mut outer, holes): (Vec<Ring>, Vec<Ring>) = sketch.outlines().into_iter().partition(|ring| area(ring) > 0.0);
    if outer.len() != 1 || outer[0].len() < 3 {
        return Err(format!("the {name} sketch must have one outer outline"));
    }
    outer.extend(holes);
    Ok(outer)
}

/// The two rings with the same number of points. The points at the same index are at the same
/// part of the length of the two outlines.
fn join(a: Ring, b: Ring) -> (Ring, Ring) {
    let (a, b) = (start(a), start(b));
    let (ta, tb) = (parts(&a), parts(&b));
    // Each corner of each outline is a point of both rings, so the loft keeps all corners.
    let mut ts: Vec<f64> = ta.iter().chain(&tb).copied().collect();
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|t, kept| *t - *kept < SAME);
    (ts.iter().map(|&t| at(&a, &ta, t)).collect(), ts.iter().map(|&t| at(&b, &tb, t)).collect())
}

/// The area of a ring, more than 0 when it turns counterclockwise.
fn area(ring: &[[f64; 2]]) -> f64 {
    let n = ring.len();
    (0..n).map(|i| ring[i][0] * ring[(i + 1) % n][1] - ring[(i + 1) % n][0] * ring[i][1]).sum::<f64>() / 2.0
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
