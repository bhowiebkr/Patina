//! `mesh.bin`: magic, vertex count, index count, then the raw vertex and u32 index arrays
//! (little-endian). Loads with one copy and no parsing.

use std::path::Path;

use bytemuck::{Pod, Zeroable};

use crate::error::{read_bytes, write_bytes};
use crate::{Error, Result};

const MAGIC: &[u8; 8] = b"MWMESH01";
const HEADER: usize = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// Atlas UV in [0, 1]; charts never overlap.
    pub uv: [f32; 2],
    /// Mean curvature mapped to [0, 1]: 0.5 flat, above convex (edges), below concave (creases).
    pub curvature: f32,
    /// Index into `AssetInfo::parts`.
    pub part: u32,
}

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn to_bytes(&self) -> Vec<u8> {
        let verts: &[u8] = bytemuck::cast_slice(&self.vertices);
        let idx: &[u8] = bytemuck::cast_slice(&self.indices);
        let mut out = Vec::with_capacity(HEADER + verts.len() + idx.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(self.vertices.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.indices.len() as u32).to_le_bytes());
        out.extend_from_slice(verts);
        out.extend_from_slice(idx);
        out
    }

    pub fn from_bytes(path: &Path, bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER || &bytes[..8] != MAGIC {
            return Err(Error::format(path, "not a Millwright mesh (bad magic)"));
        }
        let count = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
        };
        let (nv, ni) = (count(8), count(12));
        let vbytes = nv * size_of::<Vertex>();
        let ibytes = ni * 4;
        if bytes.len() != HEADER + vbytes + ibytes {
            return Err(Error::format(path, "mesh size does not match its header"));
        }
        let vertices: Vec<Vertex> = bytemuck::pod_collect_to_vec(&bytes[HEADER..HEADER + vbytes]);
        let indices: Vec<u32> = bytemuck::pod_collect_to_vec(&bytes[HEADER + vbytes..]);
        if indices.iter().any(|&i| i as usize >= nv) {
            return Err(Error::format(path, "index out of range"));
        }
        Ok(Self { vertices, indices })
    }

    pub fn load(path: &Path) -> Result<Self> {
        Self::from_bytes(path, &read_bytes(path)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_bytes(path, &self.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mesh = MeshData {
            vertices: vec![
                Vertex {
                    position: [1.0, 2.0, 3.0],
                    part: 7,
                    ..Vertex::default()
                };
                3
            ],
            indices: vec![0, 1, 2],
        };
        let path = Path::new("mem");
        let back = MeshData::from_bytes(path, &mesh.to_bytes()).unwrap();
        assert_eq!(back.vertices, mesh.vertices);
        assert_eq!(back.indices, mesh.indices);
    }

    #[test]
    fn rejects_bad_index() {
        let mesh = MeshData {
            vertices: vec![Vertex::default(); 2],
            indices: vec![0, 1, 2],
        };
        assert!(MeshData::from_bytes(Path::new("mem"), &mesh.to_bytes()).is_err());
    }
}
