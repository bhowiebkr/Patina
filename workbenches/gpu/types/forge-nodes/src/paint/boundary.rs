use forge_gpu::Target;

pub(super) struct Boundary {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    before: Option<Target>,
}

impl Boundary {
    pub fn new(device: &wgpu::Device) -> Result<Self, String> {
        let mut entries = (0..3)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .collect::<Vec<_>>();
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::ReadWrite,
                format: super::MASK_FORMAT,
                view_dimension: wgpu::TextureViewDimension::D2,
            },
            count: None,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("paint object boundary"),
            entries: &entries,
        });
        let source = include_str!("../../kernels/part_boundary.wgsl");
        let module = forge_gpu::compile(device, "part_boundary.wgsl", source)?;
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("paint object boundary"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = forge_gpu::validated(device, "part_boundary.wgsl", || {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("paint object boundary"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        })?;
        Ok(Self {
            layout,
            pipeline,
            before: None,
        })
    }

    pub fn backup(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        mask: &Target,
    ) {
        if self
            .before
            .as_ref()
            .is_none_or(|before| before.size != mask.size)
        {
            self.before = Some(Target::new(
                device,
                "paint before object guard",
                mask.size,
                super::MASK_FORMAT,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                1,
            ));
        }
        if let Some(before) = &self.before {
            encoder.copy_texture_to_texture(
                mask.texture.as_image_copy(),
                before.texture.as_image_copy(),
                mask.texture.size(),
            );
        }
    }

    pub fn restore(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        hit: &wgpu::TextureView,
        extras: &wgpu::TextureView,
        mask: &Target,
    ) {
        let Some(before) = &self.before else {
            return;
        };
        let views = [hit, extras, &before.view, &mask.view];
        let entries = views
            .iter()
            .enumerate()
            .map(|(binding, view)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect::<Vec<_>>();
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("paint object boundary"),
            layout: &self.layout,
            entries: &entries,
        });
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(mask.size[0].div_ceil(8), mask.size[1].div_ceil(8), 1);
    }
}
