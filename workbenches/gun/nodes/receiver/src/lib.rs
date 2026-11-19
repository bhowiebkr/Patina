use geometry::Seed;
use guns::{Kind, Maker, Part, Pick, part};
use patina_sdk::node;

/// One part from the seed and the maker: the body of the gun, with a socket for each other part. It sets the base stats.
/// The maker **none** gives no part, and **random** picks a maker from the seed. The same seed
/// and maker as a part of the `gun` node give the same part.
#[node(color = "#3a3c40")]
pub fn receiver(seed: &Seed, maker: &Pick<Maker>) -> Part {
    part(Kind::Receiver, seed.child("receiver"), *maker)
}
