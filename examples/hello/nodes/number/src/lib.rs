use patina_sdk::node;

/// Gives a number. Set the number in the parameter of the input.
#[node]
pub fn number(value: f64) -> f64 {
    value
}
