//! The weapon classes: the size of each part, the base stats and the chance of each extra part.

use geometry::Seed;

use crate::{Choice, Kind, Stats};

/// The class of a gun. It sets the sizes and the base stats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Class {
    #[default]
    Pistol,
    Smg,
    Rifle,
    Shotgun,
    Sniper,
    Launcher,
}

impl Choice for Class {
    const ALL: &'static [Class] = &[Class::Pistol, Class::Smg, Class::Rifle, Class::Shotgun, Class::Sniper, Class::Launcher];
    fn name(self) -> &'static str {
        ["pistol", "smg", "rifle", "shotgun", "sniper", "launcher"][self as usize]
    }
}

impl Class {
    /// The chance of each class in percent, for a random class.
    pub const WEIGHTS: [f64; 6] = [24.0, 20.0, 22.0, 14.0, 12.0, 8.0];

    /// The name for a report, for example "SMG".
    pub fn title(self) -> &'static str {
        ["pistol", "SMG", "rifle", "shotgun", "sniper rifle", "launcher"][self as usize]
    }

    /// The class of a random pick from `seed`. The gun node and the part nodes use the same
    /// sub-seed, so they agree.
    pub fn of(seed: Seed) -> Class {
        Class::ALL[seed.child("class").random().pick(&Class::WEIGHTS)]
    }

    /// The stats of a receiver of this class, before the changes of the maker.
    pub fn base(self) -> Stats {
        let [damage, rate, accuracy, magazine, reload] = [
            [30.0, 5.0, 55.0, 12.0, 1.4],
            [16.0, 12.0, 40.0, 30.0, 1.8],
            [28.0, 8.0, 55.0, 26.0, 2.2],
            [90.0, 1.5, 25.0, 6.0, 2.6],
            [140.0, 1.0, 90.0, 5.0, 2.8],
            [300.0, 0.5, 40.0, 2.0, 3.5],
        ][self as usize];
        Stats { damage, rate, accuracy, magazine, reload }
    }

    /// The chance of an extra part on a gun of this class at the rarity level `level` (0 to 4).
    /// The receiver, the barrel, the grip and the magazine are always on the gun.
    pub fn chance(self, kind: Kind, level: f64) -> f64 {
        let (base, more) = match kind {
            // A launcher has a back blast pipe at the place of the stock.
            Kind::Stock => ([0.0, 0.45, 0.9, 0.8, 1.0, 0.0], 0.1),
            Kind::Sight => ([0.25, 0.4, 0.55, 0.35, 1.0, 0.6], 0.12),
            Kind::Accessory => ([0.1, 0.15, 0.2, 0.25, 0.1, 0.15], 0.15),
            _ => return 1.0,
        };
        let base = base[self as usize];
        if base == 0.0 { 0.0 } else { (base + more * level).min(1.0) }
    }
}

/// The sizes that all parts of one gun share, so parts from different nodes fit together. The
/// receiver goes from `x = -l/2` to `l/2` and from `z = 0` to `h`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub class: Class,
    /// The length, the height and the width of the receiver.
    pub l: f64,
    pub h: f64,
    pub w: f64,
    /// The radius of the bore.
    pub bore: f64,
    /// The lengths of the barrel, the grip, the stock and the magazine.
    pub barrel: f64,
    pub grip: f64,
    pub stock: f64,
    pub mag: f64,
}

/// The ranges of l, h, w, bore, barrel, grip, stock and magazine for each class.
const RANGES: [[[f64; 2]; 8]; 6] = [
    [[0.17, 0.22], [0.08, 0.1], [0.034, 0.042], [0.01, 0.013], [0.05, 0.11], [0.09, 0.11], [0.12, 0.16], [0.08, 0.12]],
    [[0.24, 0.3], [0.085, 0.105], [0.04, 0.05], [0.01, 0.013], [0.09, 0.17], [0.09, 0.11], [0.16, 0.22], [0.14, 0.2]],
    [[0.32, 0.4], [0.095, 0.12], [0.045, 0.055], [0.012, 0.015], [0.24, 0.36], [0.1, 0.12], [0.22, 0.3], [0.13, 0.18]],
    [[0.3, 0.38], [0.1, 0.125], [0.055, 0.065], [0.018, 0.024], [0.22, 0.34], [0.1, 0.12], [0.22, 0.28], [0.08, 0.12]],
    [[0.36, 0.44], [0.095, 0.115], [0.045, 0.055], [0.013, 0.017], [0.42, 0.58], [0.1, 0.12], [0.26, 0.32], [0.07, 0.1]],
    [[0.4, 0.5], [0.15, 0.2], [0.1, 0.13], [0.055, 0.075], [0.22, 0.34], [0.11, 0.13], [0.18, 0.24], [0.1, 0.14]],
];

impl Size {
    /// The sizes of a gun of `class` from `seed`.
    pub fn new(class: Class, seed: Seed) -> Size {
        let mut r = seed.random();
        let [l, h, w, bore, barrel, grip, stock, mag] = RANGES[class as usize].map(|[low, high]| r.range(low, high));
        Size { class, l, h, w, bore, barrel, grip, stock, mag }
    }

    /// Where the barrel length is in the range of the class: 0 for the shortest, 1 for the longest.
    pub fn long(&self) -> f64 {
        let [low, high] = RANGES[self.class as usize][4];
        (self.barrel - low) / (high - low)
    }

    /// Where the magazine length is in the range of the class, from 0 to 1.
    pub fn big(&self) -> f64 {
        let [low, high] = RANGES[self.class as usize][7];
        (self.mag - low) / (high - low)
    }

    /// The height of the bore on the receiver.
    pub fn bz(&self) -> f64 {
        self.h * if self.class == Class::Launcher { 0.58 } else { 0.66 }
    }

    /// The outer radius of a plain barrel or a launcher tube.
    pub fn shroud(&self) -> f64 {
        if self.class == Class::Launcher { self.bore + 0.012 } else { self.bore * 2.0 }
    }

    /// The thickness of the magazine along X.
    pub fn mag_depth(&self) -> f64 {
        [0.028, 0.03, 0.034, 0.042, 0.038, 0.07][self.class as usize]
    }

    /// The place of each socket of the receiver.
    pub fn socket(&self, kind: Kind) -> [f64; 3] {
        let (l, h) = (self.l, self.h);
        let grip = -l * if self.class == Class::Pistol { 0.28 } else { 0.22 };
        match kind {
            Kind::Receiver => [0.0; 3],
            Kind::Barrel => [l / 2.0, 0.0, self.bz()],
            Kind::Grip => [grip, 0.0, 0.0],
            Kind::Magazine => [grip + (l * 0.36).max(0.088) + self.mag_depth() / 2.0 - 0.014, 0.0, 0.0],
            Kind::Stock => [-l / 2.0, 0.0, h * 0.55],
            Kind::Sight => [-l * 0.05, 0.0, h],
            Kind::Accessory => [l / 2.0, 0.0, (self.bz() - self.shroud() - 0.016).max(h * 0.1)],
        }
    }
}
