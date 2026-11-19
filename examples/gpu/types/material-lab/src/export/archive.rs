//! ZIP storage adds no second compression layer to the already compressed PNGs.
pub fn archive(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    fn word(out: &mut Vec<u8>, v: u16) {
        out.extend(v.to_le_bytes());
    }
    fn dword(out: &mut Vec<u8>, v: u32) {
        out.extend(v.to_le_bytes());
    }
    let count = u16::try_from(files.len()).map_err(|_| "Too many export files")?;
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for (name, bytes) in files {
        let size = u32::try_from(bytes.len()).map_err(|_| "Export file exceeds ZIP32 size")?;
        let name_size = u16::try_from(name.len()).map_err(|_| "Export filename is too long")?;
        let offset = u32::try_from(out.len()).map_err(|_| "PBR archive exceeds 4 GiB")?;
        let crc = crc32fast::hash(bytes);
        dword(&mut out, 0x04034b50);
        for value in [20, 0x800, 0, 0, 33] {
            word(&mut out, value);
        }
        for value in [crc, size, size] {
            dword(&mut out, value);
        }
        word(&mut out, name_size);
        word(&mut out, 0);
        out.extend(name.as_bytes());
        out.extend(bytes);
        dword(&mut directory, 0x02014b50);
        for value in [20, 20, 0x800, 0, 0, 33] {
            word(&mut directory, value);
        }
        for value in [crc, size, size] {
            dword(&mut directory, value);
        }
        for value in [name_size, 0, 0, 0, 0] {
            word(&mut directory, value);
        }
        dword(&mut directory, 0);
        dword(&mut directory, offset);
        directory.extend(name.as_bytes());
    }
    let offset = u32::try_from(out.len()).map_err(|_| "PBR archive exceeds 4 GiB")?;
    let size = u32::try_from(directory.len()).map_err(|_| "PBR directory exceeds ZIP32 size")?;
    out.extend(directory);
    dword(&mut out, 0x06054b50);
    for value in [0, 0, count, count] {
        word(&mut out, value);
    }
    dword(&mut out, size);
    dword(&mut out, offset);
    word(&mut out, 0);
    Ok(out)
}
