//! The roads as a scene on the terrain (task E10): asphalt ribbons with a width for each road
//! class, sidewalks with curbs around the blocks, road lines, and bridges with piers.

use geometry::{Frame, Item, Mesh, Place, Scene, Solid};

use crate::shapes::{quad, tri};
use crate::{ARTERIAL, HIGHWAY, Plots, Streets, Terrain, add, cross, dot, length, scale, sub};

const ASPHALT: u32 = 0x3a3c40;
const SIDEWALK: u32 = 0x9a978f;
const WHITE: u32 = 0xe6e2d6;
const YELLOW: u32 = 0xd9b23c;
const CONCRETE: u32 = 0x8c8a85;
const PIER: u32 = 0x7b7974;
/// The road surface is this high over the ground, and the sidewalk this much higher again.
const ROAD: f64 = 0.15;
const CURB: f64 = 0.15;
/// The free height between the water and the bridge deck, and the thickness of the deck.
const CLEARANCE: f64 = 5.0;
const DECK: f64 = 1.2;

/// The surface of one edge over a bridge: (part of the way along the edge, deck height,
/// weight of the deck from 0 on the ground to 1 on the bridge).
type Profile = Vec<(f64, f64, f64)>;

/// The right and the left corner where a street leaves a crossing, seen from the crossing.
type Mouth = [[f64; 2]; 2];

/// The roads on the terrain. Each street is a ribbon of its width that follows the ground, and
/// a crossing is filled between the streets. Each block gets a sidewalk ring of `sidewalk` with
/// a curb. A highway over water becomes a bridge with a deck, parapets and piers.
pub fn roads(streets: &Streets, blocks: &Plots, terrain: &Terrain, sidewalk: f64) -> Result<Scene, String> {
    if !(0.0..=20.0).contains(&sidewalk) {
        return Err(format!("the sidewalk must be from 0 to 20, not {sidewalk}"));
    }
    let (points, edges) = (&streets.points, &streets.edges);
    if edges.iter().any(|e| e.a >= points.len() || e.b >= points.len()) {
        return Err("a street has a point that is not in the list".into());
    }
    let step = (0.75 * terrain.cell).max(0.5);
    let area = [terrain.width() / 2.0, terrain.depth() / 2.0];
    let corners = corners(streets, area);
    let profiles = bridges(streets, terrain, step);
    let mut meshes: [Mesh; 5] = Default::default();
    let [asphalt, walks, white, yellow, bridge] = &mut meshes;
    let mut piers = Vec::new();
    // A street that touches the water stays at the water level.
    let ground = |p: [f64; 2]| terrain.height(p).max(terrain.sea) + ROAD;
    for (k, e) in edges.iter().enumerate() {
        let (p, q) = (points[e.a], points[e.b]);
        let l = length(sub(q, p));
        if e.a == e.b || l < 1e-9 {
            continue;
        }
        let (u, n) = (scale(sub(q, p), 1.0 / l), [-(q[1] - p[1]) / l, (q[0] - p[0]) / l]);
        let surface = |x: [f64; 2]| {
            let f = dot(sub(x, p), u) / l;
            match lookup(&profiles[k], f) {
                Some((h, w)) => ground(x) * (1.0 - w) + h * w,
                None => ground(x),
            }
        };
        // The right side goes from the right corner at a to the left corner at b.
        let ([ra, la], [rb, lb]) = corners[k];
        let along = ((length(sub(lb, ra)).max(length(sub(rb, la)))) / step).ceil().max(1.0) as usize;
        let across = (e.width / step).ceil().max(1.0) as usize;
        let vertex = |i: usize, j: usize| {
            let f = i as f64 / along as f64;
            let (right, left) = (lerp(ra, lb, f), lerp(la, rb, f));
            let x = lerp(right, left, j as f64 / across as f64);
            [x[0], x[1], surface(x)]
        };
        for i in 0..along {
            for j in 0..across {
                quad(asphalt, [vertex(i, j), vertex(i + 1, j), vertex(i + 1, j + 1), vertex(i, j + 1)]);
            }
        }
        // The deck sides, the parapets and the underside where the street is on a bridge.
        if !profiles[k].is_empty() {
            for i in 0..along {
                let f = (i as f64 + 0.5) / along as f64;
                if lookup(&profiles[k], f).is_none_or(|(_, w)| w <= 0.0) {
                    continue;
                }
                let (r0, r1, l0, l1) = (vertex(i, 0), vertex(i + 1, 0), vertex(i, across), vertex(i + 1, across));
                let down = |v: [f64; 3], dz: f64| [v[0], v[1], v[2] + dz];
                let inward = |v: [f64; 3], side: f64| [v[0] + n[0] * 0.3 * side, v[1] + n[1] * 0.3 * side, v[2]];
                quad(bridge, [down(r1, -DECK), down(r0, -DECK), down(l0, -DECK), down(l1, -DECK)]);
                for (a, b, side) in [(r0, r1, 1.0), (l1, l0, -1.0)] {
                    quad(bridge, [down(a, -DECK), down(b, -DECK), down(b, 1.0), down(a, 1.0)]);
                    let (ia, ib) = (inward(a, side), inward(b, side));
                    quad(bridge, [down(a, 1.0), down(b, 1.0), down(ib, 1.0), down(ia, 1.0)]);
                    quad(bridge, [down(ib, 0.0), down(ia, 0.0), down(ia, 1.0), down(ib, 1.0)]);
                }
            }
            piers_of(&profiles[k], p, u, l, e.width, terrain, &mut piers);
        }
        // The road lines start and stop at the crossings.
        let start = [ra, la].iter().map(|&c| dot(sub(c, p), u)).fold(0.0, f64::max);
        let end = [rb, lb].iter().map(|&c| dot(sub(c, p), u)).fold(l, f64::min);
        let line = |mesh: &mut Mesh, offset: f64, half: f64, dash: Option<(f64, f64)>| {
            let (mut s, period) = (start, dash.map_or(f64::INFINITY, |(on, off)| on + off));
            while s < end - 1e-6 {
                let stop = dash.map_or(end, |(on, _)| (s + on).min(end));
                stripe(mesh, add(p, scale(u, s)), add(p, scale(u, stop)), n, [offset, half], step, area, &surface);
                s += period.min(end - s + 1.0);
            }
        };
        let half = e.width / 2.0;
        match e.class {
            HIGHWAY => {
                for side in [-1.0, 1.0] {
                    line(yellow, 0.15 * side, 0.06, None);
                    line(white, (half - 0.4) * side, 0.08, None);
                    line(white, half / 2.0 * side, 0.07, Some((3.0, 9.0)));
                }
            }
            ARTERIAL => line(white, 0.0, 0.07, Some((3.0, 6.0))),
            _ => {}
        }
    }
    // The fill of each crossing: a fan from the point to the mouths of its streets.
    for (v, list) in fans(streets, &corners).into_iter().enumerate() {
        let c = points[v];
        let center = [c[0], c[1], ground(c)];
        for [a, b] in list {
            let lift = |x: [f64; 2]| [x[0], x[1], ground(x)];
            tri(asphalt, [center, lift(a), lift(b)]);
        }
    }
    if sidewalk > 0.0 {
        for block in &blocks.0 {
            ring(walks, &block.corners, sidewalk, step, &ground);
        }
    }
    let one = || vec![Place::new(&Frame::default(), 0)];
    let [asphalt, walks, white, yellow, bridge] = meshes;
    Ok(Scene::new([
        Item { name: "asphalt".into(), mesh: asphalt, color: ASPHALT, places: one() },
        Item { name: "sidewalks".into(), mesh: walks, color: SIDEWALK, places: one() },
        Item { name: "road lines".into(), mesh: white, color: WHITE, places: one() },
        Item { name: "center lines".into(), mesh: yellow, color: YELLOW, places: one() },
        Item { name: "bridges".into(), mesh: bridge, color: CONCRETE, places: one() },
        Item { name: "piers".into(), mesh: Solid::cube(1.0, 1.0, 1.0).mesh(), color: PIER, places: piers },
    ]))
}

/// For each edge, the mouth of its ribbon at point a and at point b, each seen from its point.
fn corners(streets: &Streets, area: [f64; 2]) -> Vec<(Mouth, Mouth)> {
    let (points, edges) = (&streets.points, &streets.edges);
    let mut result = vec![([[0.0; 2]; 2], [[0.0; 2]; 2]); edges.len()];
    for (v, list) in outgoing(streets).iter().enumerate() {
        let (m, c) = (list.len(), points[v]);
        for (i, &(_, k, dir)) in list.iter().enumerate() {
            let half = edges[k].width / 2.0;
            let mouth = if m < 2 {
                let n = [-dir[1] * half, dir[0] * half];
                [sub(c, n), add(c, n)].map(|corner| to_edge(c, corner, dir, area))
            } else {
                let (_, kn, dn) = list[(i + 1) % m];
                let (_, kp, dp) = list[(i + m - 1) % m];
                [meet(c, dp, edges[kp].width / 2.0, dir, half), meet(c, dir, half, dn, edges[kn].width / 2.0)]
            }
            // The ribbons lie between their corners, so no road goes out of the area.
            .map(|corner| [corner[0].clamp(-area[0], area[0]), corner[1].clamp(-area[1], area[1])]);
            if edges[k].a == v {
                result[k].0 = mouth;
            } else {
                result[k].1 = mouth;
            }
        }
    }
    result
}

/// The corner of a dead end at `c` moved along the street to the edge of the area of half size
/// `half` when `c` is on the edge. A street at an angle to the edge then ends along the edge.
fn to_edge(c: [f64; 2], corner: [f64; 2], dir: [f64; 2], half: [f64; 2]) -> [f64; 2] {
    match (0..2).find(|&i| c[i].abs() >= half[i] - 1e-6 && dir[i].abs() > 0.2) {
        Some(i) => add(corner, scale(dir, (half[i].copysign(c[i]) - corner[i]) / dir[i])),
        None => corner,
    }
}

/// The streets out of each point, sorted counterclockwise: (angle, edge, unit direction).
fn outgoing(streets: &Streets) -> Vec<Vec<(f64, usize, [f64; 2])>> {
    let (points, edges) = (&streets.points, &streets.edges);
    let mut out: Vec<Vec<(f64, usize, [f64; 2])>> = vec![Vec::new(); points.len()];
    for (k, e) in edges.iter().enumerate() {
        let d = sub(points[e.b], points[e.a]);
        let l = length(d);
        if e.a == e.b || l < 1e-9 {
            continue;
        }
        let u = scale(d, 1.0 / l);
        out[e.a].push((u[1].atan2(u[0]), k, u));
        out[e.b].push(((-u[1]).atan2(-u[0]), k, [-u[0], -u[1]]));
    }
    for list in &mut out {
        list.sort_by(|x, y| x.0.total_cmp(&y.0));
    }
    out
}

/// The point where the left side of the street in direction `d0` meets the right side of the
/// next street counterclockwise, in direction `d1`. It is not farther than 4 widths.
fn meet(c: [f64; 2], d0: [f64; 2], h0: f64, d1: [f64; 2], h1: f64) -> [f64; 2] {
    let (p0, p1) = (add(c, scale([-d0[1], d0[0]], h0)), sub(c, scale([-d1[1], d1[0]], h1)));
    let den = cross(d0, d1);
    let x = if den.abs() < 1e-6 { scale(add(p0, p1), 0.5) } else { add(p0, scale(d0, cross(sub(p1, p0), d1) / den)) };
    let (reach, most) = (length(sub(x, c)), 4.0 * h0.max(h1));
    if reach > most { add(c, scale(sub(x, c), most / reach)) } else { x }
}

/// For each point with three streets or more, the mouths of its streets.
fn fans(streets: &Streets, corners: &[(Mouth, Mouth)]) -> Vec<Vec<Mouth>> {
    let out = outgoing(streets);
    out.iter()
        .enumerate()
        .map(|(v, list)| {
            if list.len() < 3 {
                return Vec::new();
            }
            list.iter().map(|&(_, k, _)| if streets.edges[k].a == v { corners[k].0 } else { corners[k].1 }).collect()
        })
        .collect()
}

/// The deck of each highway over water. The bridges follow chains of streets through points
/// with two streets, so that a bridge can have many edges.
fn bridges(streets: &Streets, terrain: &Terrain, step: f64) -> Vec<Profile> {
    let (points, edges) = (&streets.points, &streets.edges);
    let mut profiles = vec![Vec::new(); edges.len()];
    for chain in chains(streets).into_iter().filter(|chain| chain.iter().all(|&(k, _)| edges[k].class == HIGHWAY)) {
        // The samples along the chain: (edge index in chain, part of edge, point).
        let mut samples = Vec::new();
        for (c, &(k, forward)) in chain.iter().enumerate() {
            let (p, q) = if forward { (points[edges[k].a], points[edges[k].b]) } else { (points[edges[k].b], points[edges[k].a]) };
            let parts = (length(sub(q, p)) / step).ceil().max(1.0) as usize;
            for i in usize::from(c > 0)..=parts {
                let f = i as f64 / parts as f64;
                samples.push((c, f, lerp(p, q, f)));
            }
        }
        let wet: Vec<bool> = samples.iter().map(|s| terrain.water(s.2)).collect();
        if !wet.contains(&true) {
            continue;
        }
        let ground: Vec<f64> = samples.iter().map(|s| terrain.height(s.2) + ROAD).collect();
        let (mut deck, mut weight) = (vec![0.0; samples.len()], vec![0.0_f64; samples.len()]);
        let mut i = 0;
        while i < samples.len() {
            if !wet[i] {
                i += 1;
                continue;
            }
            let mut j = i;
            while j + 1 < samples.len() && wet[j + 1] {
                j += 1;
            }
            let (b0, b1) = (i.saturating_sub(1), (j + 1).min(samples.len() - 1));
            let level = |s: usize| {
                let f = if b1 > b0 { (s - b0) as f64 / (b1 - b0) as f64 } else { 0.0 };
                (ground[b0] * (1.0 - f) + ground[b1] * f).max(terrain.sea + CLEARANCE)
            };
            for s in b0..=b1 {
                (deck[s], weight[s]) = (level(s), 1.0);
            }
            // A ramp of 1 in 12 on each side brings the road up to the deck.
            for (bank, dir) in [(b0, -1_i64), (b1, 1)] {
                let rise = (level(bank) - ground[bank]).max(0.0);
                let ramp = (rise * 12.0 / step).ceil() as i64;
                for r in 1..=ramp {
                    let s = bank as i64 + dir * r;
                    if s < 0 || s >= samples.len() as i64 {
                        break;
                    }
                    let w = 1.0 - r as f64 / (ramp + 1) as f64;
                    if w > weight[s as usize] {
                        (deck[s as usize], weight[s as usize]) = (level(bank), w);
                    }
                }
            }
            i = j + 1;
        }
        for (s, &(c, f, _)) in samples.iter().enumerate() {
            let (k, forward) = chain[c];
            profiles[k].push((if forward { f } else { 1.0 - f }, deck[s], weight[s]));
        }
        // The point at the end of an edge is the start of the next edge too.
        for (s, &(c, f, _)) in samples.iter().enumerate() {
            if c + 1 < chain.len() && f >= 1.0 {
                let (k, forward) = chain[c + 1];
                profiles[k].push((if forward { 0.0 } else { 1.0 }, deck[s], weight[s]));
            }
        }
    }
    for profile in &mut profiles {
        profile.sort_by(|a, b| a.0.total_cmp(&b.0));
        if profile.iter().all(|s| s.2 <= 0.0) {
            profile.clear();
        }
    }
    profiles
}

/// The chains of edges between the points that do not have exactly two streets, each edge with
/// true when the chain goes from its point a to its point b.
fn chains(streets: &Streets) -> Vec<Vec<(usize, bool)>> {
    let (points, edges) = (&streets.points, &streets.edges);
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); points.len()];
    for (k, e) in edges.iter().enumerate().filter(|(_, e)| e.a != e.b) {
        around[e.a].push(k);
        around[e.b].push(k);
    }
    let mut seen = vec![false; edges.len()];
    let mut out = Vec::new();
    let starts = (0..points.len()).filter(|&p| around[p].len() != 2).chain(0..points.len());
    for start in starts {
        for &first in &around[start] {
            if seen[first] {
                continue;
            }
            let (mut chain, mut at, mut k) = (Vec::new(), start, first);
            loop {
                seen[k] = true;
                let forward = edges[k].a == at;
                chain.push((k, forward));
                at = if forward { edges[k].b } else { edges[k].a };
                match around[at].iter().find(|&&e| e != k && !seen[e]) {
                    Some(&next) if around[at].len() == 2 => k = next,
                    _ => break,
                }
            }
            out.push(chain);
        }
    }
    out
}

/// The deck height and weight at part `f` of an edge, or `None` on the ground.
fn lookup(profile: &[(f64, f64, f64)], f: f64) -> Option<(f64, f64)> {
    let f = f.clamp(0.0, 1.0);
    let i = profile.iter().position(|s| s.0 >= f)?;
    let (h, w) = if i == 0 {
        (profile[0].1, profile[0].2)
    } else {
        let (a, b) = (profile[i - 1], profile[i]);
        let t = if b.0 > a.0 { (f - a.0) / (b.0 - a.0) } else { 0.0 };
        // The deck height comes from the side with the bridge, so that a ramp stays straight.
        let h = if a.2 >= b.2 { a.1 } else { b.1 };
        (h, a.2 + (b.2 - a.2) * t)
    };
    (w > 0.0).then_some((h, w.min(1.0)))
}

/// A pier about each 24 m along the full deck of an edge, from the bed up to the deck.
fn piers_of(profile: &[(f64, f64, f64)], p: [f64; 2], u: [f64; 2], l: f64, width: f64, terrain: &Terrain, piers: &mut Vec<Place>) {
    let full: Vec<&(f64, f64, f64)> = profile.iter().filter(|s| s.2 >= 1.0).collect();
    let (Some(first), Some(last)) = (full.first(), full.last()) else { return };
    let span = (last.0 - first.0) * l;
    let count = (span / 24.0).floor() as usize;
    for i in 1..=count {
        let f = first.0 + (last.0 - first.0) * i as f64 / (count + 1) as f64;
        let x = add(p, scale(u, f * l));
        let Some((deck, _)) = lookup(profile, f) else { continue };
        let (bed, top) = (terrain.height(x) - 1.0, deck - DECK);
        if top - bed < 1.0 {
            continue;
        }
        let frame = Frame { x: [u[0] * 1.6, u[1] * 1.6, 0.0], y: [-u[1] * width * 0.6, u[0] * width * 0.6, 0.0], z: [0.0, 0.0, top - bed], origin: [x[0], x[1], (top + bed) / 2.0] };
        piers.push(Place::new(&frame, 0));
    }
}

/// A thin line on the road from `a` to `b`, `offset` to the left and `half` wide on each side, a
/// little over the surface. The parts out of the area of half size `area` are left out.
#[allow(clippy::too_many_arguments)]
fn stripe(mesh: &mut Mesh, a: [f64; 2], b: [f64; 2], n: [f64; 2], [offset, half]: [f64; 2], step: f64, area: [f64; 2], surface: &impl Fn([f64; 2]) -> f64) {
    let parts = (length(sub(b, a)) / step).ceil().max(1.0) as usize;
    let at = |f: f64, side: f64| {
        let x = add(lerp(a, b, f), scale(n, offset + side * half));
        [x[0], x[1], surface(x) + 0.03]
    };
    for i in 0..parts {
        let (f0, f1) = (i as f64 / parts as f64, (i + 1) as f64 / parts as f64);
        let corners = [at(f0, -1.0), at(f1, -1.0), at(f1, 1.0), at(f0, 1.0)];
        if corners.iter().all(|c| c[0].abs() <= area[0] && c[1].abs() <= area[1]) {
            quad(mesh, corners);
        }
    }
}

/// A sidewalk ring of `width` inside a counterclockwise block, with a curb on the street side
/// and a step down on the inner side.
fn ring(mesh: &mut Mesh, corners: &[[f64; 2]], width: f64, step: f64, ground: &impl Fn([f64; 2]) -> f64) {
    let m = corners.len();
    if m < 3 {
        return;
    }
    let inner: Vec<[f64; 2]> = (0..m).map(|i| miter(corners[(i + m - 1) % m], corners[i], corners[(i + 1) % m], width)).collect();
    for i in 0..m {
        let (o0, o1, i0, i1) = (corners[i], corners[(i + 1) % m], inner[i], inner[(i + 1) % m]);
        let parts = (length(sub(o1, o0)) / step).ceil().max(1.0) as usize;
        for s in 0..parts {
            let (f0, f1) = (s as f64 / parts as f64, (s + 1) as f64 / parts as f64);
            let (a, b, c, d) = (lerp(o0, o1, f0), lerp(o0, o1, f1), lerp(i0, i1, f1), lerp(i0, i1, f0));
            let top = |x: [f64; 2]| [x[0], x[1], ground(x) + CURB];
            let low = |x: [f64; 2], dz: f64| [x[0], x[1], ground(x) + dz];
            quad(mesh, [top(a), top(b), top(c), top(d)]);
            quad(mesh, [low(a, 0.0), low(b, 0.0), top(b), top(a)]);
            quad(mesh, [low(c, -ROAD), low(d, -ROAD), top(d), top(c)]);
        }
    }
}

/// The corner `width` inside a counterclockwise polygon at `b`, not farther than 3 widths.
fn miter(a: [f64; 2], b: [f64; 2], c: [f64; 2], width: f64) -> [f64; 2] {
    let unit = |v: [f64; 2]| scale(v, 1.0 / length(v).max(1e-12));
    let (t0, t1) = (unit(sub(b, a)), unit(sub(c, b)));
    let (n0, n1) = ([-t0[1], t0[0]], [-t1[1], t1[0]]);
    let mid = unit(add(n0, n1));
    let cos = dot(mid, n0).max(1.0 / 3.0);
    add(b, scale(mid, width / cos))
}

fn lerp(a: [f64; 2], b: [f64; 2], f: f64) -> [f64; 2] {
    add(a, scale(sub(b, a), f))
}
