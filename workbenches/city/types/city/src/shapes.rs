//! Mesh parts for the roads and the buildings: faces, boxes and rings, and tests of points in
//! polygons.

use geometry::{Frame, Mesh, Place, Sketch, Solid};

use crate::terrain::{face_normal, push, segment_distance};
use crate::{cross, sub};

/// A point is on an edge when it is nearer than this, in meters.
const NEAR: f64 = 1e-6;

/// One triangle with counterclockwise corners and its face normal.
pub(crate) fn tri(mesh: &mut Mesh, corners: [[f64; 3]; 3]) {
    let t = corners.map(|p| p.map(|c| c as f32));
    push(mesh, t, [face_normal(t); 3]);
}

/// Two triangles for a quad with counterclockwise corners, each with its face normal.
pub(crate) fn quad(mesh: &mut Mesh, [a, b, c, d]: [[f64; 3]; 4]) {
    tri(mesh, [a, b, c]);
    tri(mesh, [a, c, d]);
}

/// A box from `low` to `high` on the axes.
pub(crate) fn cuboid(mesh: &mut Mesh, low: [f64; 3], high: [f64; 3]) {
    let [x0, y0, z0] = low;
    let [x1, y1, z1] = high;
    let ring = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    sides(mesh, &ring, z0, z1, true);
    quad(mesh, ring.map(|[x, y]| [x, y, z1]));
    quad(mesh, [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]]);
}

/// The walls of the prism of a counterclockwise polygon from `z0` to `z1`, which face out, or
/// in when `out` is false.
pub(crate) fn sides(mesh: &mut Mesh, corners: &[[f64; 2]], z0: f64, z1: f64, out: bool) {
    let n = corners.len();
    for i in 0..n {
        let (p, q) = (corners[i], corners[(i + 1) % n]);
        let (a, b) = if out { (p, q) } else { (q, p) };
        quad(mesh, [[a[0], a[1], z0], [b[0], b[1], z0], [b[0], b[1], z1], [a[0], a[1], z1]]);
    }
}

/// A flat ring at `z` between two polygons with the same number of corners. It faces up, or
/// down when `up` is false.
pub(crate) fn ring(mesh: &mut Mesh, outer: &[[f64; 2]], inner: &[[f64; 2]], z: f64, up: bool) {
    let n = outer.len().min(inner.len());
    for i in 0..n {
        let j = (i + 1) % n;
        let face = [outer[i], outer[j], inner[j], inner[i]].map(|p| [p[0], p[1], z]);
        quad(mesh, if up { face } else { [face[3], face[2], face[1], face[0]] });
    }
}

/// A band out from a wall: the outer faces from `z0` to `z1`, the top and the bottom.
pub(crate) fn band(mesh: &mut Mesh, wall: &[[f64; 2]], outer: &[[f64; 2]], z0: f64, z1: f64) {
    sides(mesh, outer, z0, z1, true);
    ring(mesh, outer, wall, z1, true);
    ring(mesh, outer, wall, z0, false);
}

/// A low wall around a roof: the outer and the inner faces and the top.
pub(crate) fn parapet(mesh: &mut Mesh, outer: &[[f64; 2]], inner: &[[f64; 2]], z0: f64, z1: f64) {
    sides(mesh, outer, z0, z1, true);
    sides(mesh, inner, z0, z1, false);
    ring(mesh, outer, inner, z1, true);
}

/// Adds the triangles of `mesh` to `to`, all in part 0.
pub(crate) fn append(to: &mut Mesh, mesh: &Mesh) {
    to.positions.extend_from_slice(&mesh.positions);
    to.normals.extend_from_slice(&mesh.normals);
    to.uvs.extend_from_slice(&mesh.uvs);
    to.parts.resize(to.parts.len() + mesh.triangles(), 0);
}

/// The place of a mesh with its Y axis along `out` on the ground, Z up and X to the right of
/// `out` seen from above, so the frame keeps the turn of the faces. `size` scales the axes.
pub(crate) fn place(origin: [f64; 3], out: [f64; 2], size: [f64; 3], group: u32) -> Place {
    let frame = Frame {
        x: [out[1] * size[0], -out[0] * size[0], 0.0],
        y: [out[0] * size[1], out[1] * size[1], 0.0],
        z: [0.0, 0.0, size[2]],
        origin,
    };
    Place::new(&frame, group)
}

/// The polygon with each edge moved out by `d`, or in for a negative `d`. It keeps all the
/// corners, so it is only for small distances.
pub(crate) fn grow(corners: &[[f64; 2]], d: f64) -> Vec<[f64; 2]> {
    let n = corners.len();
    let normal = |i: usize| {
        let e = sub(corners[(i + 1) % n], corners[i]);
        let l = crate::length(e).max(1e-12);
        [e[1] / l, -e[0] / l]
    };
    (0..n)
        .map(|i| {
            let (a, b) = (normal((i + n - 1) % n), normal(i));
            let k = d / (1.0 + crate::dot(a, b)).max(0.25);
            [corners[i][0] + (a[0] + b[0]) * k, corners[i][1] + (a[1] + b[1]) * k]
        })
        .collect()
}

/// A slab under the quad `top` with counterclockwise corners: the top, the bottom `thick` lower
/// and the four edges.
pub(crate) fn slab(mesh: &mut Mesh, top: [[f64; 3]; 4], thick: f64) {
    let low = top.map(|[x, y, z]| [x, y, z - thick]);
    quad(mesh, top);
    quad(mesh, [low[3], low[2], low[1], low[0]]);
    for i in 0..4 {
        let j = (i + 1) % 4;
        quad(mesh, [low[i], low[j], top[j], top[i]]);
    }
}

/// The closed prism of a polygon from `z0` to `z1`.
pub(crate) fn prism(corners: &[[f64; 2]], z0: f64, z1: f64) -> Mesh {
    Solid::extrude(&Sketch::polygon(corners), z1 - z0, 0.0, [1.0; 2], 0).translate([0.0, 0.0, z0]).mesh()
}

/// True when `p` is in the polygon or on its edge.
pub(crate) fn inside(polygon: &[[f64; 2]], p: [f64; 2]) -> bool {
    let n = polygon.len();
    let mut odd = false;
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        if segment_distance(p, a, b) < NEAR {
            return true;
        }
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) {
            odd = !odd;
        }
    }
    odd
}

/// True when the polygon `shape` is in the polygon `outer`. Edges and corners can touch.
pub(crate) fn contains(outer: &[[f64; 2]], shape: &[[f64; 2]]) -> bool {
    let edges = |poly: &[[f64; 2]]| {
        let n = poly.len();
        (0..n).map(move |i| (poly[i], poly[(i + 1) % n])).collect::<Vec<_>>()
    };
    let near_edge = |poly: &[[f64; 2]], p: [f64; 2]| edges(poly).iter().any(|&(a, b)| segment_distance(p, a, b) < NEAR);
    if !shape.iter().all(|&p| inside(outer, p)) || outer.iter().any(|&p| inside(shape, p) && !near_edge(shape, p)) {
        return false;
    }
    let (inner, outer) = (edges(shape), edges(outer));
    let side = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
        let c = cross(sub(b, a), sub(p, a));
        if c.abs() < NEAR * crate::length(sub(b, a)) { 0.0 } else { c.signum() }
    };
    !inner.iter().any(|&(p, q)| outer.iter().any(|&(a, b)| side(p, q, a) * side(p, q, b) < 0.0 && side(a, b, p) * side(a, b, q) < 0.0))
}
