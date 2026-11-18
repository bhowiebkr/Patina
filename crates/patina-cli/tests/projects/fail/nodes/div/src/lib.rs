/// A gate of a net (9.2). It panics when `b` is 0.
#[patina_sdk::node]
pub fn div(a: i32, b: i32) -> f64 {
    f64::from(a / b)
}
