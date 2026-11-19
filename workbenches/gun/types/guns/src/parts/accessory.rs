//! The accessories: they go along +X under the barrel, from the front of the receiver.

use geometry::Solid;

use super::Build;
use crate::shapes::*;
use crate::{Maker, Material, Stats};

pub(super) fn make(b: &mut Build, maker: Maker) -> Stats {
    let len = (0.6 * b.s.barrel).clamp(0.05, 0.14);
    let first = b.variant(2) == 0;
    let base = b.base();
    let mut stats = Stats::default();
    match maker {
        Maker::Blockwerk => {
            blockwerk(b, len, first);
            stats.damage = 0.15 * base.damage;
            stats.reload = 0.05 * base.reload;
        }
        Maker::Vesper => {
            vesper(b, len, first);
            stats.accuracy = 0.15 * (100.0 - base.accuracy);
        }
        Maker::Scrapjaw => {
            scrapjaw(b, len, first);
            stats.damage = 0.1 * base.damage;
            stats.rate = 0.1 * base.rate;
        }
        Maker::Halo => {
            halo(b, len, first);
            stats.reload = -0.1 * base.reload;
            stats.rate = 0.05 * base.rate;
        }
    }
    stats
}

/// The mount block at the front of the receiver.
fn mount(b: &mut Build, material: Material, x1: f64) {
    b.add(material, bevel([-0.006, x1], [-0.008, 0.006], 0.016, 0.002));
}

/// A bayonet, or a grenade tube.
fn blockwerk(b: &mut Build, len: f64, first: bool) {
    if first {
        mount(b, Material::Trim, 0.02);
        let blade = [[0.016, -0.006], [len + 0.02, -0.004], [len + 0.04, 0.0], [len + 0.02, 0.003], [0.016, 0.004]];
        b.add(Material::Metal, slab(&blade, 0.003));
        b.add(Material::Rarity, both(&decal(&[[0.022, -0.003], [0.5 * len, -0.0025], [0.5 * len, 0.0], [0.022, 0.0]], 0.0015, 0.0006)));
        return;
    }
    let r = 0.016;
    b.add(Material::Paint, bevel([-0.004, 0.4 * len], [-2.0 * r - 0.004, 0.006], 2.0 * r + 0.006, 0.003));
    b.add(Material::Metal, tube([0.0, len], r, r - 0.003, 24).translate([0.0, 0.0, -r - 0.002]));
    b.add(Material::Rarity, tube([len - 0.008, len], r + 0.002, r - 0.003, 24).translate([0.0, 0.0, -r - 0.002]));
}

/// A glow blade, or a laser.
fn vesper(b: &mut Build, len: f64, first: bool) {
    mount(b, Material::Trim, 0.024);
    if first {
        let blade = [[0.02, -0.005], [0.6 * len, -0.008], [len + 0.03, -0.004], [len + 0.04, 0.001], [0.6 * len, 0.0], [0.02, 0.004]];
        b.add(Material::Glow, slab(&blade, 0.0025));
        b.add(Material::Metal, slab(&[[0.016, -0.007], [0.03, -0.007], [0.03, 0.006], [0.016, 0.006]], 0.006));
        return;
    }
    let body = pill(&[([0.0, -0.012], 0.01), ([0.6 * len, -0.012], 0.009)], 0.016, 0.003);
    b.add(Material::Paint, body);
    b.add(Material::Glow, rod_x([0.6 * len + 0.006, 0.6 * len + 0.01], [0.005, 0.004], 16).translate([0.0, 0.0, -0.012]));
    b.add(Material::Rarity, rod_y([0.2 * len, -0.012], 0.018, 0.003, 10));
}

/// A buzzsaw on an arm, or a bar with spikes.
fn scrapjaw(b: &mut Build, len: f64, first: bool) {
    if first {
        let r = 0.028;
        let c = [len, -r * 0.4];
        let teeth = 14;
        let star: Vec<[f64; 2]> = (0..2 * teeth)
            .map(|i| {
                let a = std::f64::consts::PI * f64::from(i) / f64::from(teeth);
                let rr = if i % 2 == 0 { r } else { 0.82 * r };
                [c[0] + rr * a.cos(), c[1] + rr * a.sin()]
            })
            .collect();
        b.add(Material::Metal, slab(&star, 0.0025).difference(&rod_y(c, 0.01, 0.004, 12)));
        b.add(Material::Trim, wire(&[[0.0, 0.0], [0.5 * len, -0.01], [len, c[1]]], 0.0045));
        b.add(Material::Rarity, rod_y(c, 0.008, 0.0065, 12).difference(&rod_y(c, 0.01, 0.004, 12)));
        return;
    }
    mount(b, Material::Trim, 0.01);
    b.add(Material::Metal, bevel([0.0, len], [-0.01, -0.002], 0.008, 0.001));
    let spikes: Vec<Solid> = (0..((len / 0.02) as usize).max(2)).map(|i| Solid::cylinder(0.012, 0.0028, 0.0, 6).rotate([180.0, 0.0, 0.0]).translate([0.012 + 0.02 * i as f64, 0.0, -0.014])).collect();
    b.add(Material::Metal, Solid::union_all(&spikes));
    b.add(Material::Rarity, bevel([len - 0.01, len], [-0.012, 0.0], 0.01, 0.001));
}

/// A shield ring with a glow disk, or two prongs with glow tips.
fn halo(b: &mut Build, len: f64, first: bool) {
    mount(b, Material::Trim, 0.016);
    if first {
        let c = [len, -0.018];
        b.add(Material::Paint, bar([0.0, -0.004], c, 0.004, 10));
        b.add(Material::Metal, ring_x(len, 0.02, 0.005).translate([0.0, 0.0, c[1]]));
        b.add(Material::Glow, rod_x([len - 0.001, len + 0.001], [0.018, 0.018], 24).translate([0.0, 0.0, c[1]]));
        b.add(Material::Rarity, Solid::sphere(0.004, 12).translate([len, 0.0, c[1] + 0.022]));
        return;
    }
    for y in [-0.006, 0.006] {
        b.add(Material::Paint, bar([0.0, -0.004], [len, -0.008], 0.0025, 8).translate([0.0, y, 0.0]));
        b.add(Material::Glow, Solid::sphere(0.004, 12).translate([len, y, -0.008]));
    }
    b.add(Material::Rarity, ring_x(0.5 * len, 0.009, 0.003).translate([0.0, 0.0, -0.006]));
}
