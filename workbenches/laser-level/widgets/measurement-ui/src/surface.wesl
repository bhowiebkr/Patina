struct Scene { matrix: mat4x4f, light: vec4f }
@group(0) @binding(0) var<uniform> scene: Scene;
struct In { @location(0) position: vec3f, @location(1) normal: vec3f, @location(2) color: vec3f }
struct Out { @builtin(position) clip: vec4f, @location(0) normal: vec3f, @location(1) color: vec3f }
@vertex fn vs(v: In) -> Out {
    return Out(scene.matrix * vec4f(v.position, 1.0), v.normal, v.color);
}
fn linear(c: vec3f) -> vec3f {
    return select(pow((c + 0.055) / 1.055, vec3f(2.4)), c / 12.92, c <= vec3f(0.04045));
}
@fragment fn fs(v: Out) -> @location(0) vec4f {
    var shade = 1.0;
    if dot(v.normal, v.normal) > 0.25 {
        shade = 0.55 + 0.45 * abs(dot(normalize(v.normal), scene.light.xyz));
    }
    return vec4f(linear(v.color * shade), 1.0);
}
