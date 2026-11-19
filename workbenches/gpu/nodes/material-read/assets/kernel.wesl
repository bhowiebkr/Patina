//! name: Read
//! category: Core
//! description: One channel of a material so far, as a mask. Reading height lets the next coat settle
//! into low parts or wear from high parts of what is already there, through a Mask's breakup.
//! input: material Material "Material" primary
//! output: output Gray "Value"
//! param: channel enum(Height|Roughness|Metallic|Brightness) = height "Channel"

fn eval(p: vec2<i32>) -> Out {
    let m = material_material(p);
    var value: f32;
    switch p_channel() {
        case CHANNEL_ROUGHNESS: { value = m.roughness; }
        case CHANNEL_METALLIC: { value = m.metallic; }
        case CHANNEL_BRIGHTNESS: { value = luminance(m.basecolor.rgb); }
        default: { value = m.height; }
    }
    return Out(clamp(value, 0.0, 1.0));
}
