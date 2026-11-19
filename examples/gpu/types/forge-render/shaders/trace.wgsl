// Ray tracing against the BVH from bvh.rs, shared by the occlusion and curvature bakers (Rust
// prepends this file to theirs), and the surface point under a texel of surface.wgsl's map.

struct Node {
    lo: vec3<f32>,
    start: u32,
    hi: vec3<f32>,
    count: u32,
}

// v0.w: part index.
struct Tri {
    v0: vec4<f32>,
    e1: vec4<f32>,
    e2: vec4<f32>,
}

// `tri` is NONE and `t` the ray's end on a miss; `uv` are the hit's barycentrics on e1, e2.
struct Hit {
    t: f32,
    tri: u32,
    uv: vec2<f32>,
}

@group(0) @binding(1) var<storage, read> nodes: array<Node>;
@group(0) @binding(2) var<storage, read> tris: array<Tri>;
// Each triangle's three vertex normals, in `tris` order.
@group(0) @binding(3) var<storage, read> tri_normals: array<array<vec4<f32>, 3>>;
// Per node: the part all its triangles are on, NONE when they span several (`Bvh::node_parts`).
@group(0) @binding(9) var<storage, read> node_parts: array<u32>;

const MISS: f32 = 1e30;
const NONE: u32 = 0xffffffffu;

// Entry distance of the ray into the box, or MISS when it misses or starts past `tmax`.
fn box_t(n: Node, o: vec3<f32>, inv: vec3<f32>, tmax: f32) -> f32 {
    let t0 = (n.lo - o) * inv;
    let t1 = (n.hi - o) * inv;
    let near = min(t0, t1);
    let far = max(t0, t1);
    let enter = max(max(near.x, near.y), max(near.z, 0.0));
    let exit = min(min(far.x, far.y), min(far.z, tmax));
    return select(MISS, enter, enter <= exit);
}

// Moller-Trumbore, both faces: (t, u, v), t = -1 on a miss.
fn tri_t(tri: Tri, o: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let p = cross(d, tri.e2.xyz);
    let det = dot(tri.e1.xyz, p);
    if abs(det) < 1e-20 {
        return vec3<f32>(-1.0);
    }
    let inv = 1.0 / det;
    let s = o - tri.v0.xyz;
    let u = dot(s, p) * inv;
    if u < 0.0 || u > 1.0 {
        return vec3<f32>(-1.0);
    }
    let q = cross(s, tri.e1.xyz);
    let v = dot(d, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return vec3<f32>(-1.0);
    }
    return vec3<f32>(dot(tri.e2.xyz, q) * inv, u, v);
}

// The nearest hit with tmin < t < tmax on any triangle but `skip`, the one the ray starts on
// (NONE for none), as Cycles skips a ray's own primitive (`ray.self.prim`). With `exits`, only
// triangles the ray leaves the solid through count: their outward normal faces along the ray.
fn trace(o: vec3<f32>, d: vec3<f32>, tmin: f32, tmax: f32, skip: u32, exits: bool) -> Hit {
    return trace_group(o, d, tmin, tmax, skip, exits, NONE);
}

fn trace_group(o: vec3<f32>, d: vec3<f32>, tmin: f32, tmax: f32, skip: u32, exits: bool,
    ao_group: u32) -> Hit {
    let inv = 1.0 / select(d, vec3<f32>(1e-9), abs(d) < vec3<f32>(1e-9));
    var best = Hit(tmax, NONE, vec2<f32>(0.0));
    if box_t(nodes[0], o, inv, best.t) == MISS {
        return best;
    }
    var stack: array<u32, 64>;
    var near: array<f32, 64>;
    var sp = 0u;
    var node = 0u;
    loop {
        let n = nodes[node];
        if n.count == 0u {
            let ta = box_t(nodes[n.start], o, inv, best.t);
            let tb = box_t(nodes[n.start + 1u], o, inv, best.t);
            let a_first = ta <= tb;
            if max(ta, tb) < MISS && sp < 64u {
                stack[sp] = select(n.start, n.start + 1u, a_first);
                near[sp] = max(ta, tb);
                sp += 1u;
            }
            if min(ta, tb) < MISS {
                node = select(n.start + 1u, n.start, a_first);
                continue;
            }
        } else {
            for (var i = n.start; i < n.start + n.count; i += 1u) {
                if i == skip || (ao_group != NONE && u32(tris[i].e1.w) != ao_group) {
                    continue;
                }
                let h = tri_t(tris[i], o, d);
                if h.x > tmin && h.x < best.t && (!exits || dot(d, outward(i)) > 0.0) {
                    best = Hit(h.x, i, h.yz);
                }
            }
        }
        // Pop the next box that still starts before the nearest hit.
        loop {
            if sp == 0u {
                return best;
            }
            sp -= 1u;
            if near[sp] < best.t {
                node = stack[sp];
                break;
            }
        }
    }
    return best;
}

// The squared distance from `p` to the box, 0 inside it.
fn box_distance2(n: Node, p: vec3<f32>) -> f32 {
    let d = max(max(n.lo - p, p - n.hi), vec3<f32>(0.0));
    return dot(d, d);
}

// The squared distance from `p` to the triangle, by the Voronoi region `p` is in (Ericson, Real-Time
// Collision Detection, 5.1.5).
fn tri_distance2(tri: Tri, p: vec3<f32>) -> f32 {
    let ab = tri.e1.xyz;
    let ac = tri.e2.xyz;
    let ap = p - tri.v0.xyz;
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return dot(ap, ap);
    }
    let bp = ap - ab;
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return dot(bp, bp);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let q = ap - ab * (d1 / (d1 - d3));
        return dot(q, q);
    }
    let cp = ap - ac;
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return dot(cp, cp);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let q = ap - ac * (d2 / (d2 - d6));
        return dot(q, q);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 >= d3 && d5 >= d6 {
        let q = bp - (ac - ab) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
        return dot(q, q);
    }
    let inv = 1.0 / max(va + vb + vc, 1e-30);
    let q = ap - ab * (vb * inv) - ac * (vc * inv);
    return dot(q, q);
}

// The distance from `p` to the nearest triangle not on `part`, at most `reach`: nearest box first,
// skipping subtrees wholly on `part` and any box past the best so far.
fn distance_to_other_part(p: vec3<f32>, part: u32, reach: f32) -> f32 {
    var best = reach * reach;
    if node_parts[0] == part {
        return reach;
    }
    var stack: array<u32, 64>;
    var near: array<f32, 64>;
    var sp = 0u;
    var node = 0u;
    loop {
        let n = nodes[node];
        if n.count == 0u {
            let da = select(box_distance2(nodes[n.start], p), MISS, node_parts[n.start] == part);
            let db = select(box_distance2(nodes[n.start + 1u], p), MISS,
                node_parts[n.start + 1u] == part);
            let a_first = da <= db;
            if max(da, db) < best && sp < 64u {
                stack[sp] = select(n.start, n.start + 1u, a_first);
                near[sp] = max(da, db);
                sp += 1u;
            }
            if min(da, db) < best {
                node = select(n.start + 1u, n.start, a_first);
                continue;
            }
        } else {
            for (var i = n.start; i < n.start + n.count; i += 1u) {
                if u32(tris[i].v0.w) != part {
                    let d = tri_distance2(tris[i], p);
                    if d < best {
                        best = d;
                    }
                }
            }
        }
        // Pop the next box still nearer than the best.
        loop {
            if sp == 0u {
                return sqrt(best);
            }
            sp -= 1u;
            if near[sp] < best {
                node = stack[sp];
                break;
            }
        }
    }
    return sqrt(best);
}

// Triangle `i`'s outward normal, unnormalised: its vertex normals' sum, or the winding's normal
// when they cancel.
fn outward(i: u32) -> vec3<f32> {
    let tn = tri_normals[i];
    let s = tn[0].xyz + tn[1].xyz + tn[2].xyz;
    return select(cross(tris[i].e1.xyz, tris[i].e2.xyz), s, dot(s, s) > 1e-12);
}

// The surface under a texel of surface.wgsl's map: the point on the triangle, the interpolated
// unit vertex normal, the triangle, and the texel's weight (1 when the triangle covers the
// sample point, a trace when the texel was dilated from a neighbour, 0 when empty).
struct Surface {
    p: vec3<f32>,
    n: vec3<f32>,
    slot: u32,
    bary: vec2<f32>,
    weight: f32,
}

fn surface_at(s: vec4<f32>) -> Surface {
    var out: Surface;
    out.weight = select(select(0.0, 1e-3, s.w > 0.25), 1.0, s.w > 0.75);
    if out.weight == 0.0 {
        return out;
    }
    let slot = u32(s.x + 0.5);
    let tri = tris[slot];
    let tn = tri_normals[slot];
    let b = s.yz;
    out.p = tri.v0.xyz + b.x * tri.e1.xyz + b.y * tri.e2.xyz;
    let n = (1.0 - b.x - b.y) * tn[0].xyz + b.x * tn[1].xyz + b.y * tn[2].xyz;
    out.n = normalize(select(outward(slot), n, dot(n, n) > 1e-12));
    out.slot = slot;
    out.bary = b;
    return out;
}

// The ray origin for a smooth-shaded triangle: the flat point lifted onto the surface its vertex
// normals describe (Hanika 2021, "Hacking the Shadow Terminator", Ray Tracing Gems II ch. 4,
// listing 4-1), so rays about the shading normal leave a coarse curved mesh without facets.
// Outward it lifts convex facets; `inward` mirrors it (max for min) for rays into the solid.
fn terminator(s: Surface, inward: bool) -> vec3<f32> {
    let tri = tris[s.slot];
    let tn = tri_normals[s.slot];
    let corners = array<vec3<f32>, 3>(tri.v0.xyz, tri.v0.xyz + tri.e1.xyz, tri.v0.xyz + tri.e2.xyz);
    let w = vec3<f32>(1.0 - s.bary.x - s.bary.y, s.bary.x, s.bary.y);
    var p = s.p;
    for (var k = 0; k < 3; k += 1) {
        let len = length(tn[k].xyz);
        if len < 1e-6 {
            continue;
        }
        let n = tn[k].xyz / len;
        let d = dot(s.p - corners[k], n);
        p -= w[k] * select(min(d, 0.0), max(d, 0.0), inward) * n;
    }
    return p;
}

fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

// Per-texel rotation of the sample sequence.
fn jitter(p: vec2<u32>) -> vec2<f32> {
    let a = pcg(p.x + pcg(p.y));
    let b = pcg(a);
    return vec2<f32>(f32(a >> 8u), f32(b >> 8u)) / 16777216.0;
}

// Sample `i` of the R2 low-discrepancy sequence, rotated by `shift`.
fn r2(i: u32, shift: vec2<f32>) -> vec2<f32> {
    return fract(shift + f32(i) * vec2<f32>(0.7548776662, 0.5698402910));
}

// Sample `i` as a cosine-weighted direction about +z.
fn cosine_dir(i: u32, shift: vec2<f32>) -> vec3<f32> {
    let u = r2(i, shift);
    let phi = 6.2831853 * u.x;
    let r = sqrt(u.y);
    return vec3<f32>(r * cos(phi), r * sin(phi), sqrt(max(1.0 - u.y, 0.0)));
}

// An orthonormal frame with `n` as its third column (Duff et al. 2017).
fn basis(n: vec3<f32>) -> mat3x3<f32> {
    let s = select(-1.0, 1.0, n.z >= 0.0);
    let a = -1.0 / (s + n.z);
    let b = n.x * n.y * a;
    let t = vec3<f32>(1.0 + s * n.x * n.x * a, s * b, -s * n.x);
    let bt = vec3<f32>(b, s + n.y * n.y * a, -n.y);
    return mat3x3<f32>(t, bt, n);
}
