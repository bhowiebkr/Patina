//! The road growth (task E10), as in the method of Parish and Mueller: a road proposes a
//! segment, and local rules accept, change or refuse it. The removal of dead ends is here too.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::f64::consts::PI;

use geometry::{Random, Seed};

use crate::terrain::segment_distance;
use crate::{ARTERIAL, Density, Edge, HIGHWAY, LOCAL, Streets, Terrain, add, cross, dot, length, scale, sub};

/// The most streets that the growth makes, so that a wrong value cannot fill the memory.
pub const MAX_ROADS: usize = 200_000;
/// The steepest grade of a new street.
const MAX_GRADE: f64 = 0.18;
/// Two streets at one point have an angle of 30 degrees or more.
const MIN_COS: f64 = 0.866;

/// A street graph with a spatial hash of its points and edges, for the growth.
pub(crate) struct Network {
    pub(crate) points: Vec<[f64; 2]>,
    pub(crate) edges: Vec<Edge>,
    around: Vec<Vec<usize>>,
    size: f64,
    point_cells: HashMap<(i64, i64), Vec<usize>>,
    edge_cells: HashMap<(i64, i64), Vec<usize>>,
}

impl Network {
    /// The network of a street graph, with hash cells of `size`.
    pub(crate) fn new(streets: &Streets, size: f64) -> Self {
        let mut net = Network { points: Vec::new(), edges: Vec::new(), around: Vec::new(), size: size.max(1.0), point_cells: HashMap::new(), edge_cells: HashMap::new() };
        for &p in &streets.points {
            net.add_point(p);
        }
        let n = streets.points.len();
        for e in streets.edges.iter().filter(|e| e.a != e.b && e.a < n && e.b < n) {
            net.add_edge(e.a, e.b, e.width, e.class);
        }
        net
    }

    fn cell(&self, p: [f64; 2]) -> (i64, i64) {
        ((p[0] / self.size).floor() as i64, (p[1] / self.size).floor() as i64)
    }

    pub(crate) fn add_point(&mut self, p: [f64; 2]) -> usize {
        let k = self.points.len();
        self.points.push(p);
        self.around.push(Vec::new());
        self.point_cells.entry(self.cell(p)).or_default().push(k);
        k
    }

    pub(crate) fn add_edge(&mut self, a: usize, b: usize, width: f64, class: u8) {
        let k = self.edges.len();
        self.edges.push(Edge { a, b, width, class });
        self.around[a].push(k);
        self.around[b].push(k);
        self.hash_edge(k);
    }

    fn hash_edge(&mut self, k: usize) {
        let (p, q) = (self.points[self.edges[k].a], self.points[self.edges[k].b]);
        let (lo, hi) = (self.cell([p[0].min(q[0]), p[1].min(q[1])]), self.cell([p[0].max(q[0]), p[1].max(q[1])]));
        for i in lo.0..=hi.0 {
            for j in lo.1..=hi.1 {
                self.edge_cells.entry((i, j)).or_default().push(k);
            }
        }
    }

    /// The items of the cells that touch the box from `lo` to `hi`, each one time.
    fn near(&self, cells: &HashMap<(i64, i64), Vec<usize>>, lo: [f64; 2], hi: [f64; 2]) -> Vec<usize> {
        let (a, b) = (self.cell(lo), self.cell(hi));
        let mut out: Vec<usize> = (a.0..=b.0).flat_map(|i| (a.1..=b.1).map(move |j| (i, j))).filter_map(|c| cells.get(&c)).flatten().copied().collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Splits edge `k` at point `x`, which is on the edge.
    fn split(&mut self, k: usize, x: usize) {
        let Edge { b, width, class, .. } = self.edges[k];
        let n = self.edges.len();
        self.edges[k].b = x;
        self.edges.push(Edge { a: x, b, width, class });
        if let Some(slot) = self.around[b].iter_mut().find(|e| **e == k) {
            *slot = n;
        }
        self.around[x].extend([k, n]);
        self.hash_edge(n);
    }

    fn linked(&self, a: usize, b: usize) -> bool {
        self.around[a].iter().any(|&k| self.edges[k].a == b || self.edges[k].b == b)
    }

    /// True when a street out of point `at` in direction `dir` keeps 30 degrees from the others.
    fn apart(&self, at: usize, dir: [f64; 2]) -> bool {
        self.around[at].iter().all(|&k| {
            let e = self.edges[k];
            let other = self.points[if e.a == at { e.b } else { e.a }];
            let d = sub(other, self.points[at]);
            dot(d, dir) < MIN_COS * length(d) * length(dir)
        })
    }

    /// The crossings of the segment from `p` to `q` with the edges, as (part of the way along
    /// `p` to `q`, part along the edge, edge). The edges for which `skip` is true do not count.
    fn crossings(&self, p: [f64; 2], q: [f64; 2], skip: impl Fn(usize, &Edge) -> bool) -> Vec<(f64, f64, usize)> {
        let (lo, hi) = ([p[0].min(q[0]), p[1].min(q[1])], [p[0].max(q[0]), p[1].max(q[1])]);
        let mut out: Vec<_> = self.near(&self.edge_cells, lo, hi).into_iter().filter(|&k| !skip(k, &self.edges[k])).filter_map(|k| intersect(p, q, self.points[self.edges[k].a], self.points[self.edges[k].b]).map(|(t, u)| (t, u, k))).collect();
        out.sort_by(|x, y| x.0.total_cmp(&y.0));
        out
    }

    /// Adds the segment from point `a` to point `b`, split at each edge that it crosses. The
    /// edges that it crosses are split too.
    fn insert(&mut self, a: usize, b: usize, width: f64, class: u8) {
        let (p, q) = (self.points[a], self.points[b]);
        let span = length(sub(q, p));
        if a == b || span < 1e-9 {
            return;
        }
        let hits = self.crossings(p, q, |_, e| e.a == a || e.a == b || e.b == a || e.b == b);
        let mut chain = vec![a];
        for (t, u, k) in hits {
            let Edge { a: ea, b: eb, .. } = self.edges[k];
            let reach = length(sub(self.points[eb], self.points[ea]));
            let x = if u * reach < 1e-3 {
                ea
            } else if (1.0 - u) * reach < 1e-3 {
                eb
            } else if t * span < 1e-3 || (1.0 - t) * span < 1e-3 {
                let end = if t < 0.5 { a } else { b };
                self.split(k, end);
                continue;
            } else {
                let x = self.add_point(add(p, scale(sub(q, p), t)));
                self.split(k, x);
                x
            };
            chain.push(x);
        }
        chain.push(b);
        chain.dedup();
        for pair in chain.windows(2) {
            if !self.linked(pair[0], pair[1]) {
                self.add_edge(pair[0], pair[1], width, class);
            }
        }
    }

    pub(crate) fn into_streets(self) -> Streets {
        Streets { points: self.points, edges: self.edges }
    }
}

/// The parts of the way (t along p to q, u along a to b) where two segments cross, with the ends.
fn intersect(p: [f64; 2], q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> Option<(f64, f64)> {
    let (r, s) = (sub(q, p), sub(b, a));
    let d = cross(r, s);
    if d.abs() < 1e-12 {
        return None;
    }
    let (t, u) = (cross(sub(a, p), s) / d, cross(sub(a, p), r) / d);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

/// The street graph with a point at each crossing of two edges, so that it is planar.
pub(crate) fn planarize(streets: Streets) -> Streets {
    let (lo, hi) = bounds(&streets.points);
    let mut net = Network::new(&Streets { points: streets.points, edges: Vec::new() }, ((hi[0] - lo[0]).max(hi[1] - lo[1]) / 64.0).max(1.0));
    for e in &streets.edges {
        if e.a < net.points.len() && e.b < net.points.len() {
            net.insert(e.a, e.b, e.width, e.class);
        }
    }
    net.into_streets()
}

fn bounds(points: &[[f64; 2]]) -> ([f64; 2], [f64; 2]) {
    let start = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    let (lo, hi) = points.iter().fold(start, |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    if lo[0] <= hi[0] { (lo, hi) } else { ([0.0; 2], [0.0; 2]) }
}

/// The street graph with each edge longer than `most` cut into equal parts.
fn subdivide(streets: &Streets, most: f64) -> Streets {
    let mut out = Streets { points: streets.points.clone(), edges: Vec::new() };
    for &e in &streets.edges {
        let (p, q) = (streets.points[e.a], streets.points[e.b]);
        let parts = (length(sub(q, p)) / most).ceil().max(1.0) as usize;
        let mut previous = e.a;
        for k in 1..=parts {
            let next = if k == parts {
                e.b
            } else {
                out.points.push(add(p, scale(sub(q, p), k as f64 / parts as f64)));
                out.points.len() - 1
            };
            out.edges.push(Edge { a: previous, b: next, ..e });
            previous = next;
        }
    }
    out
}

#[derive(Clone, Copy)]
struct Proposal {
    from: usize,
    dir: [f64; 2],
    class: u8,
}

/// The end of a new street: a point of the graph or a point on an edge.
enum End {
    Point(usize),
    On(usize, [f64; 2]),
}

struct Growth<'a> {
    net: Network,
    terrain: &'a Terrain,
    density: &'a Density,
    random: Random,
    queue: BinaryHeap<Reverse<(u64, usize)>>,
    proposals: Vec<Proposal>,
    block: f64,
    widths: [f64; 3],
    threshold: f64,
}

/// Arterials and local streets that grow out of the highways where the density is `threshold`
/// or more. A segment is about `block` long and follows the ground. At the edge of the area, at
/// water or on a steep slope it turns along the edge, and it stops when no turn is good or at a
/// street that it meets. A free end goes on and branches to the sides, more often where the
/// density is high (`branch` from 0 to 1).
#[allow(clippy::too_many_arguments)]
pub fn grow(highways: &Streets, terrain: &Terrain, density: &Density, seed: Seed, block: f64, arterial: f64, street: f64, branch: f64, threshold: f64) -> Result<Streets, String> {
    if !(10.0..=1000.0).contains(&block) || !(arterial.is_finite() && arterial > 0.0 && street.is_finite() && street > 0.0) {
        return Err("the block must be from 10 to 1000 and the widths more than 0".into());
    }
    if !(0.0..=1.0).contains(&branch) || !(0.0..=1.0).contains(&threshold) {
        return Err("branch and threshold must be from 0 to 1".into());
    }
    if terrain.nx < 2 || terrain.ny < 2 || terrain.heights.len() != terrain.nx * terrain.ny || density.values.len() != terrain.nx * terrain.ny {
        return Err("the terrain and the density map must have the same samples".into());
    }
    let n = highways.points.len();
    if highways.edges.iter().any(|e| e.a >= n || e.b >= n) {
        return Err("a highway has a point that is not in the list".into());
    }
    let mut net = Network::new(&subdivide(highways, block), block);
    if net.points.is_empty() {
        net.add_point(density.centers.first().copied().unwrap_or_default());
    }
    let mut g = Growth { net, terrain, density, random: seed.child("grow").random(), queue: BinaryHeap::new(), proposals: Vec::new(), block, widths: [0.0, arterial, street], threshold };
    g.seed(branch);
    let mut count = 0_usize;
    while let Some(Reverse((time, k))) = g.queue.pop() {
        count += 1;
        if g.net.edges.len() > MAX_ROADS || count > 8 * MAX_ROADS {
            return Err(format!("the growth makes more than {MAX_ROADS} streets"));
        }
        let time = f64::from_bits(time);
        let prop = g.proposals[k];
        let Some((end, dir)) = g.place(prop) else { continue };
        let d = g.density.at(g.net.points[end]);
        g.propose(time + 1.0, Proposal { from: end, dir, class: prop.class });
        for side in [1.0, -1.0] {
            let normal = [-dir[1] * side, dir[0] * side];
            if prop.class == ARTERIAL && g.random.unit() < 0.08 {
                g.propose(time + 1.0, Proposal { from: end, dir: normal, class: ARTERIAL });
            } else if g.random.unit() < branch * d.sqrt() {
                let delay = if prop.class == ARTERIAL { 4.0 } else { 1.0 };
                g.propose(time + delay, Proposal { from: end, dir: normal, class: LOCAL });
            }
        }
    }
    Ok(g.net.into_streets())
}

impl Growth<'_> {
    fn propose(&mut self, time: f64, prop: Proposal) {
        self.queue.push(Reverse((time.max(0.0).to_bits(), self.proposals.len())));
        self.proposals.push(prop);
    }

    /// The first proposals: to the sides of the highways, and to each sub-center.
    fn seed(&mut self, branch: f64) {
        for at in 0..self.net.points.len() {
            let p = self.net.points[at];
            let ends: Vec<[f64; 2]> = self.net.around[at].iter().map(|&k| self.net.edges[k]).map(|e| self.net.points[if e.a == at { e.b } else { e.a }]).collect();
            if ends.len() != 2 {
                continue;
            }
            let tangent = sub(ends[1], ends[0]);
            let l = length(tangent);
            if l < 1e-9 {
                continue;
            }
            let d = self.density.at(p);
            for side in [1.0, -1.0] {
                let dir = [-tangent[1] / l * side, tangent[0] / l * side];
                if self.random.unit() < 0.6 * d {
                    self.propose(0.0, Proposal { from: at, dir, class: ARTERIAL });
                } else if self.random.unit() < branch * d.sqrt() {
                    self.propose(3.0, Proposal { from: at, dir, class: LOCAL });
                }
            }
        }
        if self.net.edges.is_empty() {
            let turn = self.random.range(0.0, PI / 2.0);
            for k in 0..4 {
                let a = turn + f64::from(k) * PI / 2.0;
                self.propose(0.0, Proposal { from: 0, dir: [a.cos(), a.sin()], class: ARTERIAL });
            }
        }
        for &c in self.density.centers.iter().skip(1) {
            let nearest = (0..self.net.points.len()).min_by(|&a, &b| length(sub(self.net.points[a], c)).total_cmp(&length(sub(self.net.points[b], c))));
            if let Some(at) = nearest {
                let d = sub(c, self.net.points[at]);
                if length(d) > self.block {
                    self.propose(0.0, Proposal { from: at, dir: scale(d, 1.0 / length(d)), class: ARTERIAL });
                }
            }
        }
    }

    /// Tries one proposal. It gives the new free end and the direction of the street, or
    /// `None` when the street was refused or ends at a street.
    fn place(&mut self, prop: Proposal) -> Option<(usize, [f64; 2])> {
        let (from, p) = (prop.from, self.net.points[prop.from]);
        if !self.net.apart(from, prop.dir) {
            return None;
        }
        // The blocks are smaller where the density is high.
        let reach = self.block * (1.3 - 0.6 * self.density.at(p)) * self.random.range(0.85, 1.15);
        let (turns, wide, pull): (&[f64], &[f64], f64) =
            if prop.class == ARTERIAL { (&[0.0, -10.0, 10.0, -20.0, 20.0], &[-35.0, 35.0, -65.0, 65.0, -90.0, 90.0], 0.6) } else { (&[0.0, -12.0, 12.0, -25.0, 25.0, -40.0, 40.0], &[-65.0, 65.0, -90.0, 90.0], 0.2) };
        let score = |dir: [f64; 2], turn: f64| {
            let q = add(p, scale(dir, reach));
            let grade = (self.terrain.height(q) - self.terrain.height(p)) / reach;
            40.0 * grade * grade + 0.2 * turn.abs().to_radians() - pull * self.density.at(q)
        };
        // The street turns more, along a slope or a bank, only when no small turn is good.
        let best = |turns: &[f64]| {
            turns
                .iter()
                .map(|&turn| (turn, rotate(prop.dir, turn.to_radians())))
                .filter(|&(_, dir)| self.terrain.contains(add(p, scale(dir, reach))) && self.good(p, add(p, scale(dir, reach))))
                .min_by(|a, b| score(a.1, a.0).total_cmp(&score(b.1, b.0)))
        };
        let dir = best(turns).or_else(|| best(wide))?.1;
        let target = add(p, scale(dir, reach));
        let least = if prop.class == ARTERIAL { 0.5 * self.threshold } else { self.threshold };
        if self.density.at(target) < least {
            return None;
        }
        let snap = 0.35 * self.block;
        let end = if let Some(&(_, u, k)) = self.net.crossings(p, target, |_, e| e.a == from || e.b == from).first() {
            self.on_edge(k, u)
        } else if let Some(q) = self.nearest_point(target, snap, from) {
            End::Point(q)
        } else if let Some((k, u)) = self.nearest_edge(target, snap, from) {
            self.on_edge(k, u)
        } else {
            if !self.good(p, target) {
                return None;
            }
            let q = self.net.add_point(target);
            self.net.add_edge(from, q, self.widths[prop.class as usize], prop.class);
            return Some((q, dir));
        };
        self.join(from, end, prop.class);
        None
    }

    /// The end at part `u` of edge `k`, or at an end of the edge when that is near.
    fn on_edge(&self, k: usize, u: f64) -> End {
        let e = self.net.edges[k];
        let (a, b) = (self.net.points[e.a], self.net.points[e.b]);
        let (x, reach) = (add(a, scale(sub(b, a), u)), length(sub(b, a)));
        let near = (0.3 * self.block).min(0.5 * reach);
        if u * reach < near {
            End::Point(e.a)
        } else if (1.0 - u) * reach < near {
            End::Point(e.b)
        } else {
            End::On(k, x)
        }
    }

    /// Adds the street from point `from` to `end` when the rules allow it.
    fn join(&mut self, from: usize, end: End, class: u8) {
        let p = self.net.points[from];
        let (x, skip_point, skip_edge) = match end {
            End::Point(q) => (self.net.points[q], Some(q), None),
            End::On(k, x) => (x, None, Some(k)),
        };
        let dir = sub(x, p);
        if length(dir) < 0.35 * self.block || !self.net.apart(from, dir) || !self.good(p, x) {
            return;
        }
        let back = sub(p, x);
        let fits = match end {
            End::Point(q) => q != from && !self.net.linked(from, q) && self.net.apart(q, back),
            End::On(k, _) => {
                let e = self.net.edges[k];
                let along = sub(self.net.points[e.b], self.net.points[e.a]);
                dot(along, back).abs() < MIN_COS * length(along) * length(back)
            }
        };
        let skip = |k: usize, e: &Edge| e.a == from || e.b == from || skip_point.is_some_and(|q| e.a == q || e.b == q) || skip_edge == Some(k);
        let clear = self.net.crossings(p, x, skip).is_empty();
        if !fits || !clear {
            return;
        }
        let q = match end {
            End::Point(q) => q,
            End::On(k, x) => {
                let q = self.net.add_point(x);
                self.net.split(k, q);
                q
            }
        };
        self.net.add_edge(from, q, self.widths[class as usize], class);
    }

    /// True when the segment has no water and is not too steep.
    fn good(&self, p: [f64; 2], q: [f64; 2]) -> bool {
        let wet = (0..=4).any(|k| self.terrain.water(add(p, scale(sub(q, p), f64::from(k) / 4.0))));
        let grade = (self.terrain.height(q) - self.terrain.height(p)).abs() / length(sub(q, p)).max(1e-9);
        !wet && grade <= MAX_GRADE
    }

    fn nearest_point(&self, target: [f64; 2], r: f64, from: usize) -> Option<usize> {
        let (lo, hi) = (sub(target, [r, r]), add(target, [r, r]));
        let d = |k: usize| length(sub(self.net.points[k], target));
        self.net.near(&self.net.point_cells, lo, hi).into_iter().filter(|&k| k != from && d(k) <= r).min_by(|&a, &b| d(a).total_cmp(&d(b)))
    }

    /// The nearest edge in `r` of the target that is not at point `from`, with the part of
    /// the way of the nearest point on it.
    fn nearest_edge(&self, target: [f64; 2], r: f64, from: usize) -> Option<(usize, f64)> {
        let (lo, hi) = (sub(target, [r, r]), add(target, [r, r]));
        let net = &self.net;
        let d = |k: usize| segment_distance(target, net.points[net.edges[k].a], net.points[net.edges[k].b]);
        let k = net.near(&net.edge_cells, lo, hi).into_iter().filter(|&k| net.edges[k].a != from && net.edges[k].b != from && d(k) <= r).min_by(|&a, &b| d(a).total_cmp(&d(b)))?;
        let (a, b) = (net.points[net.edges[k].a], net.points[net.edges[k].b]);
        let ab = sub(b, a);
        Some((k, (dot(sub(target, a), ab) / dot(ab, ab).max(1e-12)).clamp(0.0, 1.0)))
    }
}

fn rotate(v: [f64; 2], angle: f64) -> [f64; 2] {
    let (s, c) = angle.sin_cos();
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

/// The street graph with no dead ends: an edge with a free end goes, again and again. A
/// highway stays, because it ends at the edge of the area. Points with no edge go.
pub fn prune(streets: &Streets) -> Streets {
    let (points, edges) = (&streets.points, &streets.edges);
    let n = points.len();
    let mut alive: Vec<bool> = edges.iter().map(|e| e.a != e.b && e.a < n && e.b < n).collect();
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, e) in edges.iter().enumerate().filter(|(k, _)| alive[*k]) {
        around[e.a].push(k);
        around[e.b].push(k);
    }
    let degree = |around: &[Vec<usize>], alive: &[bool], p: usize| around[p].iter().filter(|&&k| alive[k]).count();
    let mut stack: Vec<usize> = (0..n).collect();
    while let Some(p) = stack.pop() {
        if degree(&around, &alive, p) != 1 {
            continue;
        }
        let Some(&k) = around[p].iter().find(|&&k| alive[k]) else { continue };
        if edges[k].class == HIGHWAY {
            continue;
        }
        alive[k] = false;
        stack.push(if edges[k].a == p { edges[k].b } else { edges[k].a });
    }
    let mut index = vec![usize::MAX; n];
    let mut out = Streets::default();
    for (e, _) in edges.iter().zip(&alive).filter(|(_, a)| **a) {
        let mut at = |p: usize| {
            if index[p] == usize::MAX {
                index[p] = out.points.len();
                out.points.push(points[p]);
            }
            index[p]
        };
        let (a, b) = (at(e.a), at(e.b));
        out.edges.push(Edge { a, b, ..*e });
    }
    out
}
