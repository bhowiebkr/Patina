//! `Solid`: a closed solid of the Manifold kernel. Only this file calls the kernel.

use std::ffi::{c_int, c_void};
use std::fmt;

use crate::mesh::Mesh;

#[repr(C)]
struct Raw {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}

unsafe extern "C" {
    fn manifold_alloc_manifold() -> *mut Raw;
    fn manifold_alloc_meshgl() -> *mut c_void;
    fn manifold_alloc_box() -> *mut c_void;
    fn manifold_delete_manifold(m: *mut Raw);
    fn manifold_delete_meshgl(m: *mut c_void);
    fn manifold_delete_box(b: *mut c_void);
    fn manifold_empty(mem: *mut Raw) -> *mut Raw;
    fn manifold_copy(mem: *mut Raw, m: *mut Raw) -> *mut Raw;
    fn manifold_cube(mem: *mut Raw, x: f64, y: f64, z: f64, center: c_int) -> *mut Raw;
    fn manifold_cylinder(mem: *mut Raw, height: f64, low: f64, high: f64, segments: c_int, center: c_int) -> *mut Raw;
    fn manifold_sphere(mem: *mut Raw, radius: f64, segments: c_int) -> *mut Raw;
    fn manifold_union(mem: *mut Raw, a: *mut Raw, b: *mut Raw) -> *mut Raw;
    fn manifold_difference(mem: *mut Raw, a: *mut Raw, b: *mut Raw) -> *mut Raw;
    fn manifold_intersection(mem: *mut Raw, a: *mut Raw, b: *mut Raw) -> *mut Raw;
    fn manifold_translate(mem: *mut Raw, m: *mut Raw, x: f64, y: f64, z: f64) -> *mut Raw;
    fn manifold_rotate(mem: *mut Raw, m: *mut Raw, x: f64, y: f64, z: f64) -> *mut Raw;
    fn manifold_scale(mem: *mut Raw, m: *mut Raw, x: f64, y: f64, z: f64) -> *mut Raw;
    fn manifold_as_original(mem: *mut Raw, m: *mut Raw) -> *mut Raw;
    fn manifold_original_id(m: *mut Raw) -> c_int;
    fn manifold_status(m: *mut Raw) -> c_int;
    fn manifold_num_tri(m: *mut Raw) -> usize;
    fn manifold_bounding_box(mem: *mut c_void, m: *mut Raw) -> *mut c_void;
    fn manifold_box_min(b: *mut c_void) -> Vec3;
    fn manifold_box_max(b: *mut c_void) -> Vec3;
    fn manifold_get_meshgl(mem: *mut c_void, m: *mut Raw) -> *mut c_void;
    fn manifold_meshgl_num_prop(m: *mut c_void) -> usize;
    fn manifold_meshgl_vert_properties_length(m: *mut c_void) -> usize;
    fn manifold_meshgl_tri_length(m: *mut c_void) -> usize;
    fn manifold_meshgl_run_index_length(m: *mut c_void) -> usize;
    fn manifold_meshgl_run_original_id_length(m: *mut c_void) -> usize;
    fn manifold_meshgl_vert_properties(mem: *mut f32, m: *mut c_void) -> *mut f32;
    fn manifold_meshgl_tri_verts(mem: *mut u32, m: *mut c_void) -> *mut u32;
    fn manifold_meshgl_run_index(mem: *mut u32, m: *mut c_void) -> *mut u32;
    fn manifold_meshgl_run_original_id(mem: *mut u32, m: *mut c_void) -> *mut u32;
}

/// The error texts of `ManifoldError` in `vendor/manifold/bindings/c/include/manifold/types.h`.
const ERRORS: [&str; 15] = [
    "no error",
    "a vertex is not finite",
    "the mesh is not manifold",
    "a vertex index is out of bounds",
    "the properties have the wrong length",
    "the position properties are missing",
    "the merge vectors have different lengths",
    "a merge index is out of bounds",
    "the transform has the wrong length",
    "the run index has the wrong length",
    "the face ID has the wrong length",
    "the construction is not valid",
    "the result is too large",
    "the tangents are not valid",
    "the operation was cancelled",
];

/// A closed solid. A copy is cheap: the kernel shares the data, and each operation gives a new
/// solid. Z is up. The units are free, for example meters for a city and centimeters for a gun.
pub struct Solid(*mut Raw);

// SAFETY: a Manifold value is immutable after its construction. Its lazy CSG tree locks a mutex
// at each access (csg_tree.h), so other threads can read and drop copies at the same time.
unsafe impl Send for Solid {}
// SAFETY: see `Send`. The methods with `&self` only read the solid.
unsafe impl Sync for Solid {}

impl Solid {
    /// Makes a solid in new kernel memory. `make` constructs into the memory that it gets.
    fn make(make: impl FnOnce(*mut Raw) -> *mut Raw) -> Solid {
        // SAFETY: the memory has the size and alignment of a Manifold, and `make` constructs one in it.
        Solid(make(unsafe { manifold_alloc_manifold() }))
    }

    /// A solid with no volume.
    pub fn empty() -> Solid {
        // SAFETY: `mem` is new Manifold memory.
        Solid::make(|mem| unsafe { manifold_empty(mem) })
    }

    /// A box with the sizes `x`, `y` and `z`, with its center at the origin.
    pub fn cube(x: f64, y: f64, z: f64) -> Solid {
        // SAFETY: `mem` is new Manifold memory.
        Solid::make(|mem| unsafe { manifold_cube(mem, x, y, z, 1) })
    }

    /// A cylinder or a cone along Z, with its center at the origin. `segments` 0 lets the kernel
    /// choose the number of sides from the radius.
    pub fn cylinder(height: f64, radius_low: f64, radius_high: f64, segments: i32) -> Solid {
        // SAFETY: `mem` is new Manifold memory.
        Solid::make(|mem| unsafe { manifold_cylinder(mem, height, radius_low, radius_high, segments, 1) })
    }

    /// A sphere with its center at the origin. `segments` 0 lets the kernel choose.
    pub fn sphere(radius: f64, segments: i32) -> Solid {
        // SAFETY: `mem` is new Manifold memory.
        Solid::make(|mem| unsafe { manifold_sphere(mem, radius, segments) })
    }

    /// The volume in this solid or in `other`.
    pub fn union(&self, other: &Solid) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and both inputs are valid solids.
        Solid::make(|mem| unsafe { manifold_union(mem, self.0, other.0) })
    }

    /// The volume in this solid and not in `other`.
    pub fn difference(&self, other: &Solid) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and both inputs are valid solids.
        Solid::make(|mem| unsafe { manifold_difference(mem, self.0, other.0) })
    }

    /// The volume in both solids.
    pub fn intersection(&self, other: &Solid) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and both inputs are valid solids.
        Solid::make(|mem| unsafe { manifold_intersection(mem, self.0, other.0) })
    }

    /// Moves the solid by `[x, y, z]`.
    pub fn translate(&self, [x, y, z]: [f64; 3]) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and the input is a valid solid.
        Solid::make(|mem| unsafe { manifold_translate(mem, self.0, x, y, z) })
    }

    /// Turns the solid by the angles in degrees: first about X, then Y, then Z.
    pub fn rotate(&self, [x, y, z]: [f64; 3]) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and the input is a valid solid.
        Solid::make(|mem| unsafe { manifold_rotate(mem, self.0, x, y, z) })
    }

    /// Scales the solid on each axis.
    pub fn scale(&self, [x, y, z]: [f64; 3]) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and the input is a valid solid.
        Solid::make(|mem| unsafe { manifold_scale(mem, self.0, x, y, z) })
    }

    /// Gives the solid a new part ID. The triangles keep it through the booleans, so the mesh
    /// knows the part of each triangle.
    pub fn as_part(&self) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and the input is a valid solid.
        Solid::make(|mem| unsafe { manifold_as_original(mem, self.0) })
    }

    /// The kernel ID of `as_part`, or `None` for a solid that is a mix of parts. The ID is
    /// different at each build. `Mesh::parts` has an index that stays the same.
    pub fn part(&self) -> Option<u32> {
        // SAFETY: the solid is valid.
        u32::try_from(unsafe { manifold_original_id(self.0) }).ok()
    }

    /// The kernel error of the solid, for example from a construction that is not valid.
    pub fn error(&self) -> Option<&'static str> {
        // SAFETY: the solid is valid. The status runs the operations that wait.
        let status = unsafe { manifold_status(self.0) };
        (status != 0).then(|| usize::try_from(status).ok().and_then(|i| ERRORS.get(i)).copied().unwrap_or("unknown error"))
    }

    /// The number of triangles.
    pub fn triangles(&self) -> usize {
        // SAFETY: the solid is valid.
        unsafe { manifold_num_tri(self.0) }
    }

    /// The lowest and the highest corner of the bounds. An empty solid gives infinite values.
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        // SAFETY: `mem` is new box memory that the box constructor fills, and the box is deleted after its read.
        unsafe {
            let bounds = manifold_bounding_box(manifold_alloc_box(), self.0);
            let (min, max) = (manifold_box_min(bounds), manifold_box_max(bounds));
            manifold_delete_box(bounds);
            [[min.x, min.y, min.z], [max.x, max.y, max.z]]
        }
    }

    /// The triangle mesh of the solid, with a flat normal on each triangle.
    pub fn mesh(&self) -> Mesh {
        // SAFETY: `mem` is new MeshGL memory that the constructor fills. Each array gets a buffer
        // of the length that the kernel gives, and the mesh is deleted after the copies.
        unsafe {
            let gl = manifold_get_meshgl(manifold_alloc_meshgl(), self.0);
            let stride = manifold_meshgl_num_prop(gl);
            let mut props = vec![0.0_f32; manifold_meshgl_vert_properties_length(gl)];
            let mut tris = vec![0_u32; manifold_meshgl_tri_length(gl)];
            let mut runs = vec![0_u32; manifold_meshgl_run_index_length(gl)];
            let mut ids = vec![0_u32; manifold_meshgl_run_original_id_length(gl)];
            manifold_meshgl_vert_properties(props.as_mut_ptr(), gl);
            manifold_meshgl_tri_verts(tris.as_mut_ptr(), gl);
            manifold_meshgl_run_index(runs.as_mut_ptr(), gl);
            manifold_meshgl_run_original_id(ids.as_mut_ptr(), gl);
            manifold_delete_meshgl(gl);
            Mesh::from_kernel(&props, stride, &tris, &runs, &ids)
        }
    }
}

impl Clone for Solid {
    fn clone(&self) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and the input is a valid solid.
        Solid::make(|mem| unsafe { manifold_copy(mem, self.0) })
    }
}

impl Drop for Solid {
    fn drop(&mut self) {
        // SAFETY: the solid owns its memory, from `manifold_alloc_manifold`, and drops one time.
        unsafe { manifold_delete_manifold(self.0) }
    }
}

impl Default for Solid {
    fn default() -> Solid {
        Solid::empty()
    }
}

/// Shows the triangle count and the bounds, for example "12 triangles, -1 -1 -1 to 1 1 1".
impl fmt::Debug for Solid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(error) = self.error() {
            return write!(f, "error: {error}");
        }
        crate::mesh::summary(f, self.triangles(), self.bounds())
    }
}
