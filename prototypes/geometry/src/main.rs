//! Measures the D0 cases: `cargo run -p geometry --release`.

use std::time::Instant;

use geometry::{Seed, sample};

fn main() {
    let guns = 2000;
    let start = Instant::now();
    let mut triangles = 0;
    for s in 0..guns {
        triangles += sample::gun(Seed(s)).triangles.len();
    }
    let per_gun = start.elapsed().as_secs_f64() * 1e6 / guns as f64;
    println!("gun of 30 parts: {per_gun:.1} us, {} triangles on average", triangles / guns as usize);

    let blocks = 20;
    let start = Instant::now();
    let mut triangles = 0;
    for s in 0..blocks {
        triangles += sample::block(Seed(s), 500).triangles.len();
    }
    let per_block = start.elapsed().as_secs_f64() * 1e3 / blocks as f64;
    println!("block of 500 buildings: {per_block:.2} ms, {} triangles", triangles / blocks as usize);

    let start = Instant::now();
    let city = sample::block(Seed(1), 2500);
    let built = start.elapsed().as_secs_f64() * 1e3;
    let start = Instant::now();
    let closed = city.is_closed();
    let check = start.elapsed().as_secs_f64() * 1e3;
    println!("1 km x 1 km (2500 buildings): {built:.2} ms, closed {closed} (check {check:.1} ms), volume > 0 {}", city.volume() > 0.0);
    println!("hash of gun 42: {:016x}", sample::gun(Seed(42)).hash());
}
