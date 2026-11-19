//! The receivers: the body of the gun. The receiver sets the base stats and the sockets of all
//! other parts. It goes from `x = -l/2` to `l/2` and from `z = 0` to `h`.

use geometry::Solid;

use super::mark::mark;
use super::Build;
use crate::shapes::*;
use crate::{Choice, Class, Kind, Maker, Material, Stats};

pub(super) fn make(b: &mut Build, maker: Maker) -> Stats {
    match maker {
        Maker::Blockwerk => blockwerk(b),
        Maker::Vesper => vesper(b),
        Maker::Scrapjaw => scrapjaw(b),
        Maker::Halo => halo(b),
    }
    if b.class == Class::Launcher {
        venturi(b);
    }
    for &kind in &Kind::ALL[1..] {
        let at = b.s.socket(kind);
        b.socket(kind.name(), at);
    }
    let base = b.base();
    let roll = b.range(0.92, 1.08);
    let [damage, rate, accuracy, magazine, reload] = match maker {
        Maker::Blockwerk => [1.2, 0.85, -5.0, 1.0, 1.05],
        Maker::Vesper => [0.95, 1.0, 12.0, 0.9, 1.0],
        Maker::Scrapjaw => [1.0, 1.3, -10.0, 1.4, 1.15],
        Maker::Halo => [0.9, 1.05, 3.0, 1.0, 0.75],
    };
    Stats {
        damage: base.damage * damage * roll,
        rate: base.rate * rate * (2.0 - roll),
        accuracy: (base.accuracy + accuracy).clamp(5.0, 99.0),
        magazine: (base.magazine * magazine).round().max(1.0),
        reload: base.reload * reload,
    }
}

/// Thick plates with cut edges, side armor with vents, hex bolts, a top rail and hazard stripes.
fn blockwerk(b: &mut Build) {
    let (l, h, w) = (b.s.l, b.s.h, b.s.w);
    let (x0, x1) = (-l / 2.0, l / 2.0);
    let lower = [[x0, 0.08 * h], [x0 + 0.05 * l, 0.0], [x1 - 0.08 * l, 0.0], [x1, 0.14 * h], [x1, 0.56 * h], [x0, 0.56 * h]];
    b.add(Material::Paint, plate(&lower, w, 0.003));
    let upper = [[x0, 0.5 * h], [x1, 0.5 * h], [x1, 0.78 * h], [x1 - 0.22 * l, h], [x0 + 0.04 * l, h], [x0, 0.9 * h]];
    let hw = 0.43 * w;
    // The ejection port on the right side (-Y), with the bolt inside it.
    let port = [x1 - 0.48 * l, x1 - 0.28 * l];
    let cut = block(port, [0.6 * h, 0.82 * h], 0.012).translate([0.0, -hw, 0.0]);
    b.add(Material::Paint, plate(&upper, 2.0 * hw, 0.003).difference(&cut));
    b.add(Material::Metal, block([port[0] + 0.004, port[1] - 0.012], [0.63 * h, 0.79 * h], 0.006).translate([0.0, -hw + 0.002, 0.0]));
    let knob = [port[1] - 0.008, 0.71 * h];
    b.add(Material::Metal, rod_y(knob, 0.012, 0.0035, 10).translate([0.0, -hw - 0.007, 0.0]));
    b.add(Material::Metal, Solid::sphere(0.005, 10).translate([knob[0], -hw - 0.013, knob[1]]));
    // The side armor with vent slots, held by hex bolts.
    let armor = chamfer([x0 + 0.1 * l, 0.12 * h], [x1 - 0.34 * l, 0.6 * h], 0.012);
    let vents = slots([x0 + 0.18 * l, x1 - 0.42 * l], [0.3 * h, 0.46 * h], 4, 2.0 * w);
    let panel = slab(&armor, 0.006).translate([0.0, w / 2.0 + 0.002, 0.0]).difference(&vents);
    b.add(Material::Trim, both(&panel));
    let corners = [[x0 + 0.1 * l + 0.012, 0.18 * h], [x0 + 0.1 * l + 0.012, 0.54 * h], [x1 - 0.34 * l - 0.012, 0.18 * h], [x1 - 0.34 * l - 0.012, 0.54 * h]];
    b.add(Material::Metal, bolts(&corners, w + 0.009, 0.0032));
    // The top rail, the rear cap and the hazard stripes on the front of the lower plate.
    b.add(Material::Metal, rail([x0 + 0.08 * l, x1 - 0.28 * l], 0.016, 0.007).translate([0.0, 0.0, h - 0.001]));
    b.add(Material::Trim, bevel([x0 - 0.008, x0 + 0.012], [0.05 * h, 0.92 * h], w + 0.004, 0.003));
    for i in 0..3 {
        let stripe = stripe(x1 - 0.27 * l + 0.065 * l * f64::from(i), 0.03 * l, [0.16 * h, 0.48 * h], -0.03 * l);
        b.add(Material::Rarity, both(&decal(&stripe, w / 2.0, 0.0012)));
    }
    mark(b, Maker::Blockwerk, [x0 + 0.08 * l, x1 - 0.5 * l], 0.64 * h, 0.14 * h, hw);
}

/// A white shell over a dark frame: smooth pills, a glow strip in a groove, fins and a chevron.
fn vesper(b: &mut Build) {
    let (l, h, w, bz) = (b.s.l, b.s.h, b.s.w, b.s.bz());
    let (x0, x1) = (-l / 2.0, l / 2.0);
    b.add(Material::Trim, pill(&[([x0 + 0.24 * h, 0.24 * h], 0.24 * h), ([x1 - 0.16 * h, 0.2 * h], 0.18 * h)], 0.9 * w, 0.004));
    let shell = pill(&[([x0 + 0.42 * h, 0.58 * h], 0.42 * h), ([x1 - 0.2 * h, bz], 0.2 * h)], w, 0.006);
    let (groove, strip) = inlay(&strip([x0 + 0.36 * l, 0.47 * h], [x1 - 0.3 * l, 0.56 * h], 0.05 * h), w / 2.0, 0.0022);
    b.add(Material::Paint, shell.difference(&groove));
    b.add(Material::Glow, strip);
    // The cheek fins sweep back from the middle of the frame.
    let fin = [[x0 + 0.06 * l, 0.26 * h], [x0 + 0.33 * l, 0.36 * h], [x0 + 0.03 * l, 0.62 * h]];
    b.add(Material::Trim, both(&slab(&fin, 0.004).translate([0.0, w / 2.0 + 0.001, 0.0])));
    let (cx, cz) = (x1 - 0.2 * l, 0.68 * h);
    let chevron = [[cx, cz], [cx + 0.016, cz + 0.012], [cx + 0.008, cz + 0.012], [cx - 0.006, cz], [cx + 0.008, cz - 0.012], [cx + 0.016, cz - 0.012]];
    b.add(Material::Rarity, both(&decal(&chevron, w / 2.0, 0.0012)));
    mark(b, Maker::Vesper, [x0 + 0.3 * l, x1 - 0.3 * l], 0.66 * h, 0.12 * h, w / 2.0);
    collar(b, (0.2 * h).max(b.s.shroud() + 0.004));
    // A thin dark spine on top, behind the sight.
    b.add(Material::Trim, pill(&[([x0 + 0.06 * l, 0.9 * h], 0.012), ([x0 + 0.3 * l, h - 0.006], 0.008)], 0.3 * w, 0.002));
}

/// Crooked plates, a bent pipe with clamps, tape, a weld seam, a bare spring and a spray stripe.
fn scrapjaw(b: &mut Build) {
    let (l, h, w) = (b.s.l, b.s.h, b.s.w);
    let (x0, x1) = (-l / 2.0, l / 2.0);
    let j = 0.04 * h;
    let mut body = vec![
        [x0, 0.1 * h],
        [x0 + 0.1 * l, 0.0],
        [x1 - 0.08 * l, 0.03 * h],
        [x1, 0.3 * h],
        [x1 - 0.02 * l, 0.84 * h],
        [x0 + 0.3 * l, h],
        [x0 + 0.02 * l, 0.9 * h],
    ];
    for p in &mut body[2..5] {
        p[1] += b.range(-j, j);
    }
    let bw = 0.9 * w;
    let core = plate(&body, bw, 0.003);
    // A band of tape around the rear of the body.
    let tx = x0 + b.range(0.12, 0.2) * l;
    b.add(Material::Grip, wrap(&body, bw, &block([tx, tx + 0.022], [-1.0, 1.0], 1.0), 0.0015));
    b.add(Material::Paint, core);
    // Two side plates at an angle, each in the paint or the trim color, with bolts at the corners.
    let side = bw / 2.0 + 0.002;
    for (i, (a, c)) in [([x0 + 0.06 * l, 0.14 * h], [x0 + 0.44 * l, 0.6 * h]), ([x0 + 0.5 * l, 0.1 * h], [x1 - 0.1 * l, 0.48 * h])].into_iter().enumerate() {
        let angle = b.range(-5.0, 5.0);
        let quad = turn(&[a, [c[0], a[1]], c, [a[0], c[1]]], angle);
        let material = if b.chance(0.5) { Material::Paint } else { Material::Trim };
        let t = 0.004 + 0.001 * i as f64;
        b.add(material, both(&slab(&quad, t).translate([0.0, side, 0.0])));
        let heads = inset(&quad, 0.006);
        b.add(Material::Metal, bolts(&heads, 2.0 * side + 2.0 * t, 0.0026));
    }
    // A bent pipe on the right side (-Y), held by two clamps.
    let py = -(side + 0.012);
    let pipe = [[x0 + 0.04 * l, 0.66 * h], [x0 + 0.3 * l, 0.66 * h], [x0 + 0.42 * l, 0.5 * h], [x1 - 0.06 * l, 0.5 * h]];
    b.add(Material::Metal, wire(&pipe, 0.0055).translate([0.0, py, 0.0]));
    for x in [x0 + 0.18 * l, x1 - 0.2 * l] {
        let z = if x < x0 + 0.3 * l { 0.66 * h } else { 0.5 * h };
        b.add(Material::Trim, bevel([x - 0.004, x + 0.004], [z - 0.009, z + 0.009], 0.022, 0.001).translate([0.0, py + 0.004, 0.0]));
    }
    // A weld seam between the plates, and a bare spring on the right side.
    let seam: Vec<[f64; 2]> = (0..7).map(|i| [x0 + 0.46 * l + if i % 2 == 0 { -0.003 } else { 0.003 }, 0.14 * h + 0.06 * h * f64::from(i)]).collect();
    b.add(Material::Metal, wire(&seam, 0.0016).translate([0.0, -side - 0.002, 0.0]));
    let (sx, sz) = (x0 + 0.12 * l, 0.82 * h);
    let coils = (0..6).map(|i| ring_x(sx + 0.009 * f64::from(i), 0.0055, 0.0018)).chain([rod_x([sx - 0.004, sx + 0.05], [0.002, 0.002], 6)]);
    let spring = Solid::union_all(&coils.collect::<Vec<_>>());
    b.add(Material::Metal, spring.translate([0.0, -side - 0.004, sz]));
    let spray = [[x1 - 0.3 * l, 0.56 * h], [x1 - 0.2 * l, 0.6 * h], [x1 - 0.17 * l, 0.8 * h], [x1 - 0.24 * l, 0.78 * h], [x1 - 0.28 * l, 0.84 * h]];
    b.add(Material::Rarity, both(&decal(&spray, bw / 2.0, 0.0012)));
    mark(b, Maker::Scrapjaw, [x0 + 0.47 * l, x1 - 0.32 * l], 0.62 * h, 0.13 * h, bw / 2.0);
}

/// A round body on a wide frame, a glow core in a metal rim, glow cells and a rarity nose ring.
fn halo(b: &mut Build) {
    let (l, h, w) = (b.s.l, b.s.h, b.s.w);
    let (x0, x1) = (-l / 2.0, l / 2.0);
    let body = pill(&[([x0 + 0.48 * h, 0.52 * h], 0.48 * h), ([x1 - 0.3 * h, 0.55 * h], 0.38 * h)], w, 0.008);
    let (xc, zc, rc) = (x0 + 0.35 * l, 0.52 * h, 0.22 * h);
    b.add(Material::Paint, body.difference(&rod_y([xc, zc], w + 0.01, rc, 32)));
    b.add(Material::Glow, rod_y([xc, zc], 0.6 * w, rc * 0.9, 32));
    for side in [-1.0, 1.0] {
        b.add(Material::Metal, ring_y([xc, zc], rc, 0.006).translate([0.0, side * w / 2.0, 0.0]));
    }
    b.add(Material::Trim, pill(&[([x0 + 0.2 * l, 0.12 * h], 0.12 * h), ([x1 - 0.15 * l, 0.12 * h], 0.12 * h)], 1.06 * w, 0.003));
    b.add(Material::Trim, pill(&[([x0 + 0.48 * h, 0.97 * h], 0.05 * h), ([x1 - 0.3 * h, 0.9 * h], 0.04 * h)], 0.35 * w, 0.002));
    // Three glow cells in metal rings on the front of the body.
    for i in 0..3 {
        let c = [x0 + 0.6 * l + 0.075 * l * f64::from(i), 0.42 * h];
        let r = 0.045 * h;
        b.add(Material::Glow, rod_y(c, w + 0.003, r, 16));
        b.add(Material::Metal, both(&ring_y(c, r + 0.002, 0.003).translate([0.0, w / 2.0, 0.0])));
    }
    mark(b, Maker::Halo, [xc + rc + 0.012, x1 - 0.3 * h], 0.58 * h, 0.13 * h, w / 2.0);
    let nose = (b.s.shroud() + 0.004).max(0.2 * h);
    b.add(Material::Rarity, ring_x(x1 - 0.016, nose, 0.006).translate([0.0, 0.0, b.s.bz()]));
    collar(b, nose);
}

/// The metal collar at the front of the receiver, where the barrel goes in.
fn collar(b: &mut Build, r: f64) {
    let x1 = b.s.l / 2.0;
    let ring = turned(&[[x1 - 0.012, r], [x1 + 0.003, r], [x1 + 0.007, r * 0.8]], 28);
    b.add(Material::Metal, ring.translate([0.0, 0.0, b.s.bz()]));
}

/// The back blast cone of a launcher, at the place of the stock.
fn venturi(b: &mut Build) {
    let (x0, h) = (-b.s.l / 2.0, b.s.h);
    let (r0, r1, len) = (0.28 * h, 0.4 * h, 0.07);
    let cone = lathe(&[[x0 + 0.01, r0 - 0.004], [x0 - len, r1 - 0.005], [x0 - len, r1], [x0 + 0.01, r0]], 32);
    b.add(Material::Metal, cone.translate([0.0, 0.0, 0.55 * h]));
    b.add(Material::Trim, tube([x0 - len - 0.004, x0 - len + 0.012], r1 + 0.004, r1 - 0.004, 32).translate([0.0, 0.0, 0.55 * h]));
}
