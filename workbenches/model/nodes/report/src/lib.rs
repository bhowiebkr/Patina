use geometry::Solid;
use patina_sdk::node;

/// Writes the triangle count, the bounds and the mesh hash of the solid to the standard output.
/// The same graph gives the same hash on each platform.
#[node]
pub fn report(solid: &Solid) {
    println!("{solid:?}, mesh hash {:016x}", solid.mesh().hash());
}
