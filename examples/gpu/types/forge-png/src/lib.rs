//! WESL Forge material PNG (compressed v2, legacy v1 reader): a normal thumbnail and one self-contained private PNG chunk.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
mod container;
mod metadata;
mod storage;
pub use container::{MAX_FILE, decode, encode, load, save};
pub use metadata::load_manifest;
pub use storage::{atomic_write, backup_directory};

use forge_nodes::Graph;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Png(#[from] png::EncodingError),
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    pub sha256: String,
    pub path: String,
    pub offset: usize,
    pub length: usize,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub license: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub graph: Graph,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub drafts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_nodes: Vec<String>,
    #[serde(default)]
    pub materials: BTreeMap<String, Graph>,
    pub sources: BTreeMap<String, String>,
    pub linked_wgsl: BTreeMap<String, String>,
    pub source_hashes: BTreeMap<String, String>,
    pub shader_hashes: BTreeMap<String, String>,
    pub resources: Vec<Resource>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default = "default_exposure")]
    pub exposure: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub tilt: f32,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
}

fn default_exposure() -> f32 {
    1.0
}

#[derive(Clone, Debug)]
pub struct Material {
    pub manifest: Manifest,
    pub binaries: BTreeMap<String, Vec<u8>>,
}

impl Material {
    pub fn new(
        graph: Graph,
        sources: BTreeMap<String, String>,
        linked_wgsl: BTreeMap<String, String>,
    ) -> Self {
        let source_hashes = sources
            .iter()
            .map(|(p, s)| (p.clone(), hash(s.as_bytes())))
            .collect();
        let shader_hashes = linked_wgsl
            .iter()
            .map(|(p, s)| (p.clone(), hash(s.as_bytes())))
            .collect();
        Self {
            manifest: Manifest {
                format: "WESL Forge".into(),
                version: 1,
                graph,
                drafts: BTreeMap::new(),
                custom_nodes: Vec::new(),
                materials: BTreeMap::new(),
                sources,
                linked_wgsl,
                source_hashes,
                shader_hashes,
                resources: Vec::new(),
                environment: None,
                exposure: 1.0,
                rotation: 0.0,
                tilt: 0.0,
                author: String::new(),
                license: String::new(),
            },
            binaries: BTreeMap::new(),
        }
    }

    /// Deduplicates resources by content. No paths from the PNG are used for extraction.
    pub fn add_resource(&mut self, bytes: Vec<u8>, extension: &str) -> String {
        let sha256 = hash(&bytes);
        if let Some(resource) = self.manifest.resources.iter().find(|r| r.sha256 == sha256) {
            return resource.path.clone();
        }
        let extension: String = extension
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(8)
            .collect();
        let extension = if extension.is_empty() {
            "bin"
        } else {
            &extension
        };
        let path = format!("resources/{sha256}.{extension}");
        self.manifest.resources.push(Resource {
            sha256: sha256.clone(),
            path: path.clone(),
            offset: 0,
            length: bytes.len(),
            source: String::new(),
            license: String::new(),
        });
        self.binaries.insert(sha256, bytes);
        path
    }

    pub fn validate(&self) -> Result<()> {
        self.manifest.validate()?;
        for resource in &self.manifest.resources {
            let data = self
                .binaries
                .get(&resource.sha256)
                .ok_or_else(|| Error::Invalid("Missing binary resource".into()))?;
            if data.len() != resource.length || hash(data) != resource.sha256 {
                return Err(Error::Invalid("Binary resource integrity failure".into()));
            }
        }
        Ok(())
    }
}

impl Manifest {
    pub fn validate(&self) -> Result<()> {
        let bad = |s: &str| Error::Invalid(s.into());
        let m = self;
        if m.format != "WESL Forge" || !matches!(m.version, 1 | 2) {
            return Err(bad("Unsupported WESL Forge PNG version"));
        }
        if m.version < 2
            && std::iter::once(&m.graph)
                .chain(m.materials.values())
                .any(|g| g.interface.is_some())
        {
            return Err(bad("Declared subgraphs require material schema version 2"));
        }
        if !m.exposure.is_finite()
            || !(0.01..=16.0).contains(&m.exposure)
            || !m.rotation.is_finite()
            || !m.tilt.is_finite()
        {
            return Err(bad("Invalid environment controls"));
        }
        if m.source_hashes.len() != m.sources.len() || m.shader_hashes.len() != m.linked_wgsl.len()
        {
            return Err(bad("Incomplete shader integrity manifest"));
        }
        for (files, hashes) in [
            (&m.sources, &m.source_hashes),
            (&m.linked_wgsl, &m.shader_hashes),
        ] {
            for (p, s) in files {
                if hashes.get(p) != Some(&hash(s.as_bytes())) {
                    return Err(bad("Shader content hash mismatch"));
                }
            }
        }
        let mut paths = std::collections::BTreeSet::new();
        let mut hashes = std::collections::BTreeSet::new();
        for r in &m.resources {
            let prefix = format!("resources/{}.", r.sha256);
            let ext = r.path.strip_prefix(&prefix).unwrap_or("");
            if r.sha256.len() != 64
                || ext.is_empty()
                || ext.len() > 8
                || !ext.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(bad("Invalid content-addressed resource path"));
            }
            if !paths.insert(&r.path) || !hashes.insert(&r.sha256) {
                return Err(bad("Duplicate resource descriptor"));
            }
        }
        if m.environment.as_ref().is_some_and(|p| !paths.contains(p)) {
            return Err(bad("Missing embedded environment"));
        }
        for id in m.materials.keys() {
            if id.is_empty()
                || id.len() > 128
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(bad("Invalid embedded material id"));
            }
        }
        for graph in std::iter::once(&m.graph).chain(m.materials.values()) {
            if graph.size > 13 || graph.triplanar.iter().any(|v| !v.is_finite()) {
                return Err(bad("Invalid authored graph resolution or projection"));
            }
            let ids: std::collections::BTreeSet<_> = graph.nodes.iter().map(|n| n.id).collect();
            graph.canvas.validate(&ids).map_err(Error::Invalid)?;
            graph.validate_controls().map_err(Error::Invalid)?;
            if let Some(interface) = &graph.interface {
                interface.validate(graph).map_err(Error::Invalid)?;
            }
            if ids.len() != graph.nodes.len()
                || graph.nodes.len() > 10000
                || ids.iter().any(|id| id.0 >= (1 << 24))
            {
                return Err(bad("Invalid graph node ids or node limit"));
            }
            if graph
                .links
                .iter()
                .any(|l| !ids.contains(&l.from) || !ids.contains(&l.to))
            {
                return Err(bad("Dangling graph connection"));
            }
            for node in &graph.nodes {
                let bad_size = match node.size {
                    Some(forge_nodes::Size::Absolute(size)) => size.iter().any(|v| *v > 13),
                    Some(forge_nodes::Size::Relative(size)) => {
                        size.iter().any(|v| !(-13..=13).contains(v))
                    }
                    None => false,
                };
                if bad_size
                    || node.pos.iter().any(|v| !v.is_finite() || v.abs() >= 1e8)
                    || node
                        .params
                        .values()
                        .any(|v| v.len() > 4096 || v.iter().any(|n| !n.is_finite()))
                {
                    return Err(bad("Invalid node size, position or parameter values"));
                }
                for key in ["path", "font"] {
                    if let Some(path) = node.text.get(key).filter(|s| !s.is_empty())
                        && !paths.contains(path)
                    {
                        return Err(bad("Graph references a resource outside the PNG"));
                    }
                }
                if forge_nodes::subgraph::reference(node)
                    .is_some_and(|id| !m.materials.contains_key(id))
                    || node.kind == "material_instance"
                        && forge_nodes::subgraph::reference(node).is_none()
                {
                    return Err(bad("Missing embedded material instance"));
                }
            }
        }
        Ok(())
    }
}
