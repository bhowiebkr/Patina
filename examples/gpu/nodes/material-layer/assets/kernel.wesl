//! name: Layer
//! category: Core
//! description: One physical coat over a material. A single mask places it, so colour, roughness,
//! metallic and height all change together and the coat reads as one material. Tone varies the coat
//! inside the mask. Height is carried up the stack, and Finish turns it into a normal at the end.
//! input: base Material "Base" primary
//! input: mask Gray "Mask" default=1
//! input: tone Gray "Tone" default=0
//! input: relief Gray "Relief" default=0.5
//! output: output Material "Material"
//! param: color gradient "Colour by tone"
//! param: rough_low float(0,1) = 0.5 "Roughness at tone 0"
//! param: rough_high float(0,1) = 0.5 "Roughness at tone 1"
//! param: metallic float(0,1) = 0 "Metallic"
//! param: thickness float(-1,1) = 0 "Thickness"
//! param: relief_amount float(0,1) = 0 "Relief"

fn eval(p: vec2<i32>) -> Out {
    var m = material_base(p);
    let t = gray_mask(p);
    let tone = gray_tone(p);
    m.basecolor = vec4<f32>(mix(m.basecolor.rgb, gradient_color(tone, false).rgb, t), m.basecolor.a);
    m.roughness = mix(m.roughness, mix(p_rough_low(), p_rough_high(), tone), t);
    m.metallic = mix(m.metallic, p_metallic(), t);
    m.height += t * (p_thickness() + p_relief_amount() * (gray_relief(p) - 0.5));
    return Out(m);
}
