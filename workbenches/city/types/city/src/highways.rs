//! The highways (task E10): the paths of least cost from gates on the edge to the main center.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::f64::consts::TAU;

use geometry::Seed;

use crate::grow::{Network, planarize};
use crate::{Density, HIGHWAY, Streets, Terrain, add, length, scale, sub};

/// The most gates of the highways.
pub const MAX_GATES: u32 = 12;

/// The steps of the path search: the 8 neighbors and the 8 knight moves, for smoother paths.
const STEPS: [(i64, i64); 16] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1), (1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)];

/// Highways from `gates` points on the edge of the area to the main center of the density
/// map. A step costs its length, more on a steep slope, and `bridge` times its length more
/// over water. The paths share their common parts, so the highways make a tree.
pub fn highways(terrain: &Terrain, density: &Density, seed: Seed, gates: u32, width: f64, bridge: f64) -> Result<Streets, String> {
    if !(1..=MAX_GATES).contains(&gates) || !(width.is_finite() && width > 0.0) || !(bridge.is_finite() && bridge >= 0.0) {
        return Err(format!("gates must be from 1 to {MAX_GATES}, the width more than 0 and the bridge 0 or more"));
    }
    let t = terrain;
    if t.nx < 2 || t.ny < 2 || t.heights.len() != t.nx * t.ny {
        return Err("the terrain has no samples".into());
    }
    let Some(&center) = density.centers.first() else { return Err("the density map has no center".into()) };
    let (w, d) = (t.width(), t.depth());
    let step = t.cell.max(8.0).max(w.max(d) / 400.0);
    let (nx, ny) = ((w / step) as usize + 1, (d / step) as usize + 1);
    let point = |n: usize| [(n % nx) as f64 * step - w / 2.0, (n / nx) as f64 * step - d / 2.0];
    let heights: Vec<f64> = (0..nx * ny).map(|n| t.height(point(n))).collect();
    let node = |p: [f64; 2]| {
        let i = ((p[0] + w / 2.0) / step).round().clamp(0.0, (nx - 1) as f64) as usize;
        let j = ((p[1] + d / 2.0) / step).round().clamp(0.0, (ny - 1) as f64) as usize;
        j * nx + i
    };
    // One search from the center gives the cheapest path from each node.
    let start = node(center);
    let (mut cost, mut parent) = (vec![f64::INFINITY; nx * ny], vec![usize::MAX; nx * ny]);
    let mut heap = BinaryHeap::from([Reverse((0_u64, start))]);
    cost[start] = 0.0;
    while let Some(Reverse((bits, n))) = heap.pop() {
        if f64::from_bits(bits) > cost[n] {
            continue;
        }
        let (i, j) = ((n % nx) as i64, (n / nx) as i64);
        for (di, dj) in STEPS {
            let (a, b) = (i + di, j + dj);
            if a < 0 || b < 0 || a >= nx as i64 || b >= ny as i64 {
                continue;
            }
            let m = b as usize * nx + a as usize;
            let run = step * ((di * di + dj * dj) as f64).sqrt();
            let rise = (heights[m] - heights[n]).abs() / run;
            let wet = heights[m] < t.sea || t.water(scale(add(point(n), point(m)), 0.5));
            let c = cost[n] + run * (1.0 + 30.0 * rise * rise + if wet { bridge } else { 0.0 });
            if c < cost[m] {
                (cost[m], parent[m]) = (c, n);
                heap.push(Reverse((c.to_bits(), m)));
            }
        }
    }
    // A gate is a land point on the edge, in its own sector around the center.
    let mut random = seed.child("gates").random();
    let (sector, turn) = (TAU / f64::from(gates), random.range(0.0, TAU));
    let mut ends = Vec::new();
    for k in 0..gates {
        let base = turn + (f64::from(k) + random.range(0.25, 0.75)) * sector;
        let found = [0.0, 0.1, -0.1, 0.2, -0.2, 0.3, -0.3, 0.4, -0.4].into_iter().map(|f| node(edge_point(center, base + f * sector, w, d))).find(|&n| heights[n] >= t.sea && cost[n].is_finite() && n != start);
        ends.extend(found.filter(|n| !ends.contains(n)));
    }
    // The tree of the paths: a chain goes from a gate or a fork up to the next fork or the center.
    let mut used = vec![false; nx * ny];
    let mut children = vec![0_u32; nx * ny];
    for &end in &ends {
        let mut n = end;
        while n != start && !used[n] {
            used[n] = true;
            children[parent[n]] += 1;
            n = parent[n];
        }
    }
    used[start] = true;
    let key = |n: usize| n == start || ends.contains(&n) || children[n] >= 2;
    let mut net = Network::new(&Streets::default(), step);
    let mut index = vec![usize::MAX; nx * ny];
    for n in (0..nx * ny).filter(|&n| used[n] && key(n) && n != start) {
        let mut chain = vec![point(n)];
        let mut m = parent[n];
        while !key(m) {
            chain.push(point(m));
            m = parent[m];
        }
        chain.push(point(m));
        let line = smooth(&simplify(&chain, 0.75 * step));
        let mut at = |k: usize, net: &mut Network| {
            if index[k] == usize::MAX {
                index[k] = net.add_point(point(k));
            }
            index[k]
        };
        let (first, last) = (at(n, &mut net), at(m, &mut net));
        let mut previous = first;
        for &p in &line[1..line.len() - 1] {
            let next = net.add_point(p);
            net.add_edge(previous, next, width, HIGHWAY);
            previous = next;
        }
        net.add_edge(previous, last, width, HIGHWAY);
    }
    Ok(planarize(net.into_streets()))
}

/// The point where the ray from `from` at `angle` leaves the area, a little inside.
fn edge_point(from: [f64; 2], angle: f64, w: f64, d: f64) -> [f64; 2] {
    let dir = [angle.cos(), angle.sin()];
    let reach = |p: f64, v: f64, half: f64| if v.abs() < 1e-12 { f64::INFINITY } else { ((half * v.signum()) - p) / v };
    let s = reach(from[0], dir[0], w / 2.0).min(reach(from[1], dir[1], d / 2.0)).max(0.0);
    add(from, scale(dir, s))
}

/// The line with fewer points: a point stays when it is more than `tolerance` from the line
/// between the points that stay around it (Douglas and Peucker).
pub(crate) fn simplify(line: &[[f64; 2]], tolerance: f64) -> Vec<[f64; 2]> {
    if line.len() < 3 {
        return line.to_vec();
    }
    let (a, b) = (line[0], line[line.len() - 1]);
    let far = (1..line.len() - 1).map(|k| (k, crate::terrain::segment_distance(line[k], a, b))).max_by(|x, y| x.1.total_cmp(&y.1));
    match far {
        Some((k, dist)) if dist > tolerance => {
            let mut left = simplify(&line[..=k], tolerance);
            left.pop();
            left.extend(simplify(&line[k..], tolerance));
            left
        }
        _ => vec![a, b],
    }
}

/// Two passes of corner cutting (Chaikin). The first and the last point stay.
pub(crate) fn smooth(line: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut line = line.to_vec();
    for _ in 0..2 {
        if line.len() < 3 {
            break;
        }
        let mut out = vec![line[0]];
        for pair in line.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if length(sub(b, a)) > 1e-9 {
                out.push(add(scale(a, 0.75), scale(b, 0.25)));
                out.push(add(scale(a, 0.25), scale(b, 0.75)));
            }
        }
        out.push(line[line.len() - 1]);
        // The cut next to each end goes, so that the ends keep their direction.
        if out.len() > 4 {
            out.remove(1);
            out.remove(out.len() - 2);
        }
        line = out;
    }
    line
}
