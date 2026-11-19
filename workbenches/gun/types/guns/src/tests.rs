use geometry::Seed;
use patina_sdk::Param;

use crate::gun::plan;
use crate::{Choice, Gun, Kind, Maker, Part, Pick, Rarity, assemble, generate, part, side_view};

fn gun(seed: u64) -> Gun {
    generate(Seed(seed), Pick::Random, Pick::Random).unwrap()
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
fn each_maker_makes_each_kind() {
    for &maker in Maker::ALL {
        for &kind in Kind::ALL {
            for seed in 0..3 {
                let p = part(kind, Seed(seed), Pick::Of(maker));
                let what = format!("{p:?} from seed {seed}");
                assert_eq!(p.maker, Some(maker), "{what}");
                for solid in [&p.body, &p.accent] {
                    assert_eq!(solid.error(), None, "{what}");
                    assert!(solid.triangles() > 0, "{what}");
                    assert!(finite(solid.bounds()), "{what}");
                }
            }
        }
    }
    assert_eq!(format!("{:?}", part(Kind::Stock, Seed(1), Pick::None)), "no stock");
}

#[test]
fn the_barrel_is_on_the_socket_of_the_receiver() {
    for &maker in Maker::ALL {
        let receiver = part(Kind::Receiver, Seed(3), Pick::Of(maker));
        let barrel = part(Kind::Barrel, Seed(4), Pick::Of(maker));
        let gun = assemble(&[&receiver, &barrel], Rarity::Common).unwrap();
        let socket = receiver.sockets.get("barrel").unwrap().origin;
        let placed = gun.parts.iter().find(|p| p.kind == Kind::Barrel).unwrap();
        let [low, high] = placed.body.bounds();
        assert!((low[0] - socket[0]).abs() < 1e-6, "{maker:?}: {low:?} {socket:?}");
        let muzzle = gun.sockets.get("muzzle").unwrap().origin;
        assert!((high[0] - muzzle[0]).abs() < 1e-6 && (muzzle[2] - socket[2]).abs() < 1e-9, "{maker:?}: {high:?} {muzzle:?}");
        assert_eq!(gun.sockets.get("grip").unwrap().origin, receiver.sockets.get("grip").unwrap().origin);
    }
}

#[test]
fn a_higher_rarity_gives_better_stats() {
    let parts: Vec<Part> = Kind::ALL.iter().map(|&kind| part(kind, Seed(9), Pick::Of(Maker::Halo))).collect();
    let refs: Vec<&Part> = parts.iter().collect();
    let common = assemble(&refs, Rarity::Common).unwrap().stats;
    let legendary = assemble(&refs, Rarity::Legendary).unwrap().stats;
    assert!(legendary.damage > common.damage && legendary.accuracy > common.accuracy && legendary.reload < common.reload);
    assert!(legendary.magazine >= common.magazine);
}

#[test]
fn a_random_rarity_follows_the_weights() {
    let mut count = [0; 5];
    let mut extras = [0; 5];
    for seed in 0..4000 {
        let (_, rarity, parts) = plan(Seed(seed), Pick::Random, Pick::Random).unwrap();
        count[rarity as usize] += 1;
        extras[rarity as usize] += parts.len() - 4;
        assert!(parts.iter().filter(|(kind, _)| [Kind::Receiver, Kind::Barrel, Kind::Grip, Kind::Magazine].contains(kind)).count() == 4);
        assert_eq!(parts[0], (Kind::Receiver, parts[0].1));
    }
    assert!(count.windows(2).all(|w| w[0] > w[1]), "{count:?}");
    // The mean count of extra parts goes up with the rarity.
    let mean = |r: usize| extras[r] as f64 / count[r] as f64;
    assert!(mean(4) > mean(0) + 1.0, "{extras:?} {count:?}");
    let (maker, _, parts) = plan(Seed(2), Pick::Of(Maker::Vesper), Pick::Of(Rarity::Epic)).unwrap();
    assert_eq!((maker, parts[0].1), (Maker::Vesper, Maker::Vesper));
}

#[test]
fn a_pick_is_a_text() {
    for text in ["random", "none", "vesper", "legendary"] {
        let back = if text == "legendary" { Pick::<Rarity>::from_text(text).unwrap().to_text() } else { Pick::<Maker>::from_text(text).unwrap().to_text() };
        assert_eq!(back, text);
    }
    assert_eq!(Pick::<Maker>::from_text(" Halo "), Ok(Pick::Of(Maker::Halo)));
    let error = Pick::<Maker>::from_text("acme").unwrap_err();
    assert!(error.contains("blockwerk, vesper, scrapjaw, halo"), "{error}");
}

#[test]
fn the_side_view_has_outlines() {
    let gun = gun(11);
    let view = side_view(&gun);
    assert!(view.len() >= gun.parts.len(), "{} outlines for {} parts", view.len(), gun.parts.len());
    let points = view.iter().flat_map(|o| &o.outlines).flatten();
    assert!(points.clone().count() > 20 && points.clone().all(|p| p.iter().all(|v| v.is_finite() && v.abs() < 1.0)));
    // The side view is on X and Z: the muzzle is at the far right.
    let right = points.map(|p| p[0]).fold(f64::MIN, f64::max);
    assert!(right >= gun.sockets.get("muzzle").unwrap().origin[0] - 1e-6);
}

#[test]
fn bad_inputs_give_errors() {
    assert!(generate(Seed(1), Pick::None, Pick::Random).unwrap_err().contains("maker"));
    assert!(generate(Seed(1), Pick::Random, Pick::None).unwrap_err().contains("rarity"));
    let barrel = part(Kind::Barrel, Seed(1), Pick::Of(Maker::Vesper));
    assert_eq!(assemble(&[&barrel], Rarity::Rare).unwrap_err(), "The gun needs a receiver");
    let receiver = part(Kind::Receiver, Seed(1), Pick::Of(Maker::Vesper));
    assert!(assemble(&[&receiver, &barrel, &barrel], Rarity::Rare).unwrap_err().contains("two parts"));
    let none = part(Kind::Receiver, Seed(1), Pick::None);
    assert!(assemble(&[&none, &barrel], Rarity::Rare).is_err());
}

#[test]
fn the_stats_stay_in_range() {
    for seed in 20..32 {
        let gun = gun(seed);
        let s = gun.stats;
        assert!(s.damage > 0.0 && s.rate >= 0.5 && (0.0..=100.0).contains(&s.accuracy) && s.magazine >= 1.0 && s.reload >= 0.3, "{gun:?}");
        assert!(gun.sockets.get("muzzle").is_some() && gun.sockets.get("grip").is_some());
        assert!(gun.report().starts_with(gun.maker.title()) && gun.report().lines().count() == 7, "{}", gun.report());
        assert_eq!(gun.solid().error(), None);
    }
}
