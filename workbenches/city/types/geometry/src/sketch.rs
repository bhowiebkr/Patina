//! `Sketch`: a flat shape on the XY plane, from the cross-section of the kernel.

use std::ffi::{c_int, c_void};
use std::fmt;
use std::ptr::null;

use crate::solid::{error_text, guard};

#[repr(C)]
pub(crate) struct RawSketch {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vec2 {
    x: f64,
    y: f64,
}

/// `MANIFOLD_FILL_RULE_NON_ZERO`: a polygon fills in both turn directions.
const NON_ZERO: c_int = 1;
/// `MANIFOLD_FILL_RULE_POSITIVE`: an area fills where the outlines turn more counterclockwise.
const POSITIVE: c_int = 2;

// "C-unwind": a C++ exception of the kernel goes through the Rust calls to `guard`.
unsafe extern "C-unwind" {
    fn manifold_alloc_cross_section() -> *mut RawSketch;
    fn manifold_alloc_simple_polygon() -> *mut c_void;
    fn manifold_alloc_polygons() -> *mut c_void;
    fn manifold_alloc_rect() -> *mut c_void;
    fn manifold_delete_cross_section(cs: *mut RawSketch);
    fn manifold_delete_simple_polygon(p: *mut c_void);
    fn manifold_delete_polygons(p: *mut c_void);
    fn manifold_delete_rect(r: *mut c_void);
    fn manifold_cross_section_copy(mem: *mut RawSketch, cs: *mut RawSketch) -> *mut RawSketch;
    fn manifold_cross_section_empty(mem: *mut RawSketch) -> *mut RawSketch;
    fn manifold_cross_section_square(mem: *mut RawSketch, x: f64, y: f64, center: c_int) -> *mut RawSketch;
    fn manifold_cross_section_circle(mem: *mut RawSketch, radius: f64, segments: c_int) -> *mut RawSketch;
    fn manifold_simple_polygon(mem: *mut c_void, ps: *const Vec2, length: usize) -> *mut c_void;
    fn manifold_cross_section_of_simple_polygon(mem: *mut RawSketch, p: *mut c_void, fill: c_int) -> *mut RawSketch;
    fn manifold_cross_section_of_polygons(mem: *mut RawSketch, p: *mut c_void, fill: c_int) -> *mut RawSketch;
    fn manifold_polygons_length(p: *mut c_void) -> usize;
    fn manifold_polygons_simple_length(p: *mut c_void, index: usize) -> usize;
    fn manifold_polygons_get_point(p: *mut c_void, simple: usize, point: usize) -> Vec2;
    fn manifold_cross_section_translate(mem: *mut RawSketch, cs: *mut RawSketch, x: f64, y: f64) -> *mut RawSketch;
    fn manifold_cross_section_to_polygons(mem: *mut c_void, cs: *mut RawSketch) -> *mut c_void;
    fn manifold_cross_section_area(cs: *mut RawSketch) -> f64;
    fn manifold_cross_section_num_vert(cs: *mut RawSketch) -> usize;
    fn manifold_cross_section_bounds(mem: *mut c_void, cs: *mut RawSketch) -> *mut c_void;
    fn manifold_rect_min(r: *mut c_void) -> Vec2;
    fn manifold_rect_max(r: *mut c_void) -> Vec2;
    fn manifold_polygons(mem: *mut c_void, ps: *const *mut c_void, length: usize) -> *mut c_void;
    fn manifold_alloc_triangulation() -> *mut c_void;
    fn manifold_delete_triangulation(t: *mut c_void);
    fn manifold_triangulate(mem: *mut c_void, ps: *mut c_void, epsilon: f64) -> *mut c_void;
    fn manifold_triangulation_num_tri(t: *mut c_void) -> usize;
    fn manifold_triangulation_tri_verts(mem: *mut c_int, t: *mut c_void) -> *mut c_int;
}

/// A flat shape on the XY plane, with holes. `Solid::extrude` and `Solid::revolve` make solids
/// from it. The second field is the kernel status of the construction, 0 for no error.
pub struct Sketch(*mut RawSketch, c_int);

// SAFETY: a CrossSection value is immutable after its construction, as a Manifold (see `Solid`).
unsafe impl Send for Sketch {}
// SAFETY: see `Send`. The methods with `&self` only read the sketch.
unsafe impl Sync for Sketch {}

impl Sketch {
    /// Makes a sketch in new kernel memory. `make` constructs into the memory that it gets. A C++
    /// exception, for example from a coordinate that is too large, gives an empty sketch with the
    /// error.
    fn make(make: impl FnOnce(*mut RawSketch) -> *mut RawSketch) -> Sketch {
        // SAFETY: the memory has the size and alignment of a CrossSection, and `make` constructs
        // one in it. After an exception, the memory has no CrossSection, and an empty one goes in.
        unsafe {
            let mem = manifold_alloc_cross_section();
            match guard(|| make(mem)) {
                Ok(raw) => Sketch(raw, 0),
                Err(status) => Sketch(manifold_cross_section_empty(mem), status),
            }
        }
    }

    /// A rectangle with the sizes `x` and `y`, with its center at the origin.
    pub fn rectangle(x: f64, y: f64) -> Sketch {
        // SAFETY: `mem` is new CrossSection memory.
        Sketch::make(|mem| unsafe { manifold_cross_section_square(mem, x, y, 1) })
    }

    /// A circle with its center at the origin. `segments` 0 lets the kernel choose.
    pub fn circle(radius: f64, segments: i32) -> Sketch {
        // SAFETY: `mem` is new CrossSection memory.
        Sketch::make(|mem| unsafe { manifold_cross_section_circle(mem, radius, crate::solid::limit(segments)) })
    }

    /// The shape inside the closed line through `points`. The points can turn in each
    /// direction. A line that crosses itself fills each area that it goes around.
    pub fn polygon(points: &[[f64; 2]]) -> Sketch {
        let points: Vec<Vec2> = points.iter().map(|&[x, y]| Vec2 { x, y }).collect();
        // SAFETY: the polygon copies the points into new SimplePolygon memory, the sketch copies
        // the polygon, and the polygon is deleted after the copy, also after an exception.
        unsafe {
            let polygon = manifold_simple_polygon(manifold_alloc_simple_polygon(), points.as_ptr(), points.len());
            let sketch = Sketch::make(|mem| manifold_cross_section_of_simple_polygon(mem, polygon, NON_ZERO));
            manifold_delete_simple_polygon(polygon);
            sketch
        }
    }

    /// The sketch of kernel Polygons from a projection, with the positive fill rule. The kernel
    /// docs give this rule for the outlines of `Manifold::Project`, which can cross themselves.
    ///
    /// # Safety
    /// `polygons` must be valid kernel Polygons. The caller deletes them.
    pub(crate) unsafe fn of_projection(polygons: *mut c_void) -> Sketch {
        // SAFETY: `mem` is new CrossSection memory, and the caller gives valid polygons.
        Sketch::make(|mem| unsafe { manifold_cross_section_of_polygons(mem, polygons, POSITIVE) })
    }

    /// Moves the sketch by `[x, y]`. The sketch keeps its error.
    pub fn translate(&self, [x, y]: [f64; 2]) -> Sketch {
        // SAFETY: `mem` is new CrossSection memory, and the input is a valid sketch.
        self.keep_error(Sketch::make(|mem| unsafe { manifold_cross_section_translate(mem, self.0, x, y) }))
    }

    /// `made` with the error of this sketch, when `made` has no error of its own.
    fn keep_error(&self, mut made: Sketch) -> Sketch {
        if made.1 == 0 {
            made.1 = self.1;
        }
        made
    }

    /// The kernel error of the construction, for example from a coordinate that is too large.
    pub fn error(&self) -> Option<&'static str> {
        error_text(self.1)
    }

    /// The kernel status of the construction, 0 for no error.
    pub(crate) fn status(&self) -> c_int {
        self.1
    }

    /// The sketch, or the kernel error as a text for the node.
    pub fn checked(self) -> Result<Sketch, String> {
        match self.error() {
            Some(error) => Err(format!("the kernel gave an error: {error}")),
            None => Ok(self),
        }
    }

    /// The area. The area of a hole counts as negative.
    pub fn area(&self) -> f64 {
        // SAFETY: the sketch is valid.
        guard(|| unsafe { manifold_cross_section_area(self.0) }).unwrap_or(0.0)
    }

    /// The number of corners on all the outlines.
    pub fn corners(&self) -> usize {
        // SAFETY: the sketch is valid.
        guard(|| unsafe { manifold_cross_section_num_vert(self.0) }).unwrap_or(0)
    }

    /// The lowest and the highest corner of the bounds.
    pub fn bounds(&self) -> [[f64; 2]; 2] {
        // SAFETY: `mem` is new Rect memory that the constructor fills, and the rect is deleted
        // after its read. A Rect has no destructor, so the delete is also safe after an exception.
        unsafe {
            let mem = manifold_alloc_rect();
            let bounds = guard(|| {
                let rect = manifold_cross_section_bounds(mem, self.0);
                let (min, max) = (manifold_rect_min(rect), manifold_rect_max(rect));
                [[min.x, min.y], [max.x, max.y]]
            });
            manifold_delete_rect(mem);
            bounds.unwrap_or([[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]])
        }
    }

    /// The closed outlines, each as a list of corners. An outer outline turns counterclockwise,
    /// and a hole turns clockwise.
    pub fn outlines(&self) -> Vec<Vec<[f64; 2]>> {
        self.with_polygons(|polygons| {
            // SAFETY: the polygons are valid during the call, and each index is below its length.
            unsafe {
                let corner = |i, j| {
                    let p = manifold_polygons_get_point(polygons, i, j);
                    [p.x, p.y]
                };
                let outline = |i| (0..manifold_polygons_simple_length(polygons, i)).map(|j| corner(i, j)).collect();
                (0..manifold_polygons_length(polygons)).map(outline).collect()
            }
        })
    }

    /// Calls `use_polygons` with the outlines of the sketch as kernel Polygons.
    pub(crate) fn with_polygons<R>(&self, use_polygons: impl FnOnce(*mut c_void) -> R) -> R {
        // SAFETY: `mem` is new Polygons memory that the constructor fills, or that gets no polygons
        // after an exception. `use_polygons` only reads it, and it is deleted after the call.
        unsafe {
            let mem = manifold_alloc_polygons();
            let polygons = guard(|| manifold_cross_section_to_polygons(mem, self.0)).unwrap_or_else(|_| manifold_polygons(mem, null(), 0));
            let result = use_polygons(polygons);
            manifold_delete_polygons(polygons);
            result
        }
    }
}

/// The triangles that fill the closed lines through `rings`, as indices into the rings one after
/// the other. They turn counterclockwise when the outer rings turn counterclockwise and the holes
/// turn clockwise.
pub(crate) fn triangulate(rings: &[Vec<[f64; 2]>]) -> Vec<[u32; 3]> {
    let points: Vec<Vec<Vec2>> = rings.iter().map(|ring| ring.iter().map(|&[x, y]| Vec2 { x, y }).collect()).collect();
    // SAFETY: each constructor fills new memory of its type. The polygons copy the simple
    // polygons, the buffer has the length that the kernel gives, and all is deleted after the copy.
    // The tolerance -1 lets the kernel choose. After an exception, the triangulation memory has no
    // vector, so it is not deleted.
    let corners = unsafe {
        let simple: Vec<*mut c_void> =
            points.iter().map(|p| manifold_simple_polygon(manifold_alloc_simple_polygon(), p.as_ptr(), p.len())).collect();
        let polygons = manifold_polygons(manifold_alloc_polygons(), simple.as_ptr(), simple.len());
        let mem = manifold_alloc_triangulation();
        let mut corners: Vec<c_int> = Vec::new();
        if let Ok(triangles) = guard(|| manifold_triangulate(mem, polygons, -1.0)) {
            corners = vec![0; manifold_triangulation_num_tri(triangles) * 3];
            manifold_triangulation_tri_verts(corners.as_mut_ptr(), triangles);
            manifold_delete_triangulation(triangles);
        }
        manifold_delete_polygons(polygons);
        simple.into_iter().for_each(|p| manifold_delete_simple_polygon(p));
        corners
    };
    corners.as_chunks::<3>().0.iter().map(|t| t.map(|i| i as u32)).collect()
}

impl Clone for Sketch {
    fn clone(&self) -> Sketch {
        // SAFETY: `mem` is new CrossSection memory, and the input is a valid sketch.
        self.keep_error(Sketch::make(|mem| unsafe { manifold_cross_section_copy(mem, self.0) }))
    }
}

impl Drop for Sketch {
    fn drop(&mut self) {
        // SAFETY: the sketch owns its memory, from `manifold_alloc_cross_section`, and drops one time.
        unsafe { manifold_delete_cross_section(self.0) }
    }
}

impl Default for Sketch {
    fn default() -> Sketch {
        // SAFETY: `mem` is new CrossSection memory.
        Sketch::make(|mem| unsafe { manifold_cross_section_empty(mem) })
    }
}

/// Shows "4 corners, area 2, bounds -1 -0.5 to 1 0.5", "empty" or the error.
impl fmt::Debug for Sketch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use crate::mesh::Short;
        if let Some(error) = self.error() {
            return write!(f, "error: {error}");
        }
        let corners = self.corners();
        if corners == 0 {
            return f.write_str("empty");
        }
        let [[x0, y0], [x1, y1]] = self.bounds();
        write!(f, "{corners} corners, area {}, bounds {} {} to {} {}", Short(self.area()), Short(x0), Short(y0), Short(x1), Short(y1))
    }
}
