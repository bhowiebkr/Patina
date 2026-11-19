//! View-only target inputs survive document/source rebuilds without changing saved ownership.
use super::*;
#[cfg(test)]
mod tests;

pub struct PreviewBinding {
    pub info: AssetInfo,
    pub mesh: MeshData,
    pub maps: MeshMaps,
    pub gpu: GpuAsset,
    pub camera: OrbitCamera,
    pub environment: Option<Environment>,
    pub environment_name: String,
    pub light: Light,
    pub part_table: Vec<[f32; 4]>,
}

struct Reference {
    info: AssetInfo,
    mesh: MeshData,
    maps: MeshMaps,
    gpu: GpuAsset,
    camera: OrbitCamera,
    orientation: glam::Quat,
    angles: [f32; 2],
    lighting: [f32; 3],
}

pub(super) struct Current {
    reference: Reference,
    pub light: Light,
    environment: Option<Environment>,
    environment_name: String,
    part_table: Vec<[f32; 4]>,
    pub pose: Vec<glam::Mat4>,
}

impl Session {
    pub fn display_environment_name(&self) -> &str {
        self.preview
            .as_ref()
            .map_or(&self.environment_name, |p| &p.environment_name)
    }
    pub(crate) fn validate_preview_maps(&self, graph: &Graph) -> Result<(), String> {
        if self.preview.is_some() && !self._maps.baked {
            use requirements::BakedMap;
            let required = requirements::graph(self.engine.registry(), graph.clone())?;
            let missing: Vec<_> = required
                .baked_maps
                .into_iter()
                .filter(|map| {
                    matches!(
                        map,
                        BakedMap::AmbientOcclusion
                            | BakedMap::Curvature
                            | BakedMap::Thickness
                            | BakedMap::NearOtherPart
                    )
                })
                .collect();
            if !missing.is_empty() {
                return Err(format!(
                    "Current asset has no traced bake for {missing:?}; bake its mesh maps in Paint"
                ));
            }
        }
        Ok(())
    }

    pub fn preview_is_current(&self) -> bool {
        self.preview.is_some()
    }

    pub(crate) fn document_lighting(&self) -> [f32; 3] {
        self.preview
            .as_ref()
            .map_or([self.exposure, self.rotation, self.tilt], |p| {
                p.reference.lighting
            })
    }

    pub(crate) fn set_document_lighting(&mut self, lighting: [f32; 3]) {
        if let Some(preview) = &mut self.preview {
            preview.reference.lighting = lighting;
        } else {
            [self.exposure, self.rotation, self.tilt] = lighting;
        }
    }

    pub(crate) fn configure_engine(&self, device: &wgpu::Device, engine: &mut Engine) {
        super::configure_engine(device, engine, &self._maps, &self.info);
        if let Some(preview) = &self.preview {
            engine.set_part_table(device, &preview.part_table);
        }
    }

    pub(crate) fn apply_preview_environment(&mut self, device: &wgpu::Device) {
        let environment = self
            .preview
            .as_ref()
            .map_or(Some(&self._environment), |p| p.environment.as_ref());
        self.scene.set_environment(device, environment);
    }

    pub fn bind_preview(&mut self, device: &wgpu::Device, binding: PreviewBinding) {
        let old = Reference {
            info: std::mem::replace(&mut self.info, binding.info),
            mesh: std::mem::replace(&mut self._mesh, binding.mesh),
            maps: std::mem::replace(&mut self._maps, binding.maps),
            gpu: std::mem::replace(&mut self.ball, binding.gpu),
            camera: std::mem::replace(&mut self.camera, binding.camera),
            orientation: self.ball_orientation,
            angles: [self.ball_yaw, self.ball_pitch],
            lighting: [self.exposure, self.rotation, self.tilt],
        };
        let reference = self.preview.take().map_or(old, |p| p.reference);
        self.preview = Some(Current {
            reference,
            light: binding.light,
            environment: binding.environment,
            environment_name: binding.environment_name,
            part_table: binding.part_table,
            pose: self.ball.part_pose(),
        });
        [self.exposure, self.rotation, self.tilt] = [
            binding.light.exposure,
            binding.light.env_yaw * 360.,
            binding.light.env_pitch * 360.,
        ];
        self.ball_orientation = glam::Quat::IDENTITY;
        self.ball_yaw = 0.;
        self.ball_pitch = 0.;
        self.apply_preview_environment(device);
        super::configure_engine(device, &mut self.engine, &self._maps, &self.info);
        if let Some(preview) = &self.preview {
            self.engine.set_part_table(device, &preview.part_table);
        }
        self.engine.graph_changed(&self.flat);
        self.dirty = true;
    }

    /// The saved cover: always the reference ball, even while the preview shows an asset.
    /// The asset's binding comes back marked dirty, so its next evaluation restores its material.
    pub fn cover(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<View, String> {
        let Some(mut preview) = self.preview.take() else {
            self.evaluate(device, queue)?;
            return Ok(self.thumbnail_target(device, queue));
        };
        self.swap_reference(&mut preview.reference);
        self.apply_preview_environment(device);
        super::configure_engine(device, &mut self.engine, &self._maps, &self.info);
        self.engine.graph_changed(&self.flat);
        self.dirty = true;
        let cover = self
            .evaluate(device, queue)
            .map(|_| self.thumbnail_target(device, queue));
        self.swap_reference(&mut preview.reference);
        self.preview = Some(preview);
        self.apply_preview_environment(device);
        super::configure_engine(device, &mut self.engine, &self._maps, &self.info);
        if let Some(preview) = &self.preview {
            self.engine.set_part_table(device, &preview.part_table);
        }
        self.engine.graph_changed(&self.flat);
        self.dirty = true;
        cover
    }

    fn swap_reference(&mut self, reference: &mut Reference) {
        std::mem::swap(&mut self.info, &mut reference.info);
        std::mem::swap(&mut self._mesh, &mut reference.mesh);
        std::mem::swap(&mut self._maps, &mut reference.maps);
        std::mem::swap(&mut self.ball, &mut reference.gpu);
        std::mem::swap(&mut self.camera, &mut reference.camera);
        std::mem::swap(&mut self.ball_orientation, &mut reference.orientation);
        let angles = [self.ball_yaw, self.ball_pitch];
        [self.ball_yaw, self.ball_pitch] = reference.angles;
        reference.angles = angles;
        let lighting = [self.exposure, self.rotation, self.tilt];
        [self.exposure, self.rotation, self.tilt] = reference.lighting;
        reference.lighting = lighting;
    }

    /// Follow Paint's channel view without rebinding the pinned asset.
    pub fn set_preview_channel(&mut self, channel: u32) {
        if let Some(preview) = &mut self.preview {
            preview.light.channel = channel;
        }
    }

    pub fn reference_preview(&mut self, device: &wgpu::Device) {
        let Some(preview) = self.preview.take() else {
            return;
        };
        let original = preview.reference;
        self.info = original.info;
        self._mesh = original.mesh;
        self._maps = original.maps;
        self.ball = original.gpu;
        self.camera = original.camera;
        self.ball_orientation = original.orientation;
        [self.ball_yaw, self.ball_pitch] = original.angles;
        [self.exposure, self.rotation, self.tilt] = original.lighting;
        self.apply_preview_environment(device);
        super::configure_engine(device, &mut self.engine, &self._maps, &self.info);
        self.engine.graph_changed(&self.flat);
        self.dirty = true;
    }
}
