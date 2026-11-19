//! `Scene`: meshes at places. A part that repeats, for example a window, is one mesh with many
//! places, so the view draws it as GPU instances and the build makes no boolean.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::mesh::{Mesh, summary};
use crate::sockets::Frame;

/// A frame as the columns of a 3 x 4 matrix: the X, Y and Z axes, then the origin.
pub type Matrix = [f32; 12];

/// The frame that does not move a mesh.
pub const IDENTITY: Matrix = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

/// A place of a mesh, with its group: the number of a building, or 0 for no building.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub matrix: Matrix,
    pub group: u32,
}

impl Place {
    /// The place of `frame`. The axes of the frame can have a length, to scale the mesh.
    pub fn new(frame: &Frame, group: u32) -> Place {
        let Frame { x, y, z, origin } = *frame;
        let mut matrix = [0.0; 12];
        for (to, v) in matrix.iter_mut().zip(x.into_iter().chain(y).chain(z).chain(origin)) {
            *to = v as f32;
        }
        Place { matrix, group }
    }

    /// Moves a point from the mesh into the place.
    pub fn point(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.matrix;
        std::array::from_fn(|i| m[i] * p[0] + m[3 + i] * p[1] + m[6 + i] * p[2] + m[9 + i])
    }

    /// Turns a normal into the place. It is exact for turns and for scales that keep the axes.
    pub fn normal(&self, n: [f32; 3]) -> [f32; 3] {
        let m = &self.matrix;
        let v: [f32; 3] = std::array::from_fn(|i| m[i] * n[0] + m[3 + i] * n[1] + m[6 + i] * n[2]);
        let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if length > 0.0 { v.map(|c| c / length) } else { v }
    }
}

/// One mesh with a color at one or more places.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub name: String,
    pub mesh: Mesh,
    /// The color as sRGB 0xRRGGBB.
    pub color: u32,
    pub places: Vec<Place>,
}

/// A list of items. A copy is cheap, because the items are shared.
#[derive(Clone)]
pub struct Scene {
    items: Vec<Arc<Item>>,
    serial: u64,
}

impl Scene {
    /// A scene with the items that have triangles and places.
    pub fn new(items: impl IntoIterator<Item = Item>) -> Scene {
        let items = items.into_iter().filter(|i| i.mesh.triangles() > 0 && !i.places.is_empty()).map(Arc::new);
        Scene::of(items.collect())
    }

    fn of(items: Vec<Arc<Item>>) -> Scene {
        Scene { items, serial: crate::solid::SERIAL.fetch_add(1, Ordering::Relaxed) }
    }

    /// The items of this scene, then the items of `other`.
    pub fn merge(&self, other: &Scene) -> Scene {
        Scene::of(self.items.iter().chain(&other.items).cloned().collect())
    }

    pub fn items(&self) -> &[Arc<Item>] {
        &self.items
    }

    /// A number that no other scene has, also not a copy. A view compares it to find a new value.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// The number of places of all the items.
    pub fn places(&self) -> usize {
        self.items.iter().map(|i| i.places.len()).sum()
    }

    /// The number of triangles at all the places.
    pub fn triangles(&self) -> usize {
        self.items.iter().map(|i| i.mesh.triangles() * i.places.len()).sum()
    }

    /// The groups above 0, in order.
    pub fn groups(&self) -> Vec<u32> {
        let mut groups: Vec<u32> = self.items.iter().flat_map(|i| i.places.iter().map(|p| p.group)).filter(|&g| g > 0).collect();
        groups.sort_unstable();
        groups.dedup();
        groups
    }

    /// The lowest and the highest corner of all the places. An empty scene gives infinite values.
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
        for item in &self.items {
            let [low, high] = mesh_bounds(&item.mesh);
            for place in &item.places {
                for corner in 0..8 {
                    let p = place.point(std::array::from_fn(|i| if (corner >> i) & 1 == 0 { low[i] } else { high[i] }));
                    for i in 0..3 {
                        bounds[0][i] = bounds[0][i].min(f64::from(p[i]));
                        bounds[1][i] = bounds[1][i].max(f64::from(p[i]));
                    }
                }
            }
        }
        bounds
    }

    /// One mesh with the places of `group`, or of all the groups for `None`. Each item gives a
    /// part, so a `.glb` file has one primitive for each item.
    pub fn bake(&self, group: Option<u32>) -> Mesh {
        let mut mesh = Mesh::default();
        let mut part = 0;
        for item in &self.items {
            let mut used = false;
            for place in item.places.iter().filter(|p| group.is_none_or(|g| p.group == g)) {
                used = true;
                mesh.positions.extend(item.mesh.positions.iter().map(|&p| place.point(p)));
                mesh.normals.extend(item.mesh.normals.iter().map(|&n| place.normal(n)));
                mesh.uvs.extend_from_slice(&item.mesh.uvs);
                mesh.parts.extend(std::iter::repeat_n(part, item.mesh.triangles()));
            }
            part += u32::from(used);
        }
        mesh
    }

    /// A hash of the meshes, the colors and the places, the same on each platform.
    pub fn hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for item in &self.items {
            let places = item.places.iter().flat_map(|p| p.matrix.map(f32::to_bits).into_iter().chain([p.group]));
            let words = [item.mesh.hash() as u32, (item.mesh.hash() >> 32) as u32, item.color].into_iter().chain(places);
            for byte in words.flat_map(u32::to_le_bytes) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
        hash
    }
}

impl Default for Scene {
    fn default() -> Scene {
        Scene::of(Vec::new())
    }
}

/// The lowest and the highest corner of the mesh.
pub(crate) fn mesh_bounds(mesh: &Mesh) -> [[f32; 3]; 2] {
    let mut bounds = [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]];
    for p in &mesh.positions {
        for i in 0..3 {
            bounds[0][i] = bounds[0][i].min(p[i]);
            bounds[1][i] = bounds[1][i].max(p[i]);
        }
    }
    bounds
}

/// Shows "3 items, 120 places, 1440 triangles, bounds 0 0 0 to 10 10 4".
impl fmt::Debug for Scene {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} items, {} places, ", self.items.len(), self.places())?;
        summary(f, self.triangles(), self.bounds())
    }
}
