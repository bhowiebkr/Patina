// Rasterises the mesh in UV space: each texel gets the surface point that maps to it.

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) curv: f32,
    @location(3) @interpolate(flat) part: u32,
}

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) curv: f32,
    @location(4) part: u32,
) -> VsOut {
    var o: VsOut;
    o.clip = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.5, 1.0);
    o.pos = pos;
    o.nrm = nrm;
    o.curv = curv;
    o.part = part;
    return o;
}

struct FsOut {
    @location(0) pos: vec4<f32>,
    @location(1) nrm: vec4<f32>,
    @location(2) part: vec4<f32>,
}

@fragment
fn fs(in: VsOut) -> FsOut {
    var o: FsOut;
    o.pos = vec4<f32>(in.pos, 1.0);
    o.nrm = vec4<f32>(normalize(in.nrm), in.curv);
    o.part = vec4<f32>(f32(in.part), 0.0, 0.0, 1.0);
    return o;
}
