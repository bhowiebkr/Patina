use city::Buildings;
use geometry::Scene;
use patina_sdk::node;

/// The parts on the walls of each building: windows with glass and a frame in each bay of each
/// floor, an entrance with a canopy, shop fronts with awnings and a sign band, loading doors,
/// balconies with railings and pilasters. Each part is one mesh at many places.
#[node(color = "#5a4a3c")]
pub fn facades(buildings: &Buildings) -> Result<Scene, String> {
    city::facades(buildings)
}
