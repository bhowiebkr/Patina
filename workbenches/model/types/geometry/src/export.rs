//! Writers for `.glb`, `.obj` and `.stl` files. The model has Z up. The `.glb` and the `.obj`
//! files turn it to Y up, as glTF and most game tools expect. The `.stl` file keeps Z up.

use std::fmt::Write as _;
use std::path::Path;

use crate::{Frame, Mesh, Sockets};

/// The file formats of the export.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Glb,
    Obj,
    Stl,
}

impl Format {
    /// The format of a path, from its extension.
    pub fn of(path: &Path) -> Result<Format, String> {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
        match extension.to_ascii_lowercase().as_str() {
            "glb" => Ok(Format::Glb),
            "obj" => Ok(Format::Obj),
            "stl" => Ok(Format::Stl),
            _ => Err(format!("{}: the file name must end in .glb, .obj or .stl", path.display())),
        }
    }
}

/// The bytes of the file. The sockets go into the glTF extras of the model node. `colors` has
/// the sRGB color 0xRRGGBB of each part, and each part with a color gets a glTF material. The
/// `.obj` and the `.stl` files have no sockets and no colors.
pub fn bytes(mesh: &Mesh, sockets: &Sockets, colors: &[u32], format: Format) -> Result<Vec<u8>, String> {
    if mesh.triangles() == 0 {
        return Err("The solid is empty, so there is nothing to export".into());
    }
    Ok(match format {
        Format::Glb => glb(mesh, sockets, colors),
        Format::Obj => obj(mesh).into_bytes(),
        Format::Stl => stl(mesh),
    })
}

/// Writes the file. It writes a temporary file first, so a failed write keeps the old file.
pub fn write(path: &Path, mesh: &Mesh, sockets: &Sockets, colors: &[u32]) -> Result<(), String> {
    let data = bytes(mesh, sockets, colors, Format::of(path)?)?;
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".part");
    let result = std::fs::write(&temporary, data).and_then(|()| std::fs::rename(&temporary, path));
    result.map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        format!("Cannot write {}: {e}", path.display())
    })
}

/// A point or a direction of the model with Y up: (x, y, z) goes to (x, z, -y).
fn y_up([x, y, z]: [f32; 3]) -> [f32; 3] {
    [x, z, -y]
}

/// The triangles in the order of their parts, and the start of each part in that order.
fn by_part(mesh: &Mesh) -> (Vec<usize>, Vec<usize>) {
    let mut order: Vec<usize> = (0..mesh.triangles()).collect();
    order.sort_by_key(|&t| mesh.parts[t]);
    let mut starts = vec![0];
    starts.extend((1..order.len()).filter(|&i| mesh.parts[order[i]] != mesh.parts[order[i - 1]]));
    (order, starts)
}

fn glb(mesh: &Mesh, sockets: &Sockets, colors: &[u32]) -> Vec<u8> {
    let (order, starts) = by_part(mesh);
    let corners = || order.iter().flat_map(|&t| 3 * t..3 * t + 3);
    let mut bin: Vec<u8> = Vec::with_capacity(order.len() * 3 * 32);
    bin.extend(corners().flat_map(|c| y_up(mesh.positions[c])).flat_map(f32::to_le_bytes));
    bin.extend(corners().flat_map(|c| y_up(mesh.normals[c])).flat_map(f32::to_le_bytes));
    bin.extend(corners().flat_map(|c| mesh.uvs[c]).flat_map(f32::to_le_bytes));
    let count = order.len() * 3;
    let mut accessors = Vec::new();
    let mut primitives = Vec::new();
    let mut materials: Vec<u32> = Vec::new();
    for (part, &start) in starts.iter().enumerate() {
        let end = starts.get(part + 1).copied().unwrap_or(order.len());
        let (first, corners) = (start * 3, (end - start) * 3);
        let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
        for &t in &order[start..end] {
            for p in &mesh.positions[3 * t..3 * t + 3] {
                let p = y_up(*p);
                (0..3).for_each(|i| (low[i], high[i]) = (low[i].min(p[i]), high[i].max(p[i])));
            }
        }
        let a = accessors.len();
        accessors.push(format!(
            r#"{{"bufferView":0,"byteOffset":{},"componentType":5126,"count":{corners},"type":"VEC3","min":{},"max":{}}}"#,
            first * 12,
            list(&low),
            list(&high)
        ));
        accessors.push(format!(r#"{{"bufferView":1,"byteOffset":{},"componentType":5126,"count":{corners},"type":"VEC3"}}"#, first * 12));
        accessors.push(format!(r#"{{"bufferView":2,"byteOffset":{},"componentType":5126,"count":{corners},"type":"VEC2"}}"#, first * 8));
        // Parts with the same color share one material.
        let material = colors.get(mesh.parts[order[start]] as usize).map_or(String::new(), |color| {
            let at = materials.iter().position(|c| c == color).unwrap_or_else(|| {
                materials.push(*color);
                materials.len() - 1
            });
            format!(r#","material":{at}"#)
        });
        primitives.push(format!(r#"{{"attributes":{{"POSITION":{a},"NORMAL":{},"TEXCOORD_0":{}}},"mode":4{material}}}"#, a + 1, a + 2));
    }
    let materials: Vec<String> = materials.into_iter().map(material).collect();
    let materials = if materials.is_empty() { String::new() } else { format!(r#""materials":[{}],"#, materials.join(",")) };
    let mut extras = String::new();
    for (i, (name, frame)) in sockets.iter().enumerate() {
        let _ = write!(extras, "{}{}:{}", if i == 0 { "" } else { "," }, quote(name), socket(frame));
    }
    let json = format!(
        concat!(
            r#"{{"asset":{{"version":"2.0","generator":"Patina model workbench"}},"scene":0,"scenes":[{{"nodes":[0]}}],"#,
            r#""nodes":[{{"name":"model","mesh":0,"extras":{{"sockets":{{{}}}}}}}],"meshes":[{{"primitives":[{}]}}],"#,
            r#"{}"buffers":[{{"byteLength":{}}}],"bufferViews":["#,
            r#"{{"buffer":0,"byteOffset":0,"byteLength":{},"target":34962}},"#,
            r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34962}},"#,
            r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34962}}],"accessors":[{}]}}"#
        ),
        extras,
        primitives.join(","),
        materials,
        bin.len(),
        count * 12,
        count * 12,
        count * 12,
        count * 24,
        count * 8,
        accessors.join(",")
    );
    let mut json = json.into_bytes();
    json.resize(json.len().next_multiple_of(4), b' ');
    bin.resize(bin.len().next_multiple_of(4), 0);
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    for word in [0x4654_6c67, 2, total as u32, json.len() as u32, 0x4e4f_534a] {
        out.extend(u32::to_le_bytes(word));
    }
    out.extend(json);
    out.extend(u32::to_le_bytes(bin.len() as u32));
    out.extend(u32::to_le_bytes(0x004e_4942));
    out.extend(bin);
    out
}

/// A glTF material of the sRGB color 0xRRGGBB, with no metal. The glTF base color is linear.
fn material(color: u32) -> String {
    let [r, g, b] = [16, 8, 0].map(|shift| {
        let c = ((color >> shift) & 0xff) as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    });
    format!(r#"{{"pbrMetallicRoughness":{{"baseColorFactor":{},"metallicFactor":0}}}}"#, list(&[r, g, b, 1.0]))
}

/// A socket frame in the glTF extras, with Y up.
fn socket(frame: &Frame) -> String {
    let turn = |v: [f64; 3]| list(&y_up(v.map(|x| x as f32)));
    format!(r#"{{"origin":{},"x":{},"y":{},"z":{}}}"#, turn(frame.origin), turn(frame.x), turn(frame.y), turn(frame.z))
}

/// A JSON list of numbers. JSON has no infinity, so a value that is not finite gives 0.
fn list(values: &[f32]) -> String {
    // Adding 0 turns -0 into 0.
    let items: Vec<String> = values.iter().map(|v| if v.is_finite() { (v + 0.0).to_string() } else { "0".into() }).collect();
    format!("[{}]", items.join(","))
}

/// A JSON string.
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// An `.obj` file with one group for each part. Each corner has its own position, UV and normal.
fn obj(mesh: &Mesh) -> String {
    let (order, _) = by_part(mesh);
    let corners = || order.iter().flat_map(|&t| 3 * t..3 * t + 3);
    let mut out = String::from("# Patina model workbench, Y up\no model\n");
    let mut line = |key: &str, values: &[f32]| {
        out.push_str(key);
        values.iter().for_each(|v| _ = write!(out, " {v}"));
        out.push('\n');
    };
    corners().for_each(|c| line("v", &y_up(mesh.positions[c])));
    corners().for_each(|c| line("vt", &mesh.uvs[c]));
    corners().for_each(|c| line("vn", &y_up(mesh.normals[c])));
    for (i, &t) in order.iter().enumerate() {
        if i == 0 || mesh.parts[t] != mesh.parts[order[i - 1]] {
            let _ = writeln!(out, "g part{}", mesh.parts[t]);
        }
        let n = 3 * i + 1;
        let _ = writeln!(out, "f {n}/{n}/{n} {0}/{0}/{0} {1}/{1}/{1}", n + 1, n + 2);
    }
    out
}

/// A binary `.stl` file with Z up.
fn stl(mesh: &Mesh) -> Vec<u8> {
    let mut out = vec![0; 80];
    out[..24].copy_from_slice(b"Patina model workbench Z");
    out.extend(u32::to_le_bytes(mesh.triangles() as u32));
    for t in 0..mesh.triangles() {
        let values = [mesh.normals[3 * t]].into_iter().chain(mesh.positions[3 * t..3 * t + 3].iter().copied());
        out.extend(values.flatten().flat_map(f32::to_le_bytes));
        out.extend([0, 0]);
    }
    out
}
