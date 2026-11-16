use patina_sdk::node;

/// A constant bit that the user sets.
#[node(color = "#655219")]
pub fn switch(on: bool) -> bool {
    on
}
