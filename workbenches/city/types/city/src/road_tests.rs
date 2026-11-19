use geometry::Seed;

use super::*;

/// A flat terrain at 2 m with water where `water` is true.
fn flat(width: f64, depth: f64, cell: f64, water: impl Fn([f64; 2]) -> bool) -> Terrain {
    let (nx, ny) = ((width / cell) as usize + 1, (depth / cell) as usize + 1);
    let mut t = Terrain { nx, ny, cell, heights: vec![2.0; nx * ny], sea: 0.0 };
    for j in 0..ny {
        for i in 0..nx {
            if water(t.point(i, j)) {
                t.heights[j * nx + i] = -2.0;
            }
        }
    }
    t
}

fn hills() -> Terrain {
    let land = Land { width: 600.0, depth: 450.0, cell: 5.0, relief: 25.0, hills: 200.0, coast: 80.0, sea: 0.0 };
    river(&terrain(Seed(7), &land).unwrap(), Seed(7), 18.0, 2.0).unwrap()
}

/// True when no two streets cross and no point is on a street that does not end at it.
fn planar(s: &Streets) -> bool {
    let (p, e) = (&s.points, &s.edges);
    for (i, x) in e.iter().enumerate() {
        for y in &e[i + 1..] {
            if x.a == y.a || x.a == y.b || x.b == y.a || x.b == y.b {
                continue;
            }
            let (a, b, c, d) = (p[x.a], p[x.b], p[y.a], p[y.b]);
            let side = |o: [f64; 2], q: [f64; 2], r: [f64; 2]| cross(sub(q, o), sub(r, o));
            if side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0 {
                return false;
            }
        }
        for (k, &q) in p.iter().enumerate() {
            if k != x.a && k != x.b && crate::terrain::segment_distance(q, p[x.a], p[x.b]) < 1e-6 {
                return false;
            }
        }
    }
    true
}

fn degrees(s: &Streets) -> Vec<usize> {
    let mut degree = vec![0; s.points.len()];
    for e in &s.edges {
        degree[e.a] += 1;
        degree[e.b] += 1;
    }
    degree
}

#[test]
fn the_density_is_high_at_the_center_and_zero_on_the_water() {
    let t = flat(400.0, 300.0, 5.0, |p| p[0] > 150.0);
    let d = density(&t, Seed(1), 3, 120.0).unwrap();
    assert_eq!((d.nx, d.ny, d.values.len()), (t.nx, t.ny, t.nx * t.ny));
    assert!((1..=3).contains(&d.centers.len()), "{d:?}");
    let main = d.centers[0];
    assert!(!t.water(main) && d.at(main) > 0.6, "{d:?}");
    assert_eq!(d.at([180.0, 0.0]), 0.0);
    assert!(d.at([-195.0, -145.0]) < d.at(main));
    assert_eq!(d, density(&t, Seed(1), 3, 120.0).unwrap());
    assert!(density(&t, Seed(1), 0, 120.0).is_err() && density(&t, Seed(1), 3, 0.0).is_err());
    assert!(density(&Terrain::default(), Seed(1), 3, 120.0).is_err());
}

#[test]
fn highways_join_the_gates_to_the_center_without_crossings() {
    let t = flat(600.0, 450.0, 5.0, |p| (p[1] - 120.0).abs() < 10.0);
    let d = density(&t, Seed(2), 1, 200.0).unwrap();
    let h = highways(&t, &d, Seed(2), 4, 20.0, 6.0).unwrap();
    assert!(h.edges.iter().all(|e| e.class == HIGHWAY && e.width == 20.0));
    assert!(planar(&h), "{h:?}");
    // All the points are joined, and 3 or more ends are on the edge of the area.
    let mut seen = vec![false; h.points.len()];
    let mut stack = vec![0];
    while let Some(v) = stack.pop() {
        if !std::mem::replace(&mut seen[v], true) {
            stack.extend(h.edges.iter().filter(|e| e.a == v || e.b == v).map(|e| e.a + e.b - v));
        }
    }
    assert!(seen.iter().all(|&s| s), "{h:?}");
    let ends = degrees(&h).iter().zip(&h.points).filter(|&(&n, p)| n == 1 && (p[0].abs() > 290.0 || p[1].abs() > 215.0)).count();
    assert!(ends >= 3, "{ends} ends on the edge");
    assert_eq!(h, highways(&t, &d, Seed(2), 4, 20.0, 6.0).unwrap());
    assert!(highways(&t, &d, Seed(2), 0, 20.0, 6.0).is_err() && highways(&t, &d, Seed(2), 4, 0.0, 6.0).is_err());
}

#[test]
fn grown_streets_stay_on_land_without_crossings_and_prune_to_loops() {
    let t = hills();
    let d = density(&t, Seed(7), 3, 220.0).unwrap();
    let h = highways(&t, &d, Seed(7), 4, 20.0, 6.0).unwrap();
    let g = grow(&h, &t, &d, Seed(7), 60.0, 16.0, 10.0, 0.9, 0.08).unwrap();
    assert!(g.edges.iter().filter(|e| e.class != HIGHWAY).count() > 100, "{g:?} from {h:?}");
    assert!(planar(&g), "{g:?}");
    assert!(g.edges.iter().any(|e| e.class == ARTERIAL) && g.edges.iter().any(|e| e.class == LOCAL));
    for e in g.edges.iter().filter(|e| e.class != HIGHWAY) {
        let mid = scale(add(g.points[e.a], g.points[e.b]), 0.5);
        assert!(!t.water(mid) && t.contains(mid), "{e:?}");
    }
    let p = prune(&g);
    assert!(planar(&p) && p.edges.len() < g.edges.len());
    for (v, &n) in degrees(&p).iter().enumerate() {
        assert!(n != 1 || p.edges.iter().any(|e| (e.a == v || e.b == v) && e.class == HIGHWAY), "a dead end at {:?}", p.points[v]);
    }
    assert!(blocks(&p).0.len() > 10, "{:?}", blocks(&p));
    assert_eq!(g, grow(&h, &t, &d, Seed(7), 60.0, 16.0, 10.0, 0.9, 0.08).unwrap());
    assert!(grow(&h, &t, &d, Seed(7), 5.0, 16.0, 10.0, 0.6, 0.08).is_err());
    assert!(grow(&h, &t, &d, Seed(7), 60.0, 16.0, 10.0, 2.0, 0.08).is_err());
}

#[test]
fn a_street_at_the_water_turns_along_the_bank() {
    let t = flat(400.0, 300.0, 5.0, |p| p[1] > 70.0);
    let d = density(&t, Seed(3), 1, 200.0).unwrap();
    let h = Streets { points: vec![[-200.0, 0.0], [200.0, 0.0]], edges: vec![Edge { a: 0, b: 1, width: 20.0, class: HIGHWAY }] };
    // With no branches, a street runs along the bank only when it turns at the water.
    let g = grow(&h, &t, &d, Seed(3), 60.0, 16.0, 10.0, 0.0, 0.08).unwrap();
    let along = |e: &Edge| {
        let (p, q) = (g.points[e.a], g.points[e.b]);
        (q[0] - p[0]).abs() > 2.0 * (q[1] - p[1]).abs() && p[1].min(q[1]) > 30.0
    };
    assert!(g.edges.iter().any(|e| e.class != HIGHWAY && along(e)), "{g:?}");
    assert!(g.edges.iter().all(|e| !t.water(g.points[e.a]) && !t.water(g.points[e.b])));
}

#[test]
fn a_street_at_an_angle_to_the_edge_ends_on_the_edge() {
    let t = flat(300.0, 200.0, 5.0, |_| false);
    let hw = |a, b| Edge { a, b, width: 20.0, class: HIGHWAY };
    let end = Streets { points: vec![[0.0, 0.0], [-150.0, 60.0]], edges: vec![hw(0, 1)] };
    let fork = Streets { points: vec![[0.0, 0.0], [-150.0, 60.0], [-60.0, 95.0]], edges: vec![hw(0, 1), hw(1, 2)] };
    for streets in [&end, &fork] {
        let scene = roads(streets, &blocks(streets), &t, 3.0).unwrap();
        for item in scene.items() {
            let out = item.mesh.positions.iter().find(|p| p[0] < -150.0 - 1e-3 || p[1] > 100.0 + 1e-3);
            assert!(out.is_none(), "{} goes out at {out:?}", item.name);
        }
        assert!(scene.items()[0].mesh.normals.iter().all(|n| n[2] > 0.0));
    }
    let scene = roads(&end, &blocks(&end), &t, 3.0).unwrap();
    assert!(scene.items()[0].mesh.positions.iter().any(|p| p[0] < -149.0 && p[1] > 64.0), "the end follows the edge");
}

#[test]
fn roads_follow_the_ground_and_bridge_the_water() {
    let t = flat(300.0, 200.0, 5.0, |p| p[0].abs() < 15.0);
    let (hw, lo) = (|a, b| Edge { a, b, width: 20.0, class: HIGHWAY }, |a, b| Edge { a, b, width: 10.0, class: LOCAL });
    let streets = Streets {
        points: vec![[-140.0, 0.0], [-60.0, 0.0], [60.0, 0.0], [140.0, 0.0], [-60.0, 80.0], [-140.0, 80.0]],
        edges: vec![hw(0, 1), hw(1, 2), hw(2, 3), lo(1, 4), lo(4, 5), lo(5, 0)],
    };
    let blocks = blocks(&streets);
    assert_eq!(blocks.0.len(), 1);
    let scene = roads(&streets, &blocks, &t, 3.0).unwrap();
    let names: Vec<&str> = scene.items().iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["asphalt", "sidewalks", "road lines", "center lines", "bridges", "piers"]);
    let asphalt = &scene.items()[0].mesh;
    assert!(asphalt.normals.iter().all(|n| n[2] > 0.0), "all the road faces look up");
    // The deck is 5 m over the water in the middle, and the road is on the ground far away.
    let top = |x0: f32, x1: f32| asphalt.positions.iter().filter(|p| p[0] > x0 && p[0] < x1 && p[1].abs() < 5.0).map(|p| p[2]).fold(f32::MIN, f32::max);
    assert!((top(-5.0, 5.0) - 5.0).abs() < 1e-3, "{}", top(-5.0, 5.0));
    assert!((top(100.0, 140.0) - 2.15).abs() < 1e-3, "{}", top(100.0, 140.0));
    assert_eq!(scene.items()[5].places.len(), 1, "one pier under the 30 m deck");
    assert!(roads(&streets, &blocks, &t, -1.0).is_err());
}
