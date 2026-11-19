//! Images the CPU makes for `host:` nodes (Bitmap, Text, SVG): rasterised by `forge-sources`
//! when the node's strings, the parameters they read or the file's mtime change, uploaded and
//! bound at binding 6.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use forge_gpu::Target;
use forge_sources::{Align, Pixels, Raster, TextSpec};

use crate::def::Host;
use crate::engine::TEXTURE_SIZE;
use crate::graph::{Graph, Node, NodeId};
use crate::registry::Registry;

/// Bitmaps larger than this are shrunk on load.
pub const MAX_BITMAP: u32 = 2048;

pub struct HostImage {
    pub target: Target,
}

#[derive(Default)]
pub struct Hosts {
    images: BTreeMap<NodeId, HostImage>,
    errors: BTreeMap<NodeId, String>,
    keys: BTreeMap<NodeId, u64>,
    root: PathBuf,
    resources: std::sync::Arc<BTreeMap<String, Vec<u8>>>,
}

impl Hosts {
    pub fn set_resources(&mut self, resources: std::sync::Arc<BTreeMap<String, Vec<u8>>>) {
        self.resources = resources;
        self.keys.clear();
    }
    /// Relative paths in nodes resolve against `root` (the project folder).
    pub fn set_root(&mut self, root: &Path) {
        if self.root != root {
            self.root = root.to_owned();
            self.keys.clear();
        }
    }

    pub fn view(&self, id: NodeId) -> Option<&wgpu::TextureView> {
        self.images.get(&id).map(|i| &i.target.view)
    }

    pub fn error(&self, id: NodeId) -> Option<&str> {
        self.errors.get(&id).map(String::as_str)
    }

    fn resolve(&self, path: &str) -> Option<PathBuf> {
        let path = path.trim();
        (!path.is_empty()).then(|| self.root.join(path))
    }

    /// What the host image of `node` depends on.
    fn key(&self, registry: &Registry, node: &Node, host: Host, max_side: u32) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        format!("{host:?}").hash(&mut h);
        max_side.hash(&mut h);
        let files: &[&str] = match host {
            Host::Bitmap | Host::Svg => &["path"],
            Host::Text => &["font"],
        };
        for id in files {
            let text = node.text(registry, id);
            text.hash(&mut h);
            let mtime = self
                .resolve(&text)
                .and_then(|p| std::fs::metadata(p).ok())
                .and_then(|m| m.modified().ok());
            mtime.hash(&mut h);
        }
        if host == Host::Text {
            node.text(registry, "text").hash(&mut h);
            for id in ["fontsize", "alignment"] {
                for v in node.value(registry, id) {
                    v.to_bits().hash(&mut h);
                }
            }
        }
        h.finish()
    }

    fn raster(
        &self,
        registry: &Registry,
        node: &Node,
        host: Host,
        max_side: u32,
    ) -> Result<Raster, String> {
        let first = |id| node.value(registry, id).first().copied().unwrap_or(0.0);
        let path = |id| self.resolve(&node.text(registry, id));
        let result = match host {
            Host::Bitmap => match path("path") {
                Some(p) => match self.resources.get(&node.text(registry, "path")) {
                    Some(bytes) => forge_sources::bitmap_bytes(bytes, &p, max_side),
                    None => forge_sources::bitmap(&p, max_side),
                },
                None => return Err(String::new()),
            },
            Host::Svg => match path("path") {
                Some(p) => match self.resources.get(&node.text(registry, "path")) {
                    Some(bytes) => forge_sources::svg_bytes(bytes, &p, TEXTURE_SIZE),
                    None => forge_sources::svg(&p, TEXTURE_SIZE),
                },
                None => return Err(String::new()),
            },
            Host::Text => {
                let font = path("font");
                let align = match first("alignment") as u32 {
                    0 => Align::Left,
                    2 => Align::Right,
                    _ => Align::Center,
                };
                forge_sources::text_with_font(
                    &TextSpec {
                        text: &node.text(registry, "text"),
                        font: font.as_deref(),
                        size_pt: first("fontsize"),
                        align,
                        side: (TEXTURE_SIZE * 2).min(MAX_BITMAP),
                    },
                    self.resources
                        .get(&node.text(registry, "font"))
                        .map(Vec::as_slice),
                )
            }
        };
        result.map_err(|e| e.to_string())
    }

    /// Re-rasterises host images whose inputs changed; returns the nodes whose image did.
    pub fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: &Registry,
        graph: &Graph,
    ) -> Vec<NodeId> {
        let hosted: BTreeMap<NodeId, Host> = graph
            .nodes
            .iter()
            .filter_map(|n| Some((n.id, registry.get(&n.kind)?.host?)))
            .collect();
        let mut changed: Vec<NodeId> = self
            .keys
            .keys()
            .filter(|id| !hosted.contains_key(id))
            .copied()
            .collect();
        for id in &changed {
            self.keys.remove(id);
            self.images.remove(id);
            self.errors.remove(id);
        }
        for (id, host) in hosted {
            let Some(node) = graph.node(id) else {
                continue;
            };
            let sides = node
                .size
                .unwrap_or(crate::Size::Relative([0, 0]))
                .log2(graph.size);
            let max_side = (1 << sides[0].max(sides[1]))
                .max(MAX_BITMAP)
                .min(device.limits().max_texture_dimension_2d);
            let key = self.key(registry, node, host, max_side);
            if self.keys.get(&id) == Some(&key) {
                continue;
            }
            self.keys.insert(id, key);
            changed.push(id);
            self.images.remove(&id);
            self.errors.remove(&id);
            match self.raster(registry, node, host, max_side) {
                Ok(raster) => {
                    let target = upload(device, queue, &raster);
                    self.images.insert(id, HostImage { target });
                }
                Err(e) if e.is_empty() => {}
                Err(e) => {
                    self.errors.insert(id, e);
                }
            }
        }
        changed
    }
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, raster: &Raster) -> Target {
    let (format, texel, bytes): (_, u32, &[u8]) = match &raster.pixels {
        Pixels::U8(px) => (wgpu::TextureFormat::Rgba8Unorm, 4, px),
        Pixels::F32(px) => (
            wgpu::TextureFormat::Rgba32Float,
            16,
            bytemuck::cast_slice(px),
        ),
    };
    let size = [raster.width, raster.height];
    let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
    let target = Target::new(device, "host image", size, format, usage, 1);
    forge_gpu::write_texture(queue, &target.texture, size, texel, bytes);
    target
}
