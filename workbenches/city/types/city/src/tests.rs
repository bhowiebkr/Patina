use geometry::{Seed, Solid};

use super::*;

fn grid_of(drop: f64) -> Grid {
    Grid { width: 300.0, depth: 200.0, spacing: 50.0, jitter: 0.2, drop, avenue: 3, street: 8.0, avenue_width: 14.0 }
}

fn square(size: f64) -> Plot {
    let corners = vec![[0.0, 0.0], [size, 0.0], [size, size], [0.0, size]];
    Plot { fronts: vec![true; 4], corners, ..Plot::default() }
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
    let lots = lots(&plots, Seed(2), 300.0, 5.0, 0.0).unwrap();
    assert!(lots.0.len() >= 4, "{lots:?}");
    let total: f64 = lots.0.iter().map(|l| area(&l.corners)).sum();
    assert!((total - 1600.0).abs() < 1e-6, "{total}");
    assert!(lots.0.iter().all(|l| area(&l.corners) <= 300.0 || l.park));
    // A lot with a front has an edge on the outline of the square.
    for lot in &lots.0 {
        assert_eq!(lot.corners.len(), lot.fronts.len());
        for (i, &front) in lot.fronts.iter().enumerate() {
            let (a, b) = (lot.corners[i], lot.corners[(i + 1) % lot.corners.len()]);
            let on_edge = |c: usize| (a[c].abs() < 1e-9 && b[c].abs() < 1e-9) || ((a[c] - 40.0).abs() < 1e-9 && (b[c] - 40.0).abs() < 1e-9);
            assert_eq!(front, on_edge(0) || on_edge(1), "{lot:?}");
        }
        assert_eq!(lot.park, !lot.fronts.contains(&true));
    }
    assert!(super::lots(&plots, Seed(2), 300.0, 5.0, 1.0).unwrap().0.iter().all(|l| l.park));
}

#[test]
fn inset_moves_the_edges_in_or_drops_the_plot() {
    let plots = Plots(vec![square(10.0), square(3.0)]);
    let moved = inset(&plots, 2.0).unwrap();
    assert_eq!(moved.0.len(), 1);
    assert_eq!(moved.0[0].corners, [[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]]);
    assert!(inset(&plots, -1.0).is_err());
}

#[test]
fn buildings_get_floors_roofs_windows_and_a_door() {
    let lot = Plot { corners: vec![[0.0, 0.0], [20.0, 0.0], [20.0, 10.0], [0.0, 10.0]], fronts: vec![true, false, false, false], ..Plot::default() };
    let park = Plot { park: true, ..square(10.0) };
    let tall = heights(&Plots(vec![lot, park]), Seed(3), 9.0, 30.0, 100.0, 3.0).unwrap();
    let (building, park) = (&tall.0[0], &tall.0[1]);
    assert!(building.floors >= 3 && building.height == f64::from(building.floors) * 3.0, "{building:?}");
    assert_eq!((park.floors, park.height), (0, 0.0));
    let scene = masses(&tall, Seed(3), 1.0, 3.0).unwrap();
    // The body, the roof and the lawn.
    assert_eq!(scene.items().len(), 3);
    assert_eq!(scene.groups(), [1]);
    let top = scene.bounds()[1][2];
    assert!((top - (building.height + 3.0)).abs() < 1e-3, "{top}");
    let (window, door) = (Solid::cube(1.2, 0.2, 1.4), Solid::cube(1.4, 0.3, 2.4));
    let fronts = facades(&tall, &window, &door, 3.0).unwrap();
    // 6 bays on the long walls and 3 on the short walls, on each floor, less one for the door.
    let floors = building.floors as usize;
    assert_eq!(fronts.items()[0].places.len(), 18 * floors - 1);
    assert_eq!(fronts.items()[1].places.len(), 1);
    // The door is in the middle of the street edge, at the ground.
    let at = fronts.items()[1].places[0].point([0.0; 3]);
    assert!((at[0] - 35.0 / 3.0).abs() < 1e-4 && at[1] == 0.0 && at[2] == 0.0, "{at:?}");
    assert!(facades(&tall, &window, &door, 0.5).is_err());
}

#[test]
fn roads_and_scatter_make_one_item_each() {
    let streets = grid(Seed(5), &grid_of(0.0)).unwrap();
    let blocks = blocks(&streets);
    let ground = roads(&streets, &blocks);
    assert_eq!(ground.items().len(), 2);
    assert_eq!(ground.places(), 2);
    let tree = Solid::cube(1.0, 1.0, 4.0);
    let trees = scatter(&Plots(vec![square(20.0)]), &tree, Seed(5), 5.0, 1.0, 0x336633).unwrap();
    // Each edge of the inset square is 18 m, so it has 3 trees.
    assert_eq!(trees.places(), 12);
    assert_eq!(scatter(&Plots(vec![square(20.0)]), &tree, Seed(5), 5.0, 1.0, 0x336633).unwrap().hash(), trees.hash());
    assert_eq!(color("#4f7a3a"), Ok(0x4f7a3a));
    assert!(color("+4f7a3").is_err() && color("green").is_err());
}
