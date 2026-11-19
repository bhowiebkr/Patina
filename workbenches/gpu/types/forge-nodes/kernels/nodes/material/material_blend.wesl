//! name: Material Blend
//! category: Material
//! description: Lays the foreground material over the background through the mask.
//! input: background Material "Background" primary
//! input: foreground Material "Foreground"
//! input: mask Gray "Mask" default=1
//! output: material Material
//! param: opacity float(0,1) = 1 "Opacity"

fn eval(p: vec2<i32>) -> Out {
    let t = clamp(gray_mask(p) * p_opacity(), 0.0, 1.0);
    var m = material_mix(material_background(p), material_foreground(p), t);
    // Opposed normals cancel; fall back to flat rather than NaN.
    let n = m.normal * 2.0 - 1.0;
    m.normal = select(vec3<f32>(0.0, 0.0, 1.0), normalize(n), dot(n, n) > 1e-8) * 0.5 + 0.5;
    return Out(m);
}
