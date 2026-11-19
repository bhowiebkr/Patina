//! A gun from its parts, a gun from one seed, and the views of a gun.

use geometry::{Frame, Item, Place, Scene, Seed, Solid, Sockets};

use crate::{Choice, Class, Element, Gun, Kind, Maker, Material, Part, Pick, Rarity, Stats, part};

/// Puts each part on the socket of the receiver with the name of its kind, adds up the stats,
/// applies the rarity and the element, and gives the gun a name. A part with no maker is no part.
pub fn assemble(parts: &[&Part], rarity: Rarity, element: Element) -> Result<Gun, String> {
    let parts: Vec<&Part> = parts.iter().copied().filter(|p| p.maker.is_some()).collect();
    for (i, p) in parts.iter().enumerate() {
        if parts[..i].iter().any(|q| q.kind == p.kind) {
            return Err(format!("The gun has two parts of the kind {}", p.kind.name()));
        }
    }
    let Some(receiver) = parts.iter().find(|p| p.kind == Kind::Receiver) else {
        return Err("The gun needs a receiver".into());
    };
    let mut placed = Vec::with_capacity(parts.len());
    let (mut stats, mut extra) = (Stats::default(), Stats::default());
    for p in &parts {
        let frame = match p.kind {
            Kind::Receiver => Frame::default(),
            kind => *receiver.sockets.get(kind.name()).ok_or_else(|| format!("The receiver has no {} socket", kind.name()))?,
        };
        let mut sockets = Sockets::default();
        for (name, local) in p.sockets.iter() {
            sockets.set(name, moved(&frame, local));
        }
        stats = stats + p.stats;
        if p.kind != Kind::Receiver {
            extra = extra + p.stats;
        }
        let solids = p.solids.each_ref().map(|s| s.place(&frame));
        placed.push(Part { kind: p.kind, maker: p.maker, class: p.class, scheme: p.scheme, solids, sockets, stats: p.stats });
    }
    let socket = |kind: Kind, name: &str| placed.iter().find(|p| p.kind == kind).and_then(|p| p.sockets.get(name).copied());
    let mut sockets = Sockets::default();
    let muzzle = socket(Kind::Barrel, "muzzle").or_else(|| socket(Kind::Receiver, "barrel"));
    let grip = socket(Kind::Grip, "hand").or_else(|| socket(Kind::Receiver, "grip"));
    for (name, frame) in [("muzzle", muzzle), ("grip", grip)] {
        sockets.set(name, frame.unwrap_or_default());
    }
    let level = rarity.level();
    let hit = if element == Element::None { 1.0 } else { 0.85 };
    let stats = Stats {
        damage: stats.damage * (1.0 + 0.12 * level) * hit,
        rate: (stats.rate * (1.0 + 0.04 * level)).max(0.5),
        accuracy: (stats.accuracy + 3.0 * level).clamp(0.0, 100.0),
        magazine: (stats.magazine * (1.0 + 0.1 * level)).round().max(1.0),
        reload: (stats.reload * (1.0 - 0.05 * level)).max(0.3),
    };
    let (class, maker) = (receiver.class, receiver.maker.unwrap_or_default());
    let name = name(element, extra, maker, class);
    Ok(Gun { name, class, maker, rarity, element, parts: placed, stats, sockets })
}

/// The name of a gun: the element word or the word of the largest part bonus, and the model of
/// the maker for the class, for example "Steady Kestrel".
fn name(element: Element, extra: Stats, maker: Maker, class: Class) -> String {
    const MODELS: [[&str; 6]; 4] = [
        ["Mallet", "Rivet", "Girder", "Wrecker", "Plumbline", "Demolisher"],
        ["Wren", "Swift", "Kestrel", "Gale", "Heron", "Comet"],
        ["Shiv", "Rattler", "Junker", "Gutbuster", "Longshank", "Tinbomb"],
        ["Spark", "Pulse", "Arc", "Nova", "Lance", "Quasar"],
    ];
    let model = MODELS[maker as usize][class as usize];
    let base = class.base();
    // Each bonus is a part of the base stat, less the part that a typical set of parts gives.
    let bonuses = [
        ("Brutal", extra.damage / base.damage - 0.1),
        ("Rapid", extra.rate / base.rate - 0.02),
        ("Steady", extra.accuracy / (100.0 - base.accuracy) - 0.6),
        ("Hoarding", extra.magazine / base.magazine - 0.3),
        ("Nimble", -extra.reload / base.reload + 0.05),
    ];
    let best = bonuses.iter().copied().filter(|&(_, ratio)| ratio > 0.0).max_by(|a, b| a.1.total_cmp(&b.1)).map(|(word, _)| word);
    match element.prefix().or(best) {
        Some(word) => format!("{word} {model}"),
        None => model.into(),
    }
}

/// The frame `local` of a part, after the part moves to `frame`.
fn moved(frame: &Frame, local: &Frame) -> Frame {
    let turn = |v: [f64; 3]| std::array::from_fn(|i| frame.x[i] * v[0] + frame.y[i] * v[1] + frame.z[i] * v[2]);
    let at: [f64; 3] = turn(local.origin);
    Frame { x: turn(local.x), y: turn(local.y), z: turn(local.z), origin: std::array::from_fn(|i| frame.origin[i] + at[i]) }
}

/// The plan of a gun: the class, the rarity, the element, and the kind and the maker of each
/// part. The first part is the receiver, with the maker of the gun.
pub(crate) struct Plan {
    pub class: Class,
    pub rarity: Rarity,
    pub element: Element,
    pub parts: Vec<(Kind, Maker)>,
}

/// The plan of a gun from `seed`. A higher rarity adds an element and the extra parts more
/// frequently. The class comes from the same sub-seed as the class of the part nodes.
pub(crate) fn plan(seed: Seed, class: Pick<Class>, maker: Pick<Maker>, rarity: Pick<Rarity>, element: Pick<Element>) -> Result<Plan, String> {
    let class = match class {
        Pick::Of(class) => class,
        Pick::Random => Class::of(seed),
        Pick::None => return Err("The gun needs a class: write random or the name of a class".into()),
    };
    let mut r = seed.child("gun").random();
    let maker = match maker {
        Pick::Of(maker) => maker,
        Pick::Random => Maker::ALL[r.index(Maker::ALL.len())],
        Pick::None => return Err("The gun needs a maker: write random or the name of a maker".into()),
    };
    let rarity = match rarity {
        Pick::Of(rarity) => rarity,
        Pick::Random => Rarity::ALL[r.pick(&Rarity::WEIGHTS)],
        Pick::None => return Err("The gun needs a rarity: write random or the name of a rarity".into()),
    };
    let level = rarity.level();
    let element = match element {
        Pick::Of(element) => element,
        Pick::None => Element::None,
        Pick::Random => {
            let mut e = seed.child("element").random();
            let (roll, which) = (e.unit(), 1 + e.index(Element::ALL.len() - 1));
            if roll < 0.1 + 0.15 * level { Element::ALL[which] } else { Element::None }
        }
    };
    let mut parts = Vec::new();
    for &kind in Kind::ALL {
        // Each kind uses the same count of random numbers, so a change of one kind keeps the others.
        let (keep, same, other) = (r.unit(), r.unit(), r.index(Maker::ALL.len() - 1));
        if keep >= class.chance(kind, level) {
            continue;
        }
        let part_maker =
            if kind == Kind::Receiver || same < 0.8 { maker } else { Maker::ALL.iter().copied().filter(|&m| m != maker).nth(other).unwrap_or(maker) };
        parts.push((kind, part_maker));
    }
    Ok(Plan { class, rarity, element, parts })
}

/// A full gun from one seed. All parts use the same seed, so their sizes agree.
pub fn generate(seed: Seed, class: Pick<Class>, maker: Pick<Maker>, rarity: Pick<Rarity>, element: Pick<Element>) -> Result<Gun, String> {
    let plan = plan(seed, class, maker, rarity, element)?;
    let parts: Vec<Part> = plan.parts.iter().map(|&(kind, maker)| part(kind, seed, Pick::Of(maker), Pick::Of(plan.class))).collect();
    assemble(&parts.iter().collect::<Vec<_>>(), plan.rarity, plan.element)
}

/// The color of one material of a part on `gun`, as sRGB 0xRRGGBB.
fn color(gun: &Gun, p: &Part, material: Material) -> u32 {
    let maker = p.maker.unwrap_or_default();
    let [paint, trim, grip] = maker.scheme(p.scheme);
    match material {
        Material::Paint => paint,
        Material::Trim => trim,
        Material::Grip => grip,
        Material::Metal => maker.metal(),
        Material::Rarity => gun.rarity.color(),
        Material::Glow => gun.element.color().unwrap_or(maker.glow()),
    }
}

/// The gun as a scene: one item for each material of each part, in the color of the material.
pub fn look(gun: &Gun) -> Scene {
    let places = vec![Place::new(&Frame::default(), 0)];
    Scene::new(gun.parts.iter().flat_map(|p| {
        let places = &places;
        Material::ALL.into_iter().zip(&p.solids).filter(|(_, s)| s.triangles() > 0).map(move |(material, solid)| Item {
            name: format!("{} {}", p.kind.name(), material.name()),
            mesh: solid.mesh(),
            color: color(gun, p, material),
            places: places.clone(),
        })
    }))
}

/// The outlines of one solid in the side view, in one color.
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    /// sRGB 0xRRGGBB.
    pub color: u32,
    /// Each outline as points `[x, z]` in meters. An outer outline turns counterclockwise.
    pub outlines: Vec<Vec<[f64; 2]>>,
}

/// The side view of the gun as seen from -Y: the parts from the back to the front, and in each
/// part the materials from the base to the details.
pub fn side_view(gun: &Gun) -> Vec<Outline> {
    const ORDER: [Kind; 7] = [Kind::Stock, Kind::Grip, Kind::Magazine, Kind::Barrel, Kind::Accessory, Kind::Receiver, Kind::Sight];
    const LAYERS: [Material; 6] = [Material::Metal, Material::Grip, Material::Paint, Material::Trim, Material::Rarity, Material::Glow];
    let side = |solid: &Solid, color: u32| Outline { color, outlines: solid.rotate([-90.0, 0.0, 0.0]).project().outlines() };
    ORDER
        .iter()
        .filter_map(|&kind| gun.parts.iter().find(|p| p.kind == kind))
        .flat_map(|p| LAYERS.iter().map(move |&m| side(&p.solids[m as usize], color(gun, p, m))))
        .filter(|o| !o.outlines.is_empty())
        .collect()
}
