//! The sights: they sit on top of the receiver, centered on the mount. A sniper rifle has a long
//! scope. A longer sight gives more accuracy.

use geometry::Solid;

use super::Build;
use crate::shapes::*;
use crate::{Class, Maker, Material, Stats};

pub(super) fn make(b: &mut Build, maker: Maker) -> Stats {
    let sniper = b.class == Class::Sniper;
    let sl = if sniper { b.range(0.22, 0.3) } else { b.range(0.06, 0.12) };
    let x = [-sl / 2.0, sl / 2.0];
    match maker {
        Maker::Blockwerk => blockwerk(b, x, sniper),
        Maker::Vesper => vesper(b, x, sniper),
        Maker::Scrapjaw => scrapjaw(b, x, sniper),
        Maker::Halo => halo(b, x, sniper),
    }
    let gap = 100.0 - b.base().accuracy;
    Stats { accuracy: gap * 0.2 * sl / (sl + 0.1), ..Stats::default() }
}

/// Two clamps on the rail, with bolts.
fn clamps(b: &mut Build, xs: [f64; 2], top: f64, w: f64, material: Material) {
    for x in xs {
        b.add(material, bevel([x - 0.006, x + 0.006], [-0.004, top], w, 0.0015));
        b.add(Material::Metal, bolts(&[[x, 0.4 * top]], w + 0.002, 0.002));
    }
}

/// A box with a glow lens at each end, a hood over the front lens and clamps.
fn blockwerk(b: &mut Build, [x0, x1]: [f64; 2], sniper: bool) {
    let (bottom, top, w) = if sniper { (0.014, 0.05, 0.032) } else { (0.012, 0.038, 0.026) };
    b.add(Material::Paint, bevel([x0, x1], [bottom, top], w, 0.003));
    for x in [x0 - 0.0015, x1 - 0.0005] {
        b.add(Material::Glow, block([x, x + 0.002], [bottom + 0.004, top - 0.004], w - 0.006));
    }
    b.add(Material::Trim, bevel([x1 - 0.012, x1 + 0.012], [top - 0.004, top + 0.005], w + 0.004, 0.002));
    b.add(Material::Trim, bevel([x0 + 0.1 * (x1 - x0), x0 + 0.3 * (x1 - x0)], [top - 0.001, top + 0.004], 0.6 * w, 0.0015));
    clamps(b, [x0 + 0.25 * (x1 - x0), x1 - 0.25 * (x1 - x0)], bottom + 0.002, 0.7 * w, Material::Trim);
    b.add(Material::Rarity, both(&decal(&chamfer([x0 + 0.04 * (x1 - x0), bottom + 0.004], [x0 + 0.12 * (x1 - x0), top - 0.004], 0.001), w / 2.0, 0.001)));
}

/// An oval tube on two pylons, with a turret and glow lenses.
fn vesper(b: &mut Build, [x0, x1]: [f64; 2], sniper: bool) {
    let r = if sniper { 0.014 } else { 0.01 };
    let z = 0.014 + r;
    let tube = turned(&[[x0, 1.25 * r], [x0 + 0.012, 1.25 * r], [x0 + 0.03, r], [x1 - 0.025, r], [x1, 1.35 * r]], 28);
    let hollow = rod_x([x0 - 0.001, x0 + 0.004], [r, r], 24).union(&rod_x([x1 - 0.004, x1 + 0.001], [1.15 * r, 1.15 * r], 24));
    b.add(Material::Paint, tube.difference(&hollow).scale([1.0, 0.85, 1.0]).translate([0.0, 0.0, z]));
    for (x, rr) in [(x0 + 0.003, r), (x1 - 0.003, 1.15 * r)] {
        b.add(Material::Glow, rod_x([x - 0.001, x], [rr, rr], 24).scale([1.0, 0.85, 1.0]).translate([0.0, 0.0, z]));
    }
    let span = x1 - x0;
    for x in [x0 + 0.3 * span, x1 - 0.3 * span] {
        b.add(Material::Trim, slab(&[[x - 0.008, -0.004], [x + 0.008, -0.004], [x + 0.005, z], [x - 0.005, z]], 0.008));
    }
    let mid = (x0 + x1) / 2.0;
    b.add(Material::Metal, rod_z(mid, [z, z + 1.35 * r], [0.35 * r, 0.3 * r], 14));
    b.add(Material::Metal, rod_y([mid, z], 2.2 * r, 0.3 * r, 14));
    b.add(Material::Rarity, rod_x([mid + 0.15 * span, mid + 0.15 * span + 0.004], [1.03 * r, 1.03 * r], 24).scale([1.0, 0.85, 1.0]).translate([0.0, 0.0, z]));
}

/// A tin can scope held by wire, or a bent ring and a post.
fn scrapjaw(b: &mut Build, [x0, x1]: [f64; 2], sniper: bool) {
    if sniper || b.chance(0.5) {
        let r = if sniper { 0.015 } else { 0.011 };
        let z = 0.012 + r;
        let can = tube([x0, x1], r, r - 0.002, 20).union(&tube([x1 - 0.03, x1], 1.25 * r, r - 0.002, 20));
        b.add(Material::Paint, can.translate([0.0, 0.0, z]));
        b.add(Material::Glow, rod_x([x1 - 0.006, x1 - 0.004], [r - 0.002, r - 0.002], 20).translate([0.0, 0.0, z]));
        for x in [x0 + 0.2 * (x1 - x0), x1 - 0.45 * (x1 - x0)] {
            b.add(Material::Metal, ring_x(x, r + 0.001, 0.0025).translate([0.0, 0.0, z]));
            b.add(Material::Trim, bevel([x - 0.004, x + 0.004], [-0.004, z - r + 0.002], 0.012, 0.001));
        }
        b.add(Material::Rarity, ring_x(x0 + 0.005, r + 0.001, 0.003).translate([0.0, 0.0, z]));
        return;
    }
    let ring = ring_x(x0 + 0.01, 0.009, 0.003).translate([0.0, 0.0, 0.022]);
    b.add(Material::Metal, ring.union(&wire(&[[x0 + 0.006, -0.004], [x0 + 0.01, 0.013]], 0.002)));
    b.add(Material::Trim, wire(&[[x1 - 0.006, -0.004], [x1 - 0.008, 0.014], [x1 - 0.006, 0.026]], 0.0022));
    b.add(Material::Rarity, Solid::sphere(0.003, 10).translate([x1 - 0.006, 0.0, 0.027]));
}

/// A post with a ring and a glow disk, or a capsule scope for a sniper rifle.
fn halo(b: &mut Build, [x0, x1]: [f64; 2], sniper: bool) {
    if sniper {
        let r = 0.014;
        let z = 0.016 + r;
        let capsule = turned(&[[x0, 0.6 * r], [x0 + 0.015, 1.1 * r], [x1 - 0.02, 1.1 * r], [x1, 0.7 * r]], 28);
        b.add(Material::Paint, capsule.translate([0.0, 0.0, z]));
        for x in [x0 + 0.3 * (x1 - x0), x1 - 0.3 * (x1 - x0)] {
            b.add(Material::Glow, rod_x([x, x + 0.005], [1.18 * r, 1.18 * r], 28).translate([0.0, 0.0, z]));
            b.add(Material::Trim, bevel([x - 0.006, x + 0.01], [-0.004, z - r], 0.012, 0.002));
        }
        b.add(Material::Rarity, rod_x([x1 - 0.012, x1 - 0.008], [1.12 * r, 1.12 * r], 28).translate([0.0, 0.0, z]));
        return;
    }
    let (xp, z) = (x1 - 0.015, 0.03);
    b.add(Material::Paint, slab(&[[xp - 0.012, -0.004], [xp + 0.008, -0.004], [xp + 0.003, z - 0.012], [xp - 0.004, z - 0.012]], 0.01));
    b.add(Material::Metal, ring_x(xp, 0.013, 0.004).translate([0.0, 0.0, z]));
    b.add(Material::Glow, rod_x([xp - 0.0008, xp + 0.0008], [0.011, 0.011], 24).translate([0.0, 0.0, z]));
    b.add(Material::Trim, bevel([x0, xp - 0.006], [-0.004, 0.006], 0.012, 0.002));
    b.add(Material::Rarity, rod_y([x0 + 0.008, 0.004], 0.016, 0.003, 10));
}
