//! The grips: the body goes down from the mount and leans back. The trigger guard and the trigger
//! are in front of the grip, under the receiver.

use geometry::Solid;

use super::Build;
use crate::shapes::*;
use crate::{Maker, Material, Stats};

pub(super) fn make(b: &mut Build, maker: Maker) -> Stats {
    let gl = b.s.grip;
    let gw = (0.75 * b.s.w).clamp(0.024, 0.04);
    let rake = b.range(12.0, 18.0);
    let front = b.range(0.055, 0.068);
    let (body, guard) = match maker {
        Maker::Blockwerk => blockwerk(b, gl, gw, front),
        Maker::Vesper => vesper(b, gl, gw, front),
        Maker::Scrapjaw => scrapjaw(b, gl, gw, front),
        Maker::Halo => halo(b, gl, gw, front),
    };
    for (material, solid) in body {
        b.add(material, solid.rotate([0.0, rake, 0.0]));
    }
    for (material, solid) in guard {
        b.add(material, solid);
    }
    let trigger = wire(&[[0.03, 0.002], [0.035, -0.011], [0.031, -0.022]], 0.0024);
    b.add(Material::Metal, trigger);
    let (sin, cos) = rake.to_radians().sin_cos();
    b.socket("hand", [-gl / 2.0 * sin, 0.0, -gl / 2.0 * cos]);
    let accuracy = b.base().accuracy * 0.05 * b.range(0.8, 1.2);
    Stats { accuracy, ..Stats::default() }
}

type Parts = Vec<(Material, Solid)>;

/// A faceted body with a palm swell, ribbed rubber bands, a cap and an angular guard.
fn blockwerk(b: &mut Build, gl: f64, gw: f64, front: f64) -> (Parts, Parts) {
    // Each section is a height, the back and the front on X, and the width as a part of `gw`.
    let sections = [(-gl, -0.021, 0.012, 0.86), (-0.85 * gl, -0.022, 0.017, 0.98), (-0.35 * gl, -0.023, 0.017, 1.08), (0.025, -0.019, 0.016, 0.92)];
    let core = |grow: f64| taper(&sections.map(|(z, x0, x1, w)| (z, inset(&chamfer([x0, -w * gw / 2.0], [x1, w * gw / 2.0], 0.22 * w * gw), -grow))));
    let mut body = vec![(Material::Paint, core(0.0))];
    let ribs = ((0.7 * gl / 0.009) as usize).max(3);
    let bands: Vec<Solid> = (0..ribs).map(|i| -0.12 * gl - 0.009 * i as f64).map(|z| block([-0.017, 0.01], [z - 0.005, z], 0.1)).collect();
    body.push((Material::Grip, core(0.0015).intersection(&Solid::union_all(&bands))));
    body.push((Material::Trim, bevel([-0.024, 0.016], [-gl - 0.007, -gl + 0.003], gw + 0.003, 0.002)));
    let pin = b.range(0.25, 0.4) * gl;
    body.push((Material::Metal, bolts(&[[-0.002, -pin]], 1.1 * gw + 0.003, 0.0026)));
    let outer = [[0.005, 0.004], [front, 0.004], [front, -0.018], [front - 0.012, -0.03], [0.005, -0.03]];
    let ring = slab(&outer, 0.008).difference(&slab(&inset(&outer, 0.005), 0.02));
    (body, vec![(Material::Trim, ring)])
}

/// A sculpted body with a palm swell, finger grooves, a backstrap and a round loop guard.
fn vesper(_: &mut Build, gl: f64, gw: f64, front: f64) -> (Parts, Parts) {
    let core = taper(&[
        (-gl, egg(-0.004, 0.024, 0.7 * gw, 0.15)),
        (-gl + 0.005, egg(-0.004, 0.03, 0.86 * gw, 0.15)),
        (-0.45 * gl, egg(-0.003, 0.037, 1.04 * gw, 0.2)),
        (0.03, egg(0.0, 0.036, 0.92 * gw, 0.1)),
    ]);
    let grooves: Vec<Solid> = (0..3).map(|i| rod_y([0.0195, -0.02 - 0.02 * f64::from(i)], gw + 0.01, 0.0055, 16)).collect();
    let mut body = vec![(Material::Grip, core.difference(&Solid::union_all(&grooves)))];
    body.push((Material::Trim, pill(&[([-0.015, 0.0], 0.006), ([-0.018, -gl + 0.01], 0.008)], gw + 0.002, 0.002)));
    body.push((Material::Rarity, pill(&[([-0.006, -gl + 0.004], 0.006), ([0.004, -gl + 0.004], 0.006)], gw + 0.003, 0.002)));
    let outer = pill(&[([0.02, -0.012], 0.018), ([front - 0.016, -0.012], 0.018)], 0.007, 0.002);
    let inner = pill(&[([0.02, -0.012], 0.013), ([front - 0.016, -0.012], 0.013)], 0.02, 0.0);
    (body, vec![(Material::Trim, outer.difference(&inner).intersection(&block([0.0, 0.1], [-0.05, 0.004], 0.02)))])
}

/// A wood plank with tape bands and nails, and a guard bent from wire.
fn scrapjaw(b: &mut Build, gl: f64, gw: f64, front: f64) -> (Parts, Parts) {
    let plank = turn(&[[-0.019, 0.024], [0.017, 0.024], [0.016, -gl], [-0.02, -gl]], b.range(-3.0, 3.0));
    let mut body = vec![(Material::Grip, plate(&plank, gw, 0.002))];
    for _ in 0..2 {
        let z = -b.range(0.15, 0.8) * gl;
        body.push((Material::Trim, wrap(&plank, gw, &block([-1.0, 1.0], [z, z + 0.014], 1.0), 0.0012)));
    }
    body.push((Material::Metal, bolts(&[[-0.012, 0.008], [0.009, 0.008], [-0.004, -0.6 * gl]], gw + 0.002, 0.0018)));
    let guard = wire(&[[0.004, 0.003], [0.008, -0.028], [front - 0.006, -0.03], [front, -0.008], [front - 0.002, 0.003]], 0.0018);
    (body, vec![(Material::Metal, guard)])
}

/// A round sculpted body with a glow stripe on its front, a pommel ring and a ring guard.
fn halo(_: &mut Build, gl: f64, gw: f64, _front: f64) -> (Parts, Parts) {
    let core = taper(&[
        (-gl, egg(-0.002, 0.026, 0.75 * gw, 0.0)),
        (-gl + 0.006, egg(-0.002, 0.032, 0.95 * gw, 0.0)),
        (-0.5 * gl, egg(-0.001, 0.036, 1.05 * gw, 0.1)),
        (0.029, egg(0.0, 0.034, 0.95 * gw, 0.0)),
    ]);
    let mut body = vec![(Material::Paint, core)];
    body.push((Material::Glow, pill(&[([0.0155, -0.012], 0.003), ([0.0145, -gl + 0.02], 0.003)], gw * 0.5, 0.001)));
    body.push((Material::Trim, bevel([-0.02, 0.018], [-gl - 0.004, -gl + 0.006], gw + 0.003, 0.003)));
    body.push((Material::Metal, ring_y([-0.014, -gl - 0.008], 0.007, 0.003)));
    (body, vec![(Material::Trim, ring_y([0.034, -0.012], 0.02, 0.005)), (Material::Rarity, rod_y([0.034, -0.032], 0.006, 0.003, 12))])
}
