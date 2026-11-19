use city::Plots;
use geometry::Seed;
use patina_sdk::node;

/// Splits each block across its long side again and again, until a lot has `area` or less, or is
/// narrower than two times `width`. Each edge of a lot keeps the road class of its street.
#[node]
pub fn lots(blocks: &Plots, seed: &Seed, area: f64, width: f64) -> Result<Plots, String> {
    city::lots(blocks, *seed, area, width)
}
