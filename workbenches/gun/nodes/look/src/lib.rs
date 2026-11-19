use geometry::Scene;
use guns::Gun;
use patina_sdk::node;

/// The gun as a scene for the 3D view: each body in the color of its maker, and each accent in
/// the color of the rarity.
#[node(color = "#3a3c40")]
pub fn look(gun: &Gun) -> Scene {
    guns::look(gun)
}
