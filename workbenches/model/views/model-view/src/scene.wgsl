// The mesh, the grid and the wireframe. A vertex with no normal is a line and has no light.
struct Scene { matrix: mat4x4f, light: vec4f }
@group(0) @binding(0) var<uniform> scene: Scene;

struct In { @location(0) position: vec3f, @location(1) normal: vec3f, @location(2) color: vec3f }
struct Out { @builtin(position) clip: vec4f, @location(0) normal: vec3f, @location(1) color: vec3f }

@vertex fn vs(v: In) -> Out {
    return Out(scene.matrix * vec4f(v.position, 1.0), v.normal, v.color);
}

@fragment fn fs(v: Out) -> @location(0) vec4f {
    if dot(v.normal, v.normal) < 0.25 {
        return vec4f(v.color, 1.0);
    }
    let n = normalize(v.normal);
    // A sky light from above and a key light from the camera.
    let sky = mix(0.12, 0.3, n.z * 0.5 + 0.5);
    let key = max(dot(n, scene.light.xyz), 0.0) * 0.8;
    return vec4f(v.color * (sky + key), 1.0);
}
