//! The two measure cases of task D0: a gun of 30 parts and a city block of buildings.

use crate::{Affine, Mesh, Seed};

/// A gun of 30 parts along X, with the muzzle at +X. Each part has its own sub-seed.
pub fn gun(seed: Seed) -> Mesh {
    let mut gun = Mesh::default();
    let mut r = seed.child("receiver").rng();
    let (len, high) = (r.range(0.25, 0.45), r.range(0.08, 0.14));
    gun.append(&Mesh::cuboid([len, high, 0.05]), 0);

    let mut r = seed.child("barrel").rng();
    let barrel_len = r.range(0.2, 0.6);
    let barrel = Mesh::cylinder(r.range(0.012, 0.025), barrel_len, 6 + 2 * r.below(5));
    let along_x = Affine::rotate(2, -std::f32::consts::FRAC_PI_2);
    gun.append(&barrel.transformed(along_x.then(Affine::translate([len * 0.5, high * 0.2, 0.0]))), 1);

    let mut r = seed.child("grip").rng();
    let grip = Mesh::prism(&[[-0.02, -0.018], [0.02, -0.018], [0.02, 0.018], [-0.02, 0.018]], r.range(0.08, 0.14), r.range(0.8, 1.1));
    let tilt = Affine::rotate(2, r.range(2.7, 3.0)).then(Affine::translate([-len * 0.25, -high * 0.5, 0.0]));
    gun.append(&grip.transformed(tilt), 2);

    let mut r = seed.child("stock").rng();
    let stock = Mesh::prism(&[[0.0, -0.02], [0.03, -0.02], [0.03, 0.02], [0.0, 0.02]], r.range(0.1, 0.25), r.range(0.6, 1.6));
    gun.append(&stock.transformed(Affine::rotate(2, std::f32::consts::FRAC_PI_2).then(Affine::translate([-len * 0.5, 0.0, 0.0]))), 3);

    let mut r = seed.child("magazine").rng();
    let mag = Mesh::cuboid([0.04, r.range(0.08, 0.2), 0.03]);
    gun.append(&mag.transformed(Affine::rotate(2, r.range(-0.2, 0.2)).then(Affine::translate([len * 0.15, -high, 0.0]))), 4);

    let mut r = seed.child("sight").rng();
    gun.append(&Mesh::cylinder(0.015, r.range(0.05, 0.12), 12).transformed(along_x.then(Affine::translate([-0.03, high * 0.75, 0.0]))), 5);

    // 24 accessory bits on the rail and the barrel make 30 parts.
    for i in 0..24 {
        let mut r = seed.child("accessory").index(i).rng();
        let bit = Mesh::cuboid([r.range(0.005, 0.03), r.range(0.005, 0.02), r.range(0.03, 0.06)]);
        let x = r.range(-len * 0.5, len * 0.5 + barrel_len);
        gun.append(&bit.transformed(Affine::translate([x, high * 0.5, 0.0])), 6);
    }
    gun
}

/// A block of `count` buildings on a grid. Each building has a body, a setback and a roof.
pub fn block(seed: Seed, count: u32) -> Mesh {
    let mut block = Mesh::default();
    let side = (count as f32).sqrt().ceil() as u32;
    for i in 0..count {
        let mut r = seed.index(u64::from(i)).rng();
        let (w, d, h) = (r.range(8.0, 18.0), r.range(8.0, 18.0), r.range(6.0, 60.0));
        let at = Affine::translate([(i % side) as f32 * 20.0, 0.0, (i / side) as f32 * 20.0]);
        let body = Mesh::prism(&[[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]], h, 1.0);
        block.append(&body.transformed(at), 0);
        let setback = Mesh::prism(&[[1.0, 1.0], [w - 1.0, 1.0], [w - 1.0, d - 1.0], [1.0, d - 1.0]], h * 0.2, 1.0);
        block.append(&setback.transformed(Affine::translate([0.0, h, 0.0]).then(at)), 1);
        let roof = Mesh::prism(&[[2.0, 2.0], [w - 2.0, 2.0], [w - 2.0, d - 2.0], [2.0, d - 2.0]], r.range(1.0, 4.0), 0.9);
        block.append(&roof.transformed(Affine::translate([0.0, h * 1.2, 0.0]).then(at)), 2);
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_samples_are_closed_and_repeat_for_a_seed() {
        let (a, b) = (gun(Seed(7)), gun(Seed(7)));
        assert_eq!(a.hash(), b.hash());
        assert_ne!(a.hash(), gun(Seed(8)).hash());
        assert_eq!(a.parts.iter().max(), Some(&6));
        let city = block(Seed(7), 25);
        for mesh in [a, city] {
            assert!(mesh.is_closed() && mesh.volume() > 0.0);
        }
    }
}
