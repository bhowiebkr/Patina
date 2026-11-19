use geometry::Seed;
use guns::{Gun, Maker, Pick, Rarity, generate};
use patina_sdk::node;

/// A full gun from one seed. A **random** maker or rarity comes from the seed, and a rare gun
/// is less frequent. A higher rarity gives better stats and adds a stock, a sight and an
/// accessory more frequently.
#[node(color = "#3a3c40")]
pub fn gun(seed: &Seed, maker: &Pick<Maker>, rarity: &Pick<Rarity>) -> Result<Gun, String> {
    generate(*seed, *maker, *rarity)
}
