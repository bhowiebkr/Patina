//! The shape rules of each maker for each kind of part. Each part has its mount at the origin.

use geometry::{Frame, Random, Seed, Sketch, Solid, Sockets};

use crate::{Choice, Kind, Maker, Part, Pick, Stats};

/// One part from a seed and a maker. `none` gives no part, and `random` picks a maker.
pub fn part(kind: Kind, seed: Seed, maker: Pick<Maker>) -> Part {
    let mut r = seed.random();
    let maker = match maker {
        Pick::Of(maker) => maker,
        Pick::Random => Maker::ALL[r.index(Maker::ALL.len())],
        Pick::None => return Part { kind, ..Part::default() },
    };
    let shape = match kind {
        Kind::Receiver => receiver(maker, &mut r),
        Kind::Barrel => barrel(maker, &mut r),
        Kind::Grip => grip(maker, &mut r),
        Kind::Stock => stock(maker, &mut r),
        Kind::Magazine => magazine(maker, &mut r),
        Kind::Sight => sight(maker, &mut r),
        Kind::Accessory => accessory(maker, &mut r),
    };
    Part { kind, maker: Some(maker), ..shape }
}

/// A plate with the profile on the X-Z plane and the thickness `t` on Y, centered on Y.
fn slab(profile: &[[f64; 2]], t: f64) -> Solid {
    Solid::extrude(&Sketch::polygon(profile), t, 0.0, [1.0; 2], 0).rotate([90.0, 0.0, 0.0]).translate([0.0, t / 2.0, 0.0])
}

/// A round rod along +X from the origin, with the radius `r0` at the origin and `r1` at the end.
fn rod(length: f64, r0: f64, r1: f64) -> Solid {
    Solid::cylinder(length, r0, r1, 16).rotate([0.0, 90.0, 0.0]).translate([length / 2.0, 0.0, 0.0])
}

/// A box with the size `[x, y, z]` and its center at `at`.
fn block([x, y, z]: [f64; 3], at: [f64; 3]) -> Solid {
    Solid::cube(x, y, z).translate(at)
}

/// A ring around the Z axis with the radius `radius` and the thickness `t`.
fn torus(radius: f64, t: f64) -> Solid {
    Solid::revolve(&Sketch::circle(t / 2.0, 8).translate([radius, 0.0]), 360.0, 20)
}

/// A ring around the X axis, with its center at `at`.
fn ring_x(at: [f64; 3], radius: f64, t: f64) -> Solid {
    torus(radius, t).rotate([0.0, 90.0, 0.0]).translate(at)
}

/// A ring around the Y axis, on the X-Z plane, with its center at `at`.
fn ring_y(at: [f64; 3], radius: f64, t: f64) -> Solid {
    torus(radius, t).rotate([90.0, 0.0, 0.0]).translate(at)
}

/// A bolt through a part of the width `w` at `[x, z]`, with a head on each side.
fn bolt([x, z]: [f64; 2], w: f64) -> Solid {
    Solid::cylinder(w + 0.008, 0.005, 0.005, 6).rotate([90.0, 0.0, 0.0]).translate([x, 0.0, z])
}

/// The corners of a rectangle from `[x0, z0]` to `[x1, z1]`, with each corner cut by `c`.
fn chamfer([x0, z0]: [f64; 2], [x1, z1]: [f64; 2], c: f64) -> Vec<[f64; 2]> {
    vec![[x0 + c, z0], [x1 - c, z0], [x1, z0 + c], [x1, z1 - c], [x1 - c, z1], [x0 + c, z1], [x0, z1 - c], [x0, z0 + c]]
}

fn sockets(points: &[(&str, [f64; 3])]) -> Sockets {
    let mut sockets = Sockets::default();
    for &(name, at) in points {
        sockets.set(name, Frame::at(at));
    }
    sockets
}

/// The receiver: the body from `x = -l/2` to `l/2` and from `z = 0` to `h`, the base stats, and
/// a socket for each other kind.
fn receiver(maker: Maker, r: &mut Random) -> Part {
    let [l, h, w] = match maker {
        Maker::Blockwerk => [r.range(0.28, 0.40), r.range(0.11, 0.15), r.range(0.06, 0.075)],
        Maker::Vesper => [r.range(0.30, 0.42), r.range(0.08, 0.11), r.range(0.045, 0.055)],
        Maker::Scrapjaw => [r.range(0.26, 0.36), r.range(0.10, 0.13), r.range(0.05, 0.07)],
        Maker::Halo => [r.range(0.26, 0.36), r.range(0.09, 0.12), r.range(0.05, 0.06)],
    };
    let (body, accent) = match maker {
        Maker::Blockwerk => (
            slab(&chamfer([-l / 2.0, 0.0], [l / 2.0, h], 0.015), w).union(&block([l * 0.6, w * 0.5, 0.014], [0.0, 0.0, h + 0.007])),
            block([l * 0.5, w + 0.004, 0.012], [-l * 0.05, 0.0, h * 0.55]),
        ),
        Maker::Vesper => {
            let profile = [[-l / 2.0, 0.2 * h], [-l / 2.0 + 0.03, 0.0], [l / 2.0, 0.0], [l / 2.0, 0.7 * h], [l * 0.2, h], [-l / 2.0 + 0.05, h], [-l / 2.0, 0.75 * h]];
            (slab(&profile, w), block([l * 0.55, w + 0.003, 0.006], [0.0, 0.0, 0.62 * h]))
        }
        Maker::Scrapjaw => {
            let mut pieces = vec![block([l, w, h * 0.8], [0.0, 0.0, 0.4 * h])];
            for _ in 0..r.index(2) + 2 {
                let size = [l * r.range(0.3, 0.6), w + 0.008, h * r.range(0.3, 0.5)];
                let at = [r.range(-0.3, 0.3) * l, 0.0, r.range(0.35, 0.85) * h];
                pieces.push(Solid::cube(size[0], size[1], size[2]).rotate([0.0, r.range(-8.0, 8.0), 0.0]).translate(at));
            }
            for _ in 0..4 {
                pieces.push(bolt([r.range(-0.4, 0.4) * l, r.range(0.15, 0.7) * h], w + 0.008));
            }
            (Solid::union_all(&pieces), block([0.02, w + 0.02, h * 0.9], [-l * 0.35, 0.0, 0.45 * h]))
        }
        Maker::Halo => {
            let front = Solid::cylinder(w, h / 2.0, h / 2.0, 24).rotate([90.0, 0.0, 0.0]).translate([l / 2.0 - h / 2.0, 0.0, h / 2.0]);
            let body = slab(&[[-l / 2.0, 0.0], [l / 2.0 - h / 2.0, 0.0], [l / 2.0 - h / 2.0, h], [-l / 2.0, h]], w).union(&front);
            let around = (h * h + w * w).sqrt() / 2.0 + 0.004;
            (body, Solid::union_all(&[-0.25, 0.05].map(|x| ring_x([x * l, 0.0, h / 2.0], around, 0.012))))
        }
    };
    let base = match maker {
        Maker::Blockwerk => Stats { damage: 40.0, rate: 3.0, accuracy: 38.0, magazine: 0.0, reload: 2.4 },
        Maker::Vesper => Stats { damage: 30.0, rate: 5.0, accuracy: 50.0, magazine: 0.0, reload: 2.0 },
        Maker::Scrapjaw => Stats { damage: 18.0, rate: 10.0, accuracy: 24.0, magazine: 0.0, reload: 2.6 },
        Maker::Halo => Stats { damage: 24.0, rate: 6.5, accuracy: 40.0, magazine: 0.0, reload: 1.4 },
    };
    let roll = r.range(0.9, 1.1);
    let stats = Stats { damage: base.damage * roll, rate: base.rate * (2.0 - roll), ..base };
    // The round front of a Halo receiver ends at `l/2` only at the half height.
    let (front, barrel) = if maker == Maker::Halo { (l / 2.0 - h / 2.0, 0.5 * h) } else { (l / 2.0, 0.65 * h) };
    let top = if maker == Maker::Blockwerk { h + 0.014 } else { h };
    let sockets = sockets(&[
        ("barrel", [l / 2.0, 0.0, barrel]),
        ("grip", [-0.25 * l, 0.0, 0.0]),
        ("stock", [-l / 2.0, 0.0, 0.5 * h]),
        ("magazine", [0.2 * l, 0.0, 0.0]),
        ("sight", [0.0, 0.0, top]),
        ("accessory", [front, 0.0, 0.3 * h]),
    ]);
    Part { body, accent, sockets, stats, ..Part::default() }
}

/// The barrel along +X, with the `muzzle` socket at its end. A long barrel gives accuracy.
fn barrel(maker: Maker, r: &mut Random) -> Part {
    let length = match maker {
        Maker::Blockwerk => r.range(0.18, 0.32),
        Maker::Vesper => r.range(0.25, 0.45),
        Maker::Scrapjaw => r.range(0.15, 0.30),
        Maker::Halo => r.range(0.20, 0.35),
    };
    let (body, accent) = match maker {
        Maker::Blockwerk => {
            let s = r.range(0.03, 0.04);
            let brake = block([0.045, s * 1.6, s * 1.4], [length - 0.0225, 0.0, 0.0]).difference(&block([0.012, s * 2.0, s * 0.5], [length - 0.02, 0.0, 0.0]));
            (block([length, s, s], [length / 2.0, 0.0, 0.0]).union(&brake), block([0.014, s + 0.004, s + 0.004], [length * 0.45, 0.0, 0.0]))
        }
        Maker::Vesper => (rod(length, 0.018, 0.012).union(&rod(length * 0.5, 0.025, 0.02)), rod(0.015, 0.0135, 0.013).translate([length - 0.015, 0.0, 0.0])),
        Maker::Scrapjaw => {
            let pipe = rod(length, 0.02, 0.02).difference(&rod(length + 0.02, 0.012, 0.012).translate([-0.01, 0.0, 0.0]));
            let clamps: Vec<Solid> = (0..r.index(2) + 1)
                .map(|i| Solid::cube(0.016, 0.052, 0.052).rotate([r.range(0.0, 45.0), 0.0, 0.0]).translate([length * (0.25 + 0.3 * i as f64), 0.0, 0.0]))
                .collect();
            (pipe.union(&Solid::union_all(&clamps)), block([0.02, 0.046, 0.046], [length - 0.03, 0.0, 0.0]))
        }
        Maker::Halo => {
            let count = r.index(3) + 3;
            let rings: Vec<Solid> = (0..count).map(|i| ring_x([length * (0.2 + 0.6 * i as f64 / (count - 1) as f64), 0.0, 0.0], 0.026, 0.01)).collect();
            (rod(length, 0.015, 0.015).union(&rod(length * 0.15, 0.02, 0.02).translate([length * 0.85, 0.0, 0.0])), Solid::union_all(&rings))
        }
    };
    let stats = Stats { damage: length * 20.0, accuracy: length * if maker == Maker::Vesper { 55.0 } else { 40.0 }, ..Stats::default() };
    Part { body, accent, sockets: sockets(&[("muzzle", [length, 0.0, 0.0])]), stats, ..Part::default() }
}

/// The grip down and back from the mount, with a trigger guard and the `hand` socket.
fn grip(maker: Maker, r: &mut Random) -> Part {
    let (gw, gl, t) = (r.range(0.035, 0.05), r.range(0.09, 0.13), r.range(0.028, 0.036));
    let angle = r.range(15.0, 25.0);
    let lean = gl * angle.to_radians().tan();
    let mut profile = vec![[-gw / 2.0, 0.0], [gw / 2.0, 0.0], [gw / 2.0 - lean, -gl], [-gw / 2.0 - lean, -gl]];
    if maker == Maker::Vesper {
        profile.extend([[-gw / 2.0 - lean * 0.55 - 0.012, -gl * 0.55], [-gw / 2.0 - lean * 0.2 - 0.004, -gl * 0.2]]);
    }
    let body = slab(&profile, t);
    let guard = ring_y([gw / 2.0 + 0.02, 0.0, -0.02], 0.02, 0.007);
    let (body, accent) = match maker {
        Maker::Blockwerk => (body.union(&guard), block([gw + 0.01, t + 0.004, 0.014], [-lean, 0.0, -gl])),
        Maker::Vesper => (body.union(&guard), Solid::cube(0.008, t + 0.003, gl * 0.7).rotate([0.0, angle, 0.0]).translate([gw / 2.0 - lean / 2.0, 0.0, -gl / 2.0])),
        Maker::Scrapjaw => {
            let band = |f: f64| block([gw + 0.01, t + 0.006, 0.012], [-lean * f, 0.0, -gl * f]);
            (body.union(&guard), band(0.35).union(&band(0.7)))
        }
        Maker::Halo => (body.union(&Solid::cylinder(t, gw / 2.0, gw / 2.0, 16).rotate([90.0, 0.0, 0.0]).translate([-lean, 0.0, -gl])), guard),
    };
    let stats = Stats { accuracy: r.range(3.0, 6.0), ..Stats::default() };
    Part { body, accent, sockets: sockets(&[("hand", [-lean / 2.0, 0.0, -gl / 2.0])]), stats, ..Part::default() }
}

/// The stock back from the mount, with a butt pad as the accent.
fn stock(maker: Maker, r: &mut Random) -> Part {
    let (length, drop) = (r.range(0.18, 0.30), r.range(0.06, 0.10));
    let pad = |w: f64| block([0.022, w, drop + 0.025], [-length, 0.0, (0.015 - drop) / 2.0]);
    let outer = [[0.0, 0.02], [-length, 0.015], [-length, -drop], [0.0, -0.04]];
    let (body, accent) = match maker {
        Maker::Blockwerk => (slab(&outer, 0.045), pad(0.05)),
        Maker::Vesper => {
            let hole = [[-0.04, 0.0], [-length + 0.03, -0.002], [-length + 0.03, -drop + 0.02], [-0.04, -0.03]];
            (slab(&outer, 0.032).difference(&slab(&hole, 0.05)), pad(0.036))
        }
        Maker::Scrapjaw => {
            let dz = drop - 0.045;
            let lower = rod(length.hypot(dz), 0.01, 0.01).rotate([0.0, dz.atan2(-length).to_degrees(), 0.0]).translate([0.0, 0.0, -0.035]);
            let upper = rod(length, 0.01, 0.01).rotate([0.0, 0.0, 180.0]).translate([0.0, 0.0, 0.005]);
            (upper.union(&lower).union(&block([0.03, 0.03, 0.06], [-0.01, 0.0, -0.015])), pad(0.045))
        }
        Maker::Halo => (ring_y([0.0; 3], drop / 2.0, 0.012).scale([length / drop, 1.0, 1.0]).translate([-length / 2.0, 0.0, (0.015 - drop) / 2.0]), pad(0.03)),
    };
    let stats = Stats { accuracy: r.range(8.0, 14.0), reload: 0.1, ..Stats::default() };
    Part { body, accent, stats, ..Part::default() }
}

/// The magazine down from the mount. A large magazine reloads slower.
fn magazine(maker: Maker, r: &mut Random) -> Part {
    let (body, accent, size) = match maker {
        Maker::Blockwerk => {
            let (w, l) = (r.range(0.035, 0.045), r.range(0.08, 0.12));
            (block([w, 0.03, l], [0.0, 0.0, -l / 2.0]), block([w + 0.008, 0.036, 0.012], [0.0, 0.0, -l]), r.range(6.0, 10.0))
        }
        Maker::Vesper => {
            let (w, l, bend) = (r.range(0.03, 0.038), r.range(0.10, 0.14), r.range(0.02, 0.045));
            let curve = |x: f64, i: i32| {
                let s = f64::from(i) / 6.0;
                [x + bend * s * s, -l * s]
            };
            let mut profile: Vec<[f64; 2]> = (0..=6).map(|i| curve(w / 2.0, i)).collect();
            profile.extend((0..=6).rev().map(|i| curve(-w / 2.0, i)));
            (slab(&profile, 0.026), block([w + 0.008, 0.032, 0.01], [bend, 0.0, -l]), r.range(12.0, 24.0))
        }
        Maker::Scrapjaw if r.unit() < 0.5 => {
            let radius = r.range(0.045, 0.06);
            let drum = Solid::cylinder(0.05, radius, radius, 20).rotate([90.0, 0.0, 0.0]).translate([0.0, 0.0, -radius - 0.01]);
            let hub = Solid::cylinder(0.056, 0.012, 0.012, 8).rotate([90.0, 0.0, 0.0]).translate([0.0, 0.0, -radius - 0.01]);
            (drum.union(&block([0.03, 0.026, 0.02], [0.0, 0.0, -0.01])), hub, r.range(40.0, 60.0))
        }
        Maker::Scrapjaw => {
            let l = r.range(0.14, 0.20);
            (block([0.04, 0.03, l], [0.0, 0.0, -l / 2.0]).union(&bolt([0.0, -l * 0.5], 0.03)), block([0.048, 0.036, 0.012], [0.0, 0.0, -l]), r.range(30.0, 40.0))
        }
        Maker::Halo => {
            let (radius, l) = (r.range(0.016, 0.022), r.range(0.07, 0.11));
            let rings: Vec<Solid> = [0.3, 0.6].iter().map(|f| torus(radius + 0.002, 0.007).translate([0.0, 0.0, -l * f])).collect();
            (Solid::cylinder(l, radius, radius, 16).translate([0.0, 0.0, -l / 2.0]), Solid::union_all(&rings), r.range(15.0, 30.0))
        }
    };
    let size = size.round();
    Part { body, accent, stats: Stats { magazine: size, reload: size * 0.012, ..Stats::default() }, ..Part::default() }
}

/// The sight up from the mount. It gives accuracy.
fn sight(maker: Maker, r: &mut Random) -> Part {
    let (body, accent, accuracy) = match maker {
        Maker::Blockwerk => {
            let base = block([0.05, 0.03, 0.026], [0.0, 0.0, 0.013]).difference(&block([0.06, 0.01, 0.014], [0.0, 0.0, 0.024]));
            (base, block([0.008, 0.008, 0.01], [0.02, 0.0, 0.031]), r.range(6.0, 10.0))
        }
        Maker::Vesper => {
            let length = r.range(0.12, 0.18);
            let tube = rod(length, 0.016, 0.016).translate([-length / 2.0, 0.0, 0.036]);
            let mounts = [-0.3, 0.3].map(|f| block([0.012, 0.012, 0.024], [f * length, 0.0, 0.012]));
            (tube.union(&Solid::union_all(&mounts)), rod(0.004, 0.018, 0.018).translate([length / 2.0 - 0.004, 0.0, 0.036]), r.range(10.0, 16.0))
        }
        Maker::Scrapjaw => (
            block([0.012, 0.012, 0.03], [0.0, 0.0, 0.015]).union(&ring_x([0.0, 0.0, 0.045], 0.014, 0.006)),
            block([0.03, 0.02, 0.008], [0.0, 0.0, 0.004]),
            r.range(5.0, 9.0),
        ),
        Maker::Halo => (
            block([0.04, 0.02, 0.012], [0.0, 0.0, 0.006]).union(&block([0.006, 0.006, 0.02], [0.0, 0.0, 0.02])),
            ring_x([0.0, 0.0, 0.04], 0.02, 0.006),
            r.range(8.0, 12.0),
        ),
    };
    Part { body, accent, stats: Stats { accuracy, ..Stats::default() }, ..Part::default() }
}

/// The accessory under the barrel, forward from the mount on the front of the receiver.
fn accessory(maker: Maker, r: &mut Random) -> Part {
    let (body, accent, stats) = match maker {
        Maker::Blockwerk => {
            let l = r.range(0.10, 0.14);
            let stats = Stats { damage: 8.0, reload: 0.2, ..Stats::default() };
            (block([l, 0.04, 0.04], [l / 2.0 - 0.02, 0.0, -0.02]), block([0.012, 0.044, 0.044], [l * 0.6, 0.0, -0.02]), stats)
        }
        Maker::Vesper => {
            let l = r.range(0.07, 0.10);
            (rod(l, 0.013, 0.016).translate([-0.02, 0.0, -0.02]), rod(0.004, 0.017, 0.017).translate([l - 0.02, 0.0, -0.02]), Stats { accuracy: 4.0, ..Stats::default() })
        }
        Maker::Scrapjaw => {
            let reach = r.range(0.10, 0.16);
            let blade = slab(&[[-0.02, 0.0], [reach, -0.015], [-0.02, -0.045]], 0.006);
            (blade, bolt([0.0, -0.02], 0.006), Stats { damage: 5.0, rate: 1.0, ..Stats::default() })
        }
        Maker::Halo => {
            let rings: Vec<Solid> = (0..3).map(|i| ring_x([0.03 + 0.025 * f64::from(i), 0.0, -0.02], 0.018, 0.007)).collect();
            (rod(0.12, 0.008, 0.008).translate([-0.02, 0.0, -0.02]), Solid::union_all(&rings), Stats { rate: 1.5, reload: -0.2, ..Stats::default() })
        }
    };
    Part { body, accent, stats, ..Part::default() }
}
