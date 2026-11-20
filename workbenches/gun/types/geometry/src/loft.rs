//! `Solid::loft`: a solid between two outlines at two heights, and `Solid::chamfered`.

use crate::sketch::triangulate;
use crate::{Sketch, Solid};

/// Two parts of the length of an outline that are nearer than this are one point of the loft.
const SAME: f64 = 1e-9;

type Ring = Vec<[f64; 2]>;
/// An outer outline, counterclockwise, and the holes in it, clockwise.
type Island = (Ring, Vec<Ring>);

impl Solid {
    /// The solid from the outlines of `bottom` at z = 0 to the outlines of `top` at z = `height`.
    /// Each outer outline of `bottom` joins the outer outline of `top` with the nearest center and
    /// size. Two joined outer outlines must have the same number of holes. Each hole joins the
    /// hole with the nearest center and size in the joined outer outline. A straight line joins the points at the same part of the length of two joined outlines. Each outline
    /// starts where a line from its center to +X leaves it, so the sides do not twist. Very
    /// different outlines can give sides that cross.
    pub fn loft(bottom: &Sketch, top: &Sketch, height: f64) -> Result<Solid, String> {
        if !(height.is_finite() && height > 0.0) {
            return Err(format!("the height must be more than 0, not {height}"));
        }
        let (a, b) = (islands(bottom, "bottom")?, islands(top, "top")?);
        let holes = |islands: &[Island]| islands.iter().map(|(_, holes)| holes.len()).sum::<usize>();
        if holes(&a) != holes(&b) {
            return Err(format!("the two sketches must have the same number of holes, not {} and {}", holes(&a), holes(&b)));
        }
        if a.len() != b.len() {
            return Err(format!("the two sketches must have the same number of outer outlines, not {} and {}", a.len(), b.len()));
        }
        let mut joined = Vec::new();
        for ((outer, holes), (other, other_holes)) in pairs(a, b, |(outer, _)| measure(outer)) {
            if holes.len() != other_holes.len() {
                return Err(format!("two joined outer outlines must have the same number of holes, not {} and {}", holes.len(), other_holes.len()));
            }
            joined.push(join(outer, other));
            joined.extend(pairs(holes, other_holes, measure).into_iter().map(|(a, b)| join(a, b)));
        }
        let (bottoms, tops): (Vec<Ring>, Vec<Ring>) = joined.into_iter().unzip();
        stack(&[(&bottoms, 0.0), (&tops, height)])
    }

    /// The extrusion of `sketch` from 0 to `height`, with a 45 degree chamfer of the size
    /// `chamfer` on the top edges. At the top, each edge moves `chamfer` into the material, and the
    /// corners stay sharp. A chamfer that removes an edge or that makes outlines touch gives an
    /// error.
    pub fn chamfered(sketch: &Sketch, height: f64, chamfer: f64) -> Result<Solid, String> {
        if let Some(error) = sketch.error() {
            return Err(format!("the sketch has a kernel error: {error}"));
        }
        if !(height.is_finite() && chamfer > 0.0 && chamfer < height) {
            return Err(format!("the chamfer must be more than 0 and less than the height {height}, not {chamfer}"));
        }
        let rings = sketch.outlines();
        if rings.is_empty() {
            return Ok(Solid::empty());
        }
        let top: Vec<Ring> = rings.iter().map(|ring| inset(ring, chamfer)).collect();
        // An edge that the chamfer removes turns back. Outlines that touch overlap, so the filled
        // area is not the sum of the ring areas.
        let turned = rings.iter().zip(&top).any(|(a, b)| {
            (0..a.len()).any(|i| {
                let j = (i + 1) % a.len();
                let along = (a[j][0] - a[i][0]) * (b[j][0] - b[i][0]) + (a[j][1] - a[i][1]) * (b[j][1] - b[i][1]);
                along.is_nan() || along <= 0.0
            })
        });
        let sum: f64 = top.iter().map(|ring| area(ring)).sum();
        if turned || (Sketch::of_rings(&top).area() - sum).abs() > 1e-6 * (1.0 + sum.abs()) {
            return Err(format!("the chamfer {chamfer} is too large for the sketch"));
        }
        stack(&[(&rings, 0.0), (&rings, height - chamfer), (&top, height)])
    }
}

/// The solid through the rings of each level at the height of the level, from the first level
/// to the last. All levels have the same number of rings and points, and a straight line joins
/// the points at the same index of two levels that follow.
fn stack(levels: &[(&[Ring], f64)]) -> Result<Solid, String> {
    let level = |&(rings, z): &(&[Ring], f64)| rings.iter().flatten().map(move |&[x, y]| [x as f32, y as f32, z as f32]).collect::<Vec<_>>();
    let corners: Vec<[f32; 3]> = levels.iter().flat_map(level).collect();
    let n = (corners.len() / levels.len()) as u32;
    let mut triangles = Vec::with_capacity(corners.len() * 2);
    for k in 1..levels.len() as u32 {
        let (low, high) = ((k - 1) * n, k * n);
        let mut first = 0;
        for ring in levels[0].0 {
            let m = ring.len() as u32;
            for i in 0..m {
                let (i, j) = (first + i, first + (i + 1) % m);
                triangles.extend([[low + i, low + j, high + j], [low + i, high + j, high + i]]);
            }
            first += m;
        }
    }
    let (bottom, (top, _)) = (levels[0].0, levels[levels.len() - 1]);
    // The bottom faces down, so its triangles turn the other way.
    triangles.extend(triangulate(bottom).into_iter().map(|[x, y, z]| [x, z, y]));
    let last = n * (levels.len() as u32 - 1);
    triangles.extend(triangulate(top).into_iter().map(|t| t.map(|i| i + last)));
    Solid::of_triangles(&corners, &triangles).checked()
}

/// The ring with each edge moved `distance` to its left, into the material, with sharp corners.
/// An outer outline turns counterclockwise and a hole turns clockwise, so the material is at
/// the left of each edge.
fn inset(ring: &[[f64; 2]], distance: f64) -> Ring {
    let n = ring.len();
    let left = |i: usize| {
        let (p, q) = (ring[i], ring[(i + 1) % n]);
        let length = (q[0] - p[0]).hypot(q[1] - p[1]);
        [(p[1] - q[1]) / length, (q[0] - p[0]) / length]
    };
    (0..n)
        .map(|i| {
            let (a, b) = (left((i + n - 1) % n), left(i));
            // The corner goes to where the two moved edges meet.
            let s = distance / (1.0 + a[0] * b[0] + a[1] * b[1]);
            [ring[i][0] + s * (a[0] + b[0]), ring[i][1] + s * (a[1] + b[1])]
        })
        .collect()
}

/// The outer outlines of a sketch, each with its holes.
fn islands(sketch: &Sketch, name: &str) -> Result<Vec<Island>, String> {
    if let Some(error) = sketch.error() {
        return Err(format!("the {name} sketch has a kernel error: {error}"));
    }
    let (outers, holes): (Vec<Ring>, Vec<Ring>) = sketch.outlines().into_iter().partition(|ring| area(ring) > 0.0);
    if outers.is_empty() || outers.iter().any(|ring| ring.len() < 3) {
        return Err(format!("the {name} sketch has no outer outline"));
    }
    let mut islands: Vec<Island> = outers.into_iter().map(|outer| (outer, Vec::new())).collect();
    for hole in holes {
        // A hole is in the smallest outer outline around it, because an island can be in a hole.
        let point = [(hole[0][0] + hole[1][0]) / 2.0, (hole[0][1] + hole[1][1]) / 2.0];
        let around = (0..islands.len()).filter(|&k| inside(&islands[k].0, point));
        let Some(k) = around.min_by(|&i, &j| area(&islands[i].0).total_cmp(&area(&islands[j].0))) else {
            return Err(format!("a hole of the {name} sketch is in no outer outline"));
        };
        islands[k].1.push(hole);
    }
    Ok(islands)
}

/// Each item of `a` with the item of `b` that has the nearest `measure`, in the order of `a`.
fn pairs<T>(a: Vec<T>, mut b: Vec<T>, measure: impl Fn(&T) -> [f64; 3]) -> Vec<(T, T)> {
    let mut joined = Vec::with_capacity(a.len());
    for item in a {
        let c = measure(&item);
        let far = |other: &T| measure(other).iter().zip(c).map(|(p, q)| (p - q).powi(2)).sum::<f64>();
        if let Some(k) = (0..b.len()).min_by(|&i, &j| far(&b[i]).total_cmp(&far(&b[j]))) {
            joined.push((item, b.swap_remove(k)));
        }
    }
    joined
}

/// The center of a ring and the root of its area. An island in a hole has the center of the hole,
/// so the size tells the two apart.
fn measure(ring: &Ring) -> [f64; 3] {
    let [x, y] = center(ring);
    [x, y, area(ring).abs().sqrt()]
}

/// True when `point` is in the ring, by the number of edges that a line to +X crosses.
fn inside(ring: &[[f64; 2]], [x, y]: [f64; 2]) -> bool {
    let n = ring.len();
    (0..n).filter(|&i| {
        let (p, q) = (ring[i], ring[(i + 1) % n]);
        (p[1] <= y) != (q[1] <= y) && x < p[0] + (y - p[1]) / (q[1] - p[1]) * (q[0] - p[0])
    }).count() % 2 == 1
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
