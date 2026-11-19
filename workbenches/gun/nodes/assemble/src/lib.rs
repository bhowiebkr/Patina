use guns::{Element, Gun, Part, Rarity};
use patina_sdk::node;

/// Puts each part on its socket of the receiver, adds up the stats and applies the rarity and
/// the element. A part with the maker **none** is no part, but the gun needs a receiver.
#[node(color = "#3a3c40")]
#[allow(clippy::too_many_arguments)]
pub fn assemble(
    receiver: &Part,
    barrel: &Part,
    grip: &Part,
    stock: &Part,
    magazine: &Part,
    sight: &Part,
    accessory: &Part,
    rarity: &Rarity,
    element: &Element,
) -> Result<Gun, String> {
    guns::assemble(&[receiver, barrel, grip, stock, magazine, sight, accessory], *rarity, *element)
}
