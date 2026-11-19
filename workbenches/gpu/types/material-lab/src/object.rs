//! Mesh loading is workbench code. Validate imported geometry before GPU allocation.
use forge_core::{AssetInfo, MeshData};
use patina_sdk::{Param, Text, Viewer};
use std::{
    fmt::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
mod obj;
pub use obj::parse_obj;

pub struct Geometry {
    pub info: AssetInfo,
    pub mesh: MeshData,
    pub maps_path: Option<PathBuf>,
}
#[derive(Clone, Default)]
pub struct Object(pub Option<Arc<Geometry>>);
impl Param for Object {
    fn default_value() -> Self {
        Self::default()
    }
    fn from_text(text: &str) -> Result<Self, String> {
        if text.trim().is_empty() {
            Ok(Self::default())
        } else {
            Err("Connect an Object node".into())
        }
    }
    fn to_text(&self) -> String {
        String::new()
    }
}
impl Viewer for Object {
    fn text(&self, out: &mut Text) -> fmt::Result {
        match &self.0 {
            Some(g) => write!(
                out,
                "{} / {} triangles / {} parts",
                g.info.name,
                g.mesh.indices.len() / 3,
                g.info.parts.len()
            ),
            None => write!(out, "No object"),
        }
    }
}

pub fn load(path: &Path) -> Result<Arc<Geometry>, String> {
    let extension = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let geometry = match extension.as_str() {
        "obj" => {
            let bytes = read(path, 128 << 20)?;
            parse_obj(
                std::str::from_utf8(&bytes).map_err(|_| "OBJ must use UTF-8 text")?,
                path.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Object"),
            )?
        }
        "json" => {
            let info: AssetInfo =
                serde_json::from_slice(&read(path, 4 << 20)?).map_err(|e| e.to_string())?;
            let mesh_path = path.with_file_name("mesh.bin");
            let mesh = MeshData::from_bytes(&mesh_path, &read(&mesh_path, 256 << 20)?)
                .map_err(|e| e.to_string())?;
            Geometry {
                info,
                mesh,
                maps_path: Some(path.with_file_name("maps.bin")),
            }
        }
        _ => return Err("Choose a triangulated UV-mapped OBJ or a Millwright asset.json".into()),
    };
    validate(&geometry.info, &geometry.mesh)?;
    Ok(Arc::new(geometry))
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Object file exceeds the import limit".into());
    }
    Ok(bytes)
}

pub fn validate(info: &AssetInfo, mesh: &MeshData) -> Result<(), String> {
    use glam::{Vec2, Vec3};
    if !matches!(info.atlas_size, 128 | 256 | 512 | 1024 | 2048) {
        return Err("Object atlas size must be 128, 256, 512, 1024, or 2048".into());
    }
    if mesh.vertices.is_empty()
        || mesh.vertices.len() > 3_000_000
        || mesh.indices.is_empty()
        || mesh.indices.len() > 3_000_000
        || !mesh.indices.len().is_multiple_of(3)
        || info.parts.is_empty()
        || info.parts.len() > 1024
    {
        return Err("Object needs 1 to 1,000,000 triangles and 1 to 1024 parts".into());
    }
    let min = Vec3::from(info.bounds.min);
    let max = Vec3::from(info.bounds.max);
    if !min.is_finite()
        || !max.is_finite()
        || !min.cmple(max).all()
        || info.bounds.largest_side() <= 1e-8
        || !info.bounds.radius().is_finite()
    {
        return Err("Object bounds must be finite and nonempty".into());
    }
    for vertex in &mesh.vertices {
        let p = Vec3::from(vertex.position);
        let n = Vec3::from(vertex.normal);
        if !p.is_finite()
            || !n.is_finite()
            || !(0.99..=1.01).contains(&n.length_squared())
            || vertex
                .uv
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || !vertex.curvature.is_finite()
            || vertex.part as usize >= info.parts.len()
            || !p.cmpge(min - Vec3::splat(1e-4)).all()
            || !p.cmple(max + Vec3::splat(1e-4)).all()
        {
            return Err("Object needs finite positions, unit normals, valid parts, and UV coordinates from 0 to 1".into());
        }
    }
    let mut end = 0usize;
    for (part_id, part) in info.parts.iter().enumerate() {
        if part.first_index as usize != end
            || part.index_count == 0
            || !part.index_count.is_multiple_of(3)
        {
            return Err("Object parts must cover consecutive triangle ranges".into());
        }
        end = end
            .checked_add(part.index_count as usize)
            .ok_or("Part range overflow")?;
        let indices = mesh
            .indices
            .get(part.first_index as usize..end)
            .ok_or("Part range exceeds the mesh")?;
        for triangle in indices.as_chunks::<3>().0.iter() {
            let mut p = [Vec3::ZERO; 3];
            let mut uv = [Vec2::ZERO; 3];
            for (i, index) in triangle.iter().enumerate() {
                let v = mesh
                    .vertices
                    .get(*index as usize)
                    .ok_or("Vertex index exceeds the mesh")?;
                if v.part as usize != part_id {
                    return Err("Triangle part does not match its draw range".into());
                }
                p[i] = v.position.into();
                uv[i] = v.uv.into();
            }
            if (p[1] - p[0]).cross(p[2] - p[0]).length_squared() <= 1e-20
                || (uv[1] - uv[0]).perp_dot(uv[2] - uv[0]).abs() <= 1e-12
            {
                return Err("Object contains a degenerate triangle or UV triangle".into());
            }
        }
        let m = part.material;
        if m.base_color
            .into_iter()
            .chain(m.emissive)
            .chain([m.roughness, m.metallic])
            .any(|v| !v.is_finite())
        {
            return Err("Object material values must be finite".into());
        }
    }
    if end != mesh.indices.len() {
        return Err("Object parts do not cover every triangle".into());
    }
    if let Some(view) = &info.view {
        let direction = Vec3::from(view.direction);
        let up = Vec3::from(view.up);
        if view
            .target
            .iter()
            .chain(&view.direction)
            .chain(&view.up)
            .chain(&view.sun)
            .any(|v| !v.is_finite())
            || ![
                view.distance,
                view.fov_y,
                view.sun_strength,
                view.sun_angle,
                view.sky,
                view.ground,
            ]
            .iter()
            .all(|v| v.is_finite())
            || view.distance <= 0.0
            || !(1.0..179.0).contains(&view.fov_y)
            || direction.cross(up).length_squared() <= 1e-12
        {
            return Err(
                "The fitted object view must have a valid camera and finite lighting".into(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loads_the_millwright_reference_asset_and_rejects_invalid_parts() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../forge-runtime/assets/ball/asset.json");
        let object = load(&path).unwrap();
        let mut info = object.info.clone();
        info.parts[0].first_index = 3;
        assert!(validate(&info, &object.mesh).is_err());
    }
}
