use city::Buildings;
use geometry::Scene;
use patina_sdk::node;

/// The walls of each building: the body of each tier, the gable ends, a plinth down to the
/// ground, floor bands, a cornice and a parapet. Each building is a group for
/// `export-buildings`.
#[node(color = "#5a4a3c")]
pub fn walls(buildings: &Buildings) -> Scene {
    city::walls(buildings)
}
