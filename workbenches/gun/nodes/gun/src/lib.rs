use geometry::Seed;
use guns::{Class, Element, Gun, Maker, Pick, Rarity, generate};
use patina_sdk::node;

/// A full gun from one seed. A **random** class, maker, rarity or element comes from the seed,
/// and a rare gun is less frequent. A higher rarity gives better stats, an element and more
/// extra parts more frequently. The element **none** gives no element.
#[node(color = "#3a3c40")]
pub fn gun(seed: &Seed, class: &Pick<Class>, maker: &Pick<Maker>, rarity: &Pick<Rarity>, element: &Pick<Element>) -> Result<Gun, String> {
    generate(*seed, *class, *maker, *rarity, *element)
}
