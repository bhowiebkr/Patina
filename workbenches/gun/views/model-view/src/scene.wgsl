// The faces, the grid and the wireframe. A vertex with no normal is a line and has no light.
// Each instance moves the mesh with the columns of its frame. `light.w` is 1 for toon shading,
// and `pixel.xy` is the size of one pixel in clip units.
struct Scene { matrix: mat4x4f, light: vec4f, pixel: vec4f }
@group(0) @binding(0) var<uniform> scene: Scene;

struct In {
    @location(0) position: vec3f, @location(1) normal: vec3f, @location(2) color: vec3f,
    @location(3) x: vec3f, @location(4) y: vec3f, @location(5) z: vec3f, @location(6) origin: vec3f,
}
struct Out { @builtin(position) clip: vec4f, @location(0) normal: vec3f, @location(1) color: vec3f }

@vertex fn vs(v: In) -> Out { return place(v); }

fn place(v: In) -> Out {
    let p = v.x * v.position.x + v.y * v.position.y + v.z * v.position.z + v.origin;
    let n = v.x * v.normal.x + v.y * v.normal.y + v.z * v.normal.z;
    // A scaled instance keeps a face normal of length 1, so the fragment does not see a line.
    let unit = select(vec3f(0.0), normalize(n), dot(n, n) > 1e-12);
    return Out(scene.matrix * vec4f(p, 1.0), unit, v.color);
}

// The back faces, moved out on the screen along their normals: the outline of the toon look.
@vertex fn vs_outline(v: In) -> Out {
    var out = place(v);
    let side = (scene.matrix * vec4f(out.normal, 0.0)).xy;
    if dot(side, side) > 1e-12 {
        out.clip = vec4f(out.clip.xy + normalize(side) * scene.pixel.xy * 3.0 * out.clip.w, out.clip.zw);
    }
    return out;
}

@fragment fn fs(v: Out) -> @location(0) vec4f {
    if dot(v.normal, v.normal) < 0.25 {
        return vec4f(v.color, 1.0);
    }
    let n = normalize(v.normal);
    // A sky light from above and a key light from the camera.
    let sky = mix(0.12, 0.3, n.z * 0.5 + 0.5);
    let key = max(dot(n, scene.light.xyz), 0.0) * 0.8;
    let light = sky + key;
    // The toon look has three light bands.
    let band = select(select(0.3, 0.65, light > 0.4), 1.0, light > 0.8);
    return vec4f(v.color * select(light, band, scene.light.w > 0.5), 1.0);
}

@fragment fn fs_outline(v: Out) -> @location(0) vec4f {
    return vec4f(0.008, 0.007, 0.006, 1.0);
}
