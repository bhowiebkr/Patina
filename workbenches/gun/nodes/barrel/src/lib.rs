use geometry::Seed;
use guns::{Kind, Maker, Part, Pick, part};
use patina_sdk::node;

/// One part from the seed and the maker: the barrel, with the `muzzle` socket at its end. A long barrel gives accuracy.
/// The maker **none** gives no part, and **random** picks a maker from the seed. The same seed
/// and maker as a part of the `gun` node give the same part.
#[node(color = "#3a3c40")]
pub fn barrel(seed: &Seed, maker: &Pick<Maker>) -> Part {
    part(Kind::Barrel, seed.child("barrel"), *maker)
}
