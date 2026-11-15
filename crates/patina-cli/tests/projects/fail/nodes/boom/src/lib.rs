#[patina_sdk::node]
pub fn boom(text: &str) -> f64 {
    panic!("{text} panicked")
}
