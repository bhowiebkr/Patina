//! Blocks and lots: the faces of the street graph (task E2) and their recursive split (E3).

use geometry::{Random, Seed};

use crate::zoning::thin;
use crate::{Plot, Plots, Streets, add, area, axes, cross, dot, extent, length, scale, sub};

/// The deepest split of one block, so that a small lot area cannot loop for a long time.
const MAX_DEPTH: u32 = 24;

/// A cut across the short axis gives no part that is more than this many times as long as wide.
const DEEP: f64 = 4.0;

/// The closed faces of the street graph, each inset by half the width of its streets. All the
/// edges of a block are on a street and keep its road class. A block that the inset closes is
/// not in the list.
pub fn blocks(streets: &Streets) -> Plots {
    let (points, edges) = (&streets.points, &streets.edges);
    // The half-edges out of each point, sorted by angle: (angle, other point, edge).
    let mut out: Vec<Vec<(f64, usize, usize)>> = vec![Vec::new(); points.len()];
    for (k, e) in edges.iter().enumerate() {
        if e.a == e.b || e.a >= points.len() || e.b >= points.len() {
            continue;
        }
        let d = sub(points[e.b], points[e.a]);
        out[e.a].push((d[1].atan2(d[0]), e.b, k));
        out[e.b].push(((-d[1]).atan2(-d[0]), e.a, k));
    }
    for list in &mut out {
        list.sort_by(|x, y| x.0.total_cmp(&y.0));
    }
    let mut seen: Vec<Vec<bool>> = out.iter().map(|list| vec![false; list.len()]).collect();
    let mut plots = Vec::new();
    for start in 0..points.len() {
        for slot in 0..out[start].len() {
            let (mut corners, mut widths, mut classes) = (Vec::new(), Vec::new(), Vec::new());
            let (mut u, mut s) = (start, slot);
            while !seen[u][s] && corners.len() <= edges.len() {
                seen[u][s] = true;
                let (_, v, k) = out[u][s];
                corners.push(points[u]);
                widths.push(edges[k].width / 2.0);
                classes.push(edges[k].class);
                // The face turns to the next half-edge clockwise from the way back.
                let Some(back) = out[v].iter().position(|&(_, w, e)| e == k && w == u) else { break };
                s = (back + out[v].len() - 1) % out[v].len();
                u = v;
            }
            // The outer face is clockwise, so its area is negative.
            if area(&corners) > 1e-9
                && let Some((corners, kept)) = offset(&corners, &widths)
            {
                let fronts = kept.iter().map(|&i| Some(classes[i])).collect();
                plots.push(Plot { corners, fronts, ..Plot::default() });
            }
        }
    }
    Plots(plots)
}

/// Moves each edge of each plot in by `distance`, for a sidewalk or a setback. A plot that the
/// inset closes is not in the list.
pub fn inset(plots: &Plots, distance: f64) -> Result<Plots, String> {
    if !distance.is_finite() || distance < 0.0 {
        return Err(format!("the distance must be 0 or more, not {distance}"));
    }
    let moved = plots.0.iter().filter_map(|plot| {
        let (corners, kept) = offset(&plot.corners, &vec![distance; plot.corners.len()])?;
        let fronts = kept.iter().map(|&i| plot.fronts.get(i).copied().flatten()).collect();
        Some(Plot { corners, fronts, ..plot.clone() })
    });
    Ok(Plots(moved.collect()))
}

/// Splits each block across its long axis again and again, until a lot has `area` or less, or
/// is narrower than two times `width`. An edge on a cut is not on a street. Each lot keeps
/// `width` of street edge or more: when a cut across the long axis gives a part with less, the
/// cut goes across the short axis, if no part is more than 4 times as long as wide. Else the
/// lot stays bigger. A thin part of a cut joins a lot across the cut, when the joined lot is not
/// thin.
pub fn lots(blocks: &Plots, seed: Seed, area_limit: f64, width: f64) -> Result<Plots, String> {
    if !(area_limit.is_finite() && area_limit > 0.0 && width.is_finite() && width > 0.0) {
        return Err("the lot area and the width must be more than 0".into());
    }
    let mut lots = Vec::new();
    for (i, block) in blocks.0.iter().enumerate() {
        let mut random = seed.child(&format!("lots {i}")).random();
        split(block.clone(), area_limit, width, &mut random, 0, &mut lots);
    }
    Ok(Plots(lots))
}

fn split(plot: Plot, area_limit: f64, width: f64, random: &mut Random, depth: u32, out: &mut Vec<Plot>) {
    if depth >= MAX_DEPTH || area(&plot.corners) <= area_limit {
        out.push(plot);
        return;
    }
    let (u, v) = axes(&plot.corners);
    let street = on_street(&plot, width);
    // The cut across the short axis is for a plot where the cut across the long axis gives a
    // part with no street.
    for axis in [u, v] {
        let (lo, hi) = extent(&plot.corners, axis);
        if hi - lo < 2.0 * width {
            continue;
        }
        let at = lo + (hi - lo) * random.range(0.4, 0.6);
        let parts = [1.0, -1.0].into_iter().flat_map(|side| clip(&plot, axis, at, side));
        // A thin part joins a lot of the other parts, so that it is not a park.
        let (thin_parts, parts): (Vec<Plot>, Vec<Plot>) = parts.partition(|p| thin(&p.corners));
        let deep = |p: &Plot| {
            let ((a, b), (c, d)) = (extent(&p.corners, u), extent(&p.corners, v));
            b - a > DEEP * (d - c)
        };
        if street && !parts.iter().all(|p| on_street(p, width)) || axis == v && parts.iter().any(deep) {
            continue;
        }
        let start = out.len();
        for part in parts {
            split(part, area_limit, width, random, depth + 1, out);
        }
        for part in thin_parts {
            if let Some(part) = join_next(part, &mut out[start..]) {
                split(part, area_limit, width, random, depth + 1, out);
            }
        }
        return;
    }
    out.push(plot);
}

/// True when the edges of the plot on a street are `width` long or more.
fn on_street(plot: &Plot, width: f64) -> bool {
    let n = plot.corners.len();
    let front: f64 = (0..n).filter(|&e| plot.fronts[e].is_some()).map(|e| length(sub(plot.corners[(e + 1) % n], plot.corners[e]))).sum();
    front >= width
}

/// Joins `part` to the lot that shares the longest piece of a cut edge with it, if that piece is
/// 0.5 m or more and the joined lot is not thin. It gives the part back when it does not join.
fn join_next(part: Plot, lots: &mut [Plot]) -> Option<Plot> {
    let (n, view): (usize, &[Plot]) = (part.corners.len(), lots);
    // The line of edge i of the part: its start, its direction and its length.
    let line = |i: usize| {
        let (p, q) = (part.corners[i], part.corners[(i + 1) % n]);
        let len = length(sub(q, p));
        (p, scale(sub(q, p), 1.0 / len.max(1e-300)), len)
    };
    // The piece of edge i that edge j of lot k covers the other way, from lo to hi along edge i.
    let piece = |i: usize, k: usize, j: usize| {
        let ((p, dir, len), lot) = (line(i), &view[k]);
        let (r, s) = (lot.corners[j], lot.corners[(j + 1) % lot.corners.len()]);
        let on = cross(dir, sub(r, p)).abs() < 1e-7 && cross(dir, sub(s, p)).abs() < 1e-7;
        let (lo, hi) = (dot(sub(s, p), dir).max(0.0), dot(sub(r, p), dir).min(len));
        (on && hi - lo >= 0.5).then_some((lo, hi))
    };
    let edges = (0..n).filter(|&i| part.fronts[i].is_none()).flat_map(|i| (0..view.len()).map(move |k| (i, k)));
    let pieces = edges.flat_map(|(i, k)| (0..view[k].corners.len()).filter(move |&j| view[k].fronts[j].is_none()).map(move |j| (i, k, j)));
    let mut found: Vec<_> = pieces.filter_map(|(i, k, j)| piece(i, k, j).map(|(lo, hi)| (i, k, j, lo, hi))).collect();
    found.sort_by(|x, y| (y.4 - y.3).total_cmp(&(x.4 - x.3)));
    for (i, k, j, lo, hi) in found {
        // Both plots get corners at the ends of the piece, so that they share one edge.
        let ((p, dir, len), m) = (line(i), lots[k].corners.len());
        let at = |t: f64| add(p, scale(dir, t));
        let (r, s) = (dot(sub(lots[k].corners[j], p), dir), dot(sub(lots[k].corners[(j + 1) % m], p), dir));
        let (a_lo, b_hi) = (lo > 1e-9, r - hi > 1e-9);
        let a = with_corners(&part, i, [a_lo.then(|| at(lo)), (hi < len - 1e-9).then(|| at(hi))]);
        let b = with_corners(&lots[k], j, [b_hi.then(|| at(hi)), (lo - s > 1e-9).then(|| at(lo))]);
        let lot = joined(&a, &b, i + usize::from(a_lo), j + usize::from(b_hi));
        if !thin(&lot.corners) {
            lots[k] = lot;
            return None;
        }
    }
    Some(part)
}

/// The plot with the points put into edge `e`, in order. The new edges have the front of edge e.
fn with_corners(plot: &Plot, e: usize, points: [Option<[f64; 2]>; 2]) -> Plot {
    let points: Vec<[f64; 2]> = points.into_iter().flatten().collect();
    let mut out = plot.clone();
    out.corners.splice(e + 1..e + 1, points.iter().copied());
    out.fronts.splice(e + 1..e + 1, points.iter().map(|_| plot.fronts[e]));
    out
}

/// The plot of `a` and `b`, which share edge `i` of `a` as edge `j` of `b`, the other way round.
/// The two ends of the shared edge go when they are on a straight line.
fn joined(a: &Plot, b: &Plot, i: usize, j: usize) -> Plot {
    let (na, nb) = (a.corners.len(), b.corners.len());
    let mut corners: Vec<[f64; 2]> = (1..=na).map(|k| a.corners[(i + k) % na]).collect();
    // The edge out of corner i of `a` now continues as the edge out of corner j + 1 of `b`.
    let mut fronts: Vec<_> = (1..=na).map(|k| if k == na { b.fronts[(j + 1) % nb] } else { a.fronts[(i + k) % na] }).collect();
    corners.extend((2..nb).map(|k| b.corners[(j + k) % nb]));
    fronts.extend((2..nb).map(|k| b.fronts[(j + k) % nb]));
    for c in [na - 1, 0] {
        let n = corners.len();
        let (d0, d1) = (sub(corners[c], corners[(c + n - 1) % n]), sub(corners[(c + 1) % n], corners[c]));
        if n > 3 && cross(d0, d1).abs() <= 1e-9 * length(d0) * length(d1) && dot(d0, d1) > 0.0 {
            corners.remove(c);
            fronts.remove(c);
        }
    }
    Plot { corners, fronts, ..Plot::default() }
}

/// The parts of the plot where `side * (p . axis - at) <= 0`. A concave plot can give more than
/// one part. An edge on the cut is not on a street.
fn clip(plot: &Plot, axis: [f64; 2], at: f64, side: f64) -> Vec<Plot> {
    let f = |p: [f64; 2]| side * (dot(p, axis) - at);
    let n = plot.corners.len();
    let (mut corners, mut fronts) = (Vec::with_capacity(n + 2), Vec::with_capacity(n + 2));
    // Each corner where the outline crosses the cut, and true where the outline goes out.
    let mut crossings = Vec::new();
    for i in 0..n {
        let (a, b) = (plot.corners[i], plot.corners[(i + 1) % n]);
        let front = plot.fronts.get(i).copied().flatten();
        let (fa, fb) = (f(a), f(b));
        if fa <= 0.0 {
            corners.push(a);
            fronts.push(front);
        }
        if (fa <= 0.0) != (fb <= 0.0) {
            crossings.push((corners.len(), fa <= 0.0));
            corners.push(add(a, scale(sub(b, a), fa / (fa - fb))));
            // The edge from a point where the polygon goes out follows the cut.
            fronts.push(if fa > 0.0 { front } else { None });
        }
    }
    // Along the cut, the plot is inside between crossings 0 and 1, 2 and 3, and so on. The edge
    // on the cut goes from the out crossing of a pair to its in crossing.
    let along = [-axis[1], axis[0]];
    crossings.sort_by(|x, y| dot(corners[x.0], along).total_cmp(&dot(corners[y.0], along)));
    let m = corners.len();
    let mut next = vec![None; m];
    for pair in crossings.chunks(2) {
        if let [(o, true), (i, false)] | [(i, false), (o, true)] = *pair {
            next[o] = Some(i);
        }
    }
    // Without a clear pair for each crossing, the parts stay one plot.
    let paired = next.iter().flatten().count() * 2 == crossings.len();
    let loops: Vec<Vec<usize>> = if paired && !crossings.is_empty() {
        let mut done = vec![false; m];
        let mut loops = Vec::new();
        for &(start, _) in crossings.iter().filter(|c| !c.1) {
            let (mut k, mut part) = (start, Vec::new());
            while !done[k] {
                done[k] = true;
                part.push(k);
                k = next[k].unwrap_or((k + 1) % m);
            }
            loops.push(part);
        }
        loops
    } else {
        vec![(0..m).collect()]
    };
    let part = |l: Vec<usize>| {
        // A corner on the cut can come two times. The edge out of the second copy stays.
        let len = l.len();
        let keep = |t: &usize| length(sub(corners[l[(t + 1) % len]], corners[l[*t]])) > 1e-9;
        let (corners, fronts): (Vec<_>, Vec<_>) = (0..len).filter(keep).map(|t| (corners[l[t]], fronts[l[t]])).unzip();
        (corners.len() >= 3 && area(&corners) > 1e-6).then(|| Plot { corners, fronts, ..Plot::default() })
    };
    loops.into_iter().filter_map(part).collect()
}

/// Moves edge `i` of a counterclockwise polygon in by `distances[i]`. Each new corner is where
/// two moved edges meet. An edge that the move turns around goes, the worst first, and its
/// neighbors meet again. It gives the corners and, for each new edge, the index of its old edge,
/// or `None` when fewer than three edges stay or the area goes away.
pub(crate) fn offset(corners: &[[f64; 2]], distances: &[f64]) -> Option<(Vec<[f64; 2]>, Vec<usize>)> {
    let n = corners.len();
    if n < 3 || distances.len() != n {
        return None;
    }
    let left = |t: [f64; 2]| [-t[1], t[0]];
    let dirs: Vec<[f64; 2]> = (0..n).map(|i| sub(corners[(i + 1) % n], corners[i])).map(|d| scale(d, 1.0 / length(d).max(1e-300))).collect();
    // An edge with no length has no direction, so it is not used.
    let mut kept: Vec<usize> = (0..n).filter(|&i| length(sub(corners[(i + 1) % n], corners[i])) > 1e-9).collect();
    // The corner where moved edge `j` meets the moved edge `k` after it.
    let meet = |j: usize, k: usize| {
        let (t0, t1) = (dirs[j], dirs[k]);
        let p1 = add(corners[k], scale(left(t1), distances[k]));
        let p0 = add(corners[j], scale(left(t0), distances[j]));
        let c = cross(t0, t1);
        // Two edges on nearly one line have no clear meeting point.
        if c.abs() < 1e-6 {
            let foot = add(p0, scale(t0, dot(sub(p1, p0), t0)));
            scale(add(foot, p1), 0.5)
        } else {
            add(p0, scale(t0, cross(sub(p1, p0), t1) / c))
        }
    };
    loop {
        let m = kept.len();
        if m < 3 {
            return None;
        }
        let moved: Vec<[f64; 2]> = (0..m).map(|t| meet(kept[(t + m - 1) % m], kept[t])).collect();
        let worst = (0..m).map(|t| (t, dot(sub(moved[(t + 1) % m], moved[t]), dirs[kept[t]]))).min_by(|a, b| a.1.total_cmp(&b.1));
        match worst {
            Some((t, run)) if run <= 1e-6 => {
                kept.remove(t);
            }
            _ => return (area(&moved) > 1e-6).then_some((moved, kept)),
        }
    }
}
