// The surface under the pointer: draws the posed asset into a 1x1 target whose single pixel is
// the pointer, keeping each fragment's rest-pose position and normal (the mesh maps' space) so
// a brush dab can be matched against the baked maps.

struct Frame {
    // Projection shifted so the pointer lands on the pixel centre.
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
}

struct Part {
    model: mat4x4<f32>,
    color: vec4<f32>,
    // w: visible.
    pbr: vec4<f32>,
    highlight: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<storage, read> parts: array<Part>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) rest: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world: vec3<f32>,
    @location(3) @interpolate(flat) part: u32,
}

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(4) part: u32,
) -> VsOut {
    let p = parts[part];
    let world = p.model * vec4<f32>(pos, 1.0);
    var o: VsOut;
    o.clip = frame.view_proj * world;
    if p.pbr.w < 0.5 {
        o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    }
    o.rest = pos;
    o.normal = nrm;
    o.world = world.xyz;
    o.part = part;
    return o;
}

struct FsOut {
    // Rest position, distance from the eye.
    @location(0) pos: vec4<f32>,
    // Rest normal, part index.
    @location(1) nrm: vec4<f32>,
}

@fragment
fn fs(i: VsOut) -> FsOut {
    var o: FsOut;
    o.pos = vec4<f32>(i.rest, distance(i.world, frame.eye.xyz));
    o.nrm = vec4<f32>(normalize(i.normal), f32(i.part));
    return o;
}
