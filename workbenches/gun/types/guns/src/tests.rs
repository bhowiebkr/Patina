use geometry::Seed;
use patina_sdk::Param;

use crate::gun::plan;
use crate::{Choice, Class, Element, Gun, Kind, Maker, Part, Pick, Rarity, assemble, generate, look, part, side_view};

fn gun(seed: u64) -> Gun {
    generate(Seed(seed), Pick::Random, Pick::Random, Pick::Random, Pick::Random).unwrap()
}

fn finite(bounds: [[f64; 3]; 2]) -> bool {
    bounds.iter().flatten().all(|v| v.is_finite() && v.abs() < 1.0)
}

#[test]
fn the_same_seed_gives_the_same_gun() {
    let (a, b) = (gun(7), gun(7));
    assert_eq!(a.solid().mesh().hash(), b.solid().mesh().hash());
    assert_eq!(a.report(), b.report());
    assert!((1..6).any(|s| gun(s).solid().mesh().hash() != a.solid().mesh().hash()));
}

#[test]
fn each_maker_makes_each_kind_of_each_class() {
    for &maker in Maker::ALL {
        for &class in Class::ALL {
            for &kind in Kind::ALL {
                let seed = Seed(class as u64 * 7 + maker as u64);
                let p = part(kind, seed, Pick::Of(maker), Pick::Of(class));
                let what = format!("{p:?} for a {} from {seed:?}", class.name());
                assert_eq!((p.maker, p.class), (Some(maker), class), "{what}");
                assert!(p.triangles() > 0, "{what}");
                for solid in &p.solids {
                    assert_eq!(solid.error(), None, "{what}");
                    assert!(solid.triangles() == 0 || finite(solid.bounds()), "{what}");
                }
            }
        }
    }
    assert_eq!(format!("{:?}", part(Kind::Stock, Seed(1), Pick::None, Pick::Random)), "no stock");
}

#[test]
fn a_gun_has_a_name_and_a_budget_of_triangles() {
    for seed in 0..12 {
        let gun = gun(seed);
        let words: Vec<&str> = gun.name.split(' ').collect();
        assert!(!words.is_empty() && words.len() <= 2, "{gun:?}");
        let triangles: usize = gun.parts.iter().map(Part::triangles).sum();
        assert!((1000..60_000).contains(&triangles), "{triangles} triangles in {gun:?}");
        let scene = look(&gun);
        assert!(scene.items().len() > gun.parts.len(), "{gun:?}");
    }
}

#[test]
fn the_barrel_is_on_the_socket_of_the_receiver() {
    for &maker in Maker::ALL {
        let class = Pick::Of(Class::Rifle);
        let receiver = part(Kind::Receiver, Seed(3), Pick::Of(maker), class);
        let barrel = part(Kind::Barrel, Seed(3), Pick::Of(maker), class);
        let gun = assemble(&[&receiver, &barrel], Rarity::Common, Element::None).unwrap();
        let socket = receiver.sockets.get("barrel").unwrap().origin;
        let muzzle = gun.sockets.get("muzzle").unwrap().origin;
        let placed = gun.parts.iter().find(|p| p.kind == Kind::Barrel).unwrap();
        let [_, high] = placed.solid().bounds();
        assert!(muzzle[0] > socket[0] + 0.1 && (muzzle[2] - socket[2]).abs() < 1e-9, "{maker:?}: {muzzle:?} {socket:?}");
        assert!((high[0] - muzzle[0]).abs() < 0.03, "{maker:?}: {high:?} {muzzle:?}");
        assert_eq!(gun.sockets.get("grip").unwrap().origin, receiver.sockets.get("grip").unwrap().origin);
    }
}

#[test]
fn a_higher_rarity_gives_better_stats() {
    let parts: Vec<Part> = Kind::ALL.iter().map(|&kind| part(kind, Seed(9), Pick::Of(Maker::Halo), Pick::Of(Class::Smg))).collect();
    let refs: Vec<&Part> = parts.iter().collect();
    let common = assemble(&refs, Rarity::Common, Element::None).unwrap().stats;
    let legendary = assemble(&refs, Rarity::Legendary, Element::None).unwrap().stats;
    assert!(legendary.damage > common.damage && legendary.accuracy > common.accuracy && legendary.reload < common.reload);
    assert!(legendary.magazine >= common.magazine);
    let fire = assemble(&refs, Rarity::Common, Element::Fire).unwrap();
    assert!(fire.stats.damage < common.damage && fire.name.starts_with("Searing "), "{fire:?}");
}

#[test]
fn a_random_rarity_follows_the_weights() {
    let mut count = [0; 5];
    let mut extras = [0; 5];
    let mut elements = [0; 5];
    for seed in 0..4000 {
        let plan = plan(Seed(seed), Pick::Random, Pick::Random, Pick::Random, Pick::Random).unwrap();
        let r = plan.rarity as usize;
        count[r] += 1;
        extras[r] += plan.parts.len() - 4;
        elements[r] += usize::from(plan.element != Element::None);
        assert!(plan.parts.iter().filter(|(kind, _)| [Kind::Receiver, Kind::Barrel, Kind::Grip, Kind::Magazine].contains(kind)).count() == 4);
        assert_eq!(plan.parts[0].0, Kind::Receiver);
        assert_eq!(plan.class, Class::of(Seed(seed)));
    }
    assert!(count.windows(2).all(|w| w[0] > w[1]), "{count:?}");
    // The mean count of extra parts and the chance of an element go up with the rarity.
    let mean = |list: [usize; 5], r: usize| list[r] as f64 / count[r] as f64;
    assert!(mean(extras, 4) > mean(extras, 0) + 0.5, "{extras:?} {count:?}");
    assert!(mean(elements, 4) > mean(elements, 0) + 0.3, "{elements:?} {count:?}");
    let plan = plan(Seed(2), Pick::Of(Class::Sniper), Pick::Of(Maker::Vesper), Pick::Of(Rarity::Epic), Pick::None).unwrap();
    assert_eq!((plan.class, plan.parts[0].1, plan.element), (Class::Sniper, Maker::Vesper, Element::None));
    assert!(plan.parts.iter().any(|(kind, _)| *kind == Kind::Sight), "a sniper rifle always has a sight");
}

#[test]
fn a_pick_is_a_text() {
    for text in ["random", "none", "vesper", "legendary"] {
        let back = if text == "legendary" { Pick::<Rarity>::from_text(text).unwrap().to_text() } else { Pick::<Maker>::from_text(text).unwrap().to_text() };
        assert_eq!(back, text);
    }
    assert_eq!(Pick::<Maker>::from_text(" Halo "), Ok(Pick::Of(Maker::Halo)));
    assert_eq!(Pick::<Class>::from_text("SMG"), Ok(Pick::Of(Class::Smg)));
    assert_eq!(Element::from_text("cryo"), Ok(Element::Cryo));
    let error = Pick::<Maker>::from_text("acme").unwrap_err();
    assert!(error.contains("blockwerk, vesper, scrapjaw, halo"), "{error}");
}

#[test]
fn the_side_view_has_outlines() {
    let gun = gun(11);
    let view = side_view(&gun);
    assert!(view.len() >= gun.parts.len(), "{} outlines for {} parts", view.len(), gun.parts.len());
    let points = view.iter().flat_map(|o| &o.outlines).flatten();
    assert!(points.clone().count() > 20 && points.clone().all(|p| p.iter().all(|v| v.is_finite() && v.abs() < 1.5)));
    // The side view is on X and Z: the muzzle is near the far right.
    let right = points.map(|p| p[0]).fold(f64::MIN, f64::max);
    assert!(right >= gun.sockets.get("muzzle").unwrap().origin[0] - 0.01);
}

#[test]
fn bad_inputs_give_errors() {
    assert!(generate(Seed(1), Pick::None, Pick::Random, Pick::Random, Pick::Random).unwrap_err().contains("class"));
    assert!(generate(Seed(1), Pick::Random, Pick::None, Pick::Random, Pick::Random).unwrap_err().contains("maker"));
    assert!(generate(Seed(1), Pick::Random, Pick::Random, Pick::None, Pick::Random).unwrap_err().contains("rarity"));
    let barrel = part(Kind::Barrel, Seed(1), Pick::Of(Maker::Vesper), Pick::Random);
    assert_eq!(assemble(&[&barrel], Rarity::Rare, Element::None).unwrap_err(), "The gun needs a receiver");
    let receiver = part(Kind::Receiver, Seed(1), Pick::Of(Maker::Vesper), Pick::Random);
    assert!(assemble(&[&receiver, &barrel, &barrel], Rarity::Rare, Element::None).unwrap_err().contains("two parts"));
    let none = part(Kind::Receiver, Seed(1), Pick::None, Pick::Random);
    assert!(assemble(&[&none, &barrel], Rarity::Rare, Element::None).is_err());
}

#[test]
fn the_stats_stay_in_range() {
    for seed in 20..32 {
        let gun = gun(seed);
        let s = gun.stats;
        assert!(s.damage > 0.0 && s.rate >= 0.5 && (0.0..=100.0).contains(&s.accuracy) && s.magazine >= 1.0 && s.reload >= 0.3, "{gun:?}");
        assert!(gun.sockets.get("muzzle").is_some() && gun.sockets.get("grip").is_some());
        assert!(gun.report().starts_with(&gun.name) && gun.report().lines().count() == 9, "{}", gun.report());
        assert_eq!(gun.solid().error(), None);
    }
}
