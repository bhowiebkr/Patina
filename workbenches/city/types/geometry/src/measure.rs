//! The build times of the model run (task D4). Run with `--release -- --ignored --nocapture`.

use super::*;
use std::time::Instant;

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// A tower as in `graphs/main.toml`, and a block of 500 houses with 3 windows each.
#[test]
#[ignore]
fn measure_builds() {
    for round in 0..3 {
        let start = Instant::now();
        let body = Solid::extrude(&Sketch::polygon(&[[1.0, 0.41], [0.41, 1.0], [-0.41, 1.0], [-1.0, 0.41], [-1.0, -0.41], [-0.41, -1.0], [0.41, -1.0], [1.0, -0.41]]), 3.0, 0.0, [1.0; 2], 0);
        let post = Solid::cube(0.4, 0.4, 0.6).translate([1.0, 0.0, 3.0]);
        let posts = Solid::union_all(&(0..4).map(|i| post.rotate([0.0, 0.0, f64::from(i) * 90.0])).collect::<Vec<_>>());
        let door = Solid::extrude(&Sketch::polygon(&[[-0.3, 0.0], [0.3, 0.0], [0.3, 0.8], [0.0, 1.1], [-0.3, 0.8]]), 2.0, 0.0, [1.0; 2], 0).rotate([90.0, 0.0, 0.0]);
        let tower = body.union(&posts).difference(&door);
        let tower_ms = ms(start);
        let start = Instant::now();
        let mut r = Seed(round).random();
        let window = Solid::cube(0.3, 2.0, 0.4);
        let blocks: Vec<Solid> = (0..500)
            .map(|i| {
                let (x, y, h) = (f64::from(i % 25) * 3.0, f64::from(i / 25) * 3.0, r.range(2.0, 8.0));
                let windows = Solid::union_all(&(0..3).map(|k| window.translate([f64::from(k) * 0.6 - 0.6, 0.0, h / 2.0])).collect::<Vec<_>>());
                Solid::cube(2.0, 2.0, h).translate([0.0, 0.0, h / 2.0]).difference(&windows).translate([x, y, 0.0])
            })
            .collect();
        let city = Solid::union_all(&blocks);
        let city_ms = ms(start);
        let start = Instant::now();
        let mesh = city.mesh();
        let mesh_ms = ms(start);
        let start = Instant::now();
        let small = tower.mesh();
        let small_ms = ms(start);
        println!("round {round}: tower {tower_ms:.2} ms + mesh {small_ms:.2} ms ({} triangles), 500 blocks {city_ms:.1} ms + mesh {mesh_ms:.1} ms ({} triangles)", small.triangles(), mesh.triangles());
    }
}
