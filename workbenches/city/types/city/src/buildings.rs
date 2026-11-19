//! The plan of each building (task E12): its kind, its tiers, its ground level and its style.
//! The `walls`, `facades` and `roofs` steps make the meshes from the plan.

use std::fmt::{self, Write};

use geometry::{Random, Seed};
use patina_sdk::{Text, Viewer};

use crate::plots::offset;
use crate::shapes::contains;
use crate::{Plot, Plots, Terrain, Zone, add, area, dot, length, scale, sub};

/// The kind of a building, from the zone and the floors of its lot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    House,
    Apartment,
    Shop,
    Tower,
    Warehouse,
}

/// The form of the roof of a tier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Roof {
    #[default]
    Flat,
    Gable,
    Hip,
    Sawtooth,
}

/// One block of a building: a footprint with its corners counterclockwise, from `bottom` to
/// `top` over the ground level. A gable, hip or sawtooth tier is a rectangle, and its roof is
/// `rise` high.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tier {
    pub corners: Vec<[f64; 2]>,
    /// True for an edge that faces a street.
    pub fronts: Vec<bool>,
    pub bottom: f64,
    pub top: f64,
    pub roof: Roof,
    pub rise: f64,
}

impl Tier {
    /// The first long edge of a rectangle, 0 or 1. A gable or a hip has its ridge along it.
    pub fn ridge(&self) -> usize {
        let c = &self.corners;
        usize::from(c.len() == 4 && length(sub(c[2], c[1])) > length(sub(c[1], c[0])))
    }
}

/// The colors of a building as sRGB 0xRRGGBB, the width of its bays and its window kind:
/// 0 tall, 1 wide or 2 band.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    pub wall: u32,
    pub trim: u32,
    pub roof: u32,
    pub accent: u32,
    pub bay: f64,
    pub window: usize,
}

/// The plan of one building.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Building {
    /// The number of its lot plus 1: the group of its places in a scene.
    pub group: u32,
    pub kind: Kind,
    /// The first tier stands on the ground.
    pub tiers: Vec<Tier>,
    /// The ground level: a little over the highest ground under the footprint. The plinth goes
    /// down to `low`, the lowest ground.
    pub ground: f64,
    pub low: f64,
    /// The height of the ground floor and of each floor over it.
    pub first: f64,
    pub floor: f64,
    pub style: Style,
}

impl Building {
    /// The height of the base of floor `k` over the ground level.
    pub fn level(&self, k: u32) -> f64 {
        if k == 0 { 0.0 } else { self.first + f64::from(k - 1) * self.floor }
    }

    /// The floors from the bottom to the top of a tier: (floor, base over the ground level).
    pub fn floors(&self, tier: &Tier) -> impl Iterator<Item = (u32, f64)> + '_ {
        let (bottom, top) = (tier.bottom, tier.top);
        (0..crate::MAX_FLOORS + 1).map(|k| (k, self.level(k))).skip_while(move |&(_, z)| z < bottom - 0.01).take_while(move |&(_, z)| z < top - 1.0)
    }

    /// True when the point at height `z` over the ground level is in a tier other than `skip`.
    pub fn covers(&self, skip: usize, p: [f64; 2], z: f64) -> bool {
        self.tiers.iter().enumerate().any(|(i, t)| i != skip && z >= t.bottom && z < t.top && crate::shapes::inside(&t.corners, p))
    }
}

/// A list of building plans.
#[derive(Clone, Default, PartialEq)]
pub struct Buildings(pub Vec<Building>);

/// Shows "300 buildings: 200 houses, 50 apartments, 30 shops, 10 towers, 10 warehouses".
impl fmt::Debug for Buildings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let count = |k: Kind| self.0.iter().filter(|b| b.kind == k).count();
        let (h, a, s, t, w) = (count(Kind::House), count(Kind::Apartment), count(Kind::Shop), count(Kind::Tower), count(Kind::Warehouse));
        write!(f, "{} buildings: {h} houses, {a} apartments, {s} shops, {t} towers, {w} warehouses", self.0.len())
    }
}

impl Viewer for Buildings {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }
}

const HOUSE_WALLS: [u32; 6] = [0xe8dcc4, 0xd9c7a8, 0xf1ece2, 0xc9a98c, 0xb7c2b0, 0xd8c08a];
const BRICK_WALLS: [u32; 5] = [0x9a5b46, 0x7d4a3a, 0xb59a7a, 0x8f8a82, 0xa78b6f];
const SHOP_WALLS: [u32; 6] = [0xc7b299, 0x8e6f5a, 0xd6cbb8, 0x6f7f86, 0xa4675a, 0xbfa36f];
const TOWER_WALLS: [u32; 4] = [0x7f8c95, 0x5d6b78, 0xa9aaa6, 0x4a5560];
const SHED_WALLS: [u32; 4] = [0x9aa0a3, 0x7c8b96, 0xb3a98f, 0x8a8f86];
const TRIMS: [u32; 4] = [0xf2efe8, 0x4a4a48, 0xc8bfae, 0x6b5444];
const TILES: [u32; 4] = [0xa0523a, 0x4f555c, 0x5b4636, 0x6e6a64];
const ACCENTS: [u32; 6] = [0xa8322d, 0x2f6b4f, 0x2c4f7c, 0xc28b2c, 0x2c7a7b, 0x7a3b62];
const GRAVEL: u32 = 0x5f5d58;
const SHEET: u32 = 0x55595c;

/// A plan for each lot that is not a park. The zone and the floors give the kind: a house, an
/// apartment, a shop, a tower or a warehouse. A house stands `setback` back from its street.
/// Each floor over the ground floor is `floor` high. The ground level comes from the terrain.
pub fn buildings(plots: &Plots, terrain: &Terrain, seed: Seed, floor: f64, setback: f64) -> Result<Buildings, String> {
    if !(2.0..=10.0).contains(&floor) || !(0.0..=30.0).contains(&setback) {
        return Err(format!("the floor must be from 2 to 10 and the setback from 0 to 30, not {floor} and {setback}"));
    }
    let mut out = Vec::new();
    for (i, plot) in plots.0.iter().enumerate() {
        if plot.zone == Zone::Park || plot.floors == 0 || plot.corners.len() < 3 {
            continue;
        }
        let mut random = seed.child(&format!("building {i}")).random();
        let kind = match (plot.zone, plot.floors) {
            (Zone::Industrial, _) => Kind::Warehouse,
            (Zone::Residential, 0..=3) => Kind::House,
            (Zone::Residential, _) => Kind::Apartment,
            (_, 0..=6) => Kind::Shop,
            _ => Kind::Tower,
        };
        let (first, storey) = match kind {
            Kind::House => (floor, floor),
            Kind::Apartment => (floor + 0.4, floor),
            Kind::Shop => (floor + 1.3, floor),
            Kind::Tower => (floor + 2.0, floor + 0.3),
            Kind::Warehouse => (random.range(6.0, 9.0), 4.5),
        };
        let mut building = Building { group: i as u32 + 1, kind, first, floor: storey, style: style(kind, &mut random), ..Building::default() };
        let top = building.level(plot.floors);
        building.tiers = match kind {
            Kind::House => house(plot, setback, top, storey, &mut random),
            Kind::Warehouse => shed(plot, top, &mut random),
            Kind::Apartment => block(plot, 2.0, 0.5, top).into_iter().collect(),
            Kind::Shop => block(plot, 0.0, 0.0, top).into_iter().collect(),
            Kind::Tower => tower(plot, &building, &mut random),
        };
        if building.tiers.is_empty() {
            continue;
        }
        let heights: Vec<f64> = building.tiers.iter().flat_map(|t| &t.corners).map(|&p| terrain.height(p)).collect();
        building.low = heights.iter().copied().fold(f64::INFINITY, f64::min);
        building.ground = heights.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 0.15;
        out.push(building);
    }
    Ok(Buildings(out))
}

fn style(kind: Kind, random: &mut Random) -> Style {
    let mut pick = |list: &[u32]| list[random.index(list.len())];
    let (wall, trim, roof) = match kind {
        Kind::House => (pick(&HOUSE_WALLS), pick(&TRIMS), pick(&TILES)),
        Kind::Apartment => (pick(&BRICK_WALLS), pick(&TRIMS[..3]), GRAVEL),
        Kind::Shop => (pick(&SHOP_WALLS), pick(&TRIMS), GRAVEL),
        Kind::Tower => (pick(&TOWER_WALLS), TRIMS[1], GRAVEL),
        Kind::Warehouse => (pick(&SHED_WALLS), TRIMS[1], SHEET),
    };
    let accent = pick(&ACCENTS);
    let (bay, window) = match kind {
        Kind::House => (3.0, 0),
        Kind::Apartment => (3.4, random.index(2)),
        Kind::Shop => (4.0, 1),
        Kind::Tower => (3.0, 2),
        Kind::Warehouse => (6.0, 1),
    };
    Style { wall, trim, roof, accent, bay, window }
}

/// The plot moved in by `front` at the streets and by `side` at the other edges, with the
/// street edges of the new polygon.
fn setback(plot: &Plot, front: f64, side: f64) -> Option<(Vec<[f64; 2]>, Vec<bool>)> {
    let distances: Vec<f64> = (0..plot.corners.len()).map(|e| if plot.fronts.get(e).copied().flatten().is_some() { front } else { side }).collect();
    let (corners, kept) = offset(&plot.corners, &distances)?;
    let fronts = kept.iter().map(|&e| plot.fronts.get(e).copied().flatten().is_some()).collect();
    Some((corners, fronts))
}

/// One tier on the whole lot, for an apartment or a shop.
fn block(plot: &Plot, front: f64, side: f64, top: f64) -> Option<Tier> {
    let (corners, fronts) = setback(plot, front, side)?;
    (area(&corners) > 30.0).then(|| Tier { corners, fronts, top, ..Tier::default() })
}

/// A rectangle on the street edge `edge` of a polygon: `s` along the edge from its first
/// corner, and `t` in from it.
struct Rect {
    origin: [f64; 2],
    u: [f64; 2],
    s: (f64, f64),
    t: (f64, f64),
}

impl Rect {
    fn corners(&self) -> Vec<[f64; 2]> {
        let v = [-self.u[1], self.u[0]];
        let at = |s: f64, t: f64| add(self.origin, add(scale(self.u, s), scale(v, t)));
        vec![at(self.s.0, self.t.0), at(self.s.1, self.t.0), at(self.s.1, self.t.1), at(self.s.0, self.t.1)]
    }
}

/// The largest rectangle on the middle of edge `edge` of the polygon, at most `long` along the
/// edge and `deep` in from it, that is in the polygon.
fn fit(polygon: &[[f64; 2]], edge: usize, long: f64, deep: f64) -> Option<Rect> {
    let n = polygon.len();
    let (a, b) = (polygon[edge], polygon[(edge + 1) % n]);
    let l = length(sub(b, a));
    if l < 1e-6 {
        return None;
    }
    let u = scale(sub(b, a), 1.0 / l);
    let reach = polygon.iter().map(|&p| dot(sub(p, a), [-u[1], u[0]])).fold(0.0, f64::max);
    let (mut w, mut h) = (l.min(long), reach.min(deep));
    for _ in 0..24 {
        let s0 = (l - w) / 2.0;
        let rect = Rect { origin: a, u, s: (s0, s0 + w), t: (0.0, h) };
        if w < 3.0 || h < 3.0 {
            return None;
        }
        if contains(polygon, &rect.corners()) {
            return Some(rect);
        }
        (w, h) = (w * 0.9, h * 0.9);
    }
    None
}

/// The street edge of the polygon to build on: the longest one, or the longest edge.
pub(crate) fn main_edge(corners: &[[f64; 2]], fronts: &[bool]) -> usize {
    let n = corners.len();
    let long = |e: usize| length(sub(corners[(e + 1) % n], corners[e]));
    let best = |edges: &mut dyn Iterator<Item = usize>| edges.max_by(|&a, &b| long(a).total_cmp(&long(b)));
    best(&mut (0..n).filter(|&e| fronts.get(e) == Some(&true))).or_else(|| best(&mut (0..n))).unwrap_or(0)
}

/// A house of a rectangle and, sometimes, a wing at the back for an L shape. Both have a gable
/// or a hip roof.
fn house(plot: &Plot, setback_front: f64, top: f64, storey: f64, random: &mut Random) -> Vec<Tier> {
    let Some((corners, fronts)) = setback(plot, setback_front, 1.5) else { return Vec::new() };
    let edge = main_edge(&corners, &fronts);
    let Some(rect) = fit(&corners, edge, random.range(9.0, 15.0), random.range(7.0, 11.0)) else { return Vec::new() };
    let roof = if random.unit() < 0.6 { Roof::Gable } else { Roof::Hip };
    let (w, h) = (rect.s.1 - rect.s.0, rect.t.1 - rect.t.0);
    let pitch = random.range(0.45, 0.75);
    let main = Tier { corners: rect.corners(), fronts: vec![true, false, false, false], top, roof, rise: pitch * w.min(h) / 2.0, ..Tier::default() };
    let mut tiers = vec![main];
    if random.unit() < 0.45 {
        let width = w * random.range(0.35, 0.5);
        let s0 = if random.unit() < 0.5 { rect.s.0 } else { rect.s.1 - width };
        let wing = Rect { origin: rect.origin, u: rect.u, s: (s0, s0 + width), t: (rect.t.1 - 0.5, rect.t.1 + random.range(3.5, 6.0)) };
        let wing_top = if top > storey + 0.1 && random.unit() < 0.5 { top - storey } else { top };
        if contains(&corners, &wing.corners()) {
            let (ww, wd) = (wing.s.1 - wing.s.0, wing.t.1 - wing.t.0);
            tiers.push(Tier { corners: wing.corners(), fronts: vec![false; 4], top: wing_top, roof, rise: pitch * ww.min(wd) / 2.0, ..Tier::default() });
        }
    }
    tiers
}

/// A warehouse: a large rectangle back from the street with a flat or a sawtooth roof.
fn shed(plot: &Plot, top: f64, random: &mut Random) -> Vec<Tier> {
    let Some((corners, fronts)) = setback(plot, 5.0, 2.0) else { return Vec::new() };
    let edge = main_edge(&corners, &fronts);
    let Some(rect) = fit(&corners, edge, 70.0, 45.0) else { return Vec::new() };
    let sawtooth = random.unit() < 0.5 && rect.t.1 - rect.t.0 >= 12.0;
    let (roof, rise) = if sawtooth { (Roof::Sawtooth, random.range(2.0, 3.0)) } else { (Roof::Flat, 0.0) };
    vec![Tier { corners: rect.corners(), fronts: vec![true, false, false, false], top, roof, rise, ..Tier::default() }]
}

/// A tower: a podium on the whole lot, a shaft back from its edges, and a step-back near the top
/// of a tall tower.
fn tower(plot: &Plot, building: &Building, random: &mut Random) -> Vec<Tier> {
    let Some(podium) = block(plot, 0.0, 0.0, 0.0) else { return Vec::new() };
    let floors = plot.floors;
    let base = (2 + random.index(3) as u32).min(floors - 1);
    let mut tiers = vec![Tier { top: building.level(base), ..podium.clone() }];
    let Some((inner, kept)) = offset(&podium.corners, &vec![3.0; podium.corners.len()]) else { return tiers };
    let fronts: Vec<bool> = kept.iter().map(|&e| podium.fronts[e]).collect();
    let edge = main_edge(&inner, &fronts);
    let shaft = match fit(&inner, edge, 45.0, 32.0) {
        Some(rect) => rect.corners(),
        None => inner,
    };
    if area(&shaft) < 60.0 {
        tiers[0].top = building.level(floors);
        return tiers;
    }
    let step = if floors > 20 && random.unit() < 0.6 { (f64::from(floors) * random.range(0.65, 0.8)) as u32 } else { floors };
    let n = shaft.len();
    tiers.push(Tier { corners: shaft.clone(), fronts: vec![false; n], bottom: building.level(base), top: building.level(step), ..Tier::default() });
    if step < floors
        && let Some((upper, _)) = offset(&shaft, &vec![2.0; n])
        && area(&upper) > 40.0
    {
        let m = upper.len();
        tiers.push(Tier { corners: upper, fronts: vec![false; m], bottom: building.level(step), top: building.level(floors), ..Tier::default() });
    } else if let Some(last) = tiers.last_mut() {
        last.top = building.level(floors);
    }
    tiers
}
