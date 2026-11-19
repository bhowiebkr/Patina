use geometry::Seed;
use guns::{Class, Kind, Maker, Part, Pick, part};
use patina_sdk::node;

/// One part from the seed, the class and the maker: the stock. It gives accuracy and a slower reload.
/// The maker **none** gives no part. A **random** class or maker comes from the seed. The same
/// seed, class and maker as the `gun` node give the same part.
#[node(color = "#3a3c40")]
pub fn stock(seed: &Seed, class: &Pick<Class>, maker: &Pick<Maker>) -> Part {
    part(Kind::Stock, *seed, *maker, *class)
}
