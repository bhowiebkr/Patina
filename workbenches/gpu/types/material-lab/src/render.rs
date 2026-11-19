//! The Surface PBR renderer on a Patina viewport, with explicit display color conversion.
use crate::{Material, world};
use forge_render::{GpuAsset, Light, OrbitCamera, View};
use patina_gpu::{Gpu, ViewportSurface, gpu, wgpu};
use patina_sdk::Viewport;

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;

pub struct Look {
    world: std::sync::Arc<crate::World>,
    asset: GpuAsset,
    pub camera: OrbitCamera,
    pub light: Light,
    pub target: View,
    revision: u64,
    blit: Option<Blit>,
    surface: ViewportSurface,
    pub dirty: bool,
}
struct Blit {
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
}

impl Look {
    pub fn new(size: [u32; 2]) -> Result<Self, String> {
        let gpu = gpu()?;
        let target = world()?;
        let world = &target.0;
        Ok(Self {
            asset: world.ball.preview_copy(&gpu.device, &world.scene),
            camera: world.camera,
            light: world.light(),
            target: View::new(&gpu.device, &world.scene, size),
            revision: 0,
            blit: None,
            surface: ViewportSurface::default(),
            dirty: true,
            world: target,
        })
    }
    pub fn render(&mut self, material: &Material, size: [u32; 2]) -> Result<(), String> {
        let gpu = gpu()?;
        let image = material
            .0
            .as_ref()
            .ok_or("Connect a material to Output, then run the graph")?;
        if image.socket != forge_nodes::Socket::Material {
            return Err("Preview needs a material".into());
        }
        if !std::sync::Arc::ptr_eq(&self.world, &image.target) {
            self.world = std::sync::Arc::clone(&image.target);
            self.asset = self
                .world
                .0
                .ball
                .preview_copy(&gpu.device, &self.world.0.scene);
            self.camera = self.world.0.camera;
            self.target = View::new(
                &gpu.device,
                &self.world.0.scene,
                size.map(|v| v.clamp(1, 2048)),
            );
            self.blit = None;
            self.revision = 0;
            self.dirty = true;
        }
        let world = &self.world.0;
        let size = size.map(|v| v.clamp(1, 2048));
        if size != self.target.size {
            self.target = View::new(&gpu.device, &world.scene, size);
            self.blit = None;
            self.dirty = true;
        }
        if image.revision != self.revision {
            let layers = &image.planes.layers;
            let i = image.layer as usize;
            self.asset.set_material(
                &gpu.device,
                &gpu.queue,
                &world.scene,
                Some([&layers[i], &layers[i + 1], &layers[i + 2]]),
            );
            self.revision = image.revision;
            self.dirty = true;
        }
        if self.dirty {
            world.scene.render(
                &gpu.device,
                &gpu.queue,
                &self.target,
                &self.camera,
                &self.light,
                &self.asset,
            );
            self.dirty = false;
        }
        Ok(())
    }
    pub fn draw(&mut self, viewport: &Viewport, material: &Material) -> Result<(), String> {
        self.render(material, [viewport.rect[2], viewport.rect[3]])?;
        let gpu = gpu()?;
        let source = &self.target.color.view;
        let mut error = None;
        self.surface
            .draw(viewport, wgpu::Color::BLACK, |pass, format| {
                if self.blit.as_ref().is_none_or(|b| b.format != format) {
                    match Blit::new(gpu, format, source) {
                        Ok(blit) => self.blit = Some(blit),
                        Err(e) => error = Some(e),
                    }
                }
                if let Some(blit) = &self.blit {
                    pass.set_pipeline(&blit.pipeline);
                    pass.set_bind_group(0, &blit.bind, &[]);
                    pass.draw(0..3, 0..1);
                }
            })?;
        error.map_or(Ok(()), Err)
    }
}

impl Blit {
    fn new(
        gpu: &Gpu,
        format: wgpu::TextureFormat,
        source: &wgpu::TextureView,
    ) -> Result<Self, String> {
        let module = gpu.shader("material preview blit", include_str!("blit.wgsl"))?;
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("material preview blit"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(if format.is_srgb() {
                        "linear"
                    } else {
                        "encoded"
                    }),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material preview blit"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            }],
        });
        Ok(Self {
            format,
            pipeline,
            bind,
        })
    }
}
