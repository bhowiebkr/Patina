//! `Sketch`: a flat shape on the XY plane, from the cross-section of the kernel.

use std::ffi::{c_int, c_void};
use std::fmt;

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

unsafe extern "C" {
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
    fn manifold_cross_section_translate(mem: *mut RawSketch, cs: *mut RawSketch, x: f64, y: f64) -> *mut RawSketch;
    fn manifold_cross_section_to_polygons(mem: *mut c_void, cs: *mut RawSketch) -> *mut c_void;
    fn manifold_cross_section_area(cs: *mut RawSketch) -> f64;
    fn manifold_cross_section_num_vert(cs: *mut RawSketch) -> usize;
    fn manifold_cross_section_bounds(mem: *mut c_void, cs: *mut RawSketch) -> *mut c_void;
    fn manifold_rect_min(r: *mut c_void) -> Vec2;
    fn manifold_rect_max(r: *mut c_void) -> Vec2;
}

/// A flat shape on the XY plane, with holes. `Solid::extrude` and `Solid::revolve` make solids
/// from it.
pub struct Sketch(*mut RawSketch);

// SAFETY: a CrossSection value is immutable after its construction, as a Manifold (see `Solid`).
unsafe impl Send for Sketch {}
// SAFETY: see `Send`. The methods with `&self` only read the sketch.
unsafe impl Sync for Sketch {}

impl Sketch {
    /// Makes a sketch in new kernel memory. `make` constructs into the memory that it gets.
    fn make(make: impl FnOnce(*mut RawSketch) -> *mut RawSketch) -> Sketch {
        // SAFETY: the memory has the size and alignment of a CrossSection, and `make` constructs one in it.
        Sketch(make(unsafe { manifold_alloc_cross_section() }))
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
        // the polygon, and the polygon is deleted after the copy.
        Sketch::make(|mem| unsafe {
            let polygon = manifold_simple_polygon(manifold_alloc_simple_polygon(), points.as_ptr(), points.len());
            let sketch = manifold_cross_section_of_simple_polygon(mem, polygon, NON_ZERO);
            manifold_delete_simple_polygon(polygon);
            sketch
        })
    }

    /// Moves the sketch by `[x, y]`.
    pub fn translate(&self, [x, y]: [f64; 2]) -> Sketch {
        // SAFETY: `mem` is new CrossSection memory, and the input is a valid sketch.
        Sketch::make(|mem| unsafe { manifold_cross_section_translate(mem, self.0, x, y) })
    }

    /// The area. The area of a hole counts as negative.
    pub fn area(&self) -> f64 {
        // SAFETY: the sketch is valid.
        unsafe { manifold_cross_section_area(self.0) }
    }

    /// The number of corners on all the outlines.
    pub fn corners(&self) -> usize {
        // SAFETY: the sketch is valid.
        unsafe { manifold_cross_section_num_vert(self.0) }
    }

    /// The lowest and the highest corner of the bounds.
    pub fn bounds(&self) -> [[f64; 2]; 2] {
        // SAFETY: `mem` is new Rect memory that the constructor fills, and the rect is deleted after its read.
        unsafe {
            let rect = manifold_cross_section_bounds(manifold_alloc_rect(), self.0);
            let (min, max) = (manifold_rect_min(rect), manifold_rect_max(rect));
            manifold_delete_rect(rect);
            [[min.x, min.y], [max.x, max.y]]
        }
    }

    /// Calls `use_polygons` with the outlines of the sketch as kernel Polygons.
    pub(crate) fn with_polygons<R>(&self, use_polygons: impl FnOnce(*mut c_void) -> R) -> R {
        // SAFETY: `mem` is new Polygons memory that the constructor fills. `use_polygons` only
        // reads it, and it is deleted after the call.
        unsafe {
            let polygons = manifold_cross_section_to_polygons(manifold_alloc_polygons(), self.0);
            let result = use_polygons(polygons);
            manifold_delete_polygons(polygons);
            result
        }
    }
}

impl Clone for Sketch {
    fn clone(&self) -> Sketch {
        // SAFETY: `mem` is new CrossSection memory, and the input is a valid sketch.
        Sketch::make(|mem| unsafe { manifold_cross_section_copy(mem, self.0) })
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

/// Shows "4 corners, area 2, bounds -1 -0.5 to 1 0.5", or "empty".
impl fmt::Debug for Sketch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use crate::mesh::Short;
        let corners = self.corners();
        if corners == 0 {
            return f.write_str("empty");
        }
        let [[x0, y0], [x1, y1]] = self.bounds();
        write!(f, "{corners} corners, area {}, bounds {} {} to {} {}", Short(self.area()), Short(x0), Short(y0), Short(x1), Short(y1))
    }
}
