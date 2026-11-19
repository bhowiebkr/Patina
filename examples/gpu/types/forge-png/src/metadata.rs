use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use flate2::bufread::ZlibDecoder;

use crate::{Error, MAX_FILE, Manifest, Result};

const MAX_MANIFEST: usize = 32 * 1024 * 1024;

fn bad(message: &str) -> Error {
    Error::Invalid(message.into())
}

pub fn load_manifest(path: &Path) -> Result<Manifest> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    manifest(&mut BufReader::with_capacity(16 * 1024, file), length)
}

fn number(reader: &mut impl Read) -> Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

fn json(reader: &mut impl Read, length: u64) -> Result<Manifest> {
    let size = number(reader)? as usize;
    if size > MAX_MANIFEST || size as u64 + 4 > length {
        return Err(bad("Invalid manifest size"));
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    manifest.validate()?;
    let mut expected = 0usize;
    for resource in &manifest.resources {
        if resource.offset != expected {
            return Err(bad("Overlapping or noncanonical resource ranges"));
        }
        expected = expected
            .checked_add(resource.length)
            .ok_or_else(|| bad("Resource size overflow"))?;
    }
    if expected as u64 != length - size as u64 - 4 {
        return Err(bad("Resource size does not match material header"));
    }
    Ok(manifest)
}

fn manifest(reader: &mut (impl Read + Seek), length: u64) -> Result<Manifest> {
    if length > MAX_FILE as u64 {
        return Err(bad("Material PNG exceeds 256 MiB"));
    }
    let mut signature = [0; 8];
    reader.read_exact(&mut signature)?;
    if &signature != b"\x89PNG\r\n\x1a\n" {
        return Err(bad("Not a supported material PNG"));
    }
    let mut first = true;
    let mut result = None;
    while reader.stream_position()? < length {
        let size = u64::from(number(reader)?);
        let mut kind = [0; 4];
        reader.read_exact(&mut kind)?;
        let end = reader
            .stream_position()?
            .checked_add(size)
            .and_then(|offset| offset.checked_add(4))
            .filter(|&offset| offset <= length)
            .ok_or_else(|| bad("Invalid PNG chunk length"))?;
        if first && (&kind != b"IHDR" || size != 13) {
            return Err(bad("PNG header must come first"));
        }
        first = false;
        if &kind == b"wfMA" {
            if result.is_some() || size < 8 {
                return Err(bad("Duplicate or truncated material payload"));
            }
            result = Some(match number(reader)? {
                1 => json(&mut reader.by_ref().take(size - 4), size - 4)?,
                2 => {
                    let expanded = u64::from(number(reader)?);
                    if !(4..=MAX_FILE as u64 - 1024).contains(&expanded) {
                        return Err(bad("Invalid expanded material size"));
                    }
                    let compressed = reader.by_ref().take(size - 8);
                    let mut decoder =
                        ZlibDecoder::new(BufReader::with_capacity(16 * 1024, compressed));
                    json(&mut decoder, expanded)?
                }
                _ => return Err(bad("Unsupported material container version")),
            });
        }
        reader.seek(SeekFrom::Start(end))?;
        if &kind == b"IEND" {
            if size != 0 || end != length {
                return Err(bad("Incomplete PNG or trailing data"));
            }
            return result.ok_or_else(|| bad("This is a plain PNG, without a WESL Forge material"));
        }
    }
    Err(bad("Incomplete PNG"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct Counted {
        cursor: Cursor<Vec<u8>>,
        bytes: usize,
    }

    impl Read for Counted {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let count = self.cursor.read(buffer)?;
            self.bytes += count;
            Ok(count)
        }
    }

    impl Seek for Counted {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.cursor.seek(position)
        }
    }

    fn material() -> crate::Material {
        crate::Material::new(
            forge_nodes::sample_material(),
            Default::default(),
            Default::default(),
        )
    }

    #[test]
    fn metadata_matches_full_load_without_loading_binary_resources() {
        let mut material = material();
        let mut seed = 1u32;
        let resource = (0..2 * 1024 * 1024)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed as u8
            })
            .collect();
        material.add_resource(resource, "bin");
        let bytes = crate::encode(&material, 1, 1, &[0; 4]).unwrap();
        let complete = crate::decode(&bytes).unwrap();
        let mut counted = Counted {
            cursor: Cursor::new(bytes.clone()),
            bytes: 0,
        };
        let metadata = manifest(&mut counted, bytes.len() as u64).unwrap();
        assert!(
            counted.bytes < 128 * 1024,
            "Read {} resource bytes",
            counted.bytes
        );
        assert_eq!(
            serde_json::to_value(metadata).unwrap(),
            serde_json::to_value(complete.manifest).unwrap()
        );
    }

    #[test]
    fn legacy_uncompressed_metadata_is_supported() {
        let material = material();
        let data = serde_json::to_vec(&material.manifest).unwrap();
        let mut payload = 1u32.to_be_bytes().to_vec();
        payload.extend_from_slice(&(data.len() as u32).to_be_bytes());
        payload.extend(data);
        let mut bytes = Vec::new();
        {
            let mut writer = png::Encoder::new(&mut bytes, 1, 1).write_header().unwrap();
            writer.write_image_data(&[0]).unwrap();
            writer
                .write_chunk(png::chunk::ChunkType(*b"wfMA"), &payload)
                .unwrap();
        }
        let metadata = manifest(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        assert_eq!(metadata.graph.name, material.manifest.graph.name);
    }

    #[test]
    fn rejects_invalid_header_truncation_trailing_data_and_shader_integrity() {
        let mut material = material();
        let bytes = crate::encode(&material, 1, 1, &[0; 4]).unwrap();
        assert!(manifest(&mut Cursor::new(&bytes[..20]), 20).is_err());
        let mut trailing = bytes.clone();
        trailing.push(1);
        assert!(manifest(&mut Cursor::new(&trailing), trailing.len() as u64).is_err());
        material
            .manifest
            .source_hashes
            .insert("unpaired".into(), "bad".into());
        assert!(material.manifest.validate().is_err());
    }
}
