use geometry::Solid;
use patina_sdk::node;

/// The volume that is in `a` and in `b`.
#[node]
pub fn intersect(a: &Solid, b: &Solid) -> Solid {
    a.intersection(b)
}
