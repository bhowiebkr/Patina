use patina_sdk::node;

/// A constant bit that the user sets.
#[node]
pub fn switch(on: bool) -> bool {
    on
}
