//! The parts on the walls of each building (task E12): windows, doors, shop fronts, loading
//! doors, balconies and pilasters. Each part is one mesh at many places.

use std::collections::BTreeMap;

use geometry::{Item, Mesh, Scene};

use crate::buildings::{Building, Buildings, Kind, Tier, main_edge};
use crate::shapes::{cuboid, place, slab};
use crate::{MAX_PLACES, add, length, scale, sub};

const GLASS: u32 = 0x2e4152;
const SHOP_GLASS: u32 = 0x47657a;
const DOOR: u32 = 0x4a3426;
const METAL: u32 = 0x3c3f42;
const CONCRETE: u32 = 0xa19d94;
const ROLLER: u32 = 0x8d9194;

/// The width and the height of the three window kinds: tall, wide and band.
const WINDOWS: [[f64; 2]; 3] = [[1.0, 1.6], [1.6, 1.5], [2.6, 2.2]];

/// The parts. A glass and a frame for each window kind come first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    Glass(usize),
    Frame(usize),
    Door,
    DoorFrame,
    Canopy,
    ShopBase,
    ShopGlass,
    Awning,
    Sign,
    Loading,
    Balcony,
    Railing,
    Pilaster,
}

/// The places of each part and color.
type Places = BTreeMap<(Part, u32), Vec<geometry::Place>>;

/// The windows, doors and other parts on the walls of the buildings. A wall has a bay of the
/// width of the style and a window in each bay on each floor. The street edges of the ground
/// floor have the doors, the shop fronts and the loading doors.
pub fn facades(buildings: &Buildings) -> Result<Scene, String> {
    let mut parts = Places::new();
    for b in &buildings.0 {
        for (ti, tier) in b.tiers.iter().enumerate() {
            walls_of(b, ti, tier, &mut parts);
        }
        if parts.values().map(Vec::len).sum::<usize>() > MAX_PLACES {
            return Err(format!("the facades have more than {MAX_PLACES} parts"));
        }
    }
    let items = parts.into_iter().map(|((part, color), places)| Item { name: name(part, color), mesh: mesh(part), color, places });
    Ok(Scene::new(items))
}

fn walls_of(b: &Building, ti: usize, tier: &Tier, parts: &mut Places) {
    let (n, style, group) = (tier.corners.len(), b.style, b.group);
    let entrance = (ti == 0).then(|| main_edge(&tier.corners, &tier.fronts));
    let wh = WINDOWS[style.window.min(2)][1];
    let mut put = |part: Part, color: u32, p: [f64; 2], out: [f64; 2], z: f64, size: [f64; 3]| {
        parts.entry((part, color)).or_default().push(place([p[0], p[1], b.ground + z], out, size, group));
    };
    for e in 0..n {
        let (a, c) = (tier.corners[e], tier.corners[(e + 1) % n]);
        let l = length(sub(c, a));
        if l < 1.5 {
            continue;
        }
        let t = scale(sub(c, a), 1.0 / l);
        let out = [t[1], -t[0]];
        let front = tier.fronts.get(e) == Some(&true);
        if matches!(b.kind, Kind::House | Kind::Apartment | Kind::Shop) && !b.covers(ti, add(a, scale(out, 0.3)), tier.bottom + 1.0) {
            put(Part::Pilaster, style.trim, a, out, tier.bottom, [0.4, 0.1, tier.top - tier.bottom]);
        }
        let bays = ((l - 1.0) / style.bay).floor().max(1.0) as usize;
        let w = l / bays as f64;
        if front && tier.bottom == 0.0 && b.kind == Kind::Shop {
            put(Part::Sign, style.accent, add(a, scale(t, l / 2.0)), out, b.first - 1.05, [l - 0.3, 0.12, 0.7]);
        }
        for (k, z) in b.floors(tier) {
            let h = b.level(k + 1).min(tier.top) - z;
            for j in 0..bays {
                let p = add(a, scale(t, (j as f64 + 0.5) * w));
                if b.covers(ti, add(p, scale(out, 0.5)), z + 1.0) {
                    continue;
                }
                let door = k == 0 && entrance == Some(e) && j == bays / 2;
                let ground = k == 0 && front;
                match b.kind {
                    _ if door => {
                        put(Part::Door, DOOR, p, out, 0.0, [1.0; 3]);
                        put(Part::DoorFrame, style.trim, p, out, 0.0, [1.0; 3]);
                        let wide = if b.kind == Kind::Tower { 2.2 } else { 1.0 };
                        put(Part::Canopy, METAL, p, out, 0.0, [wide, wide, 1.0]);
                    }
                    Kind::Shop | Kind::Tower if ground => {
                        let top = b.first - if b.kind == Kind::Shop { 1.3 } else { 0.8 };
                        put(Part::ShopBase, METAL, p, out, 0.0, [w - 0.2, 0.1, 0.4]);
                        put(Part::ShopGlass, SHOP_GLASS, p, out, 0.4, [w - 0.4, 0.04, top - 0.4]);
                        if b.kind == Kind::Shop {
                            put(Part::Awning, style.accent, p, out, top - 0.65, [w - 0.3, 1.3, 0.9]);
                        }
                    }
                    Kind::Warehouse if ground && j % 2 == 1 => put(Part::Loading, ROLLER, p, out, 0.0, [3.6, 0.06, 4.2]),
                    _ if h > wh + 0.4 => {
                        let sill = z + 0.6 * (h - wh);
                        put(Part::Glass(style.window), GLASS, p, out, sill, [1.0; 3]);
                        put(Part::Frame(style.window), style.trim, p, out, sill, [1.0; 3]);
                        if b.kind == Kind::Apartment && front && k > 0 && j % 2 == 1 {
                            put(Part::Balcony, CONCRETE, p, out, z, [w * 0.8, 1.2, 0.15]);
                            put(Part::Railing, METAL, p, out, z, [w * 0.8, 1.2, 1.0]);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn name(part: Part, color: u32) -> String {
    let base = match part {
        Part::Glass(k) => format!("glass {k}"),
        Part::Frame(k) => format!("window frame {k}"),
        Part::Door => "door".into(),
        Part::DoorFrame => "door frame".into(),
        Part::Canopy => "canopy".into(),
        Part::ShopBase => "shop base".into(),
        Part::ShopGlass => "shop window".into(),
        Part::Awning => "awning".into(),
        Part::Sign => "sign band".into(),
        Part::Loading => "loading door".into(),
        Part::Balcony => "balcony".into(),
        Part::Railing => "railing".into(),
        Part::Pilaster => "pilaster".into(),
    };
    format!("{base} #{color:06x}")
}

/// The mesh of a part with the wall along X, the outside at +Y and the base at z = 0. The
/// parts at a scaled place are boxes of 1 m.
fn mesh(part: Part) -> Mesh {
    let mut m = Mesh::default();
    match part {
        Part::Glass(k) => {
            let [w, h] = WINDOWS[k.min(2)];
            cuboid(&mut m, [-w / 2.0, 0.0, 0.0], [w / 2.0, 0.03, h]);
        }
        Part::Frame(k) => {
            let [w, h] = WINDOWS[k.min(2)];
            frame(&mut m, w, h);
            cuboid(&mut m, [-w / 2.0 - 0.1, 0.0, -0.08], [w / 2.0 + 0.1, 0.16, 0.0]);
        }
        Part::Door => cuboid(&mut m, [-0.55, 0.0, 0.0], [0.55, 0.04, 2.2]),
        Part::DoorFrame => frame(&mut m, 1.1, 2.2),
        Part::Canopy => cuboid(&mut m, [-0.9, 0.0, 2.45], [0.9, 1.1, 2.6]),
        Part::Awning => slab(&mut m, [[-0.5, 0.0, 1.0], [0.5, 0.0, 1.0], [0.5, 1.0, 0.45], [-0.5, 1.0, 0.45]], 0.05),
        Part::Railing => {
            cuboid(&mut m, [-0.5, 0.95, 0.0], [0.5, 1.0, 1.0]);
            cuboid(&mut m, [-0.5, 0.0, 0.0], [-0.47, 0.95, 1.0]);
            cuboid(&mut m, [0.47, 0.0, 0.0], [0.5, 0.95, 1.0]);
        }
        Part::Balcony => cuboid(&mut m, [-0.5, 0.0, -1.0], [0.5, 1.0, 0.0]),
        _ => cuboid(&mut m, [-0.5, 0.0, 0.0], [0.5, 1.0, 1.0]),
    }
    m
}

/// The two jambs and the lintel around an opening of `w` x `h`.
fn frame(m: &mut Mesh, w: f64, h: f64) {
    let x = w / 2.0;
    cuboid(m, [-x - 0.08, 0.0, 0.0], [-x, 0.08, h]);
    cuboid(m, [x, 0.0, 0.0], [x + 0.08, 0.08, h]);
    cuboid(m, [-x - 0.14, 0.0, h], [x + 0.14, 0.12, h + 0.16]);
}
