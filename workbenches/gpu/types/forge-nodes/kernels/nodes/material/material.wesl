//! name: Material
//! category: Material
//! description: Packs the PBR channels into a material; an unlinked channel takes its value.
//! input: basecolor Color "Base color"
//! input: roughness Gray "Roughness"
//! input: metallic Gray "Metallic"
//! input: height Gray "Height"
//! input: ao Gray "Ambient occlusion" default=1
//! input: normal Color "Normal" default=(0.5,0.5,1,1)
//! input: opacity Gray "Opacity" default=1
//! output: material Material
//! param: basecolor_value color = (0.5,0.5,0.5,1) "Base color"
//! param: roughness_value float(0,1) = 0.5 "Roughness"
//! param: metallic_value float(0,1) = 0 "Metallic"
//! param: height_value float(0,1) = 0.5 "Height"

fn eval(p: vec2<i32>) -> Out {
    let base = select(p_basecolor_value(), in_basecolor(p), has_basecolor());
    return Out(Material(
        vec4<f32>(base.rgb, gray_opacity(p)),
        select(p_roughness_value(), gray_roughness(p), has_roughness()),
        select(p_metallic_value(), gray_metallic(p), has_metallic()),
        select(p_height_value(), gray_height(p), has_height()),
        gray_ao(p),
        in_normal(p).xyz,
    ));
}
