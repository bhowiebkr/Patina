//! Shape helpers for the parts. Units are meters, +X points to the muzzle, Y is the width and Z
//! is up. A profile is a list of points `[x, z]` on the side of the gun.

use geometry::{Sketch, Solid};

/// A plate with the profile on the X-Z plane and the thickness `t` on Y, centered on Y.
pub fn slab(profile: &[[f64; 2]], t: f64) -> Solid {
    Solid::extrude(&Sketch::polygon(profile), t, 0.0, [1.0; 2], 0).rotate([90.0, 0.0, 0.0]).translate([0.0, t / 2.0, 0.0])
}

/// A convex profile with each edge moved inward by `d`.
pub fn inset(points: &[[f64; 2]], d: f64) -> Vec<[f64; 2]> {
    let n = points.len();
    let area: f64 = (0..n).map(|i| points[i][0] * points[(i + 1) % n][1] - points[(i + 1) % n][0] * points[i][1]).sum();
    let d = if area > 0.0 { d } else { -d };
    // Each edge as a point and a direction, moved to its left side (the inside of a counterclockwise profile).
    let edge = |i: usize| {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dz).max(1e-12);
        ([a[0] - dz / len * d, a[1] + dx / len * d], [dx, dz])
    };
    (0..n)
        .map(|i| {
            let ((p, u), (q, v)) = (edge((i + n - 1) % n), edge(i));
            let cross = u[0] * v[1] - u[1] * v[0];
            if cross.abs() < 1e-12 {
                return q;
            }
            let t = ((q[0] - p[0]) * v[1] - (q[1] - p[1]) * v[0]) / cross;
            [p[0] + u[0] * t, p[1] + u[1] * t]
        })
        .collect()
}

/// A convex plate of the thickness `t` with each edge of its two faces cut by `b`.
pub fn plate(profile: &[[f64; 2]], t: f64, b: f64) -> Solid {
    slab(&inset(profile, b), t).union(&slab(profile, t - 2.0 * b)).hull()
}

/// A box from `x[0]` to `x[1]` and from `z[0]` to `z[1]`, with the width `w` centered on Y.
pub fn block(x: [f64; 2], z: [f64; 2], w: f64) -> Solid {
    Solid::cube(x[1] - x[0], w, z[1] - z[0]).translate([(x[0] + x[1]) / 2.0, 0.0, (z[0] + z[1]) / 2.0])
}

/// A box like `block` with each edge cut by `b`.
pub fn bevel(x: [f64; 2], z: [f64; 2], w: f64, b: f64) -> Solid {
    let (sx, sz) = (x[1] - x[0], z[1] - z[0]);
    let b = b.min(sx.min(w).min(sz) * 0.45);
    let parts = [Solid::cube(sx, w - 2.0 * b, sz - 2.0 * b), Solid::cube(sx - 2.0 * b, w, sz - 2.0 * b), Solid::cube(sx - 2.0 * b, w - 2.0 * b, sz)];
    Solid::union_all(&parts).hull().translate([(x[0] + x[1]) / 2.0, 0.0, (z[0] + z[1]) / 2.0])
}

/// The hull of round bars along Y: each item is a center `[x, z]` and a radius. The side edges
/// are cut by `b`, so the profile is round and the sides are flat.
pub fn pill(circles: &[([f64; 2], f64)], w: f64, b: f64) -> Solid {
    let bar = |c: ([f64; 2], f64), r: f64, t: f64| rod_y(c.0, t, r, 24);
    let mut bars: Vec<Solid> = circles.iter().map(|&c| bar(c, c.1, w - 2.0 * b)).collect();
    bars.extend(circles.iter().map(|&c| bar(c, (c.1 - b).max(0.0005), w)));
    Solid::union_all(&bars).hull()
}

/// A solid of revolution around the X axis. Each point of the closed `profile` is
/// `[x, radius]`. Add points with the radius 0 for a solid without a hole.
pub fn lathe(profile: &[[f64; 2]], segments: i32) -> Solid {
    let points: Vec<[f64; 2]> = profile.iter().map(|&[x, r]| [r, x]).collect();
    Solid::revolve(&Sketch::polygon(&points), 360.0, segments).rotate([0.0, 90.0, 0.0])
}

/// A solid lathe from the open `profile` of `[x, radius]`, closed on the axis at its two ends.
pub fn turned(profile: &[[f64; 2]], segments: i32) -> Solid {
    let mut points = vec![[profile[0][0], 0.0]];
    points.extend_from_slice(profile);
    points.push([profile[profile.len() - 1][0], 0.0]);
    lathe(&points, segments)
}

/// A tube along X from `x[0]` to `x[1]`, with the outer radius `r` and the inner radius `hole`.
pub fn tube(x: [f64; 2], r: f64, hole: f64, segments: i32) -> Solid {
    lathe(&[[x[0], hole], [x[1], hole], [x[1], r], [x[0], r]], segments)
}

/// A cylinder along X from `x[0]` to `x[1]`, with the radius `r[0]` at `x[0]` and `r[1]` at `x[1]`.
pub fn rod_x(x: [f64; 2], r: [f64; 2], segments: i32) -> Solid {
    Solid::cylinder(x[1] - x[0], r[0], r[1], segments).rotate([0.0, 90.0, 0.0]).translate([(x[0] + x[1]) / 2.0, 0.0, 0.0])
}

/// A cylinder along Y with the length `len`, centered at `[x, 0, z]`.
pub fn rod_y([x, z]: [f64; 2], len: f64, r: f64, segments: i32) -> Solid {
    Solid::cylinder(len, r, r, segments).rotate([90.0, 0.0, 0.0]).translate([x, 0.0, z])
}

/// A cylinder along Z from `z[0]` to `z[1]` at `[x, 0]`, with the radius `r[0]` at `z[0]`.
pub fn rod_z(x: f64, z: [f64; 2], r: [f64; 2], segments: i32) -> Solid {
    Solid::cylinder(z[1] - z[0], r[0], r[1], segments).translate([x, 0.0, (z[0] + z[1]) / 2.0])
}

/// A round bar from `a` to `b` on the X-Z plane, with the radius `r`.
pub fn bar(a: [f64; 2], b: [f64; 2], r: f64, segments: i32) -> Solid {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let turn = dx.atan2(dz).to_degrees();
    Solid::cylinder(dx.hypot(dz), r, r, segments).rotate([0.0, turn, 0.0]).translate([(a[0] + b[0]) / 2.0, 0.0, (a[1] + b[1]) / 2.0])
}

/// Round bars through the points `[x, z]`, with a ball at each bend.
pub fn wire(points: &[[f64; 2]], r: f64) -> Solid {
    let mut parts: Vec<Solid> = points.windows(2).map(|p| bar(p[0], p[1], r, 8)).collect();
    parts.extend(points[1..points.len() - 1].iter().map(|p| Solid::sphere(r, 8).translate([p[0], 0.0, p[1]])));
    Solid::union_all(&parts)
}

/// A ring around the X axis at `x`, with the radius `radius` and the thickness `t`.
pub fn ring_x(x: f64, radius: f64, t: f64) -> Solid {
    Solid::revolve(&Sketch::circle(t / 2.0, 6).translate([radius, 0.0]), 360.0, 20).rotate([0.0, 90.0, 0.0]).translate([x, 0.0, 0.0])
}

/// A ring around the Y axis, on the X-Z plane, with its center at `[x, z]`.
pub fn ring_y([x, z]: [f64; 2], radius: f64, t: f64) -> Solid {
    Solid::revolve(&Sketch::circle(t / 2.0, 6).translate([radius, 0.0]), 360.0, 20).rotate([90.0, 0.0, 0.0]).translate([x, 0.0, z])
}

/// Hex bolt heads at each point `[x, z]` on the two faces of a part of the width `w`.
pub fn bolts(points: &[[f64; 2]], w: f64, r: f64) -> Solid {
    Solid::union_all(&points.iter().map(|&p| rod_y(p, w + 0.005, r, 6)).collect::<Vec<_>>())
}

/// `count` slots along X, from `x[0]` to `x[1]`, each from `z[0]` to `z[1]`, through the width `w`.
/// Subtract them for vents.
pub fn slots(x: [f64; 2], z: [f64; 2], count: usize, w: f64) -> Solid {
    let step = (x[1] - x[0]) / count as f64;
    Solid::union_all(&(0..count).map(|i| block([x[0] + step * (i as f64 + 0.25), x[0] + step * (i as f64 + 0.75)], z, w)).collect::<Vec<_>>())
}

/// A rail along X on the plane `z = 0`, with notches on its top, the width `w` and the height `t`.
pub fn rail(x: [f64; 2], w: f64, t: f64) -> Solid {
    let teeth = ((x[1] - x[0]) / 0.011).floor().max(1.0) as usize;
    let base = bevel(x, [0.0, t * 0.55], w, t * 0.15);
    base.union(&slots(x, [t * 0.45, t], teeth, w * 1.15).intersection(&block(x, [0.0, t], w * 1.15)))
}

/// The same solid on the other side of the X-Z plane, added.
pub fn both(solid: &Solid) -> Solid {
    solid.union(&solid.mirror([0.0, 1.0, 0.0]))
}

/// A thin plate on the face `y = side` of a part, for stripes, panels and glow strips. It sticks
/// out by `out` and goes in by the same depth.
pub fn decal(profile: &[[f64; 2]], side: f64, out: f64) -> Solid {
    slab(profile, out * 2.0).translate([0.0, side, 0.0])
}

/// A groove along `profile` in the faces `y = side` and `y = -side`, and the strip that fills it a
/// little under the face. Subtract the groove from the body, then add the strip.
pub fn inlay(profile: &[[f64; 2]], side: f64, depth: f64) -> (Solid, Solid) {
    (both(&decal(profile, side, depth)), both(&slab(profile, depth).translate([0.0, side - depth * 0.6, 0.0])))
}

/// A band of tape or a strap around a plate with the profile `profile` and the width `w`, in the
/// volume `cut`. It sticks out of the plate by `out`.
pub fn wrap(profile: &[[f64; 2]], w: f64, cut: &Solid, out: f64) -> Solid {
    slab(&inset(profile, -out), w + 2.0 * out).intersection(cut)
}

/// A thin band of the height `t` from `a` to `b`, as a profile.
pub fn strip(a: [f64; 2], b: [f64; 2], t: f64) -> Vec<[f64; 2]> {
    vec![a, b, [b[0], b[1] + t], [a[0], a[1] + t]]
}

/// The points turned by `degrees` around their center, counterclockwise as seen from -Y.
pub fn turn(points: &[[f64; 2]], degrees: f64) -> Vec<[f64; 2]> {
    let n = points.len() as f64;
    let c = points.iter().fold([0.0; 2], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
    let (sin, cos) = degrees.to_radians().sin_cos();
    points.iter().map(|p| [c[0] + (p[0] - c[0]) * cos - (p[1] - c[1]) * sin, c[1] + (p[0] - c[0]) * sin + (p[1] - c[1]) * cos]).collect()
}

/// A parallelogram from `x` to `x + width` at `z[0]`, leaning by `lean` at `z[1]`.
pub fn stripe(x: f64, width: f64, z: [f64; 2], lean: f64) -> Vec<[f64; 2]> {
    vec![[x, z[0]], [x + width, z[0]], [x + width + lean, z[1]], [x + lean, z[1]]]
}

/// The corners of a rectangle from `[x0, z0]` to `[x1, z1]`, with each corner cut by `c`.
pub fn chamfer([x0, z0]: [f64; 2], [x1, z1]: [f64; 2], c: f64) -> Vec<[f64; 2]> {
    vec![[x0 + c, z0], [x1 - c, z0], [x1, z0 + c], [x1, z1 - c], [x1 - c, z1], [x0 + c, z1], [x0, z1 - c], [x0, z0 + c]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inset_moves_each_edge_inward() {
        // A square of 2 x 2 in both turn directions gives a square of 1.6 x 1.6.
        let square = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
        let mut back = square;
        back.reverse();
        for points in [&square[..], &back[..]] {
            for p in inset(points, 0.2) {
                assert!(p.iter().all(|v| (v - 0.2).abs() < 1e-9 || (v - 1.8).abs() < 1e-9), "{p:?}");
            }
        }
    }

    #[test]
    fn the_helpers_give_valid_solids() {
        let solids = [
            plate(&chamfer([0.0, 0.0], [0.1, 0.05], 0.01), 0.03, 0.004),
            bevel([0.0, 0.1], [0.0, 0.05], 0.03, 0.005),
            pill(&[([0.0, 0.0], 0.02), ([0.1, 0.01], 0.01)], 0.03, 0.004),
            turned(&[[0.0, 0.02], [0.1, 0.015]], 16),
            tube([0.0, 0.1], 0.02, 0.01, 16),
            rail([0.0, 0.1], 0.02, 0.008),
            ring_x(0.0, 0.02, 0.004),
        ];
        for (i, solid) in solids.iter().enumerate() {
            assert_eq!(solid.error(), None, "{i}");
            assert!(solid.triangles() > 0, "{i}");
        }
        // The plate is inside its profile and as thick as asked.
        let [lo, hi] = solids[0].bounds();
        assert!((hi[1] - lo[1] - 0.03).abs() < 1e-6 && hi[0] <= 0.1 + 1e-6 && lo[2] >= -1e-6, "{lo:?} {hi:?}");
    }
}
