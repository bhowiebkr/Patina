use geometry::{Seed, Solid};
use patina_sdk::node;

/// Picks `a` or `b` at random. The same seed and name give the same pick. `weights` gives the
/// chance of each, for example "3 1" picks `a` 3 times as often as `b`. With no weights, each
/// has the same chance.
#[node]
pub fn pick(seed: &Seed, name: &str, weights: &str, a: &Solid, b: &Solid) -> Result<Solid, String> {
    let weights = parse(weights)?;
    Ok(if seed.child(name).random().pick(&weights) == 0 { a.clone() } else { b.clone() })
}

fn parse(weights: &str) -> Result<[f64; 2], String> {
    if weights.trim().is_empty() {
        return Ok([1.0, 1.0]);
    }
    let numbers: Option<Vec<f64>> = weights.split_whitespace().map(|w| w.parse().ok().filter(|n: &f64| n.is_finite() && *n >= 0.0)).collect();
    match numbers.as_deref() {
        Some(&[a, b]) if a + b > 0.0 => Ok([a, b]),
        _ => Err(format!("write the weights {weights:?} as two numbers from 0, for example \"3 1\"")),
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn the_weights_are_two_numbers_from_0() {
        assert_eq!(parse(" "), Ok([1.0, 1.0]));
        assert_eq!(parse("3 1"), Ok([3.0, 1.0]));
        assert_eq!(parse("0 2"), Ok([0.0, 2.0]));
        for bad in ["1", "1 2 3", "-1 2", "0 0", "a b", "nan 1"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
