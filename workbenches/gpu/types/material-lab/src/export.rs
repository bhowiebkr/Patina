//! PBR pass conversion follows Millwright Surface export.rs (MPL-2.0); see NOTICE.md.
use crate::{Material, Socket};
use patina_gpu::{gpu, wgpu};
use std::{io::Write, path::Path};
mod archive;

/// Export one finished UV atlas, including all PBR channels and opacity.
pub fn files(material: &Material) -> Result<Vec<(String, Vec<u8>)>, String> {
    let image = material.0.as_ref().ok_or("Connect a finished material")?;
    if image.socket != Socket::Material {
        return Err("Export needs a PBR material".into());
    }
    let gpu = gpu()?;
    let size = image.planes.size;
    let mut files = Vec::new();
    for layer in 0..3 {
        let mut pixels = Vec::with_capacity((size * size) as usize);
        let rows = (8 * 1024 * 1024 / (size * 16)).max(1).min(size);
        for y in (0..size).step_by(rows as usize) {
            let bytes = forge_gpu::read_texture_region(
                &gpu.device,
                &gpu.queue,
                &image.planes.texture,
                wgpu::Origin3d {
                    x: 0,
                    y,
                    z: image.layer + layer,
                },
                [size, rows.min(size - y)],
                1,
                16,
            )?;
            for p in bytes.as_chunks::<16>().0.iter() {
                let p = std::array::from_fn(|i| {
                    f32::from_le_bytes(p[i * 4..i * 4 + 4].try_into().unwrap_or_default())
                });
                if p.iter().any(|v| !v.is_finite()) {
                    return Err("Material contains a non-finite channel".into());
                }
                pixels.push(p);
            }
        }
        files.extend(encode(layer, size, &pixels)?);
    }
    files.push(("README.txt".into(), format!(
        "Object: {}\nSize: {size} x {size}\nBaseColor: sRGB RGB8. Opacity: linear gray8.\nRoughness, Metallic, AO: linear gray8.\nHeight: linear gray16, normalized surface height.\nNormal: tangent space, OpenGL +Y, linear RGB8.\nORM: R=AO, G=roughness, B=metallic, linear RGB8.\nFirst row: v=0. Mesh UV atlas is unchanged.\n", image.target.0.info.name).into_bytes()));
    Ok(files)
}

pub fn write(material: &Material, path: &Path) -> Result<usize, String> {
    if path
        .extension()
        .is_none_or(|x| !x.eq_ignore_ascii_case("zip"))
    {
        return Err("Export path must end in .zip".into());
    }
    let files = files(material)?;
    let bytes = archive::archive(&files)?;
    write_new(path, &bytes)?;
    Ok(files.len() - 1)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            format!(
                "{}: {e}. Choose a new filename; export does not replace files.",
                path.display()
            )
        })?;
    let result = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let cleanup = std::fs::remove_file(path);
        return Err(format!(
            "Export write failed: {error}; partial-file cleanup: {cleanup:?}"
        ));
    }
    Ok(())
}

fn encode(layer: u32, size: u32, pixels: &[[f32; 4]]) -> Result<Vec<(String, Vec<u8>)>, String> {
    if size == 0 || pixels.len() != size as usize * size as usize {
        return Err("Invalid map size".into());
    }
    let maps: &[(&str, &[usize], bool)] = match layer {
        0 => &[("BaseColor", &[0, 1, 2], false), ("Opacity", &[3], false)],
        1 => &[
            ("Roughness", &[0], false),
            ("Metallic", &[1], false),
            ("AO", &[3], false),
            ("Height", &[2], true),
            ("ORM", &[3, 0, 1], false),
        ],
        2 => &[("Normal", &[0, 1, 2], false)],
        _ => return Err("Invalid material plane".into()),
    };
    let mut files = Vec::new();
    for &(name, channels, sixteen) in maps {
        let mut bytes = Vec::new();
        let data: Vec<u8> = pixels
            .iter()
            .flat_map(|p| {
                channels.iter().flat_map(move |&i| {
                    let n = (p[i].clamp(0.0, 1.0) * if sixteen { 65535.0 } else { 255.0 }).round()
                        as u16;
                    let b = n.to_be_bytes();
                    b.into_iter().skip(usize::from(!sixteen))
                })
            })
            .collect();
        {
            let mut encoder = png::Encoder::new(&mut bytes, size, size);
            encoder.set_color(if channels.len() == 1 {
                png::ColorType::Grayscale
            } else {
                png::ColorType::Rgb
            });
            encoder.set_depth(if sixteen {
                png::BitDepth::Sixteen
            } else {
                png::BitDepth::Eight
            });
            if name == "BaseColor" {
                encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
            }
            let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
            writer.write_image_data(&data).map_err(|e| e.to_string())?;
            writer.finish().map_err(|e| e.to_string())?;
        }
        files.push((format!("{name}.png"), bytes));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converts_pbr_channels_height_byte_order_and_color_metadata() {
        let files = encode(1, 1, &[[0.25, 0.75, 0.5, 1.0]]).unwrap();
        for (name, bytes) in &files {
            let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
                .read_info()
                .unwrap();
            let mut data = vec![0; reader.output_buffer_size().unwrap()];
            reader.next_frame(&mut data).unwrap();
            let expected = match name.as_str() {
                "Roughness.png" => vec![64],
                "Metallic.png" => vec![191],
                "AO.png" => vec![255],
                "Height.png" => vec![128, 0],
                "ORM.png" => vec![255, 64, 191],
                _ => unreachable!(),
            };
            assert_eq!(data, expected);
            assert!(reader.info().srgb.is_none());
        }
        let color = encode(0, 1, &[[0.25, 0.5, 0.75, 0.2]]).unwrap();
        let reader = png::Decoder::new(std::io::Cursor::new(&color[0].1))
            .read_info()
            .unwrap();
        assert!(reader.info().srgb.is_some());
        let folder =
            std::env::temp_dir().join(format!("patina-material-export-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("existing.zip");
        std::fs::write(&path, b"original").unwrap();
        assert!(write_new(&path, b"replacement").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
}
