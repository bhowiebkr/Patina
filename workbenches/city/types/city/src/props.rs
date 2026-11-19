//! Props along the plot edges (task E5) and colors as text.

use geometry::{Frame, Item, Place, Scene, Seed, Solid};

use crate::plots::offset;
use crate::{Plots, Terrain, add, length, scale, sub};

/// The most places in one scene, so that a wrong value cannot fill the memory.
pub const MAX_PLACES: usize = 1_000_000;

/// Copies of the solid along the edges of each plot, `inset` in from the edges and `spacing`
/// apart, on the terrain. Each copy has a random turn and size.
pub fn scatter(plots: &Plots, terrain: &Terrain, solid: &Solid, seed: Seed, spacing: f64, inset: f64, color: u32) -> Result<Scene, String> {
    if !(spacing.is_finite() && spacing >= 0.5 && inset.is_finite() && inset >= 0.0) {
        return Err("the spacing must be 0.5 or more and the inset 0 or more".into());
    }
    let mut places = Vec::new();
    for (i, plot) in plots.0.iter().enumerate() {
        let Some((corners, _)) = offset(&plot.corners, &vec![inset; plot.corners.len()]) else { continue };
        let mut random = seed.child(&format!("scatter {i}")).random();
        let n = corners.len();
        for e in 0..n {
            let wall = sub(corners[(e + 1) % n], corners[e]);
            let count = (length(wall) / spacing).floor() as usize;
            if places.len() + count > MAX_PLACES {
                return Err(format!("scatter makes more than {MAX_PLACES} copies"));
            }
            for k in 0..count {
                let p = add(corners[e], scale(wall, (k as f64 + 0.5) / count as f64));
                if terrain.contains(p) && terrain.water(p) {
                    continue;
                }
                let (angle, size) = (random.range(0.0, 360.0), random.range(0.8, 1.2));
                let turned = Frame::turned([p[0], p[1], terrain.height(p)], [0.0, 0.0, angle]);
                let frame = Frame { x: turned.x.map(|c| c * size), y: turned.y.map(|c| c * size), z: [0.0, 0.0, size], ..turned };
                places.push(Place::new(&frame, 0));
            }
        }
    }
    Ok(Scene::new([Item { name: "scatter".into(), mesh: solid.mesh(), color, places }]))
}

/// The color of a text such as "#4f7a3a" or "4f7a3a", as sRGB 0xRRGGBB.
pub fn color(text: &str) -> Result<u32, String> {
    let hex = text.trim().trim_start_matches('#');
    u32::from_str_radix(hex, 16).ok().filter(|_| hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit())).ok_or_else(|| format!("write the color as #rrggbb, not {text:?}"))
}
