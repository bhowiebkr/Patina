use geometry::Solid;
use patina_sdk::node;

/// The volume in `a` or in `b`.
#[node]
pub fn union(a: &Solid, b: &Solid) -> Solid {
    a.union(b)
}
