use city::Buildings;
use geometry::{Scene, Seed};
use patina_sdk::node;

/// The roof of each building: a gable or hip roof with an overhang, a flat roof with gravel,
/// or a sawtooth roof with glazing. Chimneys, rooftop units, water tanks, stair houses and
/// antennas stand on the roofs.
#[node(color = "#5a4a3c")]
pub fn roofs(buildings: &Buildings, seed: &Seed) -> Result<Scene, String> {
    city::roofs(buildings, *seed)
}
