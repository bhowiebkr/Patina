use geometry::{MAX_COPIES, Solid};
use patina_sdk::node;

/// `count` copies of the solid on a line. Each copy moves by `x y z` from the copy before it.
#[node]
pub fn linear_array(solid: &Solid, count: u32, x: f64, y: f64, z: f64) -> Result<Solid, String> {
    if count > MAX_COPIES {
        return Err(format!("make {MAX_COPIES} copies or fewer, not {count}"));
    }
    let copies: Vec<Solid> = (0..count).map(f64::from).map(|i| solid.translate([x * i, y * i, z * i])).collect();
    Ok(Solid::union_all(&copies))
}
