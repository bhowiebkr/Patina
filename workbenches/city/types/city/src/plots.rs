//! Blocks and lots: the faces of the street graph (task E2) and their recursive split (E3).

use geometry::{Random, Seed};

use crate::{Plot, Plots, Streets, add, area, axes, cross, dot, extent, length, scale, sub};

/// The deepest split of one block, so that a small lot area cannot loop for a long time.
const MAX_DEPTH: u32 = 24;

/// The closed faces of the street graph, each inset by half the width of its streets. All the
/// edges of a block are on a street. A block that the inset closes is not in the list.
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
            let (mut corners, mut widths) = (Vec::new(), Vec::new());
            let (mut u, mut s) = (start, slot);
            while !seen[u][s] && corners.len() <= edges.len() {
                seen[u][s] = true;
                let (_, v, k) = out[u][s];
                corners.push(points[u]);
                widths.push(edges[k].width / 2.0);
                // The face turns to the next half-edge clockwise from the way back.
                let Some(back) = out[v].iter().position(|&(_, w, e)| e == k && w == u) else { break };
                s = (back + out[v].len() - 1) % out[v].len();
                u = v;
            }
            // The outer face is clockwise, so its area is negative.
            if area(&corners) > 1e-9
                && let Some(corners) = offset(&corners, &widths)
            {
                let fronts = vec![true; corners.len()];
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
        let corners = offset(&plot.corners, &vec![distance; plot.corners.len()])?;
        Some(Plot { corners, ..plot.clone() })
    });
    Ok(Plots(moved.collect()))
}

/// Splits each block across its long axis again and again, until a lot has `area` or less, or
/// is narrower than two times `width`. A lot with no street edge, and a random part `parks` of
/// the other lots, become parks.
pub fn lots(blocks: &Plots, seed: Seed, area_limit: f64, width: f64, parks: f64) -> Result<Plots, String> {
    if !(area_limit.is_finite() && area_limit > 0.0 && width.is_finite() && width > 0.0) {
        return Err("the lot area and the width must be more than 0".into());
    }
    if !(0.0..=1.0).contains(&parks) {
        return Err(format!("parks must be from 0 to 1, not {parks}"));
    }
    let mut lots = Vec::new();
    for (i, block) in blocks.0.iter().enumerate() {
        let mut random = seed.child(&format!("lots {i}")).random();
        let start = lots.len();
        split(block.clone(), area_limit, width, &mut random, 0, &mut lots);
        for lot in &mut lots[start..] {
            let chance = random.unit();
            lot.park = !lot.fronts.contains(&true) || chance < parks;
        }
    }
    Ok(Plots(lots))
}

fn split(plot: Plot, area_limit: f64, width: f64, random: &mut Random, depth: u32, out: &mut Vec<Plot>) {
    let (u, _) = axes(&plot.corners);
    let (lo, hi) = extent(&plot.corners, u);
    if depth >= MAX_DEPTH || area(&plot.corners) <= area_limit || hi - lo < 2.0 * width {
        out.push(plot);
        return;
    }
    let at = lo + (hi - lo) * random.range(0.4, 0.6);
    for side in [1.0, -1.0] {
        if let Some(half) = clip(&plot, u, at, side) {
            split(half, area_limit, width, random, depth + 1, out);
        }
    }
}

/// The part of the plot where `side * (p . axis - at) <= 0`. An edge on the cut is not on a
/// street.
fn clip(plot: &Plot, axis: [f64; 2], at: f64, side: f64) -> Option<Plot> {
    let f = |p: [f64; 2]| side * (dot(p, axis) - at);
    let n = plot.corners.len();
    let (mut corners, mut fronts) = (Vec::with_capacity(n + 2), Vec::with_capacity(n + 2));
    for i in 0..n {
        let (a, b) = (plot.corners[i], plot.corners[(i + 1) % n]);
        let front = plot.fronts.get(i).copied().unwrap_or(false);
        let (fa, fb) = (f(a), f(b));
        if fa <= 0.0 {
            corners.push(a);
            fronts.push(front);
        }
        if (fa <= 0.0) != (fb <= 0.0) {
            corners.push(add(a, scale(sub(b, a), fa / (fa - fb))));
            // The edge from a point where the polygon goes out follows the cut.
            fronts.push(fa > 0.0 && front);
        }
    }
    // A corner on the cut can come two times. The edge out of the second copy stays.
    let m = corners.len();
    let keep: Vec<bool> = (0..m).map(|k| length(sub(corners[(k + 1) % m], corners[k])) > 1e-9).collect();
    let (corners, fronts): (Vec<_>, Vec<_>) = corners.into_iter().zip(fronts).zip(keep).filter_map(|(c, k)| k.then_some(c)).unzip();
    (corners.len() >= 3 && area(&corners) > 1e-6).then(|| Plot { corners, fronts, ..Plot::default() })
}

/// Moves edge `i` of a counterclockwise polygon in by `distances[i]`. Each new corner is where
/// the two moved edges meet. It gives `None` when an edge turns around or the area goes away.
pub(crate) fn offset(corners: &[[f64; 2]], distances: &[f64]) -> Option<Vec<[f64; 2]>> {
    let n = corners.len();
    if n < 3 || distances.len() != n {
        return None;
    }
    let mut dirs = Vec::with_capacity(n);
    for i in 0..n {
        let d = sub(corners[(i + 1) % n], corners[i]);
        let l = length(d);
        if l < 1e-9 {
            return None;
        }
        dirs.push(scale(d, 1.0 / l));
    }
    let left = |t: [f64; 2]| [-t[1], t[0]];
    let moved: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let prev = (i + n - 1) % n;
            let (t0, t1) = (dirs[prev], dirs[i]);
            let p0 = add(corners[i], scale(left(t0), distances[prev]));
            let p1 = add(corners[i], scale(left(t1), distances[i]));
            let c = cross(t0, t1);
            // Two edges on nearly one line have no clear meeting point.
            if c.abs() < 1e-6 { scale(add(p0, p1), 0.5) } else { add(p0, scale(t0, cross(sub(p1, p0), t1) / c)) }
        })
        .collect();
    let kept = (0..n).all(|i| dot(sub(moved[(i + 1) % n], moved[i]), dirs[i]) > 1e-6);
    (kept && area(&moved) > 1e-6).then_some(moved)
}
