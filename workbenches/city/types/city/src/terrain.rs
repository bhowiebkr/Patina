//! The terrain: a height map from value noise with a coast, a river cut into it, and the ground
//! as a scene (task E9).

use std::fmt;

use geometry::{IDENTITY, Item, Mesh, Place, Scene, Seed};

/// The most samples of a height map, so that a small cell cannot fill the memory.
pub const MAX_SAMPLES: usize = 1_000_000;

const GRASS: u32 = 0x6b8a4a;
const SAND: u32 = 0xc8b98a;
const ROCK: u32 = 0x7d756b;
const BED: u32 = 0x8f8466;
const WATER: u32 = 0x3d6e8c;

/// A height map on a regular grid, with the center of the area at the origin. The water is
/// where the ground is below the sea level.
#[derive(Clone, Default, PartialEq)]
pub struct Terrain {
    /// The number of samples along X and Y.
    pub nx: usize,
    pub ny: usize,
    /// The distance between two samples.
    pub cell: f64,
    /// The heights, row by row from the lowest Y.
    pub heights: Vec<f32>,
    pub sea: f64,
}

/// The input of `terrain`.
#[derive(Clone, Copy, Debug)]
pub struct Land {
    /// The size of the area along X and Y, and the distance between two samples.
    pub width: f64,
    pub depth: f64,
    pub cell: f64,
    /// The height of the highest hill above the lowest land.
    pub relief: f64,
    /// The size of a hill.
    pub hills: f64,
    /// The width of the band where the land goes down into the sea. 0 gives no coast.
    pub coast: f64,
    /// The water is below this height.
    pub sea: f64,
}

/// Shows "201 x 151 samples of 3 m, heights -9.3 to 41.2, 12 % water".
impl fmt::Debug for Terrain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (lo, hi) = self.heights.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &h| (lo.min(h), hi.max(h)));
        let water = self.heights.iter().filter(|&&h| f64::from(h) < self.sea).count() * 100 / self.heights.len().max(1);
        write!(f, "{} x {} samples of {} m, heights {lo:.1} to {hi:.1}, {water} % water", self.nx, self.ny, self.cell)
    }
}

impl patina_sdk::Viewer for Terrain {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        fmt::Write::write_fmt(out, format_args!("{self:?}"))
    }
}

impl Terrain {
    /// The size of the area along X.
    pub fn width(&self) -> f64 {
        self.nx.saturating_sub(1) as f64 * self.cell
    }

    /// The size of the area along Y.
    pub fn depth(&self) -> f64 {
        self.ny.saturating_sub(1) as f64 * self.cell
    }

    /// True when the point is in the area.
    pub fn contains(&self, p: [f64; 2]) -> bool {
        p[0].abs() <= self.width() / 2.0 && p[1].abs() <= self.depth() / 2.0
    }

    /// The point of sample (i, j).
    pub fn point(&self, i: usize, j: usize) -> [f64; 2] {
        [i as f64 * self.cell - self.width() / 2.0, j as f64 * self.cell - self.depth() / 2.0]
    }

    fn sample(&self, i: usize, j: usize) -> f64 {
        f64::from(self.heights[j * self.nx + i])
    }

    /// The height at a point on the triangles of `ground`. A point out of the area takes the
    /// height of the nearest edge. A terrain with no samples is flat at 0.
    pub fn height(&self, p: [f64; 2]) -> f64 {
        interpolate(self.nx, self.ny, self.cell, &self.heights, p)
    }

    /// The rise of the ground for each meter along X and along Y.
    pub fn gradient(&self, p: [f64; 2]) -> [f64; 2] {
        let d = self.cell.max(1e-6) / 2.0;
        let dx = self.height([p[0] + d, p[1]]) - self.height([p[0] - d, p[1]]);
        let dy = self.height([p[0], p[1] + d]) - self.height([p[0], p[1] - d]);
        [dx / (2.0 * d), dy / (2.0 * d)]
    }

    /// The slope at a point as the rise for each meter of run.
    pub fn slope(&self, p: [f64; 2]) -> f64 {
        let g = self.gradient(p);
        g[0].hypot(g[1])
    }

    /// True when the ground at the point is below the sea level.
    pub fn water(&self, p: [f64; 2]) -> bool {
        self.height(p) < self.sea
    }
}

/// A height map from layers of value noise. The land goes down into the sea on one side, which
/// the seed picks, with a coast line that winds.
pub fn terrain(seed: Seed, land: &Land) -> Result<Terrain, String> {
    let Land { width, depth, cell, relief, hills, coast, sea } = *land;
    let positive = |v: f64| v.is_finite() && v > 0.0;
    if ![width, depth, cell, hills].into_iter().all(positive) {
        return Err("width, depth, cell and hills must be more than 0".into());
    }
    if !(0.0..=2000.0).contains(&relief) || !(0.0..=width.max(depth)).contains(&coast) || !(-1000.0..=1000.0).contains(&sea) {
        return Err("relief must be from 0 to 2000, coast from 0 to the size and sea from -1000 to 1000".into());
    }
    let (nx, ny) = ((width / cell).round() as usize + 1, (depth / cell).round() as usize + 1);
    if nx.saturating_mul(ny) > MAX_SAMPLES || nx < 2 || ny < 2 {
        return Err(format!("the terrain can have 2 x 2 to {MAX_SAMPLES} samples, not {nx} x {ny}"));
    }
    let mut random = seed.child("coast").random();
    let side = random.index(4);
    let (hill_seed, shore_seed) = (seed.child("hills").0, seed.child("shore").0);
    let mut t = Terrain { nx, ny, cell, heights: Vec::with_capacity(nx * ny), sea };
    let half = [t.width() / 2.0, t.depth() / 2.0];
    for j in 0..ny {
        for i in 0..nx {
            let p = t.point(i, j);
            let n = fbm(hill_seed, p[0] / hills, p[1] / hills, 5);
            let mut h = 1.5 + relief * n * n * (3.0 - 2.0 * n);
            if coast > 0.0 {
                // The distance from the edge of the coast side, with a winding shore.
                let edge = match side {
                    0 => p[1] + half[1],
                    1 => half[0] - p[0],
                    2 => half[1] - p[1],
                    _ => p[0] + half[0],
                };
                let wind = (fbm(shore_seed, p[0] / coast, p[1] / coast, 3) - 0.5) * coast;
                let s = smooth(((edge + wind) / coast).clamp(0.0, 1.0));
                h = h * s + (-4.0 - 0.1 * relief) * (1.0 - s);
            }
            t.heights.push(h as f32);
        }
    }
    Ok(t)
}

/// Cuts a winding river from the highest edge to the lowest edge. The bed is `depth` below the
/// sea level, so the river is water. The banks slope up over one river width.
pub fn river(terrain: &Terrain, seed: Seed, width: f64, depth: f64) -> Result<Terrain, String> {
    if !(width.is_finite() && width >= 0.0 && depth.is_finite() && depth > 0.0) {
        return Err("the width must be 0 or more and the depth more than 0".into());
    }
    let mut out = terrain.clone();
    if width == 0.0 || terrain.nx < 2 || terrain.ny < 2 || terrain.heights.len() != terrain.nx * terrain.ny {
        return Ok(out);
    }
    let (w, d) = (terrain.width(), terrain.depth());
    // The mean height of each edge, in the order of the coast sides: low Y, high X, high Y, low X.
    let edge = |k: usize| -> f64 {
        let points: Vec<[f64; 2]> = (0..=16)
            .map(|s| {
                let a = s as f64 / 16.0 - 0.5;
                [[a * w, -d / 2.0], [w / 2.0, a * d], [a * w, d / 2.0], [-w / 2.0, a * d]][k]
            })
            .collect();
        points.iter().map(|&p| terrain.height(p)).sum::<f64>() / points.len() as f64
    };
    let to = (0..4).min_by(|&a, &b| edge(a).total_cmp(&edge(b))).unwrap_or(0);
    // The river runs along `along` from the high edge to the low edge, and winds along `across`.
    let (along, across, length, span) = match to {
        0 => ([0.0, -1.0], [1.0, 0.0], d, w),
        1 => ([1.0, 0.0], [0.0, 1.0], w, d),
        2 => ([0.0, 1.0], [1.0, 0.0], d, w),
        _ => ([-1.0, 0.0], [0.0, 1.0], w, d),
    };
    let mut random = seed.child("river").random();
    let center = random.range(-0.2, 0.2) * span;
    let waves = [(0.12 * span, random.range(1.0, 1.8), random.range(0.0, 6.3)), (0.05 * span, random.range(2.5, 4.0), random.range(0.0, 6.3))];
    let offset = |s: f64| center + waves.iter().map(|&(a, f, phase)| a * (std::f64::consts::TAU * f * s + phase).sin()).sum::<f64>();
    let step = terrain.cell.max(length / 4000.0);
    let count = (length / step).ceil() as usize + 1;
    let line: Vec<[f64; 2]> = (0..count)
        .map(|k| {
            let a = (k as f64 * step).min(length);
            let (u, v) = (a - length / 2.0, offset(a / length));
            [along[0] * u + across[0] * v, along[1] * u + across[1] * v]
        })
        .collect();
    let (half, bank) = (width / 2.0, width.max(3.0 * terrain.cell));
    let reach = half + bank;
    let bed = terrain.sea - depth;
    for j in 0..terrain.ny {
        for i in 0..terrain.nx {
            let p = terrain.point(i, j);
            // The line goes one way along `along`, so only the points near `a` can be in reach.
            let a = p[0] * along[0] + p[1] * along[1] + length / 2.0;
            let first = ((a - reach) / step).floor().max(0.0) as usize;
            let last = (((a + reach) / step).ceil().max(0.0) as usize + 1).min(line.len() - 1);
            let dist = (first..last).map(|k| segment_distance(p, line[k], line[k + 1])).fold(f64::INFINITY, f64::min);
            if dist >= reach {
                continue;
            }
            let s = smooth(((dist - half) / bank).clamp(0.0, 1.0));
            let h = &mut out.heights[j * terrain.nx + i];
            *h = h.min((bed + (f64::from(*h) - bed) * s) as f32);
        }
    }
    Ok(out)
}

/// The terrain as a scene: grass, sand near the water, rock on steep slopes, the bed under the
/// water, and the water surface at the sea level.
pub fn ground(terrain: &Terrain) -> Scene {
    let t = terrain;
    let mut meshes: [Mesh; 4] = Default::default();
    if t.nx < 2 || t.ny < 2 || t.heights.len() != t.nx * t.ny {
        return Scene::new([]);
    }
    let corner = |i: usize, j: usize| {
        let p = t.point(i, j);
        [p[0] as f32, p[1] as f32, t.heights[j * t.nx + i]]
    };
    let normal = |i: usize, j: usize| {
        let (l, r) = (t.sample(i.saturating_sub(1), j), t.sample((i + 1).min(t.nx - 1), j));
        let (b, f) = (t.sample(i, j.saturating_sub(1)), t.sample(i, (j + 1).min(t.ny - 1)));
        let (dx, dy) = ((r - l) / (2.0 * t.cell), (f - b) / (2.0 * t.cell));
        let n = [-dx, -dy, 1.0];
        let length = (n[0] * n[0] + n[1] * n[1] + 1.0).sqrt();
        n.map(|c| (c / length) as f32)
    };
    for j in 0..t.ny - 1 {
        for i in 0..t.nx - 1 {
            for tri in [[(i, j), (i + 1, j), (i + 1, j + 1)], [(i, j), (i + 1, j + 1), (i, j + 1)]] {
                let corners = tri.map(|(a, b)| corner(a, b));
                let center = (corners[0][2] + corners[1][2] + corners[2][2]) as f64 / 3.0;
                let n = face_normal(corners);
                let steep = n[2] < 0.82;
                let k = if center < t.sea {
                    3
                } else if center < t.sea + 1.2 {
                    1
                } else if steep {
                    2
                } else {
                    0
                };
                push(&mut meshes[k], corners, tri.map(|(a, b)| normal(a, b)));
            }
        }
    }
    let mut water = Mesh::default();
    if t.heights.iter().any(|&h| f64::from(h) < t.sea) {
        let (x, y, z) = ((t.width() / 2.0) as f32, (t.depth() / 2.0) as f32, t.sea as f32);
        let up = [[0.0, 0.0, 1.0]; 3];
        push(&mut water, [[-x, -y, z], [x, -y, z], [x, y, z]], up);
        push(&mut water, [[-x, -y, z], [x, y, z], [-x, y, z]], up);
    }
    let [grass, sand, rock, bed] = meshes;
    let places = || vec![Place { matrix: IDENTITY, group: 0 }];
    Scene::new([
        Item { name: "grass".into(), mesh: grass, color: GRASS, places: places() },
        Item { name: "sand".into(), mesh: sand, color: SAND, places: places() },
        Item { name: "rock".into(), mesh: rock, color: ROCK, places: places() },
        Item { name: "bed".into(), mesh: bed, color: BED, places: places() },
        Item { name: "water".into(), mesh: water, color: WATER, places: places() },
    ])
}

/// The value at a point of a grid of `nx` x `ny` values, row by row from the lowest Y, with the
/// center at the origin. The cell (i, j) has the triangles (i, j), (i + 1, j), (i + 1, j + 1)
/// and (i, j), (i + 1, j + 1), (i, j + 1). A point out of the grid takes the nearest edge.
pub(crate) fn interpolate(nx: usize, ny: usize, cell: f64, values: &[f32], p: [f64; 2]) -> f64 {
    if nx < 2 || ny < 2 || values.len() != nx * ny {
        return 0.0;
    }
    let at = |i: usize, j: usize| f64::from(values[j * nx + i]);
    let x = ((p[0] / cell) + (nx - 1) as f64 / 2.0).clamp(0.0, (nx - 1) as f64);
    let y = ((p[1] / cell) + (ny - 1) as f64 / 2.0).clamp(0.0, (ny - 1) as f64);
    let (i, j) = ((x as usize).min(nx - 2), (y as usize).min(ny - 2));
    let (fx, fy) = (x - i as f64, y - j as f64);
    let (a, c) = (at(i, j), at(i + 1, j + 1));
    if fx >= fy { a + (at(i + 1, j) - a) * fx + (c - at(i + 1, j)) * fy } else { a + (at(i, j + 1) - a) * fy + (c - at(i, j + 1)) * fx }
}

/// Adds one triangle with the normals of its corners, in part 0.
pub(crate) fn push(mesh: &mut Mesh, corners: [[f32; 3]; 3], normals: [[f32; 3]; 3]) {
    mesh.positions.extend(corners);
    mesh.normals.extend(normals);
    mesh.uvs.extend(corners.map(|p| [p[0], p[1]]));
    mesh.parts.push(0);
}

/// The unit normal of a triangle with counterclockwise corners.
pub(crate) fn face_normal([a, b, c]: [[f32; 3]; 3]) -> [f32; 3] {
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if length > 0.0 { n.map(|c| c / length) } else { [0.0, 0.0, 1.0] }
}

/// The distance from `p` to the segment from `a` to `b`.
pub(crate) fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let l2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if l2 > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (ap[0] - ab[0] * t).hypot(ap[1] - ab[1] * t)
}

/// A smooth step from 0 to 1 for `x` from 0 to 1.
pub(crate) fn smooth(x: f64) -> f64 {
    x * x * (3.0 - 2.0 * x)
}

/// A number from 0 to 1 for each point of the integer grid.
fn lattice(seed: u64, x: i64, y: i64) -> f64 {
    let mut h = seed ^ (x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (y as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 31;
    (h >> 11) as f64 / (1_u64 << 53) as f64
}

/// Value noise from 0 to 1, smooth between the points of the integer grid.
fn value(seed: u64, x: f64, y: f64) -> f64 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (smooth(x - ix), smooth(y - iy));
    let (i, j) = (ix as i64, iy as i64);
    let low = lattice(seed, i, j) * (1.0 - fx) + lattice(seed, i + 1, j) * fx;
    let high = lattice(seed, i, j + 1) * (1.0 - fx) + lattice(seed, i + 1, j + 1) * fx;
    low * (1.0 - fy) + high * fy
}

/// Layers of value noise from 0 to 1. Each layer has half the size and half the height.
pub(crate) fn fbm(seed: u64, x: f64, y: f64, layers: u32) -> f64 {
    let (mut sum, mut total, mut amplitude, mut frequency) = (0.0, 0.0, 1.0, 1.0);
    for k in 0..layers {
        sum += amplitude * value(seed.wrapping_add(u64::from(k)), x * frequency, y * frequency);
        total += amplitude;
        amplitude *= 0.5;
        frequency *= 2.03;
    }
    sum / total
}
