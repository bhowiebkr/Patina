// The test scene of the viewport prototype. Plain WGSL is valid WESL (spec 10.1).
// A compute pass moves a grid of cubes. Each viewport draws the same buffer with its camera.

struct Globals { time: f32, grid: u32, pad0: u32, pad1: u32 }
struct Camera { yaw: f32, pitch: f32, dist: f32, aspect: f32 }

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read_write> cells: array<vec4<f32>>;

@group(0) @binding(2) var<uniform> camera: Camera;
@group(0) @binding(3) var<storage, read> cells_in: array<vec4<f32>>;

@compute @workgroup_size(64)
fn animate(@builtin(global_invocation_id) id: vec3<u32>) {
    let n = globals.grid * globals.grid;
    if (id.x >= n) {
        return;
    }
    let half = f32(globals.grid) * 0.5;
    let gx = f32(id.x % globals.grid) - half;
    let gz = f32(id.x / globals.grid) - half;
    let h = sin(gx * 0.25 + globals.time * 2.0) * cos(gz * 0.25 + globals.time * 1.3) * 3.0;
    cells[id.x] = vec4<f32>(gx * 1.5, h, gz * 1.5, h * 0.17 + 0.5);
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) shade: f32,
}

@vertex
fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> VsOut {
    // A cube as one triangle strip of 14 vertices.
    let b = 1u << v;
    let corner = vec3<f32>(f32((0x287au & b) != 0u), f32((0x02afu & b) != 0u), f32((0x31e3u & b) != 0u));
    let cell = cells_in[i];
    let world = cell.xyz + (corner - vec3<f32>(0.5)) * 1.2;

    let cp = cos(camera.pitch);
    let eye = camera.dist * vec3<f32>(cp * sin(camera.yaw), sin(camera.pitch), cp * cos(camera.yaw));
    let fwd = normalize(-eye);
    let right = normalize(cross(fwd, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, fwd);
    let d = world - eye;
    let vz = dot(d, fwd);
    let f = 1.0 / tan(0.45);
    let near = 0.5;
    let far = 2000.0;

    var out: VsOut;
    out.pos = vec4<f32>(dot(d, right) * f / camera.aspect, dot(d, up) * f, (vz - near) * far / (far - near), vz);
    out.world = world;
    out.shade = cell.w;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(cross(dpdx(in.world), dpdy(in.world)));
    let light = 0.35 + 0.65 * abs(dot(n, normalize(vec3<f32>(0.4, 0.8, 0.3))));
    let base = mix(vec3<f32>(0.85, 0.45, 0.15), vec3<f32>(0.2, 0.6, 0.9), clamp(in.shade, 0.0, 1.0));
    return vec4<f32>(base * light, 1.0);
}
