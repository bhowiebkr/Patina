//! Native material values and kernel state for the GPU workbench.
pub mod export;
pub mod object;
mod preview;
mod world;
pub use world::{World, bake, world};
mod value;
pub use value::{BakeTarget, Image};
pub mod render;
use forge_nodes::{NativePass, Node, NodeId, Registry};
pub use forge_nodes::{ParamType, Socket};
use patina_gpu::gpu;
use patina_sdk::{Param, Text, Viewer};
pub use preview::Thumbnail;
use std::{
    collections::BTreeMap,
    fmt::{self, Write},
    sync::{Arc, OnceLock},
};

pub const SIZE: u32 = 512;
pub const COPPER: &[u8] = include_bytes!("../../forge-runtime/assets/copper.png");

/// Complete the batch once, so every asynchronous thumbnail is ready for the first GUI draw.
pub fn finish_batch() -> Result<(), String> {
    gpu()?
        .device
        .poll(patina_gpu::wgpu::PollType::wait_indefinitely())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn recipe() -> &'static forge_png::Material {
    static RECIPE: OnceLock<forge_png::Material> = OnceLock::new();
    RECIPE
        .get_or_init(|| forge_png::decode(COPPER).expect("bundled copper recipe passed validation"))
}

fn registry() -> Registry {
    let mut files = Registry::embedded().files().clone();
    files.extend(recipe().manifest.sources.clone());
    let mut registry = Registry::from_files(files);
    registry.set_linked(recipe().manifest.linked_wgsl.clone());
    registry
}

pub fn catalog() -> &'static Registry {
    static CATALOG: OnceLock<Registry> = OnceLock::new();
    CATALOG.get_or_init(registry)
}

#[derive(Clone, Default)]
pub struct Mask(pub Option<Image>);
#[derive(Clone, Default)]
pub struct Material(pub Option<Image>);

// An unwired input selects the original kernel's default, without allocating a GPU texture.
impl Param for Mask {
    fn default_value() -> Self {
        Self::default()
    }
    fn from_text(text: &str) -> Result<Self, String> {
        if text.trim().is_empty() {
            Ok(Self::default())
        } else {
            Err("Connect a Mask wire, or leave the input empty for the kernel default".into())
        }
    }
    fn to_text(&self) -> String {
        String::new()
    }
}
impl Param for Material {
    fn default_value() -> Self {
        Self::default()
    }
    fn from_text(text: &str) -> Result<Self, String> {
        if text.trim().is_empty() {
            Ok(Self::default())
        } else {
            Err("Connect a Material wire, or leave the input empty for the kernel default".into())
        }
    }
    fn to_text(&self) -> String {
        String::new()
    }
}

fn describe(image: Option<&Image>, name: &str, out: &mut Text) -> fmt::Result {
    match image {
        Some(image) => write!(
            out,
            "{name} / {} x {} / float32",
            image.planes.size, image.planes.size
        ),
        None => write!(out, "{name} / unconnected"),
    }
}
impl Viewer for Mask {
    fn text(&self, out: &mut Text) -> fmt::Result {
        describe(self.0.as_ref(), "Mask", out)
    }
}
impl Viewer for Material {
    fn text(&self, out: &mut Text) -> fmt::Result {
        describe(self.0.as_ref(), "Material", out)
    }
}

pub struct Operator {
    pub node: Node,
    pass: Option<NativePass>,
    pub thumbnail: Thumbnail,
    pub revision: u64,
    pub error: String,
    pub source_path: Option<std::path::PathBuf>,
    source: String,
    material_preview: Option<render::Look>,
    target: Option<Arc<World>>,
    pub size: u32,
}

impl Operator {
    pub fn new(kind: &str) -> Self {
        Self {
            node: Node {
                id: NodeId(1),
                kind: kind.into(),
                title: String::new(),
                pos: [0.0; 2],
                params: BTreeMap::new(),
                text: BTreeMap::new(),
                size: None,
                uv: false,
                defaults_version: 5,
            },
            pass: None,
            thumbnail: Thumbnail::default(),
            revision: 0,
            error: String::new(),
            source_path: None,
            source: String::new(),
            material_preview: None,
            target: None,
            size: SIZE,
        }
    }
    pub fn value(&self, key: &str) -> f32 {
        let stored = self.node.params.get(key).filter(|v| v.len() == 1);
        stored.and_then(|v| v.first()).copied().unwrap_or_else(|| {
            catalog()
                .get(&self.node.kind)
                .and_then(|d| d.param(key))
                .and_then(|p| p.default.first())
                .copied()
                .unwrap_or(0.0)
        })
    }
    pub fn run_in(
        &mut self,
        target: &BakeTarget,
        params: &[(&str, f32)],
        inputs: &[Option<&Image>],
    ) -> Result<Image, String> {
        let world = target
            .0
            .as_ref()
            .or_else(|| inputs.iter().flatten().next().map(|i| &i.target));
        let world = match world {
            Some(w) => Arc::clone(w),
            None => crate::world()?,
        };
        if inputs
            .iter()
            .flatten()
            .any(|i| !Arc::ptr_eq(&i.target, &world))
        {
            return Err("Connect inputs baked for the same object and resolution".into());
        }
        if self
            .target
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &world))
        {
            self.pass = None;
            self.material_preview = None;
            self.revision = 0;
            self.size = world.1;
            self.target = Some(world);
        }
        self.run_values(params, inputs)
    }
    pub fn run(
        &mut self,
        params: &[(&str, f32)],
        inputs: &[Option<&Image>],
    ) -> Result<Image, String> {
        self.run_in(&BakeTarget::default(), params, inputs)
    }
    fn run_values(
        &mut self,
        params: &[(&str, f32)],
        inputs: &[Option<&Image>],
    ) -> Result<Image, String> {
        for &(key, value) in params {
            if !value.is_finite() {
                return Err(format!("{key} must be finite"));
            }
            let param = catalog()
                .get(&self.node.kind)
                .and_then(|d| d.param(key))
                .ok_or("Unknown material parameter")?;
            let value = match &param.ty {
                ParamType::Int { min, max } => value.round().clamp(*min as f32, *max as f32),
                ParamType::Enum(names) => value
                    .round()
                    .clamp(0.0, names.len().saturating_sub(1) as f32),
                ParamType::Float { min, max } if !param.soft_range => value.clamp(*min, *max),
                _ => value,
            };
            self.node.params.insert(key.into(), vec![value]);
        }
        let result = self.evaluate(inputs);
        self.error = result.as_ref().err().cloned().unwrap_or_default();
        result
    }
    fn evaluate(&mut self, inputs: &[Option<&Image>]) -> Result<Image, String> {
        let gpu = gpu()?;
        let target = Arc::clone(self.target.as_ref().ok_or("No bake target")?);
        let engine = &target.0.engine;
        let source = self
            .source_path
            .as_ref()
            .map(std::fs::read_to_string)
            .transpose()
            .map_err(|e| format!("Material shader: {e}"))?;
        if self.pass.is_none() || source.as_ref().is_some_and(|s| *s != self.source) {
            let candidate = NativePass::with_source(
                engine,
                &gpu.device,
                &self.node.kind,
                self.size,
                source.as_deref(),
            )?;
            self.pass = Some(candidate);
            self.source = source.unwrap_or_default();
        }
        let pass = self.pass.as_mut().ok_or("No material kernel")?;
        let mut raw = [None; 8];
        if inputs.len() > raw.len() {
            return Err("Too many kernel inputs".into());
        }
        for (out, input) in raw.iter_mut().zip(inputs) {
            *out = input.map(|i| &i.raw);
        }
        let result = Image {
            raw: pass.run(
                engine,
                &gpu.device,
                &gpu.queue,
                &self.node,
                &raw[..inputs.len()],
            )?,
            target,
        };
        if result.revision != self.revision {
            if result.socket == Socket::Material {
                if self.material_preview.is_none() {
                    self.material_preview = Some(render::Look::new([128, 128])?);
                }
                if let Some(look) = &mut self.material_preview {
                    look.render(&Material(Some(result.clone())), [128, 128])?;
                    self.thumbnail.request(gpu, &look.target.color.texture);
                }
            } else {
                self.thumbnail.request(gpu, pass.thumbnail());
            }
            self.revision = result.revision;
        }
        Ok(result)
    }
    pub fn gradient(&mut self, text: &str) -> Result<(), String> {
        let values = parse_gradient(text)?;
        self.node.params.insert("color".into(), values);
        Ok(())
    }
}

pub fn parse_gradient(text: &str) -> Result<Vec<f32>, String> {
    let values: Vec<f32> = serde_json::from_str(text).map_err(|e| format!("Color ramp: {e}"))?;
    let count = values.first().copied().unwrap_or(0.0);
    if !count.is_finite()
        || count.fract() != 0.0
        || !(1.0..=256.0).contains(&count)
        || values.len() != 1 + count as usize * 5
        || values.iter().any(|v| !v.is_finite())
    {
        return Err(
            "Color ramp needs 1 to 256 stops: count, then position and RGBA for each stop".into(),
        );
    }
    let mut last = -1.0;
    for stop in values[1..].as_chunks::<5>().0.iter() {
        if !(0.0..=1.0).contains(&stop[0]) || stop[0] < last {
            return Err("Color ramp positions must increase from 0 to 1".into());
        }
        last = stop[0];
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unwired_material_inputs_select_kernel_defaults() {
        assert!(
            Mask::from_text(&Mask::default_value().to_text())
                .unwrap()
                .0
                .is_none()
        );
        assert!(
            Material::from_text(&Material::default_value().to_text())
                .unwrap()
                .0
                .is_none()
        );
        assert!(Mask::from_text("bad").is_err());
        assert!(Material::from_text("bad").is_err());
    }
    #[test]
    fn ramp_keeps_float_channels_and_rejects_invalid_layouts() {
        let text = "[2,0,0.12345679,0.4,0.7,0.25,1,1.2,0.2,0.1,1]";
        let ramp = parse_gradient(text).unwrap();
        assert_eq!(ramp[2], 0.12345679);
        assert_eq!(ramp[8], 0.2);
        assert!(parse_gradient("[2,0,1,1,1,1]").is_err());
        assert!(parse_gradient("[1.5,0,1,1,1,1]").is_err());
        assert!(parse_gradient("[2,1,1,1,1,1,0,1,1,1,1]").is_err());
    }
    #[test]
    fn copper_keeps_its_sources_and_all_nodes() {
        assert_eq!(recipe().manifest.graph.nodes.len(), 28);
        assert_eq!(recipe().manifest.graph.links.len(), 42);
        assert!(catalog().errors().is_empty());
        assert_eq!(
            catalog().files()["prelude.wesl"],
            recipe().manifest.sources["prelude.wesl"]
        );
    }
    #[test]
    fn editable_kernel_assets_start_with_the_reference_sources() {
        for kind in [
            "noise", "cells", "surface", "mask", "combine", "spread", "flow", "read", "layer",
            "finish",
        ] {
            let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../nodes/material-{kind}/assets/kernel.wesl"));
            let source = std::fs::read_to_string(file).unwrap();
            assert!(
                source
                    .lines()
                    .eq(catalog().get(kind).unwrap().source.lines()),
                "{kind}"
            );
        }
    }
}
