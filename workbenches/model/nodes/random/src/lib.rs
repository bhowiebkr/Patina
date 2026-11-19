use geometry::Seed;
use patina_sdk::node;

/// A random number from `low` to `high`. The same seed and name give the same number. Give
/// each random value its own name, for example "length".
#[node]
pub fn random(seed: &Seed, name: &str, low: f64, #[param(default = 1.0)] high: f64) -> f64 {
    seed.child(name).random().range(low, high)
}
