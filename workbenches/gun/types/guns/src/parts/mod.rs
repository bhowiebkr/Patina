//! The shape rules of each maker for each kind of part. Each part has its mount at the origin.
//! Each maker has its own shape language:
//!
//! - Blockwerk: thick plates with cut edges, side armor, vents, hex bolts and hazard stripes.
//! - Vesper: smooth pills and tapers, a white shell on a dark frame, glow strips and fins.
//! - Scrapjaw: crooked plates, pipes, tape, wood, clamps, welds and springs.
//! - Halo: round bodies, rings and coils, glow cores and cells.

mod accessory;
mod barrel;
mod grip;
mod magazine;
mod mark;
mod receiver;
mod sight;
mod stock;

use geometry::{Frame, Random, Seed, Solid, Sockets};

use crate::{Choice, Class, Kind, Maker, Material, Part, Pick, Size, Stats};

/// One part from the seed of a gun, a maker and a class. `none` gives no part, and `random`
/// picks from the seed. Parts with the same seed and class have the same sizes, so they fit.
pub fn part(kind: Kind, seed: Seed, maker: Pick<Maker>, class: Pick<Class>) -> Part {
    let maker = match maker {
        Pick::Of(maker) => maker,
        Pick::Random => Maker::ALL[seed.child(kind.name()).child("maker").random().index(Maker::ALL.len())],
        Pick::None => return Part { kind, ..Part::default() },
    };
    let class = match class {
        Pick::Of(class) => class,
        Pick::Random => Class::of(seed),
        Pick::None => return Part { kind, ..Part::default() },
    };
    let mut b = Build {
        s: Size::new(class, seed.child("size")),
        class,
        r: seed.child(kind.name()).random(),
        kit: Default::default(),
        sockets: Sockets::default(),
    };
    let stats = match kind {
        Kind::Receiver => receiver::make(&mut b, maker),
        Kind::Barrel => barrel::make(&mut b, maker),
        Kind::Grip => grip::make(&mut b, maker),
        Kind::Stock => stock::make(&mut b, maker),
        Kind::Magazine => magazine::make(&mut b, maker),
        Kind::Sight => sight::make(&mut b, maker),
        Kind::Accessory => accessory::make(&mut b, maker),
    };
    let scheme = seed.child("paint").random().index(3) as u8;
    let solids = b.kit.map(|list| if list.is_empty() { Solid::empty() } else { Solid::union_all(&list) });
    Part { kind, maker: Some(maker), class, scheme, solids, sockets: b.sockets, stats }
}

/// The solids of one part in work, by material, and the random values of the part.
pub(crate) struct Build {
    pub s: Size,
    pub class: Class,
    pub r: Random,
    kit: [Vec<Solid>; 6],
    sockets: Sockets,
}

impl Build {
    pub fn add(&mut self, material: Material, solid: Solid) {
        self.kit[material as usize].push(solid);
    }

    pub fn socket(&mut self, name: &str, at: [f64; 3]) {
        self.sockets.set(name, Frame::at(at));
    }

    pub fn range(&mut self, low: f64, high: f64) -> f64 {
        self.r.range(low, high)
    }

    /// True with the chance `p` (0 to 1).
    pub fn chance(&mut self, p: f64) -> bool {
        self.r.unit() < p
    }

    /// One of `count` shape variants.
    pub fn variant(&mut self, count: usize) -> usize {
        self.r.index(count)
    }

    /// The base stats of the class.
    pub fn base(&self) -> Stats {
        self.class.base()
    }
}
