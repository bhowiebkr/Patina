//! The roofs of each building (task E12): gable and hip roofs with an overhang, flat roofs with
//! gravel, sawtooth roofs with glazing, and the parts on the roofs.

use geometry::{IDENTITY, Item, Mesh, Place, Random, Scene, Seed, Solid};

use crate::buildings::{Building, Buildings, Kind, Roof, Tier, main_edge};
use crate::shapes::{append, cuboid, grow, inside, place, prism, quad, slab, tri};
use crate::walls::PARAPET_IN;
use crate::{MAX_PLACES, add, area, centroid, length, scale, sub};

/// How far a pitched roof goes past the walls, and the thickness of its panels.
const OVERHANG: f64 = 0.4;
const PANEL: f64 = 0.12;

const BRICK: u32 = 0x8a4b3a;
const UNIT: u32 = 0xb9bcbd;
const TANK: u32 = 0x6e5a44;
const CONCRETE: u32 = 0x9d9a92;
const ANTENNA: u32 = 0xb0b3b5;
const SKYLIGHT: u32 = 0x5d7f95;

/// The shared parts on the roofs, each with its color.
const PARTS: [(&str, u32); 7] = [
    ("chimneys", BRICK),
    ("rooftop units", UNIT),
    ("water tanks", TANK),
    ("tank stands", CONCRETE),
    ("stair houses", CONCRETE),
    ("antennas", ANTENNA),
    ("skylights", SKYLIGHT),
];

/// One item "roof {group}" for each building in its roof color, and the parts on the roofs:
/// chimneys on houses, rooftop units on flat roofs, water tanks and stair houses on apartments,
/// antennas on tall towers and glazing on sawtooth roofs.
pub fn roofs(buildings: &Buildings, seed: Seed) -> Result<Scene, String> {
    let mut items = Vec::new();
    let mut parts: [Vec<Place>; 7] = Default::default();
    for b in &buildings.0 {
        let mut random = seed.child(&format!("roof {}", b.group)).random();
        let mut mesh = Mesh::default();
        for (ti, tier) in b.tiers.iter().enumerate() {
            let z = b.ground + tier.top;
            match tier.roof {
                Roof::Gable | Roof::Hip if tier.corners.len() == 4 => pitched(&mut mesh, tier, z, tier.roof == Roof::Gable),
                Roof::Sawtooth if tier.corners.len() == 4 => sawtooth(&mut mesh, tier, z, b.group, &mut parts[6]),
                _ => append(&mut mesh, &prism(&grow(&tier.corners, -PARAPET_IN), z, z + 0.08)),
            }
            on_roof(b, ti, tier, &mut random, &mut parts);
        }
        items.push(Item { name: format!("roof {}", b.group), mesh, color: b.style.roof, places: vec![Place { matrix: IDENTITY, group: b.group }] });
        if parts.iter().map(Vec::len).sum::<usize>() > MAX_PLACES {
            return Err(format!("the roofs have more than {MAX_PLACES} parts"));
        }
    }
    let (mut boxes, mut drum, mut panel) = (Mesh::default(), Mesh::default(), Mesh::default());
    cuboid(&mut boxes, [-0.5, -0.5, 0.0], [0.5, 0.5, 1.0]);
    append(&mut drum, &Solid::cylinder(1.0, 0.5, 0.5, 16).mesh());
    cuboid(&mut panel, [-0.5, 0.0, 0.0], [0.5, 1.0, 1.0]);
    for (i, places) in parts.into_iter().enumerate() {
        let mesh = match i {
            2 | 5 => drum.clone(),
            6 => panel.clone(),
            _ => boxes.clone(),
        };
        items.push(Item { name: PARTS[i].0.into(), mesh, color: PARTS[i].1, places });
    }
    Ok(Scene::new(items))
}

/// The parts on the roof of a tier, at free spots that no other tier covers.
fn on_roof(b: &Building, ti: usize, tier: &Tier, random: &mut Random, parts: &mut [Vec<Place>; 7]) {
    let (c, z) = (&tier.corners, b.ground + tier.top);
    let e = main_edge(c, &tier.fronts);
    let t = sub(c[(e + 1) % c.len()], c[e]);
    let t = scale(t, 1.0 / length(t).max(1e-9));
    let out = [t[1], -t[0]];
    let at = |p: [f64; 2], dz: f64| [p[0], p[1], z + dz];
    let mut put = |part: usize, p: [f64; 2], dz: f64, size: [f64; 3]| parts[part].push(place(at(p, dz), out, size, b.group));
    if matches!(tier.roof, Roof::Gable | Roof::Hip) {
        if b.kind == Kind::House && ti == 0 && random.unit() < 0.6 {
            let p = add(centroid(c), scale(t, length(sub(c[1], c[0])).min(length(sub(c[2], c[1]))) * random.range(-0.3, 0.3)));
            put(0, p, -0.5, [0.6, 0.8, tier.rise + 1.4]);
        }
        return;
    }
    let free = grow(c, -(PARAPET_IN + 1.5));
    let spot = |random: &mut Random| {
        let (lo, hi) = free.iter().fold(([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        (0..20).map(|_| [random.range(lo[0], hi[0]), random.range(lo[1], hi[1])]).find(|&p| inside(&free, p) && !b.covers(ti, p, tier.top + 0.5))
    };
    if b.kind == Kind::House || tier.roof == Roof::Sawtooth || area(&free) < 12.0 {
        return;
    }
    let units = (area(&free) / 150.0).round().clamp(1.0, 8.0) as usize;
    for _ in 0..units {
        if let Some(p) = spot(random) {
            put(1, p, 0.08, [2.2, 1.6, 1.3]);
        }
    }
    let top = b.tiers.iter().all(|other| other.top <= tier.top);
    if top && b.kind == Kind::Apartment && tier.top >= b.level(5) && random.unit() < 0.4 && let Some(p) = spot(random) {
        put(3, p, 0.08, [2.0, 2.0, 1.2]);
        put(2, p, 1.28, [2.6, 2.6, 3.0]);
    }
    if top && matches!(b.kind, Kind::Apartment | Kind::Tower) && let Some(p) = spot(random) {
        put(4, p, 0.08, [3.0, 4.0, 2.8]);
    }
    if top && b.kind == Kind::Tower && tier.top >= b.level(25) {
        put(5, centroid(c), 0.08, [0.35, 0.35, random.range(10.0, 16.0)]);
    }
}

/// A gable or a hip roof on a rectangle tier from the top of its walls at `z`, with its ridge
/// along the long edge and an overhang on each side.
fn pitched(m: &mut Mesh, tier: &Tier, z: f64, gable: bool) {
    let c = &tier.corners;
    let k = tier.ridge();
    let (p0, p1, p3) = (c[k], c[(k + 1) % 4], c[(k + 3) % 4]);
    let (lu, lv) = (length(sub(p1, p0)), length(sub(p3, p0)));
    if lu < 1e-6 || lv < 1e-6 {
        return;
    }
    let (u, v) = (scale(sub(p1, p0), 1.0 / lu), scale(sub(p3, p0), 1.0 / lv));
    let center = add(p0, add(scale(u, lu / 2.0), scale(v, lv / 2.0)));
    let (hl, hw) = (lu / 2.0 + OVERHANG, lv / 2.0 + OVERHANG);
    let (eave, ridge) = (z - tier.rise * OVERHANG / (lv / 2.0), z + tier.rise);
    let at = |s: f64, t: f64, z: f64| {
        let q = add(center, add(scale(u, s), scale(v, t)));
        [q[0], q[1], z]
    };
    let r = if gable { hl } else { hl - hw };
    slab(m, [at(-hl, -hw, eave), at(hl, -hw, eave), at(r, 0.0, ridge), at(-r, 0.0, ridge)], PANEL);
    slab(m, [at(hl, hw, eave), at(-hl, hw, eave), at(-r, 0.0, ridge), at(r, 0.0, ridge)], PANEL);
    if !gable {
        wedge(m, [at(hl, -hw, eave), at(hl, hw, eave), at(r, 0.0, ridge)]);
        wedge(m, [at(-hl, hw, eave), at(-hl, -hw, eave), at(-r, 0.0, ridge)]);
    }
}

/// A triangle slab under `top` with counterclockwise corners: the top, the bottom and the
/// edge under the eave from the first to the second corner.
fn wedge(m: &mut Mesh, top: [[f64; 3]; 3]) {
    let low = top.map(|[x, y, z]| [x, y, z - PANEL]);
    tri(m, top);
    tri(m, [low[2], low[1], low[0]]);
    quad(m, [low[0], low[1], top[1], top[0]]);
}

/// Teeth across a rectangle tier from `z`, each with a slope up to a vertical face of glass.
fn sawtooth(m: &mut Mesh, tier: &Tier, z: f64, group: u32, glass: &mut Vec<Place>) {
    let c = &tier.corners;
    let k = tier.ridge();
    let (p0, p1, p3) = (c[k], c[(k + 1) % 4], c[(k + 3) % 4]);
    let (lu, lv) = (length(sub(p1, p0)), length(sub(p3, p0)));
    if lu < 2.0 || lv < 2.0 {
        return;
    }
    let (u, v) = (scale(sub(p1, p0), 1.0 / lu), scale(sub(p3, p0), 1.0 / lv));
    let at = |s: f64, t: f64, z: f64| {
        let q = add(p0, add(scale(u, s), scale(v, t)));
        [q[0], q[1], z]
    };
    let teeth = (lv / 8.0).round().max(1.0) as usize;
    let (w, h) = (lv / teeth as f64, tier.rise);
    for i in 0..teeth {
        let (t0, t1) = (i as f64 * w, (i + 1) as f64 * w);
        quad(m, [at(0.0, t0, z), at(lu, t0, z), at(lu, t1, z + h), at(0.0, t1, z + h)]);
        quad(m, [at(lu, t1, z), at(0.0, t1, z), at(0.0, t1, z + h), at(lu, t1, z + h)]);
        tri(m, [at(lu, t0, z), at(lu, t1, z), at(lu, t1, z + h)]);
        tri(m, [at(0.0, t1, z), at(0.0, t0, z), at(0.0, t1, z + h)]);
        let p = add(p0, add(scale(u, lu / 2.0), scale(v, t1)));
        glass.push(place([p[0], p[1], z + 0.1 * h], v, [lu - 1.0, 0.05, 0.8 * h], group));
    }
}
