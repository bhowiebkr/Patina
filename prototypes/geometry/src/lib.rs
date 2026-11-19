//! Task D0 of `docs/workbenches-plan.md`: the baseline with no geometry kernel. It has seeds,
//! closed prisms, transforms and merges, but no booleans. Merged parts can overlap.

use std::collections::HashMap;

pub mod sample;

/// A seed. `child` gives a sub-seed for a named part, so a change to one part does not change
/// the other parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seed(pub u64);

impl Seed {
    /// The sub-seed for the part `name`.
    pub fn child(self, name: &str) -> Seed {
        let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ self.0;
        for b in name.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        Seed(mix(h))
    }

    /// The sub-seed for item `i` of a list, for example a building of a block.
    pub fn index(self, i: u64) -> Seed {
        Seed(mix(self.0 ^ i.wrapping_mul(0x9e37_79b9_7f4a_7c15)))
    }

    pub fn rng(self) -> Rng {
        Rng(self.0)
    }
}

/// The finalizer of SplitMix64.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// SplitMix64: integer steps only, so the same seed gives the same numbers on each platform.
pub struct Rng(u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.0)
    }

    /// A value in `lo..hi`, with 24 bits.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let unit = (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32;
        lo + (hi - lo) * unit
    }

    /// A value in `0..n`.
    pub fn below(&mut self, n: u32) -> u32 {
        (((self.next_u64() >> 32) * u64::from(n)) >> 32) as u32
    }
}

/// A 3 x 3 matrix and a translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub m: [[f32; 3]; 3],
    pub t: [f32; 3],
}

impl Affine {
    pub const IDENTITY: Affine = Affine { m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: [0.0; 3] };

    pub fn translate(t: [f32; 3]) -> Affine {
        Affine { t, ..Self::IDENTITY }
    }

    pub fn scale(s: [f32; 3]) -> Affine {
        Affine { m: [[s[0], 0.0, 0.0], [0.0, s[1], 0.0], [0.0, 0.0, s[2]]], t: [0.0; 3] }
    }

    /// A rotation of `angle` radians about the axis 0 (X), 1 (Y) or 2 (Z).
    pub fn rotate(axis: usize, angle: f32) -> Affine {
        let (s, c) = angle.sin_cos();
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut m = Self::IDENTITY.m;
        m[a][a] = c;
        m[a][b] = -s;
        m[b][a] = s;
        m[b][b] = c;
        Affine { m, t: [0.0; 3] }
    }

    /// `self`, then `next`.
    pub fn then(self, next: Affine) -> Affine {
        let mut m = [[0.0; 3]; 3];
        for (r, row) in m.iter_mut().enumerate() {
            for (c, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| next.m[r][k] * self.m[k][c]).sum();
            }
        }
        Affine { m, t: next.apply(self.t) }
    }

    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|r| self.m[r][0] * p[0] + self.m[r][1] * p[1] + self.m[r][2] * p[2] + self.t[r])
    }

    fn det(&self) -> f32 {
        let m = &self.m;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }
}

/// A triangle mesh with a part ID for each triangle. Each triangle turns counterclockwise when
/// seen from outside.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub parts: Vec<u16>,
}

impl Mesh {
    /// A closed prism from y = 0 to y = `height`. `outline` is a convex polygon in (x, z), with a
    /// positive area. The top ring is `outline` x `top_scale`, so a small scale makes a cone.
    /// shortcut: convex outlines only (fan caps), add ear clipping when a node needs L shapes.
    pub fn prism(outline: &[[f32; 2]], height: f32, top_scale: f32) -> Mesh {
        let n = outline.len() as u32;
        let ring = |y: f32, s: f32| outline.iter().map(move |p| [p[0] * s, y, p[1] * s]);
        let positions: Vec<_> = ring(0.0, 1.0).chain(ring(height, top_scale)).collect();
        let mut triangles = Vec::with_capacity(4 * n as usize);
        for i in 1..n - 1 {
            triangles.push([0, i, i + 1]);
            triangles.push([n, n + i + 1, n + i]);
        }
        for i in 0..n {
            let j = (i + 1) % n;
            triangles.push([i, n + j, j]);
            triangles.push([i, n + i, n + j]);
        }
        let parts = vec![0; triangles.len()];
        Mesh { positions, triangles, parts }
    }

    /// A box with its center at the origin.
    pub fn cuboid(size: [f32; 3]) -> Mesh {
        let [x, y, z] = size.map(|s| s * 0.5);
        Mesh::prism(&[[-x, -z], [x, -z], [x, z], [-x, z]], size[1], 1.0).transformed(Affine::translate([0.0, -y, 0.0]))
    }

    /// A cylinder along Y from y = 0 to y = `height`, with `segments` sides.
    pub fn cylinder(radius: f32, height: f32, segments: u32) -> Mesh {
        let outline: Vec<_> = (0..segments)
            .map(|i| {
                let (s, c) = (i as f32 * std::f32::consts::TAU / segments as f32).sin_cos();
                [radius * c, radius * s]
            })
            .collect();
        Mesh::prism(&outline, height, 1.0)
    }

    /// A copy moved by `a`. A mirror turns the triangles, so the mesh stays outward.
    pub fn transformed(&self, a: Affine) -> Mesh {
        let positions = self.positions.iter().map(|&p| a.apply(p)).collect();
        let flip = a.det() < 0.0;
        let triangles = self.triangles.iter().map(|&[x, y, z]| if flip { [x, z, y] } else { [x, y, z] }).collect();
        Mesh { positions, triangles, parts: self.parts.clone() }
    }

    /// Adds `other` as part `part`, with no boolean.
    pub fn append(&mut self, other: &Mesh, part: u16) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.triangles.extend(other.triangles.iter().map(|t| t.map(|i| i + base)));
        self.parts.extend(std::iter::repeat_n(part, other.triangles.len()));
    }

    /// True when each edge has exactly one triangle on each side, with opposite directions.
    pub fn is_closed(&self) -> bool {
        let mut edges: HashMap<(u32, u32), u32> = HashMap::with_capacity(self.triangles.len() * 3);
        for t in &self.triangles {
            for k in 0..3 {
                *edges.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
            }
        }
        edges.iter().all(|(&(a, b), &n)| n == 1 && edges.get(&(b, a)) == Some(&1))
    }

    /// The signed volume. It is positive when the triangles face outward.
    pub fn volume(&self) -> f32 {
        let p = |i: u32| self.positions[i as usize];
        let tet = |[a, b, c]: [u32; 3]| {
            let (a, b, c) = (p(a), p(b), p(c));
            a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
        };
        self.triangles.iter().map(|&t| tet(t)).sum::<f32>() / 6.0
    }

    /// A hash of the triangles and of the positions on a 0.1 mm grid.
    pub fn hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        let mut eat = |v: u64| h = (h ^ v).wrapping_mul(0x0100_0000_01b3);
        for p in &self.positions {
            p.iter().for_each(|c| eat((c * 10_000.0).round() as i64 as u64));
        }
        self.triangles.iter().flatten().for_each(|&i| eat(u64::from(i)));
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_are_closed_and_face_outward() {
        let cone = Mesh::prism(&[[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]], 3.0, 0.5);
        for mesh in [Mesh::cuboid([1.0, 2.0, 3.0]), Mesh::cylinder(1.0, 2.0, 24), cone] {
            assert!(mesh.is_closed());
            assert!(mesh.volume() > 0.0);
        }
        assert!((Mesh::cuboid([1.0, 2.0, 3.0]).volume() - 6.0).abs() < 1e-5);
        let mirrored = Mesh::cuboid([1.0, 2.0, 3.0]).transformed(Affine::scale([-1.0, 1.0, 1.0]));
        assert!(mirrored.is_closed() && (mirrored.volume() - 6.0).abs() < 1e-5);
        let mut open = Mesh::cuboid([1.0; 3]);
        open.triangles.pop();
        assert!(!open.is_closed());
    }

    #[test]
    fn a_child_seed_is_stable_and_independent() {
        let seed = Seed(42);
        assert_eq!(seed.child("barrel"), seed.child("barrel"));
        assert_ne!(seed.child("barrel"), seed.child("grip"));
        assert_ne!(seed.index(0), seed.index(1));
        let mut rng = seed.rng();
        assert!((0..1000).all(|_| (2.0..3.0).contains(&rng.range(2.0, 3.0)) && rng.below(7) < 7));
    }

    #[test]
    fn rotate_then_translate() {
        let a = Affine::rotate(2, std::f32::consts::FRAC_PI_2).then(Affine::translate([1.0, 0.0, 0.0]));
        let p = a.apply([1.0, 0.0, 0.0]);
        assert!((p[0] - 1.0).abs() < 1e-6 && (p[1] - 1.0).abs() < 1e-6);
    }
}
