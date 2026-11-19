//! The magazines: they go down from the mount under the receiver. Each has a collar that goes up
//! into the receiver. A larger magazine holds more and takes longer to reload.

use geometry::Solid;

use super::Build;
use crate::shapes::*;
use crate::{Class, Maker, Material, Stats};

pub(super) fn make(b: &mut Build, maker: Maker) -> Stats {
    let (ml, md, mw) = (b.s.mag, b.s.mag_depth(), (0.6 * b.s.w).max(0.022));
    b.add(Material::Trim, bevel([-md / 2.0 - 0.005, md / 2.0 + 0.005], [-0.012, 0.02], mw + 0.008, 0.002));
    let base = b.base();
    let big = b.s.big();
    let mut stats = Stats { magazine: base.magazine * (0.6 * big - 0.2), reload: base.reload * 0.15 * big, ..Stats::default() };
    if b.class == Class::Launcher {
        rockets(b, ml, md, mw);
        return stats;
    }
    match maker {
        Maker::Blockwerk => blockwerk(b, ml, md, mw),
        Maker::Vesper => vesper(b, ml, md, mw),
        Maker::Scrapjaw => {
            if scrapjaw(b, ml, md, mw) {
                stats.magazine += 0.5 * base.magazine;
            }
        }
        Maker::Halo => {
            halo(b, ml, md, mw);
            stats.reload -= 0.1 * base.reload;
        }
    }
    stats
}

/// A box that leans forward, with ribs, a window and a base plate.
fn blockwerk(b: &mut Build, ml: f64, md: f64, mw: f64) {
    let lean = 0.15 * ml;
    let outline = [[-md / 2.0, 0.015], [md / 2.0, 0.015], [md / 2.0 + lean, -ml], [-md / 2.0 + lean, -ml]];
    let slit = [[-0.003 + 0.2 * lean, -0.2 * ml], [0.003 + 0.2 * lean, -0.2 * ml], [0.003 + 0.6 * lean, -0.6 * ml], [-0.003 + 0.6 * lean, -0.6 * ml]];
    let (groove, window) = inlay(&slit, mw / 2.0, 0.0018);
    b.add(Material::Paint, plate(&outline, mw, 0.002).difference(&groove));
    b.add(Material::Glow, window);
    for i in 0..3 {
        let t = 0.7 + 0.08 * f64::from(i);
        let rib = stripe(-md / 2.0 + t * lean - 0.001, md + 0.002, [-t * ml, -t * ml + 0.004], 0.0);
        b.add(Material::Trim, slab(&rib, mw + 0.003));
    }
    b.add(Material::Trim, bevel([-md / 2.0 + lean - 0.004, md / 2.0 + lean + 0.004], [-ml - 0.006, -ml + 0.002], mw + 0.006, 0.002));
    b.add(Material::Rarity, both(&decal(&stripe(-md / 2.0 + 0.4 * lean, md * 0.5, [-0.42 * ml, -0.38 * ml], 0.0), mw / 2.0, 0.001)));
}

/// A curved magazine of six segments with dark ribs between them and a glow cap.
fn vesper(b: &mut Build, ml: f64, md: f64, mw: f64) {
    let arc = 25.0_f64.to_radians();
    let radius = (ml + 0.015) / arc;
    let at = |t: f64| {
        let a = arc * t;
        ([radius * (1.0 - a.cos()), 0.015 - radius * a.sin()], -a.to_degrees())
    };
    let slice = |t: f64, grow: f64, thick: f64| {
        let (c, turn) = at(t);
        Solid::cube(md * grow, mw * grow, thick).rotate([0.0, turn, 0.0]).translate([c[0], 0.0, c[1]])
    };
    let segments: Vec<Solid> = (0..6).map(|i| slice(f64::from(i) / 6.0, 1.0, 0.001).union(&slice(f64::from(i + 1) / 6.0, 1.0, 0.001)).hull()).collect();
    b.add(Material::Paint, Solid::union_all(&segments));
    for i in 1..6 {
        b.add(Material::Trim, slice(f64::from(i) / 6.0, 1.08, 0.003));
    }
    b.add(Material::Glow, slice(1.0, 1.06, 0.004));
    b.add(Material::Rarity, slice(1.0 - 0.006 / ml, 1.1, 0.003));
}

/// A drum with a crank, or two magazines taped together. True for the drum.
fn scrapjaw(b: &mut Build, ml: f64, md: f64, mw: f64) -> bool {
    if b.chance(0.4) {
        let r = (0.45 * ml).clamp(0.03, 0.06);
        let c = [0.2 * r, -0.012 - r];
        b.add(Material::Paint, rod_y(c, 1.6 * mw, r, 32));
        b.add(Material::Trim, rod_y(c, 1.6 * mw + 0.004, 0.55 * r, 24));
        b.add(Material::Metal, both(&ring_y(c, 0.95 * r, 0.004).translate([0.0, 0.8 * mw, 0.0])));
        b.add(Material::Metal, bevel([-md / 2.0, md / 2.0], [c[1] + 0.6 * r, 0.015], mw, 0.002));
        let crank = rod_y(c, 1.6 * mw + 0.016, 0.003, 8).union(&bar(c, [c[0] + 0.6 * r, c[1] - 0.3 * r], 0.0025, 6).translate([0.0, -0.8 * mw - 0.008, 0.0]));
        b.add(Material::Metal, crank);
        b.add(Material::Rarity, rod_y([c[0] + 0.6 * r, c[1] - 0.3 * r], 1.6 * mw + 0.024, 0.004, 10));
        return true;
    }
    let lean = b.range(-0.06, 0.1) * ml;
    let outline = [[-md / 2.0, 0.015], [md / 2.0, 0.015], [md / 2.0 + lean, -ml], [-md / 2.0 + lean, -ml]];
    b.add(Material::Paint, plate(&outline, mw, 0.0015));
    let spare = plate(&turn(&outline, 180.0), mw, 0.0015).translate([0.0, mw + 0.002, -0.35 * ml]);
    b.add(Material::Trim, spare);
    let z = -0.55 * ml;
    let tape = bevel([-md / 2.0 + 0.5 * lean - 0.006, md / 2.0 + 0.5 * lean + 0.006], [z, z + 0.016], 2.0 * mw + 0.006, 0.001);
    b.add(Material::Grip, tape.translate([0.0, mw / 2.0 + 0.001, 0.0]));
    false
}

/// A battery: a cylinder with glow bands, metal caps and a rarity ring.
fn halo(b: &mut Build, ml: f64, md: f64, mw: f64) {
    let r = 0.55 * md.min(mw);
    b.add(Material::Paint, rod_z(0.0, [-ml + 0.008, 0.015], [r, r], 28));
    b.add(Material::Metal, rod_z(0.0, [-ml - 0.002, -ml + 0.012], [0.85 * r, r + 0.002], 28));
    let count = ((ml / 0.03) as usize).clamp(2, 5);
    for i in 0..count {
        let z = -0.1 * ml - (0.75 * ml) * i as f64 / count as f64;
        b.add(Material::Glow, rod_z(0.0, [z - 0.004, z], [r + 0.0015, r + 0.0015], 28));
    }
    b.add(Material::Rarity, rod_z(0.0, [-ml + 0.012, -ml + 0.016], [r + 0.002, r + 0.002], 28));
}

/// A box of rockets for a launcher, with a rocket nose in a window at the front.
fn rockets(b: &mut Build, ml: f64, md: f64, mw: f64) {
    let window = block([0.0, md], [-0.5 * ml, -0.15 * ml], 2.0 * mw).translate([md / 2.0 - 0.012, 0.0, 0.0]);
    b.add(Material::Paint, bevel([-md / 2.0, md / 2.0], [-ml, 0.015], mw, 0.004).difference(&window));
    let nose = turned(&[[-md / 2.0 + 0.004, 0.32 * md], [md / 2.0 - 0.02, 0.32 * md], [md / 2.0 - 0.004, 0.05 * md]], 24);
    b.add(Material::Metal, nose.translate([0.0, 0.0, -0.32 * ml]));
    b.add(Material::Rarity, both(&decal(&stripe(-md / 2.0 + 0.006, 0.4 * md, [-0.85 * ml, -0.78 * ml], 0.008), mw / 2.0, 0.0012)));
    b.add(Material::Trim, bevel([-md / 2.0 - 0.003, md / 2.0 + 0.003], [-ml - 0.006, -ml + 0.004], mw + 0.006, 0.002));
}
