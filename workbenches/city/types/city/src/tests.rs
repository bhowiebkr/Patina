use geometry::{Seed, Solid};

use super::*;

fn grid_of(drop: f64) -> Grid {
    Grid { width: 300.0, depth: 200.0, spacing: 50.0, jitter: 0.2, drop, avenue: 3, street: 8.0, avenue_width: 14.0 }
}

fn square(size: f64) -> Plot {
    let corners = vec![[0.0, 0.0], [size, 0.0], [size, size], [0.0, size]];
    Plot { fronts: vec![Some(LOCAL); 4], corners, ..Plot::default() }
}

#[test]
fn a_grid_has_avenues_and_drops_inner_streets_only() {
    let full = grid(Seed(1), &grid_of(0.0)).unwrap();
    // 6 x 4 cells: 7 x 5 points, 6 x 5 + 7 x 4 streets.
    assert_eq!((full.points.len(), full.edges.len()), (35, 58));
    assert_eq!(full.edges.iter().filter(|e| e.width == 14.0).count(), 6 * 2 + 4 * 3);
    let dropped = grid(Seed(1), &grid_of(1.0)).unwrap();
    assert!(dropped.edges.len() < full.edges.len());
    assert_eq!(dropped.points, full.points, "drop does not change the crossings");
    // Each crossing keeps 2 streets or more.
    let mut degree = vec![0; dropped.points.len()];
    for e in &dropped.edges {
        degree[e.a] += 1;
        degree[e.b] += 1;
    }
    assert!(degree.iter().all(|&d| d >= 2), "{degree:?}");
    assert_eq!(grid(Seed(1), &grid_of(0.5)).unwrap(), grid(Seed(1), &grid_of(0.5)).unwrap());
    assert!(grid(Seed(1), &Grid { spacing: 0.0, ..grid_of(0.0) }).is_err());
    assert!(grid(Seed(1), &Grid { spacing: 0.01, ..grid_of(0.0) }).is_err());
}

#[test]
fn blocks_are_the_faces_inset_by_half_the_street() {
    let streets = grid(Seed(4), &Grid { jitter: 0.0, ..grid_of(0.0) }).unwrap();
    let blocks = blocks(&streets);
    assert_eq!(blocks.0.len(), 24);
    // A cell between two avenues and two streets: 50 m less 7 m and 4 m on each axis.
    let areas: Vec<f64> = blocks.0.iter().map(|b| area(&b.corners)).collect();
    assert!(areas.iter().all(|&a| a > 0.0));
    assert!(areas.iter().any(|&a| (a - 39.0 * 39.0).abs() < 1e-6), "{areas:?}");
    let total: f64 = areas.iter().sum();
    assert!(total < 300.0 * 200.0);
    // A dropped street joins two blocks.
    let merged = blocks_of(&grid(Seed(4), &Grid { jitter: 0.0, ..grid_of(1.0) }).unwrap());
    assert!(merged < 24 && merged > 0, "{merged}");
}

fn blocks_of(streets: &Streets) -> usize {
    blocks(streets).0.len()
}

#[test]
fn lots_split_to_the_area_and_keep_the_street_edges() {
    let plots = Plots(vec![square(40.0)]);
    let lots = lots(&plots, Seed(2), 300.0, 5.0).unwrap();
    assert!(lots.0.len() >= 4, "{lots:?}");
    let total: f64 = lots.0.iter().map(|l| area(&l.corners)).sum();
    assert!((total - 1600.0).abs() < 1e-6, "{total}");
    assert!(lots.0.iter().all(|l| area(&l.corners) <= 300.0));
    // A lot with a front has an edge on the outline of the square, with the class of its street.
    for lot in &lots.0 {
        assert_eq!(lot.corners.len(), lot.fronts.len());
        for (i, &front) in lot.fronts.iter().enumerate() {
            let (a, b) = (lot.corners[i], lot.corners[(i + 1) % lot.corners.len()]);
            let on_edge = |c: usize| (a[c].abs() < 1e-9 && b[c].abs() < 1e-9) || ((a[c] - 40.0).abs() < 1e-9 && (b[c] - 40.0).abs() < 1e-9);
            assert_eq!(front, (on_edge(0) || on_edge(1)).then_some(LOCAL), "{lot:?}");
        }
    }
}

#[test]
fn a_cut_across_a_concave_block_gives_separate_lots() {
    // A U of two arms of 10 m x 30 m on a base of 30 m x 10 m. One cut across the arms gives
    // three lots, not one lot for the two arms.
    let u = [[0.0, 0.0], [30.0, 0.0], [30.0, 40.0], [20.0, 40.0], [20.0, 10.0], [10.0, 10.0], [10.0, 40.0], [0.0, 40.0]];
    let block = Plot { corners: u.to_vec(), fronts: vec![Some(LOCAL); 8], ..Plot::default() };
    let lots = lots(&Plots(vec![block]), Seed(4), 600.0, 2.0).unwrap();
    let total: f64 = lots.0.iter().map(|l| area(&l.corners)).sum();
    assert!((total - 900.0).abs() < 1e-6, "{total}");
    assert_eq!(lots.0.len(), 3, "{lots:?}");
    let width = |l: &&Plot| extent(&l.corners, [1.0, 0.0]);
    assert_eq!(lots.0.iter().map(|l| width(&l)).filter(|(lo, hi)| (hi - lo - 10.0).abs() < 1e-9).count(), 2, "{lots:?}");
}

#[test]
fn inset_moves_the_edges_in_or_drops_the_plot() {
    let plots = Plots(vec![square(10.0), square(3.0)]);
    let moved = inset(&plots, 2.0).unwrap();
    assert_eq!(moved.0.len(), 1);
    assert_eq!(moved.0[0].corners, [[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]]);
    assert!(inset(&plots, -1.0).is_err());
}

fn flat() -> Terrain {
    terrain(Seed(3), &Land { relief: 0.0, coast: 0.0, ..land() }).unwrap()
}

fn even(d: f32) -> Density {
    Density { nx: 2, ny: 2, cell: 1000.0, values: vec![d; 4], centers: Vec::new() }
}

#[test]
fn zoning_and_heights_follow_the_density() {
    let (t, lone) = (flat(), Plot { fronts: vec![None; 4], ..square(30.0) });
    let plots = Plots(vec![square(30.0), lone]);
    let zones = |d: f32, parks: f64, industry: f64| zoning(&plots, &t, &even(d), Seed(4), parks, industry).unwrap().0.iter().map(|p| p.zone).collect::<Vec<_>>();
    assert_eq!(zones(0.1, 0.0, 0.0), [Zone::Residential, Zone::Park]);
    assert_eq!(zones(0.9, 0.0, 0.0), [Zone::Commercial, Zone::Park]);
    assert_eq!(zones(0.3, 0.0, 1.0), [Zone::Industrial, Zone::Park]);
    assert_eq!(zones(0.3, 1.0, 0.0), [Zone::Park, Zone::Park]);
    assert!(zoning(&plots, &t, &even(0.1), Seed(4), 1.5, 0.0).is_err());
    let strip = Plot { corners: vec![[0.0, 0.0], [40.0, 0.0], [40.0, 3.0], [0.0, 3.0]], ..square(30.0) };
    assert_eq!(zoning(&Plots(vec![strip]), &t, &even(0.1), Seed(4), 0.0, 0.0).unwrap().0[0].zone, Zone::Park, "a thin lot");
    let zoned = |zone: Zone| Plots(vec![Plot { zone, ..square(30.0) }]);
    let floors = |zone: Zone, d: f32| heights(&zoned(zone), &even(d), Seed(4), 40).unwrap().0[0].floors;
    assert_eq!(floors(Zone::Park, 1.0), 0);
    assert_eq!(floors(Zone::Residential, 0.1), 1);
    assert!((1..=2).contains(&floors(Zone::Industrial, 0.5)));
    assert!((22..=40).contains(&floors(Zone::Commercial, 1.0)));
    assert!(heights(&zoned(Zone::Park), &even(0.5), Seed(4), 0).is_err());
}

/// A house, an apartment, a shop, a tower, a warehouse and a park, in a row on flat land.
fn street_of_lots() -> Plots {
    let lot = |x: f64, w: f64, d: f64, zone: Zone, floors: u32| {
        let corners = vec![[x, 0.0], [x + w, 0.0], [x + w, d], [x, d]];
        Plot { corners, fronts: vec![Some(LOCAL), None, None, None], zone, floors }
    };
    Plots(vec![
        lot(-180.0, 20.0, 32.0, Zone::Residential, 2),
        lot(-150.0, 30.0, 30.0, Zone::Residential, 6),
        lot(-110.0, 24.0, 30.0, Zone::Commercial, 3),
        lot(-70.0, 50.0, 50.0, Zone::Commercial, 30),
        lot(0.0, 70.0, 50.0, Zone::Industrial, 1),
        lot(90.0, 30.0, 30.0, Zone::Park, 0),
    ])
}

#[test]
fn buildings_get_a_kind_tiers_and_a_ground_level() {
    let (plots, t) = (street_of_lots(), flat());
    let list = buildings(&plots, &t, Seed(6), 3.0, 4.0).unwrap();
    let kinds: Vec<Kind> = list.0.iter().map(|b| b.kind).collect();
    assert_eq!(kinds, [Kind::House, Kind::Apartment, Kind::Shop, Kind::Tower, Kind::Warehouse]);
    assert_eq!(list.0.iter().map(|b| b.group).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    assert_eq!(list, buildings(&plots, &t, Seed(6), 3.0, 4.0).unwrap());
    for b in &list.0 {
        assert!((b.ground - 1.65).abs() < 1e-9 && (b.low - 1.5).abs() < 1e-9, "{b:?}");
        let lot = &plots.0[b.group as usize - 1].corners;
        assert!(b.tiers.iter().all(|tier| crate::shapes::contains(lot, &tier.corners)), "{b:?}");
        assert_eq!(b.tiers[0].bottom, 0.0);
        let top = b.tiers.iter().fold(0.0_f64, |a, tier| a.max(tier.top));
        assert!((top - b.level(plots.0[b.group as usize - 1].floors)).abs() < 1e-9, "{b:?}");
    }
    let (house, tower) = (&list.0[0], &list.0[3]);
    assert!(matches!(house.tiers[0].roof, Roof::Gable | Roof::Hip) && house.tiers[0].rise > 0.0);
    // The house stands back from its street edge at y = 0.
    assert!(house.tiers.iter().flat_map(|tier| &tier.corners).all(|p| p[1] >= 4.0 - 1e-9), "{house:?}");
    // A tower has a podium and a narrower shaft over it.
    assert!(tower.tiers.len() >= 2 && area(&tower.tiers[1].corners) < area(&tower.tiers[0].corners), "{tower:?}");
    assert!(buildings(&plots, &t, Seed(6), 1.0, 4.0).is_err());
    assert!(buildings(&plots, &t, Seed(6), 3.0, -1.0).is_err());
    assert_eq!(format!("{list:?}"), "5 buildings: 1 houses, 1 apartments, 1 shops, 1 towers, 1 warehouses");
}

#[test]
fn walls_facades_and_roofs_dress_each_building() {
    let list = buildings(&street_of_lots(), &flat(), Seed(6), 3.0, 4.0).unwrap();
    let count = |scene: &geometry::Scene, name: &str| scene.items().iter().filter(|i| i.name.starts_with(name)).map(|i| i.places.len()).sum::<usize>();
    let body = walls(&list);
    assert_eq!(body.groups(), [1, 2, 3, 4, 5]);
    assert_eq!((count(&body, "walls "), count(&body, "trim ")), (5, 5));
    // The tower is the highest: its top floor and its parapet.
    let top = list.0[3].ground + list.0[3].level(30) + 0.6;
    assert!((body.bounds()[1][2] - top).abs() < 1e-3, "{:?}", body.bounds());
    let fronts = facades(&list).unwrap();
    assert_eq!(count(&fronts, "door #"), 5, "one entrance on each building");
    assert!(count(&fronts, "glass ") > 100 && count(&fronts, "shop window") > 0 && count(&fronts, "awning") > 0);
    assert!(count(&fronts, "loading door") > 0 && count(&fronts, "balcony") > 0 && count(&fronts, "sign band") == 1);
    assert_eq!(fronts.hash(), facades(&list).unwrap().hash());
    let tops = roofs(&list, Seed(7)).unwrap();
    assert_eq!(count(&tops, "roof "), 5);
    assert_eq!(count(&tops, "antennas"), 1, "only the tower is tall enough");
    assert!(count(&tops, "rooftop units") > 0 && count(&tops, "stair houses") == 2 && count(&tops, "skylights") + count(&tops, "chimneys") <= 30);
    assert_eq!(tops.hash(), roofs(&list, Seed(7)).unwrap().hash());
}

#[test]
fn scatter_makes_one_item() {
    let tree = Solid::cube(1.0, 1.0, 4.0);
    let trees = scatter(&Plots(vec![square(20.0)]), &Terrain::default(), &tree, Seed(5), 5.0, 1.0, 0x336633).unwrap();
    // Each edge of the inset square is 18 m, so it has 3 trees.
    assert_eq!(trees.places(), 12);
    assert_eq!(scatter(&Plots(vec![square(20.0)]), &Terrain::default(), &tree, Seed(5), 5.0, 1.0, 0x336633).unwrap().hash(), trees.hash());
    assert_eq!(color("#4f7a3a"), Ok(0x4f7a3a));
    assert!(color("+4f7a3").is_err() && color("green").is_err());
}

fn land() -> Land {
    Land { width: 400.0, depth: 300.0, cell: 5.0, relief: 30.0, hills: 150.0, coast: 80.0, sea: 0.0 }
}

#[test]
fn a_terrain_has_hills_inland_and_sea_on_one_side() {
    let t = terrain(Seed(3), &land()).unwrap();
    assert_eq!((t.nx, t.ny, t.heights.len()), (81, 61, 81 * 61));
    assert_eq!((t.width(), t.depth()), (400.0, 300.0));
    assert_eq!(t, terrain(Seed(3), &land()).unwrap());
    assert!(t.heights.iter().all(|h| h.is_finite()));
    // The center is land, and some edge is sea.
    assert!(!t.water([0.0, 0.0]));
    let edges = [[0.0, -150.0], [200.0, 0.0], [0.0, 150.0], [-200.0, 0.0]];
    assert_eq!(edges.iter().filter(|&&p| t.water(p)).count(), 1, "{t:?}");
    // The height between the samples is between them, and out of the area it is the edge.
    assert_eq!(t.height(t.point(4, 7)), f64::from(t.heights[7 * 81 + 4]));
    assert_eq!(t.height([1e6, 1e6]), f64::from(*t.heights.last().unwrap()));
    let flat = terrain(Seed(3), &Land { relief: 0.0, coast: 0.0, ..land() }).unwrap();
    assert!(flat.heights.iter().all(|&h| h == 1.5) && flat.slope([10.0, 10.0]) == 0.0);
    assert!(terrain(Seed(3), &Land { cell: 0.0, ..land() }).is_err());
    assert!(terrain(Seed(3), &Land { cell: 0.1, ..land() }).is_err(), "too many samples");
    assert!(terrain(Seed(3), &Land { relief: f64::NAN, ..land() }).is_err());
}

#[test]
fn erosion_cuts_the_slopes_and_keeps_or_loses_soil() {
    let t = terrain(Seed(3), &land()).unwrap();
    let e = erode(&t, Seed(4), 1.0).unwrap();
    assert_eq!(e, erode(&t, Seed(4), 1.0).unwrap());
    assert_eq!(erode(&t, Seed(4), 0.0).unwrap(), t);
    assert!(erode(&t, Seed(4), MAX_DROPS + 1.0).is_err() && erode(&t, Seed(4), f64::NAN).is_err());
    assert!(e.heights.iter().all(|h| h.is_finite()), "{e:?}");
    let sum = |t: &Terrain| t.heights.iter().map(|&h| f64::from(h)).sum::<f64>();
    let peak = |t: &Terrain| t.heights.iter().fold(f32::MIN, |a, &b| a.max(b));
    // A drop can take soil out of the area at the edge, but it never makes soil.
    assert!(sum(&e) <= sum(&t) + 1e-3 && peak(&e) < peak(&t), "{t:?} to {e:?}");
    let cut = t.heights.iter().zip(&e.heights).map(|(a, b)| a - b).fold(0.0, f32::max);
    assert!(cut > 0.5, "the deepest cut is {cut} m");
}

#[test]
fn a_river_cuts_water_across_and_never_raises_the_ground() {
    let t = terrain(Seed(5), &Land { coast: 0.0, ..land() }).unwrap();
    let r = river(&t, Seed(5), 20.0, 2.0, 40.0).unwrap();
    assert!(r.heights.iter().zip(&t.heights).all(|(a, b)| a <= b));
    let water = r.heights.iter().filter(|&&h| f64::from(h) < r.sea).count();
    assert!(water > 0 && water < r.heights.len() / 4, "{r:?}");
    // The water reaches two opposite edges.
    let row = |j: usize| (0..r.nx).any(|i| r.water(r.point(i, j)));
    let column = |i: usize| (0..r.ny).any(|j| r.water(r.point(i, j)));
    assert!((row(0) && row(r.ny - 1)) || (column(0) && column(r.nx - 1)), "{r:?}");
    assert_eq!(river(&t, Seed(5), 0.0, 2.0, 40.0).unwrap(), t);
    assert!(river(&t, Seed(5), 20.0, 0.0, 40.0).is_err() && river(&t, Seed(5), 20.0, 2.0, -1.0).is_err());
}

#[test]
fn a_river_follows_the_valley() {
    // A valley at x = 60 on ground that slopes down to the low y edge.
    let (nx, ny, cell) = (61, 41, 5.0);
    let mut t = Terrain { nx, ny, cell, heights: vec![0.0; nx * ny], sea: 0.0 };
    for j in 0..ny {
        for i in 0..nx {
            let p = t.point(i, j);
            t.heights[j * nx + i] = (10.0 + 0.1 * (p[0] - 60.0).abs() + 0.02 * p[1]) as f32;
        }
    }
    for seed in 1..6 {
        let r = river(&t, Seed(seed), 10.0, 2.0, 0.0).unwrap();
        for j in [5, 20, 35] {
            let wet: Vec<f64> = (0..nx).map(|i| r.point(i, j)).filter(|&p| r.water(p)).map(|p| p[0]).collect();
            let mean = wet.iter().sum::<f64>() / wet.len() as f64;
            assert!(!wet.is_empty() && (mean - 60.0).abs() < 10.0, "seed {seed}, row {j}: {wet:?}");
        }
        // A valley of 60 m sides cuts the slopes beside the bank, but not the far edge.
        let v = river(&t, Seed(seed), 10.0, 2.0, 60.0).unwrap();
        assert!(v.heights.iter().zip(&r.heights).all(|(a, b)| a <= b));
        let (side, far) = (20 * nx + 48, 20 * nx);
        assert!(r.heights[side] - v.heights[side] > 0.5, "seed {seed}: {} to {}", r.heights[side], v.heights[side]);
        assert_eq!(v.heights[far], t.heights[far]);
        assert!(v.heights.iter().all(|&h| f64::from(h) >= v.sea - 2.0));
    }
}

#[test]
fn a_river_follows_a_valley_that_turns() {
    // A valley from the high y edge at x = -60 down to y = 0, along y = 0 to x = 60 and down to
    // the low y edge. The floor goes down along the valley.
    let (nx, ny, cell) = (61, 41, 5.0);
    let (valley, lengths) = ([[-60.0, 100.0], [-60.0, 0.0], [60.0, 0.0], [60.0, -100.0]], [100.0, 120.0, 100.0]);
    let mut t = Terrain { nx, ny, cell, heights: vec![0.0; nx * ny], sea: 0.0 };
    for j in 0..ny {
        for i in 0..nx {
            let p = t.point(i, j);
            // The distance to the valley, and the length of the valley after the nearest point.
            let (dist, after) = (0..3)
                .map(|k| {
                    let (a, b): ([f64; 2], [f64; 2]) = (valley[k], valley[k + 1]);
                    let u = (((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / (lengths[k] * lengths[k])).clamp(0.0, 1.0);
                    let dist = (p[0] - a[0] - u * (b[0] - a[0])).hypot(p[1] - a[1] - u * (b[1] - a[1]));
                    (dist, (1.0 - u) * lengths[k] + lengths[k + 1..].iter().sum::<f64>())
                })
                .fold((f64::INFINITY, 0.0), |m, x| if x.0 < m.0 { x } else { m });
            t.heights[j * nx + i] = (10.0 + 0.1 * dist + 0.02 * after) as f32;
        }
    }
    for seed in 1..4 {
        let r = river(&t, Seed(seed), 10.0, 2.0, 0.0).unwrap();
        let wet = [[-60.0, 50.0], [0.0, 0.0], [60.0, -50.0]].map(|p| r.water(p));
        let dry = [[0.0, 50.0], [0.0, -50.0]].map(|p| r.water(p));
        assert!(wet == [true; 3] && dry == [false; 2], "seed {seed}: {wet:?} {dry:?}");
    }
}

#[test]
fn the_ground_covers_each_cell_with_two_triangles_and_adds_water() {
    let t = river(&terrain(Seed(3), &land()).unwrap(), Seed(3), 20.0, 2.0, 40.0).unwrap();
    let scene = ground(&t);
    let names: Vec<&str> = scene.items().iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["grass", "sand", "rock", "bed", "water"]);
    let land: usize = scene.items()[..4].iter().map(|i| i.mesh.triangles()).sum();
    assert_eq!(land, 2 * 80 * 60);
    assert_eq!(scene.items()[4].mesh.triangles(), 2);
    assert!(scene.items().iter().all(|i| i.mesh.normals.iter().all(|n| n[2] > 0.0)), "all faces look up");
    assert_eq!(ground(&Terrain::default()).items().len(), 0);
}
