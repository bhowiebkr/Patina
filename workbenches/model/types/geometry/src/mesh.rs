//! `Mesh`: the triangles of a solid, for the view and the export.

use std::fmt;

/// A triangle mesh with three corners for each triangle, so each triangle has its own flat
/// normal and UV. Z is up.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// The corners, three for each triangle, in counterclockwise order from the outside.
    pub positions: Vec<[f32; 3]>,
    /// The normal of each corner.
    pub normals: Vec<[f32; 3]>,
    /// The UV of each corner: a box projection along the main axis of the normal.
    pub uvs: Vec<[f32; 2]>,
    /// The part of each triangle: 0 for the first part of the mesh, 1 for the next part, and
    /// so on. Each primitive and each `Solid::as_part` makes a part.
    pub parts: Vec<u32>,
}

impl Mesh {
    /// Makes the mesh from the arrays of a Manifold `MeshGL`. `stride` is the number of
    /// properties of a vertex, and the first three are the position. `runs` has the start of
    /// each run in `tris`, and one more item at the end. `ids` has the part ID of each run.
    pub(crate) fn from_kernel(props: &[f32], stride: usize, tris: &[u32], runs: &[u32], ids: &[u32]) -> Mesh {
        let count = tris.len() / 3;
        let mut mesh = Mesh {
            positions: Vec::with_capacity(count * 3),
            normals: Vec::with_capacity(count * 3),
            uvs: Vec::with_capacity(count * 3),
            parts: vec![0; count],
        };
        let vertex = |i: u32| {
            let at = i as usize * stride;
            props.get(at..at + 3).map_or([0.0; 3], |p| [p[0], p[1], p[2]])
        };
        for tri in tris.chunks_exact(3) {
            let corners = [vertex(tri[0]), vertex(tri[1]), vertex(tri[2])];
            let normal = normal(corners);
            mesh.positions.extend(corners);
            mesh.normals.extend([normal; 3]);
            mesh.uvs.extend(corners.map(|p| project(p, normal)));
        }
        // The kernel IDs come from a global counter, so each build gives other IDs for the same
        // graph. The index of the first run of each ID is the same at each build.
        let mut seen: Vec<u32> = Vec::new();
        for (run, &id) in runs.windows(2).zip(ids) {
            let part = seen.iter().position(|&s| s == id).unwrap_or_else(|| {
                seen.push(id);
                seen.len() - 1
            });
            let (start, end) = ((run[0] / 3) as usize, ((run[1] / 3) as usize).min(count));
            if let Some(parts) = mesh.parts.get_mut(start..end) {
                parts.fill(part as u32);
            }
        }
        mesh
    }

    /// The number of triangles.
    pub fn triangles(&self) -> usize {
        self.parts.len()
    }

    /// A hash of the positions and the parts, the same on each platform for the same mesh.
    pub fn hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let bits = self.positions.iter().flatten().map(|v| v.to_bits()).chain(self.parts.iter().copied());
        for byte in bits.flat_map(u32::to_le_bytes) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
        hash
    }
}

/// The unit normal of a triangle, or zero for a triangle with no area.
fn normal([a, b, c]: [[f32; 3]; 3]) -> [f32; 3] {
    let (u, v) = (sub(b, a), sub(c, a));
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if length > 0.0 { n.map(|x| x / length) } else { [0.0; 3] }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The UV of a corner: the two coordinates that are not along the main axis of the normal.
fn project([x, y, z]: [f32; 3], [nx, ny, nz]: [f32; 3]) -> [f32; 2] {
    let (ax, ay, az) = (nx.abs(), ny.abs(), nz.abs());
    if az >= ax && az >= ay {
        [x, y]
    } else if ax >= ay {
        [y, z]
    } else {
        [x, z]
    }
}

/// Writes "12 triangles, bounds -1 -1 -1 to 1 1 1", or "empty" for no triangles.
pub(crate) fn summary(f: &mut fmt::Formatter, triangles: usize, [low, high]: [[f64; 3]; 2]) -> fmt::Result {
    if triangles == 0 {
        return write!(f, "empty");
    }
    write!(f, "{triangles} triangles, bounds")?;
    for (i, v) in low.iter().chain(&high).enumerate() {
        write!(f, "{}{}", if i == 3 { " to " } else { " " }, Short(*v))?;
    }
    Ok(())
}

/// A number with at most 3 decimals and no trailing zeros, for example "1.5" and "-0.125".
struct Short(f64);

impl fmt::Display for Short {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let text = format!("{:.3}", self.0);
        let text = if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.') } else { &text };
        f.write_str(if text == "-0" { "0" } else { text })
    }
}
