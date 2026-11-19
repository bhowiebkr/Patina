use geometry::Scene;
use patina_sdk::node;

/// Writes the counts and the hash of the scene to the standard output. The same graph gives the
/// same hash on each platform.
#[node]
pub fn report(scene: &Scene) {
    println!("{scene:?}, scene hash {:016x}", scene.hash());
}
