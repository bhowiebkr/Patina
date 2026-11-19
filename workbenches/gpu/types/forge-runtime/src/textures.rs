//! Embedded image imports preserve original bytes; interpretation happens in the GPU kernel.
use super::*;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureEncoding {
    Srgb,
    LinearColor,
    Data,
}
impl TextureEncoding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Srgb => "Color / sRGB",
            Self::LinearColor => "Color / linear",
            Self::Data => "Data / raw samples",
        }
    }
    fn value(self) -> f32 {
        match self {
            Self::Srgb => 0.,
            Self::LinearColor => 1.,
            Self::Data => 2.,
        }
    }
}
pub struct TextureInfo {
    pub width: u32,
    pub height: u32,
    pub encoding: TextureEncoding,
    pub resolution: [u32; 2],
}
pub struct TextureImport {
    pub owner: Option<String>,
    pub position: [f32; 2],
    pub name: String,
    pub bytes: Vec<u8>,
    pub encoding: TextureEncoding,
    pub resolution: [u32; 2],
}
impl TextureInfo {
    pub fn inspect(name: &str, bytes: &[u8]) -> Result<Self, String> {
        let info = forge_sources::image_info(bytes, Path::new(name)).map_err(|e| e.to_string())?;
        Ok(Self {
            width: info.width,
            height: info.height,
            encoding: if info.linear_color {
                TextureEncoding::LinearColor
            } else {
                TextureEncoding::Srgb
            },
            resolution: [info.width, info.height].map(|n| n.next_power_of_two().ilog2().min(13)),
        })
    }
}
impl Session {
    pub fn import_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: TextureImport,
    ) -> Result<forge_nodes::NodeId, String> {
        let TextureImport {
            owner,
            position: pos,
            name,
            bytes,
            encoding,
            resolution,
        } = request;
        TextureInfo::inspect(&name, &bytes)?;
        if resolution.iter().any(|n| *n > 13) {
            return Err("Authored resolution cannot exceed 8192 per side".into());
        }
        let mut state = self.snapshot();
        let extension = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let extension: String = extension
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(8)
            .collect();
        let path = format!("resources/{}.{}", forge_png::hash(&bytes), extension);
        Arc::make_mut(&mut state.resources).insert(path.clone(), bytes);
        let graph = match owner {
            Some(id) => state
                .materials
                .get_mut(&id)
                .ok_or("Destination subgraph no longer exists")?,
            None => &mut state.graph,
        };
        let node = graph.add("imported_bitmap", pos);
        if let Some(node) = graph.node_mut(node) {
            node.title = Path::new(&name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Imported texture")
                .into();
            node.text.insert("path".into(), path);
            node.params
                .insert("encoding".into(), vec![encoding.value()]);
            node.size = Some(forge_nodes::Size::Absolute(resolution));
        }
        let mut files = self.engine.registry().files().clone();
        let embedded = Registry::embedded();
        let def = embedded
            .get("imported_bitmap")
            .ok_or("Missing texture import implementation")?;
        files
            .entry(def.path.clone())
            .or_insert_with(|| def.source.clone());
        state.sources = Arc::new(SourceSet {
            files,
            linked: self.engine.registry().linked_sources().clone(),
        });
        self.restore(device, queue, state)?;
        Ok(node)
    }
}
