//! Hydraulic erosion of a terrain: drops of rain run down the slopes. A drop takes up soil where
//! it speeds up and puts it down where it slows down, so the valleys get deeper and flatter. Then
//! the water that collects cuts streams down to the sea or the edge.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::f64::consts::SQRT_2;

use geometry::Seed;

use crate::Terrain;
use crate::terrain::smooth;

/// The most drops for each sample, so that a large value cannot stop the program for long.
pub const MAX_DROPS: f64 = 8.0;

/// The steps of one drop, and the radius of the area that a drop wears down, in samples.
const STEPS: usize = 40;
const RADIUS: i64 = 3;
/// The part of the old direction that a drop keeps at each step.
const INERTIA: f64 = 0.05;
/// The soil that a drop can carry for each meter of fall, each unit of speed and of water.
const CAPACITY: f64 = 4.0;
/// The smallest fall for the capacity, in meters, so a drop on flat ground carries a little soil.
const MIN_FALL: f64 = 0.01;
/// The parts of the free capacity that a drop takes up, and of the extra soil that it puts down.
const ERODE: f64 = 0.3;
const DEPOSIT: f64 = 0.3;
/// The part of the water that goes at each step, and the gain of speed for each unit of slope.
const EVAPORATE: f64 = 0.02;
const GRAVITY: f64 = 4.0;
/// The area that drains into a sample where a stream starts, in square meters. The cut of a
/// stream for each unit of the log of its area over this start, in meters at 1 drop.
const STREAM_START: f64 = 2500.0;
const STREAM_CUT: f64 = 1.2;
/// The half width of the valley of a stream for each meter of cut.
const STREAM_WIDE: f64 = 4.0;
/// The slope where a stream cuts all of its cut. On a lower slope it cuts less, so that flat
/// land stays flat.
const STREAM_SLOPE: f64 = 0.02;
/// The 8 neighbors of a sample.
const NEIGHBORS: [(i64, i64); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// The terrain after `drops` drops of rain for each sample. Each drop starts at a random point and
/// runs down the slope. It stops at the sea, at the edge or after its last step. Then the streams
/// cut their beds, deeper with more drops. 0 drops give the same terrain.
pub fn erode(terrain: &Terrain, seed: Seed, drops: f64) -> Result<Terrain, String> {
    if !(0.0..=MAX_DROPS).contains(&drops) {
        return Err(format!("erosion must be from 0 to {MAX_DROPS} drops for each sample"));
    }
    let mut out = terrain.clone();
    let (nx, ny) = (terrain.nx, terrain.ny);
    if nx < 3 || ny < 3 || terrain.heights.len() != nx * ny || !terrain.cell.is_finite() || terrain.cell <= 0.0 {
        return Ok(out);
    }
    let (high_x, high_y) = ((nx - 1) as f64, (ny - 1) as f64);
    let brush = brush();
    let mut random = seed.random();
    for _ in 0..(drops * (nx * ny) as f64).round() as usize {
        let mut p = [random.range(0.0, high_x), random.range(0.0, high_y)];
        let (mut dir, mut speed, mut water, mut soil) = ([0.0, 0.0], 1.0, 1.0, 0.0);
        for _ in 0..STEPS {
            let (h, g) = surface(&out, p);
            dir = [0, 1].map(|k| dir[k] * INERTIA - g[k] * (1.0 - INERTIA));
            let length = dir[0].hypot(dir[1]);
            if length < 1e-9 {
                break;
            }
            dir = dir.map(|d| d / length);
            let next = [p[0] + dir[0], p[1] + dir[1]];
            let out_of_area = !(0.0..=high_x).contains(&next[0]) || !(0.0..=high_y).contains(&next[1]);
            let fall = if out_of_area { 0.0 } else { h - surface(&out, next).0 };
            if out_of_area || h - fall < terrain.sea {
                // The drop leaves the area, or the sea carries its soil away.
                soil = 0.0;
                break;
            }
            let capacity = fall.max(MIN_FALL) * speed * water * CAPACITY;
            if fall < 0.0 || soil > capacity {
                // Uphill, the drop fills the pit behind it up to the next height.
                let amount = if fall < 0.0 { soil.min(-fall) } else { (soil - capacity) * DEPOSIT };
                soil -= amount;
                add(&mut out, p, amount);
            } else {
                soil += take(&mut out, p, ((capacity - soil) * ERODE).min(fall), &brush);
            }
            speed = (speed * speed + fall / terrain.cell * GRAVITY).max(0.0).sqrt();
            water *= 1.0 - EVAPORATE;
            p = next;
        }
        add(&mut out, p, soil);
    }
    if drops > 0.0 {
        streams(&mut out, STREAM_CUT * drops.sqrt());
    }
    Ok(out)
}

/// Cuts a stream where at least STREAM_START drains into a sample. The water of each sample runs to
/// a neighbor and, over the pits, to the sea or the edge. The stream cuts `cut` meters for each
/// unit of the log of its area over STREAM_START, less on a slope below STREAM_SLOPE. Its bed goes
/// down along the stream and stays at the sea level or above. The ground never goes up.
fn streams(t: &mut Terrain, cut: f64) {
    let (nx, ny, sea) = (t.nx, t.ny, t.sea);
    let n = nx * ny;
    let ground: Vec<f64> = t.heights.iter().map(|&h| f64::from(h)).collect();
    // A priority flood from the sea and the edge gives each sample a neighbor that its water runs
    // to. Where the flood goes down into a pit, it cuts a path out: `level` is the ground with
    // these paths, and it goes down along the water.
    let mut down = vec![usize::MAX; n];
    let mut level = ground.clone();
    let mut heap = BinaryHeap::new();
    for (c, &h) in ground.iter().enumerate() {
        let (i, j) = (c % nx, c / nx);
        if h < sea || i == 0 || j == 0 || i == nx - 1 || j == ny - 1 {
            down[c] = c;
            heap.push(Reverse((key(h), c)));
        }
    }
    let mut order = Vec::with_capacity(n);
    while let Some(Reverse((_, c))) = heap.pop() {
        order.push(c);
        for (m, _) in neighbors(nx, ny, c) {
            if down[m] == usize::MAX {
                down[m] = c;
                let (mut z, mut p) = (ground[m], c);
                while level[p] >= z - 1e-6 {
                    z -= 1e-6;
                    level[p] = z;
                    if down[p] == p {
                        break;
                    }
                    p = down[p];
                }
                heap.push(Reverse((key(ground[m]), m)));
            }
        }
    }
    order.sort_by(|&a, &b| level[a].total_cmp(&level[b]));
    // On a slope, the water of a sample spreads to its lower neighbors by their slope, so that a
    // plain slope gets no lines of water. In a stream, all the water runs to the steepest neighbor.
    // The order goes down, so all water comes in before it goes on.
    let mut area = vec![t.cell * t.cell; n];
    for &c in order.iter().rev() {
        if down[c] == c {
            continue;
        }
        let slope = |(m, dist): (usize, f64)| (m, ((level[c] - level[m]) / dist).max(0.0));
        down[c] = neighbors(nx, ny, c).map(slope).fold((down[c], 0.0), |a, x| if x.1 > a.1 { x } else { a }).0;
        if area[c] > STREAM_START {
            area[down[c]] += area[c];
            continue;
        }
        let total: f64 = neighbors(nx, ny, c).map(|x| slope(x).1).sum();
        for (m, w) in neighbors(nx, ny, c).map(slope) {
            area[m] += area[c] * w / total;
        }
    }
    // From the outlets up, the bed of a stream is never below the bed that its water runs to.
    let mut bed = ground.clone();
    for &c in &order {
        if area[c] > STREAM_START {
            let floor = if down[c] == c { sea } else { bed[down[c]] };
            let steep = neighbors(nx, ny, c).map(|(m, dist)| (level[c] - level[m]).abs() / dist).fold(0.0, f64::max) / t.cell;
            let depth = cut * (area[c] / STREAM_START).ln() * (steep / STREAM_SLOPE).min(1.0);
            bed[c] = ground[c].min((level[c] - depth).max(floor).max(sea));
        }
    }
    for (c, (&b, &g)) in bed.iter().zip(&ground).enumerate() {
        // A cut of less than 1 cm is left out, so that flat land stays the same.
        if g - b < 0.01 {
            continue;
        }
        // The sides of the valley go up from the bed over STREAM_WIDE x the cut.
        let wide = STREAM_WIDE * (g - b) + t.cell;
        let reach = (wide / t.cell).ceil() as i64;
        let (i, j) = ((c % nx) as i64, (c / nx) as i64);
        for y in (j - reach).max(0)..=(j + reach).min(ny as i64 - 1) {
            for x in (i - reach).max(0)..=(i + reach).min(nx as i64 - 1) {
                let dist = (((x - i).pow(2) + (y - j).pow(2)) as f64).sqrt() * t.cell;
                if dist < wide {
                    let m = y as usize * nx + x as usize;
                    let z = b + (ground[m] - b) * smooth(dist / wide);
                    if z < f64::from(t.heights[m]) {
                        t.heights[m] = z as f32;
                    }
                }
            }
        }
    }
}

/// The neighbors of the sample `c` in a grid of `nx` x `ny` samples, with their distances.
fn neighbors(nx: usize, ny: usize, c: usize) -> impl Iterator<Item = (usize, f64)> {
    let (i, j) = ((c % nx) as i64, (c / nx) as i64);
    NEIGHBORS.into_iter().filter_map(move |(di, dj)| {
        let (a, b) = (i + di, j + dj);
        let inside = a >= 0 && b >= 0 && a < nx as i64 && b < ny as i64;
        inside.then(|| (b as usize * nx + a as usize, if di != 0 && dj != 0 { SQRT_2 } else { 1.0 }))
    })
}

/// A key for a height that sorts as the height does.
fn key(h: f64) -> u64 {
    let bits = h.to_bits();
    if bits >> 63 == 1 { !bits } else { bits | 1 << 63 }
}

/// The height and the rise for each sample along X and Y at a point in samples, on the bilinear
/// surface of the four samples around it.
fn surface(t: &Terrain, p: [f64; 2]) -> (f64, [f64; 2]) {
    let (i, j, u, v) = cell(t, p);
    let at = |a: usize, b: usize| f64::from(t.heights[b * t.nx + a]);
    let (h00, h10, h01, h11) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
    let h = h00 * (1.0 - u) * (1.0 - v) + h10 * u * (1.0 - v) + h01 * (1.0 - u) * v + h11 * u * v;
    (h, [(h10 - h00) * (1.0 - v) + (h11 - h01) * v, (h01 - h00) * (1.0 - u) + (h11 - h10) * u])
}

/// The low corner of the grid cell of a point in samples, and the place of the point in it.
fn cell(t: &Terrain, p: [f64; 2]) -> (usize, usize, f64, f64) {
    let (i, j) = ((p[0] as usize).min(t.nx - 2), (p[1] as usize).min(t.ny - 2));
    (i, j, p[0] - i as f64, p[1] - j as f64)
}

/// Puts `amount` meters of soil on the four samples around a point.
fn add(t: &mut Terrain, p: [f64; 2], amount: f64) {
    let (i, j, u, v) = cell(t, p);
    let nx = t.nx;
    let corners = [(i, j, (1.0 - u) * (1.0 - v)), (i + 1, j, u * (1.0 - v)), (i, j + 1, (1.0 - u) * v), (i + 1, j + 1, u * v)];
    for (a, b, w) in corners {
        t.heights[b * nx + a] += (amount * w) as f32;
    }
}

/// Takes up to `amount` meters of soil from the samples near a point, and gives the soil taken.
/// No sample goes below the sea, so the erosion makes no new water.
fn take(t: &mut Terrain, p: [f64; 2], amount: f64, brush: &[(i64, i64, f64)]) -> f64 {
    let (ci, cj) = (p[0].round() as i64, p[1].round() as i64);
    let mut taken = 0.0;
    for &(di, dj, w) in brush {
        let (a, b) = (ci + di, cj + dj);
        if (0..t.nx as i64).contains(&a) && (0..t.ny as i64).contains(&b) {
            let h = &mut t.heights[b as usize * t.nx + a as usize];
            let cut = (amount * w).min(f64::from(*h) - t.sea).max(0.0);
            *h -= cut as f32;
            taken += cut;
        }
    }
    taken
}

/// The samples within RADIUS of a sample, with weights that fall to 0 at RADIUS and add up to 1.
fn brush() -> Vec<(i64, i64, f64)> {
    let mut brush = Vec::new();
    for dj in -RADIUS..=RADIUS {
        for di in -RADIUS..=RADIUS {
            let w = RADIUS as f64 - ((di * di + dj * dj) as f64).sqrt();
            if w > 0.0 {
                brush.push((di, dj, w));
            }
        }
    }
    let total: f64 = brush.iter().map(|b| b.2).sum();
    brush.iter().map(|&(di, dj, w)| (di, dj, w / total)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_cuts_down_along_the_valley_and_out_of_a_pit() {
        // A valley at x = 60, column 42, on ground that slopes down to the low y edge. Row 20 of the
        // valley has a pit 3.5 m deep.
        let (nx, ny, axis) = (61, 41, 42);
        let mut t = Terrain { nx, ny, cell: 5.0, heights: vec![0.0; nx * ny], sea: 0.0 };
        for j in 0..ny {
            for i in 0..nx {
                let p = t.point(i, j);
                t.heights[j * nx + i] = (2.0 + 0.1 * (p[0] - 60.0).abs() + 0.02 * (p[1] + 100.0)) as f32;
            }
        }
        t.heights[20 * nx + axis] -= 3.5;
        let mut s = t.clone();
        streams(&mut s, 1.2);
        assert!(s.heights.iter().zip(&t.heights).all(|(a, b)| a <= b && f64::from(*a) >= s.sea));
        for j in [10, 30] {
            let cut = |i: usize| t.heights[j * nx + i] - s.heights[j * nx + i];
            let deepest = (0..nx).max_by(|&a, &b| cut(a).total_cmp(&cut(b))).unwrap();
            assert!(deepest == axis && cut(axis) > 0.5, "row {j}: column {deepest}, {} m", cut(axis));
        }
        // The bed goes down to the low edge, also below the pit.
        let bed: Vec<f32> = (0..ny).map(|j| s.heights[j * nx + axis]).collect();
        assert!(bed.windows(2).all(|w| w[0] <= w[1] + 1e-4), "{bed:?}");
    }
}
