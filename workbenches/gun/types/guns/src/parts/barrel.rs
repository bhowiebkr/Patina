//! The barrels: they go along +X from the mount, with the bore on the X axis. The muzzle socket
//! is at the end. A longer barrel gives more accuracy and damage.

use geometry::Solid;

use super::Build;
use crate::shapes::*;
use crate::{Class, Maker, Material, Stats};

pub(super) fn make(b: &mut Build, maker: Maker) -> Stats {
    let len = b.s.barrel;
    let end = match maker {
        Maker::Blockwerk => blockwerk(b, len),
        Maker::Vesper => vesper(b, len),
        Maker::Scrapjaw => scrapjaw(b, len),
        Maker::Halo => halo(b, len),
    };
    b.socket("muzzle", [end, 0.0, 0.0]);
    let base = b.base();
    let k = len / (len + b.s.l);
    let mut stats = Stats { accuracy: (100.0 - base.accuracy) * k * 0.5, damage: base.damage * 0.3 * k, ..Stats::default() };
    match maker {
        Maker::Blockwerk => stats.damage *= 1.3,
        Maker::Vesper => stats.accuracy *= 1.3,
        Maker::Scrapjaw => stats.rate += 0.1 * base.rate,
        Maker::Halo => stats.reload -= 0.05 * base.reload,
    }
    stats
}

/// The hole of the bore, from `x0` to past the muzzle.
fn bore(b: &Build, x0: f64, end: f64) -> Solid {
    rod_x([x0, end + 0.01], [b.s.bore, b.s.bore], 20)
}

/// A band in the rarity color around the barrel at `x`.
fn band(b: &mut Build, x: f64, r: f64) {
    b.add(Material::Rarity, rod_x([x, x + 0.006], [r, r], 28));
}

/// A box shroud with vents, or a stepped barrel under a heat guard. A shotgun has a pump, a
/// sniper rifle a large brake and a launcher a square tube.
fn blockwerk(b: &mut Build, len: f64) -> f64 {
    let sr = b.s.shroud();
    if b.class == Class::Launcher {
        let hole = rod_x([-0.01, len + 0.01], [sr - 0.005, sr - 0.005], 28);
        b.add(Material::Paint, bevel([0.0, len], [-1.15 * sr, 1.15 * sr], 2.3 * sr, 0.006).difference(&hole));
        for x in [0.0, len - 0.03] {
            b.add(Material::Metal, bevel([x, x + 0.03], [-1.25 * sr, 1.25 * sr], 2.5 * sr, 0.004).difference(&hole));
        }
        let mark = stripe(0.35 * len, 0.03, [-0.6 * sr, 0.6 * sr], 0.02);
        b.add(Material::Rarity, both(&decal(&mark, 1.15 * sr, 0.0012)));
        b.add(Material::Trim, bevel([0.5 * len, 0.62 * len], [1.1 * sr, 1.4 * sr], 0.012, 0.002));
        return len;
    }
    let inner = 1.4 * b.s.bore;
    b.add(Material::Metal, rod_x([0.0, len], [inner, inner], 20));
    let brake_len = if b.class == Class::Sniper { 0.07 } else { 0.035 };
    let body = len - brake_len;
    if b.variant(2) == 0 || b.class == Class::Shotgun {
        let vents = slots([0.12 * body, 0.85 * body], [-0.45 * sr, 0.45 * sr], ((body / 0.03) as usize).clamp(2, 9), 3.0 * sr);
        b.add(Material::Paint, bevel([0.0, body], [-1.05 * sr, 1.05 * sr], 1.8 * sr, 0.003).difference(&vents));
        b.add(Material::Trim, rail([0.05 * body, 0.9 * body], 0.5 * sr, 0.005).translate([0.0, 0.0, 1.05 * sr - 0.001]));
    } else {
        let step = turned(&[[0.0, 1.15 * sr], [0.12 * body, 1.15 * sr], [0.15 * body, 0.8 * sr], [0.95 * body, 0.75 * sr], [body, 0.65 * sr]], 24);
        b.add(Material::Metal, step);
        let guard = bevel([0.2 * body, 0.75 * body], [0.3 * sr, 1.15 * sr], 1.7 * sr, 0.002);
        let holes = slots([0.24 * body, 0.72 * body], [0.6 * sr, 2.0 * sr], 5, 0.6 * sr);
        b.add(Material::Paint, guard.difference(&holes));
    }
    band(b, 0.02, 0.95 * sr);
    // The ported brake at the muzzle.
    let r = if b.class == Class::Sniper { 1.2 * sr } else { 0.95 * sr };
    let ports = slots([body + 0.15 * brake_len, len - 0.1 * brake_len], [-0.5 * r, 0.5 * r], 3, 3.0 * r);
    let brake = bevel([body, len], [-r, r], 1.7 * r, 0.002).difference(&ports).difference(&bore(b, body - 0.01, len));
    b.add(Material::Metal, brake);
    if b.class == Class::Shotgun {
        let z = -1.95 * sr;
        b.add(Material::Metal, rod_x([0.0, 0.9 * len], [0.75 * sr, 0.75 * sr], 20).translate([0.0, 0.0, z]));
        b.add(Material::Trim, bevel([0.85 * len, 0.92 * len], [z - 0.9 * sr, 0.0], 1.6 * sr, 0.002));
        let (p0, p1) = (0.15 * len, 0.55 * len);
        b.add(Material::Grip, bevel([p0, p1], [z - 1.1 * sr, z + 1.0 * sr], 2.0 * sr, 0.004));
        for i in 0..5 {
            let x = p0 + 0.01 + (p1 - p0 - 0.02) * f64::from(i) / 4.0;
            b.add(Material::Grip, bevel([x - 0.003, x + 0.003], [z - 1.2 * sr, z + 1.1 * sr], 2.2 * sr, 0.0015));
        }
    }
    len
}

/// A tapered shroud with a glow ring and a fin, or a split rail around a glow core. A sniper rifle
/// has a long suppressor and a launcher a flared bell.
fn vesper(b: &mut Build, len: f64) -> f64 {
    let sr = b.s.shroud();
    if b.class == Class::Launcher {
        let bell = lathe(&[[0.0, sr - 0.005], [0.7 * len, sr - 0.005], [len, 1.45 * sr - 0.006], [len, 1.45 * sr], [0.7 * len, sr], [0.0, sr]], 36);
        b.add(Material::Paint, bell);
        for x in [0.08 * len, 0.62 * len] {
            b.add(Material::Trim, tube([x, x + 0.025], sr + 0.004, sr - 0.002, 36));
        }
        b.add(Material::Glow, tube([0.66 * len, 0.68 * len], sr + 0.006, sr - 0.002, 36));
        b.add(Material::Rarity, tube([0.12 * len, 0.13 * len], sr + 0.006, sr - 0.002, 36));
        return len;
    }
    if b.class == Class::Sniper || b.variant(2) == 0 {
        let taper = if b.class == Class::Sniper { 0.55 } else { 0.86 };
        let shroud = turned(&[[0.0, 1.25 * sr], [0.08 * len, 1.3 * sr], [taper * len, 0.95 * sr], [(taper + 0.03) * len, 0.9 * sr]], 32);
        b.add(Material::Paint, shroud);
        b.add(Material::Glow, rod_x([(taper + 0.03) * len, (taper + 0.05) * len], [0.95 * sr, 0.95 * sr], 32));
        let fin = [[0.12 * len, 1.1 * sr], [0.7 * taper * len, 0.9 * sr], [0.55 * taper * len, 1.75 * sr], [0.2 * len, 1.8 * sr]];
        b.add(Material::Trim, slab(&fin, 0.004));
        band(b, 0.05 * len, 1.3 * sr);
        if b.class == Class::Sniper {
            let can = turned(&[[0.55 * len, 1.1 * sr], [0.6 * len, 1.4 * sr], [0.97 * len, 1.4 * sr], [len, 1.2 * sr]], 32);
            b.add(Material::Trim, can.difference(&bore(b, 0.5 * len, len)));
            for x in [0.68 * len, 0.86 * len] {
                b.add(Material::Glow, rod_x([x, x + 0.008], [1.45 * sr, 1.45 * sr], 32));
            }
        } else {
            let tip = turned(&[[(taper + 0.05) * len, 0.8 * sr], [len, 0.62 * sr]], 28);
            b.add(Material::Metal, tip.difference(&bore(b, 0.5 * len, len)));
        }
    } else {
        for side in [-1.0, 1.0] {
            b.add(Material::Paint, bevel([0.0, 0.92 * len], [0.35 * sr, 1.0 * sr], 1.3 * sr, 0.003).translate([0.0, 0.0, if side < 0.0 { -1.35 * sr } else { 0.0 }]));
        }
        b.add(Material::Glow, rod_x([0.0, 0.92 * len], [0.3 * sr, 0.3 * sr], 20));
        for x in [0.3 * len, 0.62 * len] {
            b.add(Material::Trim, bevel([x, x + 0.012], [-1.1 * sr, 1.1 * sr], 1.45 * sr, 0.002));
        }
        let tip = turned(&[[0.9 * len, 0.9 * sr], [len, 0.6 * sr]], 28);
        b.add(Material::Metal, tip.difference(&bore(b, 0.8 * len, len)));
        band(b, 0.02 * len, 1.05 * sr);
    }
    len
}

/// A plain pipe with tape, clamps, a heat shield and spikes. A shotgun has two sawed-off pipes and
/// a launcher a wide pipe.
fn scrapjaw(b: &mut Build, len: f64) -> f64 {
    let sr = b.s.shroud();
    let pipes: Vec<f64> = if b.class == Class::Shotgun { vec![-0.95 * sr, 0.95 * sr] } else { vec![0.0] };
    let r = if b.class == Class::Launcher { sr } else { 0.85 * sr };
    let hole = if b.class == Class::Launcher { sr - 0.004 } else { b.s.bore };
    for &y in &pipes {
        b.add(Material::Metal, tube([0.0, len], r, hole, 24).translate([0.0, y, 0.0]));
        b.add(Material::Trim, tube([len - 0.012, len], r + 0.003, hole, 24).translate([0.0, y, 0.0]));
    }
    let spread = if pipes.len() > 1 { 1.9 * sr } else { 0.0 };
    // Tape bands at random places and two clamps around all pipes.
    for _ in 0..2 {
        let x = b.range(0.1, 0.7) * len;
        let tape = bevel([x, x + 0.02], [-r - 0.002, r + 0.002], 2.0 * r + spread + 0.004, 0.004);
        b.add(Material::Grip, tape);
    }
    for x in [0.04 * len, 0.8 * len] {
        let clamp = bevel([x, x + 0.01], [-r - 0.004, r + 0.004], 2.0 * r + spread + 0.008, 0.002);
        b.add(Material::Trim, clamp);
        b.add(Material::Metal, rod_y([x + 0.005, r + 0.006], 2.0 * r + spread + 0.014, 0.0025, 6));
    }
    // A heat shield with holes on top of the pipes.
    let (h0, h1) = (0.15 * len, 0.6 * len);
    let shield = slab(&[[h0, r + 0.001], [h1, r + 0.001], [h1 - 0.01, r + 0.006], [h0 + 0.01, r + 0.006]], 1.4 * r + spread);
    let holes: Vec<Solid> = (0..((h1 - h0) / 0.018) as usize).map(|i| rod_z(h0 + 0.014 + 0.018 * i as f64, [r - 0.01, r + 0.02], [0.003, 0.003], 8)).collect();
    b.add(Material::Paint, shield.difference(&Solid::union_all(&holes)));
    band(b, 0.02, r + 0.0015);
    if b.class != Class::Launcher {
        for angle in [0.0, 120.0, 240.0] {
            let spike = rod_x([len, len + 0.022], [0.0035, 0.0], 8).translate([0.0, 0.0, r + 0.002]).rotate([angle, 0.0, 0.0]);
            b.add(Material::Metal, spike);
        }
        return len + 0.022;
    }
    len
}

/// A core rod in coils of metal and glow with prongs at the muzzle, or a capsule with glow bands.
fn halo(b: &mut Build, len: f64) -> f64 {
    let sr = b.s.shroud();
    if b.class == Class::Launcher || b.variant(2) == 0 {
        let root = turned(&[[0.0, 1.35 * sr], [0.16 * len, 1.35 * sr], [0.22 * len, 0.8 * sr]], 32);
        let hole = if b.class == Class::Launcher { sr - 0.004 } else { b.s.bore };
        b.add(Material::Paint, root.difference(&rod_x([-0.01, len], [hole, hole], 24)));
        b.add(Material::Metal, tube([0.0, 0.9 * len], 0.6 * sr.max(hole + 0.004), hole, 24));
        let count = (((0.84 - 0.24) * len / 0.02) as usize).clamp(3, 14);
        let step = 0.6 * len / count as f64;
        for i in 0..count {
            let x = 0.24 * len + step * i as f64;
            let material = if i % 2 == 0 { Material::Metal } else { Material::Glow };
            b.add(material, tube([x, x + 0.45 * step], 1.05 * sr, hole, 28));
        }
        for angle in [0.0, 120.0, 240.0] {
            b.add(Material::Trim, bar([0.84 * len, 0.9 * sr], [len + 0.015, 0.55 * sr], 0.003, 8).rotate([angle, 0.0, 0.0]));
        }
        band(b, 0.18 * len, 1.36 * sr);
        return len + 0.015;
    }
    let capsule = turned(&[[0.0, 1.2 * sr], [0.04 * len, 1.3 * sr], [0.8 * len, 1.2 * sr], [0.88 * len, 0.9 * sr]], 32);
    b.add(Material::Paint, capsule);
    for x in [0.2 * len, 0.42 * len, 0.64 * len] {
        b.add(Material::Glow, rod_x([x, x + 0.007], [1.32 * sr, 1.32 * sr], 32));
    }
    let tip = turned(&[[0.88 * len, 0.75 * sr], [len, 0.6 * sr]], 28);
    b.add(Material::Metal, tip.difference(&bore(b, 0.8 * len, len)));
    band(b, 0.08 * len, 1.32 * sr);
    len
}
