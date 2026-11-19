use city::Terrain;
use geometry::Scene;
use patina_sdk::node;

/// The terrain as a scene: grass, sand near the water, rock on steep slopes, the bed under the
/// water and the water surface at the sea level.
#[node(color = "#3f4a3a")]
pub fn ground(terrain: &Terrain) -> Scene {
    city::ground(terrain)
}
