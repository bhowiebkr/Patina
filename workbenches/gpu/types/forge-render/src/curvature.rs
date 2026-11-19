//! Curvature for the normal map's w, ray traced against the BVH of the rest-pose mesh
//! (`bvh.rs`): `SAMPLES` neighbouring surface points per sample point from a disk scaled by the
//! bounding box's largest side, for each of `surface::SAMPLES` sample points per texel (2x2
//! antialiasing), normalised to the map's range.
//!
//! The estimator is built from published methods. The
//! neighbours are found the way Cycles' bevel shader finds them (`kernel/svm/bevel.h`, after
//! King et al. 2013, "BSSRDF Importance Sampling"): rays down one of three axes through a disk
//! with a cubic falloff, weighted by multiple importance sampling. Cycles frames the disk on the
//! geometric normal; here it is the interpolated vertex normal. The curvature is the
//! least-squares fit of Rusinkiewicz's finite-difference constraint `II d = dn` ("Estimating
//! Curvatures and Their Derivatives on Triangle Meshes", 2004) for an isotropic `II`, which is
//! the mean curvature `H = sum(w dn.d) / sum(w |d|^2)` over the neighbours `d` away whose
//! normals differ by `dn`. It stays finite across hard edges, where `dn.d / |d|^2` per
//! neighbour does not. The map stores `0.5 + 0.5 H / bound`: flat 0.5, convex brighter, concave
//! darker. The bound is the largest `|H|` over the map, measured on the
//! material ball; a 99.9th-percentile clip read 1.48x as
//! contrasty as the reference map, which smart materials' curvature masks are tuned to. Against
//! that map: r 0.980, ours = 0.883 reference + 0.056, mean difference 0.006; the residue is a
//! darker concave tail (0.1st percentile 0.16 vs 0.21) and a lower convex 99th (0.72 vs 0.77).

use bytemuck::{Pod, Zeroable};
use forge_core::Bounds;
use forge_gpu::Target;

use crate::bvh;
use crate::compute;
use crate::maps::NORMAL;
use crate::surface;

/// `--nb-second-rays`, raised from the default 32 ("A high value will produce less noise"): at 32
/// the one-texel edge lines of a 2048 map came out dotted.
const SAMPLES: u32 = 128;
const BATCHES: u32 = 4;
/// Sampling radius, of the bounding box's largest side: this estimator matches the reference
/// map on the material ball best at 0.0033 (correlation 0.980;
/// the smaller radii tested read 0.978 down to 0.954).
const SAMPLING_RADIUS: f32 = 0.0033;
/// The histogram's bins and the bound after them (curvature.wgsl).
const BINS: usize = 4096;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    first: u32,
    samples: u32,
    radius: f32,
    flags: u32,
}

const PARAMS: usize = std::mem::size_of::<Params>();

pub struct Curvature {
    out: Target,
    bind: wgpu::BindGroup,
    accumulate: wgpu::ComputePipeline,
    measure: wgpu::ComputePipeline,
    bound: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
    /// The batches' parameter offsets, `BATCHES` per sample point.
    offsets: Vec<u32>,
    size: u32,
}

impl Curvature {
    /// `surface` is the surface map (`surface.rs`), `normal` the dilated normal map.
    pub fn new(
        device: &wgpu::Device,
        bvh: &bvh::Buffers,
        bounds: &Bounds,
        [surface, normal]: [&Target; 2],
        size: u32,
    ) -> Result<Self, String> {
        let source = concat!(
            include_str!("../shaders/trace.wgsl"),
            include_str!("../shaders/curvature.wgsl")
        );
        let module = forge_gpu::compile(device, "curvature.wgsl", source)?;
        let acc = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("curvature sums"),
            size: u64::from(size) * u64::from(size) * 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let hist = compute::storage_buffer(
            device,
            "curvature histogram",
            bytemuck::cast_slice(&[0u32; BINS + 1]),
        );

        let radius = SAMPLING_RADIUS * bounds.largest_side().max(1e-4);
        let blocks: Vec<Params> = (0..surface::SAMPLES * BATCHES)
            .map(|i| {
                let batch = i % BATCHES;
                Params {
                    first: i * SAMPLES / BATCHES,
                    samples: SAMPLES / BATCHES,
                    radius,
                    flags: u32::from(batch == 0) | (u32::from(batch == BATCHES - 1) << 1),
                }
            })
            .collect();
        let (uniforms, offsets) = compute::batches(device, "curvature params", &blocks);
        let out = Target::new(
            device,
            "normal map",
            [size; 2],
            NORMAL,
            wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            1,
        );

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("curvature"),
            entries: &[
                compute::uniform(0, PARAMS, true),
                compute::storage(1, true),
                compute::storage(2, true),
                compute::storage(3, true),
                compute::texture(4),
                compute::texture(5),
                compute::storage(6, false),
                compute::storage(7, false),
                compute::storage_texture(8, NORMAL),
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("curvature"),
            layout: &bgl,
            entries: &[
                compute::block_entry(0, &uniforms, PARAMS),
                compute::buffer_entry(1, &bvh.nodes),
                compute::buffer_entry(2, &bvh.tris),
                compute::buffer_entry(3, &bvh.normals),
                compute::texture_entry(4, surface),
                compute::texture_entry(5, normal),
                compute::buffer_entry(6, &acc),
                compute::buffer_entry(7, &hist),
                compute::texture_entry(8, &out),
            ],
        });
        Ok(Self {
            out,
            accumulate: compute::pipeline(device, &module, &bgl, "accumulate"),
            measure: compute::pipeline(device, &module, &bgl, "measure"),
            bound: compute::pipeline(device, &module, &bgl, "bound"),
            resolve: compute::pipeline(device, &module, &bgl, "resolve"),
            bind,
            offsets,
            size,
        })
    }

    /// Adds the curvature at sample point `sample`, with the surface map drawn for it.
    pub fn accumulate(&self, device: &wgpu::Device, queue: &wgpu::Queue, sample: u32) {
        let per = BATCHES as usize;
        for offset in self.offsets.iter().skip(sample as usize * per).take(per) {
            compute::dispatch(
                device,
                queue,
                &self.accumulate,
                &self.bind,
                &[*offset],
                self.size,
            );
        }
    }

    /// The normal map with the curvature of every sample point added in w.
    pub fn resolve(self, device: &wgpu::Device, queue: &wgpu::Queue) -> Target {
        let passes = [
            (&self.measure, self.size),
            (&self.bound, 1),
            (&self.resolve, self.size),
        ];
        for (pipeline, size) in passes {
            compute::dispatch(device, queue, pipeline, &self.bind, &[0], size);
        }
        self.out
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use glam::Vec3;

    use crate::gpu_test::{Gpu, cube, ell};

    #[test]
    fn a_cube_is_flat_on_its_faces_and_convex_at_its_edges() {
        let Some(gpu) = Gpu::new() else { return };
        let top = (Vec3::new(0.0, 0.0, 1.0), Vec3::Z);
        let edge = (Vec3::new(0.999, 0.0, 1.0), Vec3::Z);
        let corner = (Vec3::new(0.999, 0.999, 1.0), Vec3::Z);
        let c = gpu.curvature(&cube(), &[top, edge, corner]);
        assert!((c[0] - 0.5).abs() < 1e-3, "top {}", c[0]);
        assert!(c[1] > 0.6, "edge {}", c[1]);
        assert!(c[2] >= c[1], "corner {} edge {}", c[2], c[1]);
    }

    #[test]
    fn an_inside_corner_is_concave() {
        let Some(gpu) = Gpu::new() else { return };
        let floor = (Vec3::new(1.0, 0.0, 0.0), Vec3::Z);
        let corner = (Vec3::new(0.001, 0.0, 0.0), Vec3::Z);
        let c = gpu.curvature(&ell(), &[floor, corner]);
        assert!((c[0] - 0.5).abs() < 1e-3, "floor {}", c[0]);
        assert!(c[1] < 0.4, "corner {}", c[1]);
    }
}
