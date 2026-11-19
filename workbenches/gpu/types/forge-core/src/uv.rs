//! UV unwrap settings, saved per asset as `assets/<id>/uv.json`, and the stable hash the UV and
//! mesh-map caches key on.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::error::{read_json, write_json};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UvSettings {
    /// Atlas width and height in texels.
    pub atlas_size: u32,
    /// Texels kept free round every chart.
    pub gutter: f32,
    /// Most symmetric Dirichlet energy a chart may flatten with (OptCuts' bound).
    pub max_distortion: f64,
    /// Merge neighbouring charts, longest seam first.
    pub merge: bool,
    /// PartUV's tree search for fewer charts per shell.
    pub tree_search: bool,
    /// Most area distortion the tree search accepts.
    pub area_bound: f64,
    /// Most boundary length Box Cutter's cuts may add, as a fraction of the start (0 = off).
    pub seam_budget: f64,
}

impl Default for UvSettings {
    fn default() -> Self {
        Self {
            atlas_size: 2048,
            gutter: 3.0,
            max_distortion: 4.1,
            merge: true,
            tree_search: true,
            area_bound: 1.25,
            seam_budget: 0.5,
        }
    }
}

impl UvSettings {
    pub const FILE: &'static str = "uv.json";

    /// The settings in an asset folder, or the defaults when it has none.
    pub fn load(asset_dir: &Path) -> Result<Self> {
        let path = asset_dir.join(Self::FILE);
        if path.exists() {
            read_json(&path)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, asset_dir: &Path) -> Result<()> {
        write_json(&asset_dir.join(Self::FILE), self)
    }
}

/// 64-bit FNV-1a over the parts in order: stable across runs and builds, unlike std's hasher.
pub fn fnv1a(parts: &[&[u8]]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for part in parts {
        for &b in *part {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_matches_the_reference_vectors() {
        assert_eq!(fnv1a(&[]), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(&[b"a"]), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(&[b"foo", b"bar"]), fnv1a(&[b"foobar"]));
    }

    #[test]
    fn missing_fields_take_the_defaults() {
        let s: UvSettings = serde_json::from_str(r#"{"gutter": 5.0}"#).unwrap_or_default();
        assert_eq!(s.gutter, 5.0);
        assert_eq!(s.atlas_size, 2048);
    }
}
