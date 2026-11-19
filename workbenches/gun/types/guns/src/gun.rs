//! A gun from its parts, a gun from one seed, and the views of a gun.

use geometry::{Frame, Item, Place, Scene, Seed, Solid, Sockets};

use crate::{Choice, Gun, Kind, Maker, Part, Pick, Rarity, Stats, part};

/// Puts each part on the socket of the receiver with the name of its kind, adds up the stats and
/// applies the rarity. A part with no maker is no part.
pub fn assemble(parts: &[&Part], rarity: Rarity) -> Result<Gun, String> {
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
    let mut stats = Stats::default();
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
        placed.push(Part { kind: p.kind, maker: p.maker, body: p.body.place(&frame), accent: p.accent.place(&frame), sockets, stats: p.stats });
    }
    let socket = |kind: Kind, name: &str| placed.iter().find(|p| p.kind == kind).and_then(|p| p.sockets.get(name).copied());
    let mut sockets = Sockets::default();
    let muzzle = socket(Kind::Barrel, "muzzle").or_else(|| socket(Kind::Receiver, "barrel"));
    let grip = socket(Kind::Grip, "hand").or_else(|| socket(Kind::Receiver, "grip"));
    for (name, frame) in [("muzzle", muzzle), ("grip", grip)] {
        sockets.set(name, frame.unwrap_or_default());
    }
    let level = rarity.level();
    let stats = Stats {
        damage: stats.damage * (1.0 + 0.12 * level),
        rate: (stats.rate * (1.0 + 0.04 * level)).max(0.5),
        accuracy: (stats.accuracy + 3.0 * level).clamp(0.0, 100.0),
        magazine: (stats.magazine * (1.0 + 0.1 * level)).round().max(1.0),
        reload: (stats.reload * (1.0 - 0.05 * level)).max(0.3),
    };
    Ok(Gun { maker: receiver.maker.unwrap_or_default(), rarity, parts: placed, stats, sockets })
}

/// The frame `local` of a part, after the part moves to `frame`.
fn moved(frame: &Frame, local: &Frame) -> Frame {
    let turn = |v: [f64; 3]| std::array::from_fn(|i| frame.x[i] * v[0] + frame.y[i] * v[1] + frame.z[i] * v[2]);
    let at: [f64; 3] = turn(local.origin);
    Frame { x: turn(local.x), y: turn(local.y), z: turn(local.z), origin: std::array::from_fn(|i| frame.origin[i] + at[i]) }
}

/// The maker and the rarity of a gun, and the kind and the maker of each part.
pub(crate) type Plan = (Maker, Rarity, Vec<(Kind, Maker)>);

/// The plan of a gun from `seed`. A higher rarity adds a stock, a sight and an accessory more
/// frequently.
pub(crate) fn plan(seed: Seed, maker: Pick<Maker>, rarity: Pick<Rarity>) -> Result<Plan, String> {
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
    let mut parts = Vec::new();
    for &kind in Kind::ALL {
        // Each kind uses the same count of random numbers, so a change of one kind keeps the others.
        let (keep, same, other) = (r.unit(), r.unit(), r.index(Maker::ALL.len() - 1));
        let chance = match kind {
            Kind::Stock => 0.4 + 0.12 * level,
            Kind::Sight => 0.3 + 0.15 * level,
            Kind::Accessory => 0.15 + 0.2 * level,
            _ => 1.0,
        };
        if keep >= chance {
            continue;
        }
        let part_maker =
            if kind == Kind::Receiver || same < 0.8 { maker } else { Maker::ALL.iter().copied().filter(|&m| m != maker).nth(other).unwrap_or(maker) };
        parts.push((kind, part_maker));
    }
    Ok((maker, rarity, parts))
}

/// A full gun from one seed: a sub-seed for each part, and a weighted pick of a random rarity.
pub fn generate(seed: Seed, maker: Pick<Maker>, rarity: Pick<Rarity>) -> Result<Gun, String> {
    let (_, rarity, kinds) = plan(seed, maker, rarity)?;
    let parts: Vec<Part> = kinds.iter().map(|&(kind, maker)| part(kind, seed.child(kind.name()), Pick::Of(maker))).collect();
    assemble(&parts.iter().collect::<Vec<_>>(), rarity)
}

/// The gun as a scene: each body in the color of its maker and each accent in the color of the rarity.
pub fn look(gun: &Gun) -> Scene {
    let places = vec![Place::new(&Frame::default(), 0)];
    Scene::new(gun.parts.iter().flat_map(|p| {
        let maker = p.maker.unwrap_or_default();
        let item = |what: &str, solid: &Solid, color: u32| Item { name: format!("{} {what}", p.kind.name()), mesh: solid.mesh(), color, places: places.clone() };
        [item(maker.name(), &p.body, maker.color()), item("accent", &p.accent, gun.rarity.color())]
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

/// The side view of the gun as seen from -Y: the bodies from the back to the front, then the accents.
pub fn side_view(gun: &Gun) -> Vec<Outline> {
    const ORDER: [Kind; 7] = [Kind::Stock, Kind::Grip, Kind::Magazine, Kind::Barrel, Kind::Accessory, Kind::Receiver, Kind::Sight];
    let side = |solid: &Solid, color: u32| Outline { color, outlines: solid.rotate([-90.0, 0.0, 0.0]).project().outlines() };
    let ordered = || ORDER.iter().filter_map(|&kind| gun.parts.iter().find(|p| p.kind == kind));
    let bodies = ordered().map(|p| side(&p.body, p.maker.unwrap_or_default().color()));
    let accents = ordered().map(|p| side(&p.accent, gun.rarity.color()));
    bodies.chain(accents).filter(|o| !o.outlines.is_empty()).collect()
}
