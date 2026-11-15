use patina_sdk::node;

/// Writes the value to the standard output.
#[node]
pub fn show(value: &f64) {
    println!("{value}");
}
