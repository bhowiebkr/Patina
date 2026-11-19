use geometry::{MAX_COPIES, Solid};
use patina_sdk::node;

/// `count` copies of the solid around the Z axis, at equal angles.
#[node]
pub fn polar_array(solid: &Solid, count: u32) -> Result<Solid, String> {
    if count > MAX_COPIES {
        return Err(format!("make {MAX_COPIES} copies or fewer, not {count}"));
    }
    let step = 360.0 / f64::from(count);
    let copies: Vec<Solid> = (0..count).map(|i| solid.rotate([0.0, 0.0, step * f64::from(i)])).collect();
    Ok(Solid::union_all(&copies))
}
