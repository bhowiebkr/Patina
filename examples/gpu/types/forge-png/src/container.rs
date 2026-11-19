use super::{Error, Manifest, Material, Result};
use flate2::{Compression, bufread::ZlibDecoder, write::ZlibEncoder};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

pub const MAX_FILE: usize = 256 * 1024 * 1024;
const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const CHUNK: &[u8; 4] = b"wfMA";
const MAX_MANIFEST: usize = 32 * 1024 * 1024;

pub fn load(path: &Path) -> Result<Material> {
    if std::fs::metadata(path)?.len() > MAX_FILE as u64 {
        return Err(Error::Invalid("Material PNG exceeds 256 MiB".into()));
    }
    decode(&std::fs::read(path)?)
}

pub fn save(path: &Path, material: &Material, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let bytes = encode(material, width, height, rgba)?;
    crate::atomic_write(path, &bytes, true)
}

pub fn encode(material: &Material, width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    material.validate()?;
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || rgba.len() != width as usize * height as usize * 4
    {
        return Err(Error::Invalid("Invalid PNG thumbnail dimensions".into()));
    }
    let mut manifest = material.manifest.clone();
    let mut binary = Vec::new();
    for r in &mut manifest.resources {
        r.offset = binary.len();
        let data = &material.binaries[&r.sha256];
        r.length = data.len();
        binary.extend_from_slice(data);
    }
    let json = serde_json::to_vec(&manifest)?;
    if json.len() > MAX_MANIFEST || 4 + json.len() + binary.len() > MAX_FILE - 1024 {
        return Err(Error::Invalid("Material payload exceeds size limit".into()));
    }
    let mut compressed = ZlibEncoder::new(Vec::new(), Compression::best());
    compressed.write_all(&(json.len() as u32).to_be_bytes())?;
    compressed.write_all(&json)?;
    compressed.write_all(&binary)?;
    let mut payload = Vec::new();
    payload.extend_from_slice(&2u32.to_be_bytes());
    payload.extend_from_slice(&((4 + json.len() + binary.len()) as u32).to_be_bytes());
    payload.extend_from_slice(&compressed.finish()?);
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::High);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgba)?;
        writer.write_chunk(png::chunk::ChunkType(*CHUNK), &payload)?;
        writer.finish()?;
    }
    if out.len() > MAX_FILE {
        return Err(Error::Invalid("Material PNG exceeds file limit".into()));
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Material> {
    let bad = |s: &str| Error::Invalid(s.into());
    if bytes.len() > MAX_FILE || !bytes.starts_with(SIGNATURE) {
        return Err(bad("Not a supported material PNG"));
    }
    let mut at = 8usize;
    let mut payload = None;
    let mut ended = false;
    let mut first = true;
    while at < bytes.len() {
        if bytes.len() - at < 12 {
            return Err(bad("Truncated PNG chunk"));
        }
        let length = u32::from_be_bytes(bytes[at..at + 4].try_into().map_err(|_| bad("Bad chunk"))?)
            as usize;
        let end = at
            .checked_add(12)
            .and_then(|v| v.checked_add(length))
            .filter(|v| *v <= bytes.len())
            .ok_or_else(|| bad("Invalid PNG chunk length"))?;
        let name = &bytes[at + 4..at + 8];
        let body = &bytes[at + 8..end - 4];
        if first && name != b"IHDR" {
            return Err(bad("PNG header must come first"));
        }
        first = false;
        let expected =
            u32::from_be_bytes(bytes[end - 4..end].try_into().map_err(|_| bad("Bad CRC"))?);
        if crc32fast::hash(&bytes[at + 4..end - 4]) != expected {
            return Err(bad("PNG chunk CRC mismatch"));
        }
        if name == CHUNK && payload.replace(body).is_some() {
            return Err(bad("Duplicate WESL Forge payload"));
        }
        at = end;
        if name == b"IEND" {
            ended = true;
            break;
        }
    }
    if !ended || at != bytes.len() {
        return Err(bad("Incomplete PNG or trailing data"));
    }
    // Validate the ordinary thumbnail with the standard decoder too.
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: 64 * 1024 * 1024,
    });
    let mut reader = decoder.read_info().map_err(|e| bad(&e.to_string()))?;
    if reader.info().width > 4096 || reader.info().height > 4096 {
        return Err(bad("Thumbnail too large"));
    }
    let count = reader
        .output_buffer_size()
        .ok_or_else(|| bad("Thumbnail exceeds memory limits"))?;
    reader
        .next_frame(&mut vec![0; count])
        .map_err(|e| bad(&e.to_string()))?;
    let data = payload.ok_or_else(|| bad("This is a plain PNG, without a WESL Forge material"))?;
    if data.len() < 8 {
        return Err(bad("Truncated material header"));
    }
    let inflated;
    let body = match u32::from_be_bytes(
        data[..4]
            .try_into()
            .map_err(|_| bad("Bad container version"))?,
    ) {
        1 => &data[4..],
        2 => {
            let length = u32::from_be_bytes(
                data[4..8]
                    .try_into()
                    .map_err(|_| bad("Bad expanded size"))?,
            ) as usize;
            if !(4..=MAX_FILE - 1024).contains(&length) {
                return Err(bad("Invalid expanded material size"));
            }
            let mut decoder = ZlibDecoder::new(Cursor::new(&data[8..]));
            let mut bytes = Vec::new();
            decoder
                .by_ref()
                .take(length as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| bad("Invalid compressed material"))?;
            if bytes.len() != length || decoder.total_in() as usize != data.len() - 8 {
                return Err(bad("Compressed material size mismatch or trailing data"));
            }
            inflated = bytes;
            inflated.as_slice()
        }
        _ => return Err(bad("Unsupported material container version")),
    };
    if body.len() < 4 {
        return Err(bad("Truncated manifest header"));
    }
    let n = u32::from_be_bytes(
        body[..4]
            .try_into()
            .map_err(|_| bad("Bad manifest length"))?,
    ) as usize;
    if n > MAX_MANIFEST || n > body.len() - 4 {
        return Err(bad("Invalid manifest size"));
    }
    let manifest: Manifest = serde_json::from_slice(&body[4..4 + n])?;
    let blobs = &body[4 + n..];
    let mut binaries = BTreeMap::new();
    let mut expected = 0;
    for r in &manifest.resources {
        let end = r
            .offset
            .checked_add(r.length)
            .filter(|e| *e <= blobs.len())
            .ok_or_else(|| bad("Resource outside payload"))?;
        if r.offset != expected {
            return Err(bad("Overlapping or noncanonical resource ranges"));
        }
        expected = end;
        if binaries
            .insert(r.sha256.clone(), blobs[r.offset..end].to_vec())
            .is_some()
        {
            return Err(bad("Duplicate binary resource"));
        }
    }
    if expected != blobs.len() {
        return Err(bad("Unreferenced binary data"));
    }
    let material = Material { manifest, binaries };
    material.validate()?;
    Ok(material)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn material() -> Material {
        Material::new(
            forge_nodes::Graph::default(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
    }
    fn with_payload(payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0; 4]).unwrap();
            writer
                .write_chunk(png::chunk::ChunkType(*CHUNK), payload)
                .unwrap();
            writer.finish().unwrap();
        }
        out
    }
    #[test]
    fn legacy_uncompressed_container_still_loads() {
        let m = material();
        let json = serde_json::to_vec(&m.manifest).unwrap();
        let mut payload = 1u32.to_be_bytes().to_vec();
        payload.extend_from_slice(&(json.len() as u32).to_be_bytes());
        payload.extend_from_slice(&json);
        let loaded = decode(&with_payload(&payload)).unwrap();
        assert_eq!(
            serde_json::to_value(&loaded.manifest).unwrap(),
            serde_json::to_value(&m.manifest).unwrap()
        );
    }
    #[test]
    fn compressed_payload_preserves_exact_sources_and_binary_bytes() {
        let source = "fn eval() { /* repeated shader metadata */ }\n".repeat(5000);
        let mut m = Material::new(
            forge_nodes::Graph::default(),
            BTreeMap::from([("prelude.wesl".into(), source.clone())]),
            BTreeMap::new(),
        );
        m.add_resource(vec![43; 400_000], "bin");
        let encoded = encode(&m, 1, 1, &[0; 4]).unwrap();
        assert!(
            encoded.len() < 10_000,
            "Shader metadata and resources must actually be compressed"
        );
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded.binaries, m.binaries);
        assert_eq!(decoded.manifest.sources["prelude.wesl"], source);
    }
    #[test]
    fn compressed_size_limits_corruption_and_trailing_bytes_are_rejected() {
        let mut body = Vec::new();
        let json = serde_json::to_vec(&material().manifest).unwrap();
        body.extend_from_slice(&(json.len() as u32).to_be_bytes());
        body.extend_from_slice(&json);
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&body).unwrap();
        let compressed = encoder.finish().unwrap();
        let payload = |length: u32, stream: &[u8]| {
            let mut p = 2u32.to_be_bytes().to_vec();
            p.extend_from_slice(&length.to_be_bytes());
            p.extend_from_slice(stream);
            p
        };
        assert!(decode(&with_payload(&payload(body.len() as u32, &compressed))).is_ok());
        for length in [
            0,
            3,
            10,
            body.len() as u32 - 1,
            body.len() as u32 + 1,
            MAX_FILE as u32,
            u32::MAX,
        ] {
            assert!(decode(&with_payload(&payload(length, &compressed))).is_err());
        }
        let mut trailing = compressed.clone();
        trailing.push(0);
        assert!(decode(&with_payload(&payload(body.len() as u32, &trailing))).is_err());
        let mut corrupted = compressed.clone();
        corrupted[0] ^= 255;
        assert!(decode(&with_payload(&payload(body.len() as u32, &corrupted))).is_err());
        assert!(
            decode(&with_payload(&payload(
                body.len() as u32,
                &compressed[..compressed.len() - 4]
            )))
            .is_err()
        );
    }
    #[test]
    fn round_trip_deduplicates_binary_and_preserves_sources() {
        let mut m = material();
        let a = m.add_resource(vec![0, 255, 2, 0, 128], "png");
        let b = m.add_resource(vec![0, 255, 2, 0, 128], "png");
        assert_eq!(a, b);
        let encoded = encode(&m, 2, 2, &[255; 16]).unwrap();
        let read = decode(&encoded).unwrap();
        assert_eq!(read.binaries, m.binaries);
        assert_eq!(read.manifest.resources.len(), 1);
    }
    #[test]
    fn corruption_and_truncation_are_rejected() {
        let mut bytes = encode(&material(), 1, 1, &[0; 4]).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(decode(&bytes).is_err());
        assert!(decode(&bytes[..last]).is_err());
    }
    #[test]
    fn missing_resource_and_path_escape_are_rejected() {
        let mut m = material();
        m.add_resource(vec![1], "bin");
        m.manifest.resources[0].path = "../../outside".into();
        assert!(m.validate().is_err());
    }
    #[test]
    fn shader_hash_mismatch_is_rejected() {
        let mut m = Material::new(
            forge_nodes::Graph::default(),
            BTreeMap::from([("prelude.wesl".into(), "original".into())]),
            BTreeMap::new(),
        );
        m.manifest
            .sources
            .insert("prelude.wesl".into(), "changed".into());
        assert!(m.validate().is_err());
    }
    #[test]
    fn ordinary_png_is_not_a_material() {
        let mut bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut bytes, 1, 1);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&[0; 4]).unwrap();
        }
        assert!(decode(&bytes).is_err());
    }
}
