//! Buildings, roads and props as scenes (tasks E4 and E5). A part that repeats is one mesh with
//! many places, so the build makes no boolean for it.

use geometry::{Frame, IDENTITY, Item, Mesh, Place, Scene, Seed, Sketch, Solid};

use crate::{Plots, Streets, add, axes, centroid, dot, extent, length, scale, sub};

/// The most places in one scene, so that a wrong value cannot fill the memory.
pub const MAX_PLACES: usize = 1_000_000;

const WALLS: [u32; 6] = [0xb8a48c, 0x9c8f84, 0xc9c1b4, 0x8a6f5c, 0xa7a9ad, 0x6e7378];
const ROOFS: [u32; 3] = [0x5b4a42, 0x4a4f55, 0x7a4b3a];
const LAWN: u32 = 0x4f7a3a;
const ASPHALT: u32 = 0x3a3c40;
const SIDEWALK: u32 = 0x9a978f;
const GLASS: u32 = 0x2e4152;
const DOOR: u32 = 0x5a3b28;

/// Gives each plot that is not a park a height from a density map: high at the center and low
/// at `radius` and farther, with a random part. The height is in full floors of `floor`.
pub fn heights(plots: &Plots, seed: Seed, low: f64, high: f64, radius: f64, floor: f64) -> Result<Plots, String> {
    if !(0.0..=high).contains(&low) || high > 1000.0 {
        return Err("low and high must be from 0 to 1000, and low not more than high".into());
    }
    if !(radius.is_finite() && radius > 0.0 && floor.is_finite() && floor >= 1.0) {
        return Err("the radius must be more than 0 and the floor 1 or more".into());
    }
    let mut out = plots.clone();
    for (i, plot) in out.0.iter_mut().enumerate() {
        if plot.park {
            (plot.floors, plot.height) = (0, 0.0);
            continue;
        }
        let r = seed.child(&format!("height {i}")).random().unit();
        let d = length(centroid(&plot.corners));
        let h = low + (high - low) * (-(d / radius).powi(2)).exp() * (0.5 + 0.5 * r);
        plot.floors = ((h / floor).round() as u32).max(1);
        plot.height = f64::from(plot.floors) * floor;
    }
    Ok(out)
}

/// The extruded footprint of each building with a flat, gable or hip roof, and a lawn for each
/// park. `pitched` is the part of the roofs with a slope, and `rise` the height of the slope.
/// The group of a building is its plot number plus 1.
pub fn masses(plots: &Plots, seed: Seed, pitched: f64, rise: f64) -> Result<Scene, String> {
    if !(0.0..=1.0).contains(&pitched) || !(rise.is_finite() && rise > 0.0) {
        return Err("pitched must be from 0 to 1 and rise more than 0".into());
    }
    let mut items = Vec::new();
    let mut lawn = Mesh::default();
    for (i, plot) in plots.0.iter().enumerate() {
        let footprint = Sketch::polygon(&plot.corners);
        if plot.park {
            append(&mut lawn, &Solid::extrude(&footprint, 0.2, 0.0, [1.0; 2], 0).mesh());
            continue;
        }
        if plot.floors == 0 {
            continue;
        }
        let group = i as u32 + 1;
        let mut random = seed.child(&format!("mass {i}")).random();
        let color = WALLS[random.index(WALLS.len())];
        let roof_color = ROOFS[random.index(ROOFS.len())];
        let kind = if random.unit() < pitched { 1 + random.index(2) } else { 0 };
        let body = Solid::extrude(&footprint, plot.height, 0.0, [1.0; 2], 0);
        let places = vec![Place { matrix: IDENTITY, group }];
        items.push(Item { name: format!("building {group}"), mesh: body.mesh(), color, places: places.clone() });
        if kind > 0 {
            let mesh = roof(&plot.corners, kind == 1, rise).translate([0.0, 0.0, plot.height]).mesh();
            items.push(Item { name: format!("roof {group}"), mesh, color: roof_color, places });
        }
    }
    items.push(Item { name: "lawns".into(), mesh: lawn, color: LAWN, places: vec![Place { matrix: IDENTITY, group: 0 }] });
    Ok(Scene::new(items))
}

/// A gable or a hip roof on the footprint, from z = 0 to `rise`. The ridge is on the long axis.
fn roof(corners: &[[f64; 2]], gable: bool, rise: f64) -> Solid {
    let (u, v) = axes(corners);
    let ((u0, u1), (v0, v1)) = (extent(corners, u), extent(corners, v));
    let center = add(scale(u, (u0 + u1) / 2.0), scale(v, (v0 + v1) / 2.0));
    let local: Vec<[f64; 2]> = corners.iter().map(|&p| [dot(sub(p, center), u), dot(sub(p, center), v)]).collect();
    let (long, wide) = (u1 - u0, v1 - v0);
    let footprint = Sketch::polygon(&local);
    let shape = if gable {
        let triangle = Sketch::polygon(&[[-wide / 2.0, 0.0], [wide / 2.0, 0.0], [0.0, rise]]);
        // The prism goes along X, across Y and up Z after the two turns.
        let prism = Solid::extrude(&triangle, long + 2.0, 0.0, [1.0; 2], 0).translate([0.0, 0.0, -long / 2.0 - 1.0]);
        prism.rotate([90.0, 0.0, 0.0]).rotate([0.0, 0.0, 90.0]).intersection(&Solid::extrude(&footprint, rise, 0.0, [1.0; 2], 0))
    } else {
        let ridge = ((long - wide) / long.max(1e-9)).max(0.02);
        Solid::extrude(&footprint, rise, 0.0, [ridge, 0.02], 0)
    };
    shape.rotate([0.0, 0.0, u[1].atan2(u[0]).to_degrees()]).translate([center[0], center[1], 0.0])
}

/// Windows on each floor and bay of each wall, and a door on the longest street edge. The
/// window and the door are solids with the wall along X, the outside at +Y and the base at
/// z = 0.
pub fn facades(plots: &Plots, window: &Solid, door: &Solid, bay: f64) -> Result<Scene, String> {
    if !(bay.is_finite() && bay >= 1.0) {
        return Err(format!("the bay must be 1 or more, not {bay}"));
    }
    let (mut windows, mut doors) = (Vec::new(), Vec::new());
    for (i, plot) in plots.0.iter().enumerate().filter(|(_, p)| !p.park && p.floors > 0) {
        let (corners, group, floor) = (&plot.corners, i as u32 + 1, plot.height / f64::from(plot.floors));
        let n = corners.len();
        let wall = |e: usize| sub(corners[(e + 1) % n], corners[e]);
        let door_edge = (0..n).filter(|&e| plot.fronts.get(e) == Some(&true)).max_by(|&a, &b| length(wall(a)).total_cmp(&length(wall(b))));
        for (e, &start) in corners.iter().enumerate() {
            let l = length(wall(e));
            let bays = (l / bay).floor() as usize;
            if bays == 0 {
                continue;
            }
            if windows.len() + bays * plot.floors as usize > MAX_PLACES {
                return Err(format!("the facades have more than {MAX_PLACES} windows"));
            }
            let t = scale(wall(e), 1.0 / l);
            for f in 0..plot.floors {
                for k in 0..bays {
                    let p = add(start, scale(t, (k as f64 + 0.5) * l / bays as f64));
                    let is_door = f == 0 && door_edge == Some(e) && k == bays / 2;
                    let z = if is_door { 0.0 } else { f64::from(f) * floor + 0.9 };
                    let frame = Frame { x: [t[0], t[1], 0.0], y: [t[1], -t[0], 0.0], z: [0.0, 0.0, 1.0], origin: [p[0], p[1], z] };
                    (if is_door { &mut doors } else { &mut windows }).push(Place::new(&frame, group));
                }
            }
        }
    }
    Ok(Scene::new([
        Item { name: "windows".into(), mesh: window.mesh(), color: GLASS, places: windows },
        Item { name: "doors".into(), mesh: door.mesh(), color: DOOR, places: doors },
    ]))
}

/// The asphalt under the streets, with the top at z = 0, and a sidewalk slab on each block.
pub fn roads(streets: &Streets, blocks: &Plots) -> Scene {
    let ([x0, y0], [x1, y1]) = streets.points.iter().fold(([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]), |(lo, hi), p| {
        ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
    });
    let margin = streets.edges.iter().map(|e| e.width).fold(0.0, f64::max);
    let mut asphalt = Mesh::default();
    if x0 <= x1 && y0 <= y1 {
        let ground = Solid::cube(x1 - x0 + margin, y1 - y0 + margin, 0.1).translate([(x0 + x1) / 2.0, (y0 + y1) / 2.0, -0.05]);
        asphalt = ground.mesh();
    }
    let mut sidewalks = Mesh::default();
    for block in &blocks.0 {
        append(&mut sidewalks, &Solid::extrude(&Sketch::polygon(&block.corners), 0.15, 0.0, [1.0; 2], 0).mesh());
    }
    let places = vec![Place { matrix: IDENTITY, group: 0 }];
    Scene::new([
        Item { name: "asphalt".into(), mesh: asphalt, color: ASPHALT, places: places.clone() },
        Item { name: "sidewalks".into(), mesh: sidewalks, color: SIDEWALK, places },
    ])
}

/// Copies of the solid along the edges of each plot, `inset` in from the edges and `spacing`
/// apart, each with a random turn and a random size from 0.8 to 1.2.
pub fn scatter(plots: &Plots, solid: &Solid, seed: Seed, spacing: f64, inset: f64, color: u32) -> Result<Scene, String> {
    if !(spacing.is_finite() && spacing >= 0.5 && inset.is_finite() && inset >= 0.0) {
        return Err("the spacing must be 0.5 or more and the inset 0 or more".into());
    }
    let mut places = Vec::new();
    for (i, plot) in plots.0.iter().enumerate() {
        let Some(corners) = crate::plots::offset(&plot.corners, &vec![inset; plot.corners.len()]) else { continue };
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
                let (angle, size) = (random.range(0.0, 360.0), random.range(0.8, 1.2));
                let turned = Frame::turned([p[0], p[1], 0.0], [0.0, 0.0, angle]);
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

/// Adds the triangles of `mesh` to `to`, all in part 0.
fn append(to: &mut Mesh, mesh: &Mesh) {
    to.positions.extend_from_slice(&mesh.positions);
    to.normals.extend_from_slice(&mesh.normals);
    to.uvs.extend_from_slice(&mesh.uvs);
    to.parts.resize(to.parts.len() + mesh.triangles(), 0);
}
