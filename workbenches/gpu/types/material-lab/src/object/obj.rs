use super::{Geometry, validate};
use forge_core::{AssetInfo, Bounds, MeshData, PartInfo, Vertex};
use glam::{Vec2, Vec3};

/// OBJ import requires triangles and an existing non-overlapping atlas. It does not unwrap UVs.
pub fn parse_obj(text: &str, name: &str) -> Result<Geometry, String> {
    let mut positions = Vec::<Vec3>::new();
    let mut normals = Vec::<Vec3>::new();
    let mut uvs = Vec::<Vec2>::new();
    let mut mesh = MeshData::default();
    let mut parts = Vec::<PartInfo>::new();
    let mut group = String::from("Object");
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for (line, raw) in text.lines().enumerate() {
        let result = (|| -> Result<(), String> {
            let mut fields = raw.split('#').next().unwrap_or("").split_whitespace();
            let Some(kind) = fields.next() else {
                return Ok(());
            };
            let mut number = || -> Result<f32, String> {
                let v: f32 = fields
                    .next()
                    .ok_or("Missing coordinate")?
                    .parse()
                    .map_err(|_| "Invalid coordinate")?;
                if !v.is_finite() {
                    return Err("Coordinate must be finite".into());
                }
                Ok(v)
            };
            match kind {
                "v" => positions.push(Vec3::new(number()?, number()?, number()?)),
                "vt" => uvs.push(Vec2::new(number()?, number()?)),
                "vn" => {
                    let n = Vec3::new(number()?, number()?, number()?);
                    normals.push(n.try_normalize().ok_or("Normal must be nonzero")?);
                }
                "g" | "o" | "usemtl" => group = fields.collect::<Vec<_>>().join(" "),
                "f" => {
                    let corners: Vec<_> = fields.collect();
                    if corners.len() != 3 {
                        return Err("Triangulate the OBJ before import".into());
                    }
                    if mesh.indices.len() >= 3_000_000 {
                        return Err("Object exceeds 1,000,000 triangles".into());
                    }
                    if parts.last().is_none_or(|p| p.name != group) {
                        if parts.len() >= 1024 {
                            return Err("Object exceeds 1024 parts".into());
                        }
                        parts.push(PartInfo {
                            name: group.clone(),
                            first_index: mesh.indices.len() as u32,
                            ..Default::default()
                        });
                    }
                    let part = (parts.len() - 1) as u32;
                    let mut triangle = [Vertex::default(); 3];
                    let mut needs_normal = [false; 3];
                    for (i, corner) in corners.iter().enumerate() {
                        let fields: Vec<_> = corner.split('/').collect();
                        if !(2..=3).contains(&fields.len()) {
                            return Err("Each OBJ corner needs a UV index".into());
                        }
                        let p = positions[index(fields[0], positions.len())?];
                        let uv = uvs[index(fields[1], uvs.len())?];
                        let normal = match fields.get(2).filter(|s| !s.is_empty()) {
                            Some(s) => normals[index(s, normals.len())?],
                            None => {
                                needs_normal[i] = true;
                                Vec3::ZERO
                            }
                        };
                        min = min.min(p);
                        max = max.max(p);
                        triangle[i] = Vertex {
                            position: p.into(),
                            normal: normal.into(),
                            uv: uv.into(),
                            curvature: 0.5,
                            part,
                        };
                    }
                    let p = triangle.map(|v| Vec3::from(v.position));
                    let flat = (p[1] - p[0])
                        .cross(p[2] - p[0])
                        .try_normalize()
                        .ok_or("Degenerate triangle")?;
                    for (i, mut v) in triangle.into_iter().enumerate() {
                        if needs_normal[i] {
                            v.normal = flat.into();
                        }
                        mesh.indices.push(mesh.vertices.len() as u32);
                        mesh.vertices.push(v);
                    }
                    parts.last_mut().ok_or("Missing part")?.index_count += 3;
                }
                "mtllib" | "s" => (),
                _ => return Err(format!("Unsupported OBJ record: {kind}")),
            }
            if positions.len().max(uvs.len()).max(normals.len()) > 3_000_000 {
                return Err("OBJ coordinate limit exceeded".into());
            }
            Ok(())
        })();
        result.map_err(|e| format!("OBJ line {}: {e}", line + 1))?;
    }
    let info = AssetInfo {
        id: name.into(),
        name: name.into(),
        bounds: Bounds {
            min: min.into(),
            max: max.into(),
        },
        parts,
        atlas_size: 512,
        ..Default::default()
    };
    validate(&info, &mesh)?;
    Ok(Geometry {
        info,
        mesh,
        maps_path: None,
    })
}
fn index(text: &str, length: usize) -> Result<usize, String> {
    let i: i64 = text.parse().map_err(|_| "Missing or invalid OBJ index")?;
    let n = if i < 0 { length as i64 + i } else { i - 1 };
    if i == 0 || n < 0 || n >= length as i64 {
        Err("OBJ index is out of range".into())
    } else {
        Ok(n as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TRIANGLE: &str = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nf -3/1 -2/2 -1/3\n";
    #[test]
    fn imports_uv_triangles_with_negative_indices_and_generated_normals() {
        let g = parse_obj(TRIANGLE, "Triangle").unwrap();
        assert_eq!(g.mesh.vertices[0].normal, [0.0, 0.0, 1.0]);
        assert_eq!(g.info.parts[0].index_count, 3);
        for bad in [
            TRIANGLE.replace("-3/1", "0/1"),
            TRIANGLE.replace("vt 0 1", "vt 0 0"),
            TRIANGLE.replace("vt 0 1", "vt 0 2"),
            TRIANGLE.replace("v 0 1 0", "v NaN 1 0"),
            TRIANGLE.replace("-1/3", "-1/3 -1/3"),
        ] {
            assert!(parse_obj(&bad, "Invalid").is_err());
        }
    }
}
