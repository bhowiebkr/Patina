//! The stocks: they go back along -X from the mount at the rear of the receiver. The butt is at
//! `x = -sl`. A stock gives accuracy and makes the reload a little slower.

use geometry::Solid;

use super::Build;
use crate::shapes::*;
use crate::{Maker, Material, Stats};

pub(super) fn make(b: &mut Build, maker: Maker) -> Stats {
    let (sl, h, sw) = (b.s.stock, b.s.h, (0.6 * b.s.w).max(0.022));
    match maker {
        Maker::Blockwerk => blockwerk(b, sl, h, sw),
        Maker::Vesper => vesper(b, sl, h, sw),
        Maker::Scrapjaw => scrapjaw(b, sl, h, sw),
        Maker::Halo => halo(b, sl, h, sw),
    }
    let base = b.base();
    let accuracy = (100.0 - base.accuracy) * 0.25 * b.range(0.8, 1.2);
    Stats { accuracy, reload: 0.05 * base.reload, ..Stats::default() }
}

/// A frame with a hole, a rubber butt pad, a cheek rest and bolts.
fn blockwerk(b: &mut Build, sl: f64, h: f64, sw: f64) {
    let outline = [[0.01, 0.3 * h], [-sl, 0.32 * h], [-sl, -0.6 * h], [-0.04 * sl, -0.3 * h], [0.01, -0.3 * h]];
    let hole = inset(&[[-0.12 * sl, 0.3 * h], [-sl, 0.32 * h], [-sl, -0.6 * h], [-0.12 * sl, -0.3 * h]], 0.012);
    b.add(Material::Paint, plate(&outline, sw, 0.003).difference(&slab(&hole, 2.0 * sw)));
    b.add(Material::Grip, bevel([-sl - 0.014, -sl + 0.002], [-0.63 * h, 0.35 * h], sw + 0.004, 0.003));
    let cheek = [[-0.3 * sl, 0.28 * h], [-0.85 * sl, 0.29 * h], [-0.85 * sl, 0.44 * h], [-0.4 * sl, 0.42 * h]];
    b.add(Material::Trim, plate(&cheek, 0.9 * sw, 0.002));
    b.add(Material::Metal, bolts(&[[-0.004, 0.18 * h], [-0.004, -0.18 * h], [-0.5 * sl, 0.36 * h], [-0.75 * sl, 0.36 * h]], sw + 0.002, 0.0026));
    b.add(Material::Rarity, both(&decal(&stripe(-0.92 * sl, 0.035, [-0.5 * h, 0.2 * h], -0.01), sw / 2.0, 0.0012)));
}

/// A sculpted body with a narrow wrist, a cheek swell, a thumbhole, a soft pad and a glow line.
fn vesper(b: &mut Build, sl: f64, h: f64, sw: f64) {
    // The outline at `x`, `wide` x sw wide and smaller by `inset` on each side and by `round` more
    // at the top and the bottom. From x0 to x1, the top goes from 0.28 h to 0.3 h and the bottom
    // from -0.28 h to -0.6 h.
    let (x0, x1) = (-0.1 * h, -sl + 0.45 * h);
    let section = |x: f64, wide: f64, inset: f64, round: f64| {
        let t = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        let (top, bottom) = ((0.28 + 0.02 * t) * h - inset - round, -(0.28 + 0.32 * t) * h + inset + round);
        (-x, squircle((top + bottom) / 2.0, top - bottom, wide * sw - 2.0 * inset - round, -0.12, 3.5))
    };
    // The top and the bottom edges of the butt are round, with the radius r.
    let r = 0.12 * h;
    let body = |inset: f64| {
        let mut sections: Vec<_> = [(0.1 * h, 0.9), (-0.12 * sl, 0.82), (-0.35 * sl, 0.85), (-0.65 * sl, 1.0)].map(|(x, wide)| section(x, wide, inset, 0.0)).into();
        sections.extend([0f64, 45.0, 75.0, 90.0].map(|a| section(-sl + r * (1.0 - a.to_radians().sin()), 1.0, inset, r * (1.0 - a.to_radians().cos()))));
        along_x(&sections)
    };
    let hole = pill(&[([-0.25 * sl, -0.12 * h], 0.1 * h), ([-sl + 0.5 * h, -0.2 * h], 0.12 * h)], 2.0 * sw, 0.0);
    // The glow line fills a groove in the round sides, a little under the surface.
    let band = slab(&strip([-0.1 * sl, 0.16 * h], [-0.75 * sl, 0.2 * h], 0.03 * h), 2.0 * sw);
    b.add(Material::Paint, body(0.0).difference(&band.difference(&body(0.0018))).difference(&hole));
    b.add(Material::Glow, band.intersection(&body(0.0006)));
    b.add(Material::Grip, along_x(&[section(-sl, 1.0, -0.001, r), section(-sl - 0.004, 1.0, 0.002, r)]));
    b.add(Material::Trim, pill(&[([-0.02 * sl, -0.22 * h], 0.012), ([-0.18 * sl, -0.25 * h], 0.01)], 0.8 * sw, 0.002));
}

/// A sculpted solid along -X: each item is the distance back from x = 0 and an outline on the
/// Z-Y plane, as `[z, y]` points. The distances go up.
fn along_x(sections: &[(f64, Vec<[f64; 2]>)]) -> Solid {
    taper(sections).rotate([0.0, -90.0, 0.0])
}

/// A wood plank with a strap, or a bent pipe with a pad.
fn scrapjaw(b: &mut Build, sl: f64, h: f64, sw: f64) {
    if b.variant(2) == 0 {
        let plank = turn(&[[0.01, 0.25 * h], [-sl, 0.3 * h], [-sl, -0.6 * h], [0.01, -0.22 * h]], b.range(-4.0, 4.0));
        b.add(Material::Grip, plate(&plank, sw, 0.002));
        let x = -b.range(0.35, 0.6) * sl;
        b.add(Material::Trim, wrap(&plank, sw, &block([x, x + 0.018], [-1.0, 1.0], 1.0), 0.0015));
        b.add(Material::Metal, bevel([-0.02, 0.012], [-0.24 * h, 0.26 * h], sw + 0.006, 0.002));
        b.add(Material::Metal, bolts(&[[-0.008, 0.15 * h], [-0.008, -0.12 * h], [-0.85 * sl, 0.1 * h], [-0.85 * sl, -0.4 * h]], sw + 0.006, 0.002));
        b.add(Material::Rarity, both(&decal(&[[-0.2 * sl, -0.1 * h], [-0.3 * sl, -0.05 * h], [-0.28 * sl, 0.1 * h]], sw / 2.0, 0.001)));
        return;
    }
    let path = [[0.01, 0.12 * h], [-0.85 * sl, 0.12 * h], [-sl + 0.008, -0.1 * h], [-sl + 0.008, -0.5 * h], [-0.5 * sl, -0.3 * h], [0.01, -0.2 * h]];
    b.add(Material::Metal, wire(&path, 0.0065));
    b.add(Material::Grip, bevel([-sl - 0.008, -sl + 0.006], [-0.62 * h, 0.2 * h], sw, 0.003));
    let tape = rod_x([-0.6 * sl, -0.45 * sl], [0.0085, 0.0085], 12).translate([0.0, 0.0, 0.12 * h]);
    b.add(Material::Trim, tape);
    b.add(Material::Rarity, rod_x([-0.3 * sl, -0.27 * sl], [0.0085, 0.0085], 12).translate([0.0, 0.0, 0.12 * h]));
}

/// A round spine with a waist and fins under it, a ring, a glow bar and a pad.
fn halo(b: &mut Build, sl: f64, h: f64, sw: f64) {
    // Each section is `x`, the center height, the height and the width, in units of h and sw.
    let spine = |grow: f64| {
        let sections = [(0.1 * h, 0.08, 0.28, 0.7), (-0.35 * sl, 0.085, 0.22, 0.6), (-0.75 * sl, 0.095, 0.24, 0.7), (-sl + 0.004, 0.1, 0.25, 0.72)];
        along_x(&sections.map(|(x, z, depth, wide)| (-x, egg(z * h, depth * h + 2.0 * grow, wide * sw + 2.0 * grow, 0.0))))
    };
    b.add(Material::Paint, spine(0.0));
    let fins: Vec<Solid> = (0..3)
        .map(|i| {
            let x = -0.25 * sl - 0.22 * sl * f64::from(i);
            slab(&[[x + 0.02, 0.0], [x - 0.02, 0.0], [x - 0.035, -0.45 * h], [x - 0.01, -0.45 * h]], 0.006)
        })
        .collect();
    b.add(Material::Trim, Solid::union_all(&fins));
    b.add(Material::Metal, ring_y([-0.12 * sl, -0.12 * h], 0.11 * h, 0.006));
    // The glow bar stands 1 mm out of the round sides.
    b.add(Material::Glow, pill(&[([-0.08 * sl, 0.08 * h], 0.03 * h), ([-0.85 * sl, 0.1 * h], 0.03 * h)], 2.0 * sw, 0.0).intersection(&spine(0.001)));
    b.add(Material::Grip, bevel([-sl - 0.012, -sl + 0.006], [-0.6 * h, 0.28 * h], sw, 0.004));
    b.add(Material::Rarity, rod_y([-sl - 0.003, -0.15 * h], sw + 0.003, 0.05 * h, 16));
}
