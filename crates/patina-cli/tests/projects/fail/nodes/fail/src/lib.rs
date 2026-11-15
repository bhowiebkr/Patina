#[patina_sdk::node]
pub fn fail(value: f64) -> Result<f64, String> {
    Err(format!("cannot use {value}"))
}
