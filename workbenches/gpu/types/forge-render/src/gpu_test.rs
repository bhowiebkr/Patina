//! Bakers on hand-placed texels: a headless device, flat-shaded test meshes and readback.
#![allow(clippy::unwrap_used)]

use forge_core::{Bounds, MeshData, Vertex};
use forge_gpu::Target;
use glam::Vec3;

use crate::bvh;
use crate::curvature::Curvature;
use crate::occlusion::Occlusion;
use crate::surface;

/// The maps' side; texels go in row-major order, the rest stay empty.
const SIZE: u32 = 4;

#[test]
fn preview_uses_loaded_environment_and_restores_asset_light() {
    let Some(gpu) = Gpu::new() else { return };
    let info = forge_core::AssetInfo::default();
    let mut scene = crate::SceneRenderer::new(&gpu.device, &gpu.queue).unwrap();
    let original = scene.preview_light(&info);
    let panorama = crate::Environment::new(&gpu.device, &gpu.queue, 8, 4, &[2.0; 128]).unwrap();
    scene.set_environment(&gpu.device, Some(&panorama));
    let light = scene.preview_light(&info);
    assert_eq!(light.env, 1.0);
    assert_eq!(light.strength, 0.0);
    assert_eq!(light.exposure, 1.0);
    assert_eq!(light.env_yaw, 0.0);
    assert!(light.aces);
    scene.set_environment(&gpu.device, None);
    let restored = scene.preview_light(&info);
    assert_eq!(restored.env, original.env);
    assert_eq!(restored.strength, original.strength);
    assert_eq!(restored.aces, original.aces);
}

/// What the near-other-part map should read at `distance` on a mesh whose largest side is `side`.
fn near_at(distance: f32, side: f32) -> f32 {
    (distance / (0.1 * side)).min(1.0)
}

/// One test on one device: `cargo test` hung 12 of 60 runs with these as five tests against 1 of
/// 60 before them, stuck in the NVIDIA driver at a test thread's exit, and never with one thread.
#[test]
fn near_other_part_reads_the_distance_to_a_different_part() {
    let Some(gpu) = Gpu::new() else { return };
    near_reads_low_beside_a_part_and_high_away_from_it(&gpu);
    near_reads_far_on_a_one_part_mesh(&gpu);
    near_reads_zero_where_parts_touch(&gpu);
    near_matches_every_box_checked_by_hand(&gpu);
    a_bake_from_before_near_misses(&gpu);
}

fn near_reads_low_beside_a_part_and_high_away_from_it(gpu: &Gpu) {
    // Part 0 is [-1, 1]^3, part 1 starts at x = 1.1: a 0.1 gap, largest side 4.1.
    let mesh = boxes(&[(Vec3::ZERO, Vec3::ONE, 0), (Vec3::X * 2.1, Vec3::ONE, 1)]);
    let side = 4.1;
    let m = gpu.near_other_part(
        &mesh,
        &[
            // Part 0 across the gap, and part 1 facing it.
            (Vec3::X, Vec3::X),
            (Vec3::new(1.1, 0.0, 0.0), -Vec3::X),
            // Part 0's top, 0.1 in from the edge: 0.2 from part 1.
            (Vec3::new(0.9, 0.0, 1.0), Vec3::Z),
            // Far on part 0, its own triangles all around: reads 0 if they count.
            (-Vec3::X, -Vec3::X),
            (Vec3::new(-0.5, 0.0, 1.0), Vec3::Z),
        ],
    );
    println!("near {:?}", &m[..5]);
    for (texel, expected) in [(0, 0.1), (1, 0.1), (2, 0.2)] {
        let want = near_at(expected, side);
        assert!(
            (m[texel] - want).abs() < 1e-3,
            "texel {texel}: {} want {want}",
            m[texel]
        );
    }
    assert_eq!(m[3], 1.0, "far side of part 0");
    assert_eq!(m[4], 1.0, "own triangles must not count");
    // Texels with no surface under them stay neutral.
    assert!(m[5..].iter().all(|&v| v == 1.0), "empty texels {m:?}");
}

fn near_reads_far_on_a_one_part_mesh(gpu: &Gpu) {
    let m = gpu.near_other_part(
        &cube(),
        &[(Vec3::Z, Vec3::Z), (Vec3::new(0.999, 0.0, 1.0), Vec3::Z)],
    );
    println!("one part {:?}", &m[..2]);
    assert!(m.iter().all(|&v| v == 1.0), "{m:?}");
}

fn near_reads_zero_where_parts_touch(gpu: &Gpu) {
    // Part 1 starts at x = 1, on part 0's face. Largest side 4.
    let mesh = boxes(&[(Vec3::ZERO, Vec3::ONE, 0), (Vec3::X * 2.0, Vec3::ONE, 1)]);
    let m = gpu.near_other_part(
        &mesh,
        &[
            (Vec3::X, Vec3::X),
            (Vec3::X, -Vec3::X),
            // Touching along an edge of part 1's end, then 0.01 back from the contact.
            (Vec3::new(1.0, 0.0, 1.0), Vec3::Z),
            (Vec3::new(0.99, 0.0, 1.0), Vec3::Z),
        ],
    );
    println!("touching {:?}", &m[..4]);
    for (texel, value) in m[..3].iter().enumerate() {
        assert!(*value < 1e-4, "texel {texel}: {value}");
    }
    assert!((m[3] - near_at(0.01, 4.0)).abs() < 1e-3, "{}", m[3]);
}

fn near_matches_every_box_checked_by_hand(gpu: &Gpu) {
    let halves = |i: u32| Vec3::splat(0.55 + 0.35 * (i as f32 * 0.618).fract());
    let cell = |i: u32| {
        let at = |k: u32| (i >> (2 * k) & 3) as f32 * 2.0;
        Vec3::new(at(0), at(1), at(2))
    };
    // Checkered parts mix every subtree; parts by octant leave whole subtrees on one part.
    let layouts: [fn(u32) -> u32; 2] = [
        |i| (i & 1) ^ (i >> 2 & 1) ^ (i >> 4 & 1),
        |i| (i >> 1 & 1) | (i >> 3 & 1) << 1 | (i >> 5 & 1) << 2,
    ];
    for parts in layouts {
        let list: Vec<_> = (0..64).map(|i| (cell(i), halves(i), parts(i))).collect();
        let mesh = boxes(&list);
        let corners = mesh.vertices.iter().map(|v| Vec3::from(v.position));
        let lo = corners.clone().fold(Vec3::MAX, Vec3::min);
        let hi = corners.fold(Vec3::MIN, Vec3::max);
        let side = (hi - lo).max_element();
        let texels: Vec<Texel> = (0..16u32)
            .map(|k| {
                let i = (k * 17 + 3) % 64;
                let (center, half, _) = list[i as usize];
                let shift = Vec3::new(0.4 * half.x, -0.3 * half.y, 0.0);
                (center + shift + Vec3::Z * half.z, Vec3::Z)
            })
            .collect();
        let m = gpu.near_other_part(&mesh, &texels);
        for (k, (texel, read)) in texels.iter().zip(&m).enumerate() {
            let own = parts((k as u32 * 17 + 3) % 64);
            let nearest = list
                .iter()
                .filter(|(_, _, part)| *part != own)
                .map(|(center, half, _)| {
                    let d = (*center - *half - texel.0).max(texel.0 - *center - *half);
                    d.max(Vec3::ZERO).length()
                })
                .fold(f32::MAX, f32::min);
            let want = near_at(nearest, side);
            assert!(
                (read - want).abs() < 2e-3,
                "texel {k}: {read} want {want} (distance {nearest})"
            );
        }
    }
}

/// Extras alpha meant "reserved, always 1" before `MWMAPS03`; such a bake must not read as "far
/// from every other part".
fn a_bake_from_before_near_misses(gpu: &Gpu) {
    let mesh = cube();
    let info = forge_core::AssetInfo {
        bounds: Bounds {
            min: [-1.0; 3],
            max: [1.0; 3],
        },
        parts: vec![forge_core::PartInfo::default()],
        ..Default::default()
    };
    let scene = crate::SceneRenderer::new(&gpu.device, &gpu.queue).unwrap();
    let asset = crate::GpuAsset::new(&gpu.device, &scene, &info, &mesh);
    let load = |bytes: &[u8]| {
        crate::MeshMaps::from_bytes(&gpu.device, &gpu.queue, &asset, &info, &mesh, 4, bytes)
            .unwrap()
            .is_some()
    };
    let mut bytes = crate::maps::MAGIC.to_vec();
    bytes.extend_from_slice(&crate::maps::key(&mesh, 4).to_le_bytes());
    bytes.resize(bytes.len() + 2 * 4 * 4 * crate::maps::TEXEL as usize, 0);
    assert!(load(&bytes), "current format");
    bytes[..8].copy_from_slice(b"MWMAPS02");
    assert!(!load(&bytes), "older format");
}

/// `cargo test -p forge-render near_other_part_at_2048 -- --ignored --nocapture` prints the slowest
/// dispatch: the bake's one risk here is a single submit outlasting the driver's 2 s timeout.
#[test]
#[ignore = "heavy: a 2048 x 2048 atlas fully covered by two million triangles"]
fn near_other_part_at_2048_stays_under_the_gpu_timeout() {
    let Some(gpu) = Gpu::new() else { return };
    const SIDE: u32 = 2048;
    const QUADS: u32 = 1024;
    let wave = |u: f32, v: f32| 0.02 * ((u * 40.0).sin() + (v * 40.0).cos());
    let checker = |cells: f32, u: f32, v: f32| ((u * cells) as u32 + (v * cells) as u32) % 2;
    let coarse = move |u, v| (Vec3::new(u, v, wave(u, v)), checker(5.0, u, v));
    let fine = move |u, v| (Vec3::new(u, v, wave(u, v)), checker(64.0, u, v));
    let specks = move |u: f32, v: f32| {
        let speck = (u * 5.0).fract() < 0.01 && (v * 5.0).fract() < 0.01;
        (Vec3::new(u, v, wave(u, v)), u32::from(speck))
    };
    let across = move |u, v: f32| {
        let (rise, part, y) = if v < 0.5 {
            (0.0, 0, v * 2.0)
        } else {
            (0.05, 1, (v - 0.5) * 2.0)
        };
        (Vec3::new(u, y, rise + wave(u, y)), part)
    };
    let scenes: [(&str, Place); 4] = [
        // Cell centres sit at the map's reach, the farthest a neighbour can be and still count.
        ("checker of 5", &coarse),
        ("checker of 64", &fine),
        // Single triangles of the other part keep every subtree above them from being skipped.
        ("specks", &specks),
        // Every texel has a dense sheet of the other part right across from it.
        ("parallel sheets", &across),
    ];
    for (name, scene) in scenes {
        let mesh = sheet(QUADS, scene);
        let tree = bvh::Bvh::build(&mesh);
        let buffers = tree.upload(&gpu.device);
        let surface = surface::Surface::new(&gpu.device, &tree, &mesh, SIDE).unwrap();
        let part = Target::new(
            &gpu.device,
            "part",
            [SIDE; 2],
            wgpu::TextureFormat::R32Float,
            wgpu::TextureUsages::TEXTURE_BINDING,
            1,
        );
        let bounds = Bounds {
            min: [0.0, 0.0, -0.05],
            max: [1.0, 1.0, 0.1],
        };
        let baker =
            Occlusion::new(&gpu.device, &buffers, &bounds, [&surface.map, &part], SIDE).unwrap();
        let mut worst = std::time::Duration::ZERO;
        for sample in 0..surface::SAMPLES {
            surface.draw(&gpu.device, &gpu.queue, sample);
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            let start = std::time::Instant::now();
            baker.near_part(&gpu.device, &gpu.queue);
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            worst = worst.max(start.elapsed());
        }
        let out = baker.resolve(&gpu.device, &gpu.queue);
        let bytes = forge_gpu::read_texture(&gpu.device, &gpu.queue, &out.texture, [SIDE; 2], 1, 8)
            .unwrap();
        let near: Vec<f32> = bytes
            .chunks_exact(8)
            .map(|t| f16(u16::from_le_bytes([t[6], t[7]])))
            .collect();
        let (lo, hi) = near
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        println!(
            "{name}: {} triangles, worst near dispatch {worst:?}, near {lo}..{hi}",
            tree.tris.len()
        );
        assert!(lo < 0.1, "{name}: no texel near another part ({lo})");
        assert!(
            worst < std::time::Duration::from_secs(1),
            "{name}: {worst:?}"
        );
    }
}

/// A surface point and its normal, one per texel.
pub type Texel = (Vec3, Vec3);
/// Where a sheet's UV goes, and the part there.
type Place<'a> = &'a dyn Fn(f32, f32) -> (Vec3, u32);

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    /// `None` on a machine without a GPU adapter, where the tests pass without running.
    pub fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).ok()?;
        Some(Self { device, queue })
    }

    /// The normal map's w (curvature) at each texel.
    pub fn curvature(&self, mesh: &MeshData, texels: &[Texel]) -> Vec<f32> {
        let (bvh, bounds, [surface, nrm, _]) = self.inputs(mesh, texels);
        let baker = Curvature::new(&self.device, &bvh, &bounds, [&surface, &nrm], SIZE).unwrap();
        for sample in 0..surface::SAMPLES {
            baker.accumulate(&self.device, &self.queue, sample);
        }
        let out = baker.resolve(&self.device, &self.queue);
        self.read(&out, texels.len()).iter().map(|t| t[3]).collect()
    }

    /// (AO, thickness) at each texel.
    pub fn occlusion(&self, mesh: &MeshData, texels: &[Texel]) -> Vec<[f32; 2]> {
        let (bvh, bounds, [surface, _, part]) = self.inputs(mesh, texels);
        let baker = Occlusion::new(&self.device, &bvh, &bounds, [&surface, &part], SIZE).unwrap();
        for sample in 0..surface::SAMPLES {
            baker.accumulate(&self.device, &self.queue, sample);
        }
        let out = baker.resolve(&self.device, &self.queue);
        self.read(&out, texels.len())
            .iter()
            .map(|t| [t[0], t[1]])
            .collect()
    }

    /// The extras map's near other part at every texel of the map, `texels` first.
    pub fn near_other_part(&self, mesh: &MeshData, texels: &[Texel]) -> Vec<f32> {
        let (bvh, bounds, [surface, _, part]) = self.inputs(mesh, texels);
        let baker = Occlusion::new(&self.device, &bvh, &bounds, [&surface, &part], SIZE).unwrap();
        for sample in 0..surface::SAMPLES {
            baker.accumulate(&self.device, &self.queue, sample);
        }
        let out = baker.resolve(&self.device, &self.queue);
        self.read(&out, (SIZE * SIZE) as usize)
            .iter()
            .map(|t| t[3])
            .collect()
    }

    /// The BVH, the bounds and the surface, normal and part maps holding `texels`, each on the
    /// triangle it lies on (the same at every sample point).
    fn inputs(&self, mesh: &MeshData, texels: &[Texel]) -> (bvh::Buffers, Bounds, [Target; 3]) {
        assert!(texels.len() <= (SIZE * SIZE) as usize);
        let tree = bvh::Bvh::build(mesh);
        let mut at = vec![[0f32; 4]; (SIZE * SIZE) as usize];
        let mut nrm = at.clone();
        for (i, &(p, n)) in texels.iter().enumerate() {
            at[i] = on_triangle(&tree, p, n);
            nrm[i] = n.normalize().extend(0.5).to_array();
        }
        let part = vec![0f32; (SIZE * SIZE) as usize];
        let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let map = |label, format, texel, bytes: &[u8]| {
            let t = Target::new(&self.device, label, [SIZE; 2], format, usage, 1);
            forge_gpu::write_texture(&self.queue, &t.texture, [SIZE; 2], texel, bytes);
            t
        };
        let rgba = wgpu::TextureFormat::Rgba32Float;
        let maps = [
            map("surface", rgba, 16, bytemuck::cast_slice(&at)),
            map("normal", rgba, 16, bytemuck::cast_slice(&nrm)),
            map(
                "part",
                wgpu::TextureFormat::R32Float,
                4,
                bytemuck::cast_slice(&part),
            ),
        ];
        let corners = mesh.vertices.iter().map(|v| Vec3::from(v.position));
        let min = corners.clone().fold(Vec3::MAX, Vec3::min);
        let max = corners.fold(Vec3::MIN, Vec3::max);
        let bounds = Bounds {
            min: min.to_array(),
            max: max.to_array(),
        };
        (tree.upload(&self.device), bounds, maps)
    }

    /// The first `n` texels of an Rgba16Float map.
    fn read(&self, target: &Target, n: usize) -> Vec<[f32; 4]> {
        let bytes =
            forge_gpu::read_texture(&self.device, &self.queue, &target.texture, [SIZE; 2], 1, 8);
        bytes
            .unwrap()
            .chunks_exact(8)
            .take(n)
            .map(|t| [0, 1, 2, 3].map(|k| f16(u16::from_le_bytes([t[2 * k], t[2 * k + 1]]))))
            .collect()
    }
}

/// The surface-map texel (BVH slot, barycentrics on e1 and e2, covered) of the triangle facing
/// `n` that `p` lies on.
fn on_triangle(tree: &bvh::Bvh, p: Vec3, n: Vec3) -> [f32; 4] {
    for (slot, tri) in tree.tris.iter().enumerate() {
        let v0 = Vec3::from_slice(&tri.v0[..3]);
        let e1 = Vec3::from_slice(&tri.e1[..3]);
        let e2 = Vec3::from_slice(&tri.e2[..3]);
        let ng = e1.cross(e2).normalize();
        let d = p - v0;
        if ng.dot(n.normalize()) < 0.5 || ng.dot(d).abs() > 1e-4 {
            continue;
        }
        let (a, b, c) = (e1.dot(e1), e1.dot(e2), e2.dot(e2));
        let (x, y) = (d.dot(e1), d.dot(e2));
        let det = a * c - b * b;
        let (b1, b2) = ((c * x - b * y) / det, (a * y - b * x) / det);
        if b1 >= -1e-5 && b2 >= -1e-5 && b1 + b2 <= 1.0 + 1e-5 {
            return [slot as f32, b1, b2, 1.0];
        }
    }
    panic!("no triangle under {p} facing {n}");
}

/// IEEE half to f32, as in the app's export.
fn f16(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let man = f32::from(bits & 0x3ff);
    sign * match (bits >> 10) & 0x1f {
        0 => man * 2f32.powi(-24),
        31 => f32::INFINITY,
        e => (1.0 + man / 1024.0) * 2f32.powi(i32::from(e) - 15),
    }
}

/// Flat-shaded quads, each `(corner, u, v)` with its normal along `u x v`.
pub fn quads(quads: &[(Vec3, Vec3, Vec3)]) -> MeshData {
    let mut mesh = MeshData {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    for &(c, u, v) in quads {
        let first = mesh.vertices.len() as u32;
        let normal = u.cross(v).normalize().to_array();
        for p in [c, c + u, c + u + v, c + v] {
            mesh.vertices.push(Vertex {
                position: p.to_array(),
                normal,
                uv: [0.0; 2],
                curvature: 0.5,
                part: 0,
            });
        }
        mesh.indices.extend([0, 1, 2, 0, 2, 3].map(|i| i + first));
    }
    mesh
}

/// The cube [-1, 1]^3 with hard edges.
pub fn cube() -> MeshData {
    const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];
    let mut faces = Vec::new();
    for i in 0..3 {
        for s in [1.0, -1.0] {
            let n = AXES[i] * s;
            let u = AXES[(i + 1) % 3] * 2.0;
            let v = AXES[(i + 2) % 3] * 2.0 * s;
            faces.push((n - u * 0.5 - v * 0.5, u, v));
        }
    }
    quads(&faces)
}

/// `cube()` scaled by each `half`, moved to its `center` and put on its part.
pub fn boxes(list: &[(Vec3, Vec3, u32)]) -> MeshData {
    let unit = cube();
    let mut mesh = MeshData::default();
    for &(center, half, part) in list {
        let first = mesh.vertices.len() as u32;
        mesh.vertices.extend(unit.vertices.iter().map(|v| Vertex {
            position: (center + Vec3::from(v.position) * half).to_array(),
            part,
            ..*v
        }));
        mesh.indices.extend(unit.indices.iter().map(|i| i + first));
    }
    mesh
}

/// `quads` x `quads` cells covering the whole atlas; `at` places each UV and names its part. A
/// triangle takes the part of its first corner.
fn sheet(quads: u32, at: Place) -> MeshData {
    let row = quads + 1;
    let mut mesh = MeshData::default();
    for j in 0..row {
        for i in 0..row {
            let (u, v) = (i as f32 / quads as f32, j as f32 / quads as f32);
            let (position, part) = at(u, v);
            mesh.vertices.push(Vertex {
                position: position.to_array(),
                normal: [0.0, 0.0, 1.0],
                uv: [u, v],
                curvature: 0.5,
                part,
            });
        }
    }
    for j in 0..quads {
        for i in 0..quads {
            let a = j * row + i;
            mesh.indices
                .extend([a, a + 1, a + row + 1, a, a + row + 1, a + row]);
        }
    }
    mesh
}

/// A floor (z = 0, x in 0..2, facing up) meeting a wall (x = 0, z in 0..2, facing +x) along
/// the y axis: an inside corner.
pub fn ell() -> MeshData {
    let corner = Vec3::new(0.0, -1.0, 0.0);
    quads(&[
        (corner, Vec3::X * 2.0, Vec3::Y * 2.0),
        (corner, Vec3::Y * 2.0, Vec3::Z * 2.0),
    ])
}
