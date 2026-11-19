use geometry::Solid;
use patina_sdk::node;

/// The smallest convex solid around `a` and `b`.
#[node]
pub fn hull(a: &Solid, b: &Solid) -> Solid {
    a.union(b).hull()
}
