use geometry::Scene;
use patina_sdk::node;

/// The items of `a`, then the items of `b`, in one scene.
#[node]
pub fn merge(a: &Scene, b: &Scene) -> Scene {
    a.merge(b)
}
