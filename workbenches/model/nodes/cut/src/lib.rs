use geometry::Solid;
use patina_sdk::node;

/// The volume of `a` that is not in `b`.
#[node]
pub fn cut(a: &Solid, b: &Solid) -> Solid {
    a.difference(b)
}
