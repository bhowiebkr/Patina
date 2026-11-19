//! The walls and the trim of each building (task E12): the bodies of the tiers, the gable ends,
//! a plinth down to the ground, floor bands, a cornice and a parapet.

use geometry::{IDENTITY, Item, Mesh, Place, Scene};

use crate::buildings::{Buildings, Kind, Roof};
use crate::shapes::{band, grow, parapet, sides, tri};
use crate::{add, scale};

/// The height of the parapet on a flat roof, and its thickness.
pub(crate) const PARAPET: f64 = 0.9;
pub(crate) const PARAPET_IN: f64 = 0.25;

/// Two items for each building: "walls {group}" in the wall color and "trim {group}" in the
/// trim color.
pub fn walls(buildings: &Buildings) -> Scene {
    let mut items = Vec::new();
    for b in &buildings.0 {
        let (mut wall, mut trim) = (Mesh::default(), Mesh::default());
        let g = b.ground;
        if let Some(first) = b.tiers.first() {
            band(&mut trim, &first.corners, &grow(&first.corners, 0.12), b.low - 0.5, g + 0.3);
        }
        for tier in &b.tiers {
            let (c, top) = (&tier.corners, g + tier.top);
            sides(&mut wall, c, g + tier.bottom, top, true);
            if tier.roof == Roof::Gable && c.len() == 4 {
                let k = tier.ridge();
                for e in [(k + 1) % 4, (k + 3) % 4] {
                    let (p, q) = (c[e], c[(e + 1) % 4]);
                    let m = scale(add(p, q), 0.5);
                    tri(&mut wall, [[p[0], p[1], top], [q[0], q[1], top], [m[0], m[1], top + tier.rise]]);
                }
            }
            if matches!(b.kind, Kind::Apartment | Kind::Shop | Kind::Tower) {
                let out = grow(c, 0.06);
                for (_, z) in b.floors(tier).skip(1) {
                    band(&mut trim, c, &out, g + z - 0.11, g + z + 0.11);
                }
            }
            if tier.roof == Roof::Flat || tier.roof == Roof::Sawtooth {
                let high = if b.kind == Kind::Tower { 0.6 } else { PARAPET };
                if matches!(b.kind, Kind::Apartment | Kind::Shop) {
                    band(&mut trim, c, &grow(c, 0.3), top - 0.45, top);
                }
                parapet(&mut trim, &grow(c, 0.02), &grow(c, -PARAPET_IN), top, top + high);
            }
        }
        let places = vec![Place { matrix: IDENTITY, group: b.group }];
        items.push(Item { name: format!("walls {}", b.group), mesh: wall, color: b.style.wall, places: places.clone() });
        items.push(Item { name: format!("trim {}", b.group), mesh: trim, color: b.style.trim, places });
    }
    Scene::new(items)
}
