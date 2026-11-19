use geometry::Seed;
use guns::{Kind, Maker, Part, Pick, part};
use patina_sdk::node;

/// One part from the seed and the maker: the accessory under the barrel. Each maker changes other stats.
/// The maker **none** gives no part, and **random** picks a maker from the seed. The same seed
/// and maker as a part of the `gun` node give the same part.
#[node(color = "#3a3c40")]
pub fn accessory(seed: &Seed, maker: &Pick<Maker>) -> Part {
    part(Kind::Accessory, seed.child("accessory"), *maker)
}
