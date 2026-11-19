//! name: Imported texture
//! category: Material
//! description: Embedded original image, retaining its bit depth and authored resolution. Color is stored sRGB; linear color is encoded on the GPU. Raw data samples are unchanged.
//! output: output Color
//! param: encoding enum(Stored sRGB color|Linear color|Raw data) = 0 "Interpretation"
//! param: colormode enum(Grayscale|Color) = 1 "Color Mode"
//! param: path file = "" "Bitmap"
//! param: resizemethod enum(Smooth stretch|Nearest stretch) = 0 "Resize method"
//! domain: texture
//! host: bitmap

fn imported_encode_color(c: vec3<f32>) -> vec3<f32> {
    let high = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, c * 12.92, c <= vec3<f32>(0.0031308));
}

fn eval(p: vec2<i32>) -> Out {
    if !has_host() { return Out(vec4<f32>(0.0, 0.0, 0.0, 1.0)); }
    var c = host_smooth(uv(p), 1.0 / size_f());
    if p_resizemethod() == RESIZEMETHOD_NEAREST_STRETCH { c = host_nearest(uv(p)); }
    if p_encoding() == ENCODING_LINEAR_COLOR { c = vec4<f32>(imported_encode_color(c.rgb), c.a); }
    if p_colormode() == COLORMODE_GRAYSCALE { c = vec4<f32>(vec3<f32>(dot(c.rgb, LUMA)), 1.0); }
    return Out(c);
}
