// The view map: the mesh from the fixed 3/4 preview camera, each pixel holding the UV it sees and
// a simple light, so mesh-domain node thumbnails can show the node's output on the model.

struct View {
    view_proj: mat4x4<f32>,
    sun: vec4<f32>,
}

@group(0) @binding(0) var<uniform> view: View;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) nrm: vec3<f32>,
}

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VsOut {
    var o: VsOut;
    o.clip = view.view_proj * vec4<f32>(pos, 1.0);
    o.uv = uv;
    o.nrm = nrm;
    return o;
}

@fragment
fn fs(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let n = normalize(select(-in.nrm, in.nrm, front));
    let sun = max(dot(n, view.sun.xyz), 0.0);
    let sky = n.y * 0.5 + 0.5;
    return vec4<f32>(in.uv, 0.3 + 0.55 * sun + 0.15 * sky, 1.0);
}
