use patina_sdk::node;

/// Adds two numbers.
#[node(symbol = "adder")]
pub fn add(a: f64, b: f64) -> f64 {
    a + b
}
