use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{read_json, write_json};
use crate::{AssetInfo, Result};

pub const MANIFEST: &str = "millwright.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: u32,
    /// Asset ids, in gallery order; each is a folder under `assets/`.
    pub assets: Vec<String>,
    /// Material graph ids; each is `materials/<id>.graph.json`.
    #[serde(default)]
    pub materials: Vec<String>,
    /// Material id to its folder in the Materials tab's explorer; unlisted ones are top level.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub folders: BTreeMap<String, String>,
    /// HDR panorama for preview lighting, project-relative or absolute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<PathBuf>,
    /// The CAD pipeline's folder (`tools/cad/build.py`, `cad/<id>.mesh.json`), absolute or
    /// relative to the project; the Model tab's Build runs there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cad_root: Option<PathBuf>,
    /// Asset id to its own CAD folder, in place of `cad_root`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cad_roots: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
}

impl Project {
    /// The CAD folder asset `id` builds in, when it has one.
    pub fn cad_root(&self, id: &str) -> Option<PathBuf> {
        let m = &self.manifest;
        let dir = m.cad_roots.get(id).or(m.cad_root.as_ref())?;
        Some(self.root.join(dir))
    }

    pub fn open(root: &Path) -> Result<Self> {
        let manifest = read_json(&root.join(MANIFEST))?;
        Ok(Self {
            root: root.to_owned(),
            manifest,
        })
    }

    /// Opens the project, or starts an empty one named after the folder.
    pub fn open_or_new(root: &Path) -> Result<Self> {
        if root.join(MANIFEST).exists() {
            return Self::open(root);
        }
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self {
            root: root.to_owned(),
            manifest: Manifest {
                name,
                version: 1,
                ..Manifest::default()
            },
        })
    }

    pub fn save_manifest(&self) -> Result<()> {
        write_json(&self.root.join(MANIFEST), &self.manifest)
    }

    pub fn asset_dir(&self, id: &str) -> PathBuf {
        self.root.join("assets").join(id)
    }

    pub fn material_path(&self, id: &str) -> PathBuf {
        self.root.join("materials").join(format!("{id}.graph.json"))
    }

    pub fn load_asset_info(&self, id: &str) -> Result<AssetInfo> {
        read_json(&self.asset_dir(id).join("asset.json"))
    }
}
