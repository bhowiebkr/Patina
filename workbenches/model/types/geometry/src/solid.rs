//! `Solid`: a closed solid of the Manifold kernel. Only this file and `sketch.rs` call the kernel.

use std::ffi::{c_int, c_void};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::mesh::Mesh;
use crate::sketch::Sketch;
use crate::sockets::Frame;

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

// "C-unwind": a C++ exception of the kernel goes through the Rust calls to `patina_guard`.
unsafe extern "C-unwind" {
    fn patina_guard(run: unsafe extern "C-unwind" fn(*mut c_void), data: *mut c_void) -> c_int;
    fn patina_manifold_failed(mem: *mut Raw, status: c_int) -> *mut Raw;
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
    fn manifold_mirror(mem: *mut Raw, m: *mut Raw, x: f64, y: f64, z: f64) -> *mut Raw;
    fn manifold_transform(
        mem: *mut Raw, m: *mut Raw, x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64, x3: f64, y3: f64, z3: f64,
        x4: f64, y4: f64, z4: f64,
    ) -> *mut Raw;
    fn manifold_hull(mem: *mut Raw, m: *mut Raw) -> *mut Raw;
    fn manifold_meshgl(mem: *mut c_void, props: *const f32, n_verts: usize, n_props: usize, tris: *const u32, n_tris: usize) -> *mut c_void;
    fn manifold_of_meshgl(mem: *mut Raw, mesh: *mut c_void) -> *mut Raw;
    fn manifold_project(mem: *mut c_void, m: *mut Raw) -> *mut c_void;
    fn manifold_polygons(mem: *mut c_void, ps: *const *mut c_void, length: usize) -> *mut c_void;
    fn manifold_alloc_polygons() -> *mut c_void;
    fn manifold_delete_polygons(p: *mut c_void);
    fn manifold_extrude(mem: *mut Raw, cs: *mut c_void, height: f64, slices: c_int, twist: f64, sx: f64, sy: f64) -> *mut Raw;
    fn manifold_revolve(mem: *mut Raw, cs: *mut c_void, segments: c_int, degrees: f64) -> *mut Raw;
    fn manifold_alloc_manifold_vec() -> *mut c_void;
    fn manifold_delete_manifold_vec(v: *mut c_void);
    fn manifold_manifold_vec(mem: *mut c_void, size: usize) -> *mut c_void;
    fn manifold_manifold_vec_set(v: *mut c_void, index: usize, m: *mut Raw);
    fn manifold_batch_boolean(mem: *mut Raw, v: *mut c_void, op: c_int) -> *mut Raw;
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
pub struct Solid(*mut Raw, u64);

/// The serial of the next solid.
pub(crate) static SERIAL: AtomicU64 = AtomicU64::new(1);

// SAFETY: a Manifold value is immutable after its construction. Its lazy CSG tree locks a mutex
// at each access (csg_tree.h), so other threads can read and drop copies at the same time.
unsafe impl Send for Solid {}
// SAFETY: see `Send`. The methods with `&self` only read the solid.
unsafe impl Sync for Solid {}

impl Solid {
    /// Makes a solid in new kernel memory. `make` constructs into the memory that it gets. A C++
    /// exception gives a solid with the error.
    fn make(make: impl FnOnce(*mut Raw) -> *mut Raw) -> Solid {
        // SAFETY: the memory has the size and alignment of a Manifold, and `make` constructs one
        // in it. After an exception, the memory has no Manifold, and the failed one goes into it.
        let raw = unsafe {
            let mem = manifold_alloc_manifold();
            guard(|| make(mem)).unwrap_or_else(|status| patina_manifold_failed(mem, status))
        };
        Solid(raw, SERIAL.fetch_add(1, Ordering::Relaxed))
    }

    /// Makes a solid with `make`, and runs its boolean at once. The kernel collapses a lazy boolean
    /// tree by the reference counts at its evaluation, so a later evaluation can give other triangles.
    fn boolean(make: impl FnOnce(*mut Raw) -> *mut Raw) -> Solid {
        let solid = Solid::make(make);
        // SAFETY: the solid is valid. The status runs the boolean and keeps its result.
        match guard(|| unsafe { manifold_status(solid.0) }) {
            Ok(_) => solid,
            Err(status) => Solid::failed(status),
        }
    }

    /// An empty solid with the kernel error `status`.
    fn failed(status: c_int) -> Solid {
        // SAFETY: `mem` is new Manifold memory.
        Solid::make(|mem| unsafe { patina_manifold_failed(mem, status) })
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
        Solid::make(|mem| unsafe { manifold_cylinder(mem, height, radius_low, radius_high, limit(segments), 1) })
    }

    /// A sphere with its center at the origin. `segments` 0 lets the kernel choose.
    pub fn sphere(radius: f64, segments: i32) -> Solid {
        // SAFETY: `mem` is new Manifold memory.
        Solid::make(|mem| unsafe { manifold_sphere(mem, radius, limit(segments)) })
    }

    /// The volume in this solid or in `other`.
    pub fn union(&self, other: &Solid) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and both inputs are valid solids.
        Solid::boolean(|mem| unsafe { manifold_union(mem, self.0, other.0) })
    }

    /// The volume in this solid and not in `other`.
    pub fn difference(&self, other: &Solid) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and both inputs are valid solids.
        Solid::boolean(|mem| unsafe { manifold_difference(mem, self.0, other.0) })
    }

    /// The volume in both solids.
    pub fn intersection(&self, other: &Solid) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and both inputs are valid solids.
        Solid::boolean(|mem| unsafe { manifold_intersection(mem, self.0, other.0) })
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

    /// Makes the sketch thick along Z, from 0 to `height`. The top turns by `twist` degrees and
    /// scales by `scale`. `slices` 0 makes one slice, or more when the top turns.
    /// A sketch with an error gives a solid with the same error.
    pub fn extrude(sketch: &Sketch, height: f64, twist: f64, scale: [f64; 2], slices: i32) -> Solid {
        if sketch.status() != 0 {
            return Solid::failed(sketch.status());
        }
        sketch.with_polygons(|polygons| {
            // SAFETY: `mem` is new Manifold memory, and the polygons are valid during the call.
            Solid::make(|mem| unsafe { manifold_extrude(mem, polygons, height, limit(slices), twist, scale[0], scale[1]) })
        })
    }

    /// Turns the sketch around its Y axis by `degrees`, and makes that axis the Z axis. The kernel
    /// uses only the part of the sketch at x > 0. `segments` 0 lets the kernel choose.
    /// A sketch with an error gives a solid with the same error.
    pub fn revolve(sketch: &Sketch, degrees: f64, segments: i32) -> Solid {
        if sketch.status() != 0 {
            return Solid::failed(sketch.status());
        }
        // SAFETY: `mem` is new Manifold memory, and the polygons are valid during the call.
        sketch.with_polygons(|polygons| Solid::make(|mem| unsafe { manifold_revolve(mem, polygons, limit(segments), degrees) }))
    }

    /// The union of all the solids, in one kernel operation. It is faster than a chain of unions.
    pub fn union_all(solids: &[Solid]) -> Solid {
        // SAFETY: the vector gets copies of valid solids (the kernel shares their data), and it is
        // deleted after the boolean. 0 is `MANIFOLD_ADD`.
        unsafe {
            let all = manifold_manifold_vec(manifold_alloc_manifold_vec(), solids.len());
            for (i, solid) in solids.iter().enumerate() {
                manifold_manifold_vec_set(all, i, solid.0);
            }
            let union = Solid::boolean(|mem| manifold_batch_boolean(mem, all, 0));
            manifold_delete_manifold_vec(all);
            union
        }
    }

    /// Mirrors the solid on the plane through the origin with the normal `[x, y, z]`.
    pub fn mirror(&self, [x, y, z]: [f64; 3]) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and the input is a valid solid.
        Solid::make(|mem| unsafe { manifold_mirror(mem, self.0, x, y, z) })
    }

    /// Moves the solid from the axes of the model into `frame`: its origin goes to the origin of
    /// the frame, and its X, Y and Z axes go to the axes of the frame.
    pub fn place(&self, frame: &Frame) -> Solid {
        let Frame { x, y, z, origin: o } = *frame;
        // SAFETY: `mem` is new Manifold memory, and the input is a valid solid.
        Solid::make(|mem| unsafe {
            manifold_transform(mem, self.0, x[0], x[1], x[2], y[0], y[1], y[2], z[0], z[1], z[2], o[0], o[1], o[2])
        })
    }

    /// The smallest convex solid around this solid.
    pub fn hull(&self) -> Solid {
        // SAFETY: `mem` is new Manifold memory, and the input is a valid solid.
        Solid::make(|mem| unsafe { manifold_hull(mem, self.0) })
    }

    /// The solid that the triangles close, with their corners counterclockwise from the outside.
    /// The solid has the kernel error of a mesh that is not closed or of an index that is too large.
    pub fn of_triangles(corners: &[[f32; 3]], triangles: &[[u32; 3]]) -> Solid {
        // SAFETY: the MeshGL copies the two arrays of the given lengths into new MeshGL memory,
        // the solid copies the mesh into new Manifold memory, and the mesh is deleted after the copy,
        // also after an exception.
        unsafe {
            let (props, tris) = (corners.as_flattened(), triangles.as_flattened());
            let mesh = manifold_meshgl(manifold_alloc_meshgl(), props.as_ptr(), corners.len(), 3, tris.as_ptr(), triangles.len());
            let solid = Solid::make(|mem| manifold_of_meshgl(mem, mesh));
            manifold_delete_meshgl(mesh);
            solid
        }
    }

    /// The solid, or the kernel error as a text for the node.
    pub fn checked(self) -> Result<Solid, String> {
        match self.error() {
            Some(error) => Err(format!("the kernel gave an error: {error}")),
            None => Ok(self),
        }
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
        guard(|| unsafe { manifold_original_id(self.0) }).ok().and_then(|id| u32::try_from(id).ok())
    }

    /// The outline of the solid as seen from +Z, on the XY plane. Turn the solid first to see
    /// another side, for example `rotate([-90.0, 0.0, 0.0])` for the side view on X and Z.
    pub fn project(&self) -> Sketch {
        // SAFETY: `mem` is new Polygons memory that the kernel fills from the valid solid, or that
        // gets no polygons after an exception. The sketch copies the polygons, and they are
        // deleted after the copy.
        unsafe {
            let mem = manifold_alloc_polygons();
            let polygons = guard(|| manifold_project(mem, self.0)).unwrap_or_else(|_| manifold_polygons(mem, std::ptr::null(), 0));
            let sketch = Sketch::of_projection(polygons);
            manifold_delete_polygons(polygons);
            sketch
        }
    }

    /// The kernel error of the solid, for example from a construction that is not valid.
    pub fn error(&self) -> Option<&'static str> {
        // SAFETY: the solid is valid. The status runs the operations that wait.
        error_text(guard(|| unsafe { manifold_status(self.0) }).unwrap_or_else(|status| status))
    }

    /// A number that no other solid has, also not a copy. A view compares it to find a new value.
    pub fn serial(&self) -> u64 {
        self.1
    }

    /// The number of triangles.
    pub fn triangles(&self) -> usize {
        // SAFETY: the solid is valid.
        guard(|| unsafe { manifold_num_tri(self.0) }).unwrap_or(0)
    }

    /// The lowest and the highest corner of the bounds. An empty solid gives infinite values.
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        // SAFETY: `mem` is new box memory that the box constructor fills, and the box is deleted
        // after its read. A Box has no destructor, so the delete is also safe after an exception.
        unsafe {
            let mem = manifold_alloc_box();
            let bounds = guard(|| {
                let bounds = manifold_bounding_box(mem, self.0);
                let (min, max) = (manifold_box_min(bounds), manifold_box_max(bounds));
                [[min.x, min.y, min.z], [max.x, max.y, max.z]]
            });
            manifold_delete_box(mem);
            bounds.unwrap_or([[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]])
        }
    }

    /// The triangle mesh of the solid, with a flat normal on each triangle. An exception of the
    /// kernel gives an empty mesh.
    pub fn mesh(&self) -> Mesh {
        // SAFETY: `mem` is new MeshGL memory that the constructor fills, or that gets an empty
        // mesh after an exception. Each array gets a buffer of the length that the kernel gives,
        // and the mesh is deleted after the copies.
        unsafe {
            let mem = manifold_alloc_meshgl();
            let gl = guard(|| manifold_get_meshgl(mem, self.0))
                .unwrap_or_else(|_| manifold_meshgl(mem, std::ptr::null(), 0, 3, std::ptr::null(), 0));
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

/// The most segments or slices of one shape. A larger count from a parameter could use all the memory.
pub const MAX_SEGMENTS: i32 = 1024;

/// The most copies that an array node makes, for the same reason.
pub const MAX_COPIES: u32 = 1000;

/// A segment or slice count for the kernel: 0 (the kernel chooses) to `MAX_SEGMENTS`.
pub(crate) fn limit(segments: i32) -> i32 {
    segments.clamp(0, MAX_SEGMENTS)
}

/// The text of the kernel status `status`, or `None` for 0.
pub(crate) fn error_text(status: c_int) -> Option<&'static str> {
    (status != 0).then(|| usize::try_from(status).ok().and_then(|i| ERRORS.get(i)).copied().unwrap_or("unknown error"))
}

/// Runs the kernel calls in `run`, which must not panic. A C++ exception of the kernel gives the
/// kernel status of its error and does not stop the program: "the result is too large" for a
/// `bad_alloc`, or else "the construction is not valid". The callers make their temporary kernel
/// objects before `run` and delete them after it, so that an exception does not leave them.
pub(crate) fn guard<R, F: FnOnce() -> R>(run: F) -> Result<R, c_int> {
    unsafe extern "C-unwind" fn call<R, F: FnOnce() -> R>(data: *mut c_void) {
        // SAFETY: `data` is the pair of `guard`, which lives until `patina_guard` returns.
        let (run, result) = unsafe { &mut *data.cast::<(Option<F>, Option<R>)>() };
        *result = run.take().map(|run| run());
    }
    let mut pair: (Option<F>, Option<R>) = (Some(run), None);
    // SAFETY: `call` gets a pointer to the pair, which lives during the call.
    let status = unsafe { patina_guard(call::<R, F>, (&raw mut pair).cast()) };
    pair.1.ok_or(status)
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

#[test]
fn a_kernel_exception_gives_an_error() {
    // SAFETY: a vector of 2^50 solids cannot get its memory, so the kernel throws `bad_alloc`.
    let status = guard(|| unsafe { manifold_manifold_vec(manifold_alloc_manifold_vec(), 1 << 50) });
    assert_eq!(status.err(), Some(12));
    // SAFETY: `mem` is new Manifold memory.
    let failed = Solid::make(|mem| unsafe { patina_manifold_failed(mem, 12) });
    assert_eq!(failed.union(&Solid::cube(1.0, 1.0, 1.0)).translate([1.0; 3]).error(), Some("the result is too large"));
}
