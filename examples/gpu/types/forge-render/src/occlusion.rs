//! Ambient occlusion and thickness for the extras map, ray traced against a BVH of the
//! rest-pose mesh (`bvh.rs`). Each texel shoots cosine-distributed rays over the full
//! hemisphere from its surface point, as Cycles' AO does (`kernel/svm/ao.h`): about the
//! interpolated vertex normal, skipping the triangle the ray starts on, every other triangle
//! occluding from either side ("Ignore Backface: Never"). AO rays go outward, where an occluder
//! darkens linearly less the farther it is; thickness rays go inward and stop only where they
//! leave the solid (backfaces ignored), the mean distance divided by the map's largest (on
//! the material ball the thinnest texels stay well
//! above 0, so the min is taken over ray distances, which reach 0 at every crease). On
//! smooth-shaded triangles the rays start from the point lifted onto the smooth surface
//! (`terminator` in trace.wgsl), so coarse curved meshes don't bake their facets. Rays run
//! for each of `surface::SAMPLES` sample points per texel (`accumulate`), in batches of one
//! submit each, so no dispatch runs long enough to trip the driver's timeout.
//!
//! The same pass bakes how close each texel is to a different part: at each sample point, from the
//! flat surface point, the BVH's nearest triangle on another part (`distance_to_other_part` in
//! trace.wgsl), as a factor of `NEAR_MAX_DIST`. It reads the rest pose and ignores AO isolation.

use bytemuck::{Pod, Zeroable};
use forge_core::Bounds;
use forge_gpu::Target;

use crate::bvh;
use crate::compute;
use crate::surface;

pub const EXTRAS: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Rays per sample point: 2x2 antialiasing with at least 128 secondary rays for AO. Distribution cosine, spread angle 180.
const RAYS: u32 = 128;
const BATCH: u32 = 8;
/// Distances as factors of the bounding box's largest side ("Relative to Bounding Box"), with
/// min occluder distance 1e-5 for
/// both, max occluder distance 1.0 for AO and 0.1 for thickness. Against a reference bake of the
/// material ball the largest side gives r 0.999 and a mean
/// difference of 0.006 for both maps; the diagonal gave thickness r 0.981, mean 0.072.
const MIN_DIST: f32 = 1e-5;
const AO_MAX_DIST: f32 = 1.0;
const THICK_MAX_DIST: f32 = 0.1;
/// Where the near-other-part map reaches 1, as a factor of the same largest side.
const NEAR_MAX_DIST: f32 = 0.1;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    first: u32,
    rays: u32,
    min_dist: f32,
    ao_max: f32,
    thick_max: f32,
    near_max: f32,
    pad: [f32; 2],
}

const PARAMS: usize = std::mem::size_of::<Params>();

pub struct Occlusion {
    extras: Target,
    bind: wgpu::BindGroup,
    accumulate: wgpu::ComputePipeline,
    near_part: wgpu::ComputePipeline,
    measure: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
    /// The batches' parameter offsets, `RAYS / BATCH` per sample point.
    offsets: Vec<u32>,
    size: u32,
}

impl Occlusion {
    /// `surface` is the surface map (`surface.rs`), `part` the dilated part map.
    pub fn new(
        device: &wgpu::Device,
        bvh: &bvh::Buffers,
        bounds: &Bounds,
        [surface, part]: [&Target; 2],
        size: u32,
    ) -> Result<Self, String> {
        let source = concat!(
            include_str!("../shaders/trace.wgsl"),
            include_str!("../shaders/occlusion.wgsl")
        );
        let module = forge_gpu::compile(device, "occlusion.wgsl", source)?;
        let acc = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("occlusion sums"),
            size: u64::from(size) * u64::from(size) * 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let near_sums = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("near-other-part sums"),
            size: u64::from(size) * u64::from(size) * 8,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let range = compute::storage_buffer(
            device,
            "thickness range",
            bytemuck::cast_slice(&[f32::MAX.to_bits(), 0u32]),
        );

        let side = bounds.largest_side().max(1e-4);
        let blocks: Vec<Params> = (0..surface::SAMPLES * RAYS / BATCH)
            .map(|i| Params {
                first: i * BATCH,
                rays: BATCH,
                min_dist: MIN_DIST * side,
                ao_max: AO_MAX_DIST * side,
                thick_max: THICK_MAX_DIST * side,
                near_max: NEAR_MAX_DIST * side,
                pad: [0.0; 2],
            })
            .collect();
        let (uniforms, offsets) = compute::batches(device, "occlusion params", &blocks);
        let extras = Target::new(
            device,
            "extras map",
            [size; 2],
            EXTRAS,
            wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            1,
        );

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("occlusion"),
            entries: &[
                compute::uniform(0, PARAMS, true),
                compute::storage(1, true),
                compute::storage(2, true),
                compute::storage(3, true),
                compute::texture(4),
                compute::texture(5),
                compute::storage(6, false),
                compute::storage(7, false),
                compute::storage_texture(8, EXTRAS),
                compute::storage(9, true),
                compute::storage(10, false),
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("occlusion"),
            layout: &bgl,
            entries: &[
                compute::block_entry(0, &uniforms, PARAMS),
                compute::buffer_entry(1, &bvh.nodes),
                compute::buffer_entry(2, &bvh.tris),
                compute::buffer_entry(3, &bvh.normals),
                compute::texture_entry(4, surface),
                compute::texture_entry(5, part),
                compute::buffer_entry(6, &acc),
                compute::buffer_entry(7, &range),
                compute::texture_entry(8, &extras),
                compute::buffer_entry(9, &bvh.parts),
                compute::buffer_entry(10, &near_sums),
            ],
        });
        Ok(Self {
            extras,
            accumulate: compute::pipeline(device, &module, &bgl, "accumulate"),
            near_part: compute::pipeline(device, &module, &bgl, "near_part"),
            measure: compute::pipeline(device, &module, &bgl, "measure"),
            resolve: compute::pipeline(device, &module, &bgl, "resolve"),
            bind,
            offsets,
            size,
        })
    }

    /// Adds the rays of sample point `sample`, with the surface map drawn for it.
    pub fn accumulate(&self, device: &wgpu::Device, queue: &wgpu::Queue, sample: u32) {
        let per = (RAYS / BATCH) as usize;
        let first = sample as usize * per;
        for offset in self.offsets.iter().skip(first).take(per) {
            compute::dispatch(
                device,
                queue,
                &self.accumulate,
                &self.bind,
                &[*offset],
                self.size,
            );
        }
        self.near_part(device, queue);
    }

    /// Adds the near-other-part distance of the sample point the surface map is drawn for.
    pub(crate) fn near_part(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        compute::dispatch(device, queue, &self.near_part, &self.bind, &[0], self.size);
    }

    /// The extras map (r: AO, g: thickness, b: part index, a: near other part) from every sample
    /// point added.
    pub fn resolve(self, device: &wgpu::Device, queue: &wgpu::Queue) -> Target {
        for pipeline in [&self.measure, &self.resolve] {
            compute::dispatch(device, queue, pipeline, &self.bind, &[0], self.size);
        }
        self.extras
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use glam::Vec3;

    use crate::gpu_test::{Gpu, cube, ell};

    #[test]
    fn a_cube_top_is_open_and_thinner_at_its_edge() {
        let Some(gpu) = Gpu::new() else { return };
        let top = (Vec3::new(0.0, 0.0, 1.0), Vec3::Z);
        let edge = (Vec3::new(0.999, 0.0, 1.0), Vec3::Z);
        let m = gpu.occlusion(&cube(), &[top, edge]);
        assert!(m[0][0] > 0.999, "top AO {}", m[0][0]);
        assert!(
            m[1][1] < m[0][1],
            "edge thickness {} top {}",
            m[1][1],
            m[0][1]
        );
    }

    #[test]
    fn an_inside_corner_is_occluded() {
        let Some(gpu) = Gpu::new() else { return };
        let floor = (Vec3::new(1.9, 0.0, 0.0), Vec3::Z);
        let corner = (Vec3::new(0.001, 0.0, 0.0), Vec3::Z);
        let m = gpu.occlusion(&ell(), &[floor, corner]);
        assert!(m[1][0] < 0.9, "corner AO {}", m[1][0]);
        assert!(m[1][0] < m[0][0], "corner AO {} floor {}", m[1][0], m[0][0]);
    }
}
