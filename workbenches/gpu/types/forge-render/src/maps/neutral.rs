use forge_gpu::Target;

use super::NORMAL;
use crate::{compute, occlusion::EXTRAS};

pub(super) fn maps(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    [normal, part]: [&Target; 2],
    size: u32,
) -> Result<(Target, Target), String> {
    let module = forge_gpu::compile(
        device,
        "maps_neutral.wgsl",
        include_str!("../../shaders/maps_neutral.wgsl"),
    )?;
    let usage = wgpu::TextureUsages::STORAGE_BINDING
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_SRC;
    let output_normal = Target::new(device, "neutral normal", [size; 2], NORMAL, usage, 1);
    let extras = Target::new(device, "neutral extras", [size; 2], EXTRAS, usage, 1);
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("neutral maps"),
        entries: &[
            compute::texture(0),
            compute::texture(1),
            compute::storage_texture(2, NORMAL),
            compute::storage_texture(3, EXTRAS),
        ],
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("neutral maps"),
        layout: &layout,
        entries: &[
            compute::texture_entry(0, normal),
            compute::texture_entry(1, part),
            compute::texture_entry(2, &output_normal),
            compute::texture_entry(3, &extras),
        ],
    });
    let pipeline = compute::pipeline(device, &module, &layout, "main");
    compute::dispatch(device, queue, &pipeline, &bind, &[], size);
    Ok((output_normal, extras))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GpuAsset, SceneRenderer, bvh, curvature::Curvature, occlusion::Occlusion, surface::Surface,
    };
    use forge_core::{AssetInfo, Bounds, MeshData, PartInfo, Vertex};

    #[test]
    fn fast_neutral_maps_match_original_unsampled_bakers_exactly() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut mesh = MeshData::default();
        for part in 0..2 {
            for (corner, uv) in [[0.0, 0.0], [0.4, 0.0], [0.0, 1.0]].into_iter().enumerate() {
                mesh.vertices.push(Vertex {
                    position: [uv[0] + part as f32, uv[1], 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [uv[0] + part as f32 * 0.5, uv[1]],
                    curvature: 0.9,
                    part,
                });
                mesh.indices.push(part * 3 + corner as u32);
            }
        }
        let info = AssetInfo {
            bounds: Bounds {
                min: [0.0; 3],
                max: [2.0, 1.0, 0.0],
            },
            parts: vec![PartInfo::default(); 2],
            ..Default::default()
        };
        let scene = SceneRenderer::new(&device, &queue).unwrap();
        let gpu = GpuAsset::new(&device, &scene, &info, &mesh);
        let [_, normal, part] = super::super::raster(&device, &queue, &gpu, 16).unwrap();
        let tree = bvh::Bvh::build(&mesh);
        let buffers = tree.upload(&device);
        let surface = Surface::new(&device, &tree, &mesh, 16).unwrap();
        let old_ao = Occlusion::new(&device, &buffers, &info.bounds, [&surface.map, &part], 16)
            .unwrap()
            .resolve(&device, &queue);
        let old_normal =
            Curvature::new(&device, &buffers, &info.bounds, [&surface.map, &normal], 16)
                .unwrap()
                .resolve(&device, &queue);
        let (new_normal, new_ao) = maps(&device, &queue, [&normal, &part], 16).unwrap();
        let read = |target: &Target| {
            forge_gpu::read_texture(&device, &queue, &target.texture, [16; 2], 1, 8).unwrap()
        };
        assert_eq!(read(&old_ao), read(&new_ao));
        assert_eq!(read(&old_normal), read(&new_normal));
    }
}
