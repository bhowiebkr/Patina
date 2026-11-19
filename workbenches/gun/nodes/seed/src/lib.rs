use geometry::Seed;
use patina_sdk::node;

/// A seed for the nodes of one part, for example "barrel". A change to the random values of one
/// part then does not change the other parts. With no name, the seed goes out with no change.
#[node]
pub fn seed(seed: &Seed, name: &str) -> Seed {
    if name.is_empty() { *seed } else { seed.child(name) }
}
