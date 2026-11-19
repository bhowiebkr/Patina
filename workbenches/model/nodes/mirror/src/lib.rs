use geometry::Solid;
use patina_sdk::node;

/// Mirrors the solid on the plane through the origin with the normal `x y z`. For example,
/// x 1 mirrors left and right. `keep` also keeps the solid that goes in.
#[node]
pub fn mirror(solid: &Solid, x: f64, y: f64, z: f64, keep: bool) -> Result<Solid, String> {
    if x == 0.0 && y == 0.0 && z == 0.0 {
        return Err("set x, y or z to give the plane a normal".into());
    }
    let mirrored = solid.mirror([x, y, z]);
    Ok(if keep { solid.union(&mirrored) } else { mirrored })
}
