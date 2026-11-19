use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// `assets/<id>/asset.json`: everything about an asset except the vertex data.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AssetInfo {
    pub id: String,
    pub name: String,
    /// Where it was imported from (informational).
    #[serde(default)]
    pub source: String,
    /// Footprint in grid cells, for projects that place assets on a grid.
    #[serde(default)]
    pub footprint: Option<[f32; 2]>,
    pub bounds: Bounds,
    pub parts: Vec<PartInfo>,
    #[serde(default)]
    pub groups: BTreeMap<String, Group>,
    /// Animation clip file next to asset.json, if the asset moves.
    #[serde(default)]
    pub anim: Option<String>,
    /// Side of the square texture atlas the UVs were packed for.
    pub atlas_size: u32,
    /// Charts in the atlas.
    #[serde(default)]
    pub charts: u32,
    /// The camera and light its reference render was fitted under, if it has one.
    #[serde(default)]
    pub view: Option<FittedView>,
}

/// A fitted preview (the CAD pipeline's models/<asset>/fit.json and light.json), viewer axes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FittedView {
    pub target: [f32; 3],
    /// Unit vector from the target to the eye.
    pub direction: [f32; 3],
    /// The image's up.
    pub up: [f32; 3],
    pub distance: f32,
    /// Vertical, degrees.
    pub fov_y: f32,
    /// Direction to the sun.
    pub sun: [f32; 3],
    pub sun_strength: f32,
    /// The sun's angular diameter, degrees.
    pub sun_angle: f32,
    /// Sky dome radiance above the horizon, ground below.
    pub sky: f32,
    pub ground: f32,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bounds {
    pub fn center(&self) -> glam::Vec3 {
        (glam::Vec3::from(self.min) + glam::Vec3::from(self.max)) * 0.5
    }

    pub fn radius(&self) -> f32 {
        (glam::Vec3::from(self.max) - glam::Vec3::from(self.min)).length() * 0.5
    }

    /// The longest side, which bakers scale "relative to bounding box" distances by.
    pub fn largest_side(&self) -> f32 {
        (glam::Vec3::from(self.max) - glam::Vec3::from(self.min)).max_element()
    }
}

/// One part: a contiguous index range in mesh.bin, drawn with its own transform.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PartInfo {
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub lod: u32,
    pub first_index: u32,
    pub index_count: u32,
    pub material: PartMaterial,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PartMaterial {
    /// sRGB, alpha last.
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    #[serde(default)]
    pub emissive: [f32; 3],
}

impl Default for PartMaterial {
    fn default() -> Self {
        Self {
            base_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0; 3],
        }
    }
}

/// A mechanism group: how its parts move (the baked clip holds the actual motion).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Group {
    #[serde(default)]
    pub pivot: [f32; 3],
    #[serde(default)]
    pub axis: [f32; 3],
    /// `rotate`, `slide` or `track`.
    #[serde(default)]
    pub motion: String,
    #[serde(default)]
    pub range: Option<[f32; 2]>,
    #[serde(default)]
    pub turns: Option<String>,
    #[serde(default)]
    pub peak: Option<f32>,
}
