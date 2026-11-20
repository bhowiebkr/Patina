//! A bounding-volume hierarchy over the rest-pose triangles, built on the CPU with binned SAH
//! and traced on the GPU (`shaders/trace.wgsl`). Interior nodes keep their two children side by
//! side at `start`; leaves hold `count` triangles from `start`.

use bytemuck::{Pod, Zeroable};
use forge_core::MeshData;
use glam::Vec3;

const BINS: usize = 16;
/// Leaves never go past this depth, so the shader's 64-entry stack can't overflow.
const MAX_DEPTH: u32 = 48;
const MAX_LEAF: usize = 8;
/// `Bvh::node_parts` of a node whose triangles span several parts (the shader's `NONE`).
const MIXED: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Node {
    pub lo: [f32; 3],
    pub start: u32,
    pub hi: [f32; 3],
    pub count: u32,
}

/// `v0.w` holds the part index as a float; `e1`, `e2` are the edges from `v0`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Tri {
    pub v0: [f32; 4],
    pub e1: [f32; 4],
    pub e2: [f32; 4],
}

/// `normals[i]` holds triangle `i`'s three vertex normals (w unused), in `tris` order;
/// `order[i]` is that triangle's index in the mesh.
pub struct Bvh {
    pub nodes: Vec<Node>,
    pub tris: Vec<Tri>,
    pub normals: Vec<[[f32; 4]; 3]>,
    pub order: Vec<u32>,
}

/// The BVH in storage buffers, for bindings 1 (nodes), 2 (triangles), the normals and each node's
/// part (`Bvh::node_parts`).
pub struct Buffers {
    pub nodes: wgpu::Buffer,
    pub tris: wgpu::Buffer,
    pub normals: wgpu::Buffer,
    pub parts: wgpu::Buffer,
}

#[derive(Clone, Copy)]
struct Aabb {
    lo: Vec3,
    hi: Vec3,
}

impl Aabb {
    const EMPTY: Self = Self {
        lo: Vec3::splat(f32::MAX),
        hi: Vec3::splat(f32::MIN),
    };

    fn grow(&mut self, other: &Self) {
        self.lo = self.lo.min(other.lo);
        self.hi = self.hi.max(other.hi);
    }

    fn area(&self) -> f32 {
        let d = (self.hi - self.lo).max(Vec3::ZERO);
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }
}

impl Bvh {
    pub fn build(mesh: &MeshData) -> Self {
        let pos = |i: u32| Vec3::from(mesh.vertices[i as usize].position);
        let corners: Vec<[Vec3; 3]> = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| [pos(t[0]), pos(t[1]), pos(t[2])])
            .collect();
        let boxes: Vec<Aabb> = corners
            .iter()
            .map(|c| Aabb {
                lo: c[0].min(c[1]).min(c[2]),
                hi: c[0].max(c[1]).max(c[2]),
            })
            .collect();
        let centers: Vec<Vec3> = boxes.iter().map(|b| (b.lo + b.hi) * 0.5).collect();
        let mut order: Vec<u32> = (0..corners.len() as u32).collect();
        let mut nodes = vec![Node::zeroed()];
        // (node, start, end, depth)
        let mut todo = vec![(0usize, 0usize, order.len(), 0u32)];
        while let Some((node, start, end, depth)) = todo.pop() {
            let mut bounds = Aabb::EMPTY;
            let mut cbounds = Aabb::EMPTY;
            for &t in &order[start..end] {
                bounds.grow(&boxes[t as usize]);
                let c = centers[t as usize];
                cbounds.grow(&Aabb { lo: c, hi: c });
            }
            let split = if end - start <= 2 || depth >= MAX_DEPTH {
                None
            } else {
                best_split(&order[start..end], &boxes, &centers, &cbounds, &bounds)
            };
            let leaf = Node {
                lo: bounds.lo.to_array(),
                start: start as u32,
                hi: bounds.hi.to_array(),
                count: (end - start) as u32,
            };
            let Some((axis, cut)) = split else {
                nodes[node] = leaf;
                continue;
            };
            let mid =
                start + partition(&mut order[start..end], |t| centers[t as usize][axis] < cut);
            let mid = if mid == start || mid == end {
                (start + end) / 2
            } else {
                mid
            };
            let child = nodes.len();
            nodes.push(Node::zeroed());
            nodes.push(Node::zeroed());
            nodes[node] = Node {
                start: child as u32,
                count: 0,
                ..leaf
            };
            todo.push((child, start, mid, depth + 1));
            todo.push((child + 1, mid, end, depth + 1));
        }
        let mut tris: Vec<Tri> = order
            .iter()
            .map(|&t| {
                let [a, b, c] = corners[t as usize];
                let first = mesh.indices[t as usize * 3] as usize;
                let part = mesh.vertices[first].part as f32;
                Tri {
                    v0: [a.x, a.y, a.z, part],
                    e1: (b - a).extend(0.0).to_array(),
                    e2: (c - a).extend(0.0).to_array(),
                }
            })
            .collect();
        let mut normals: Vec<[[f32; 4]; 3]> = order
            .iter()
            .map(|&t| {
                [0, 1, 2].map(|k| {
                    let v = mesh.indices[t as usize * 3 + k] as usize;
                    Vec3::from(mesh.vertices[v].normal).extend(0.0).to_array()
                })
            })
            .collect();
        // Storage bindings can't be empty.
        if tris.is_empty() {
            tris.push(Tri::zeroed());
            normals.push([[0.0; 4]; 3]);
        }
        Self {
            nodes,
            tris,
            normals,
            order,
        }
    }

    /// For each node, the part all its triangles are on, or `MIXED`. A near-other-part query
    /// skips a whole subtree on its own part, so its own triangles cost it nothing.
    fn node_parts(&self) -> Vec<u32> {
        let part = |tri: &Tri| tri.v0[3] as u32;
        let mut parts = vec![MIXED; self.nodes.len()];
        // Children come after their parent. An empty mesh's root has no children.
        for (i, n) in self.nodes.iter().enumerate().rev() {
            let (start, count) = (n.start as usize, n.count as usize);
            parts[i] = if count > 0 {
                let tris = &self.tris[start..start + count];
                let first = part(&tris[0]);
                if tris.iter().all(|t| part(t) == first) {
                    first
                } else {
                    MIXED
                }
            } else if start > 0 && parts[start] == parts[start + 1] {
                parts[start]
            } else {
                MIXED
            };
        }
        parts
    }

    pub fn upload(&self, device: &wgpu::Device) -> Buffers {
        let buffer = |label, bytes| crate::compute::storage_buffer(device, label, bytes);
        Buffers {
            nodes: buffer("bvh nodes", bytemuck::cast_slice(&self.nodes)),
            tris: buffer("bvh triangles", bytemuck::cast_slice(&self.tris)),
            normals: buffer("bvh normals", bytemuck::cast_slice(&self.normals)),
            parts: buffer("bvh node parts", bytemuck::cast_slice(&self.node_parts())),
        }
    }
}

/// The SAH-best (axis, centroid cut), or `None` when a leaf is cheaper.
fn best_split(
    tris: &[u32],
    boxes: &[Aabb],
    centers: &[Vec3],
    cbounds: &Aabb,
    bounds: &Aabb,
) -> Option<(usize, f32)> {
    let mut best: Option<(f32, usize, f32)> = None;
    for axis in [0, 1, 2] {
        let lo = cbounds.lo[axis];
        let extent = cbounds.hi[axis] - lo;
        if extent <= 0.0 {
            continue;
        }
        let scale = BINS as f32 / extent;
        let mut bins = [(Aabb::EMPTY, 0usize); BINS];
        for &t in tris {
            let b = (((centers[t as usize][axis] - lo) * scale) as usize).min(BINS - 1);
            bins[b].0.grow(&boxes[t as usize]);
            bins[b].1 += 1;
        }
        let mut right = [0.0f32; BINS];
        let (mut acc, mut n) = (Aabb::EMPTY, 0usize);
        for i in (1..BINS).rev() {
            acc.grow(&bins[i].0);
            n += bins[i].1;
            right[i] = acc.area() * n as f32;
        }
        let (mut acc, mut n) = (Aabb::EMPTY, 0usize);
        for i in 0..BINS - 1 {
            acc.grow(&bins[i].0);
            n += bins[i].1;
            let cost = acc.area() * n as f32 + right[i + 1];
            if n > 0 && n < tris.len() && best.is_none_or(|(c, _, _)| cost < c) {
                best = Some((cost, axis, lo + (i + 1) as f32 / scale));
            }
        }
    }
    let (cost, axis, cut) = best?;
    // One traversal step costs about one triangle test.
    let leaf_cost = bounds.area() * tris.len() as f32;
    if tris.len() <= MAX_LEAF && cost + bounds.area() >= leaf_cost {
        return None;
    }
    Some((axis, cut))
}

/// Moves the items where `left` holds to the front; returns how many there are.
fn partition(items: &mut [u32], left: impl Fn(u32) -> bool) -> usize {
    let mut mid = 0;
    for i in 0..items.len() {
        if left(items[i]) {
            items.swap(i, mid);
            mid += 1;
        }
    }
    mid
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Vertex;

    /// A strip of `n` unit quads along x, two triangles each.
    fn strip(n: u32) -> MeshData {
        let vertex = |x: f32, y: f32| Vertex {
            position: [x, y, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0; 2],
            curvature: 0.5,
            part: 0,
        };
        let vertices = (0..=n)
            .flat_map(|i| [vertex(i as f32, 0.0), vertex(i as f32, 1.0)])
            .collect();
        let indices = (0..n)
            .flat_map(|i| {
                let a = i * 2;
                [a, a + 2, a + 1, a + 1, a + 2, a + 3]
            })
            .collect();
        MeshData { vertices, indices }
    }

    #[test]
    fn every_triangle_is_in_one_leaf_inside_its_parents() {
        let mesh = strip(100);
        let bvh = Bvh::build(&mesh);
        let mut seen = 0;
        let mut todo = vec![0usize];
        while let Some(i) = todo.pop() {
            let n = bvh.nodes[i];
            if n.count > 0 {
                seen += n.count;
                continue;
            }
            for c in [n.start as usize, n.start as usize + 1] {
                let child = bvh.nodes[c];
                for k in 0..3 {
                    assert!(child.lo[k] >= n.lo[k] && child.hi[k] <= n.hi[k]);
                }
                todo.push(c);
            }
        }
        assert_eq!(seen, 200);
        assert_eq!(bvh.tris.len(), 200);
        assert!(bvh.nodes.len() > 20);
    }

    #[test]
    fn node_parts_name_a_part_only_when_every_triangle_below_is_on_it() {
        let one_part = Bvh::build(&strip(100));
        assert!(one_part.node_parts().iter().all(|&part| part == 0));

        let mut mesh = strip(100);
        for vertex in &mut mesh.vertices {
            vertex.part = u32::from(vertex.position[0] >= 50.0);
        }
        let bvh = Bvh::build(&mesh);
        let parts = bvh.node_parts();
        assert_eq!(parts[0], MIXED);
        assert!(parts.contains(&0) && parts.contains(&1));
        for (node, &part) in bvh.nodes.iter().zip(&parts) {
            if part != MIXED && node.count > 0 {
                let leaf = &bvh.tris[node.start as usize..(node.start + node.count) as usize];
                assert!(leaf.iter().all(|tri| tri.v0[3] as u32 == part));
            }
        }
        assert_eq!(Bvh::build(&MeshData::default()).node_parts(), [MIXED]);
    }

    #[test]
    fn normals_follow_the_triangle_order() {
        let mut mesh = strip(50);
        // Tilt each vertex normal by its x, so every triangle's normals tell where it is.
        for v in &mut mesh.vertices {
            v.normal = Vec3::new(v.position[0], 0.0, 1.0).normalize().to_array();
        }
        let bvh = Bvh::build(&mesh);
        for (tri, normals) in bvh.tris.iter().zip(&bvh.normals) {
            let x = tri.v0[0];
            let n = Vec3::from_slice(&normals[0][..3]);
            assert!((n.x / n.z - x).abs() < 1e-4, "corner at {x}, normal {n}");
        }
    }
}
