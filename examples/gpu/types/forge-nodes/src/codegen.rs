//! Turns a definition into WGSL: typed accessors for its params and inputs, the `Out` struct
//! its `eval` returns, and the `main` entry point that stores `Out` into the output layers.
//! `link` places it after the kernel file and the node's code text.

use std::fmt::Write;

use crate::def::{Def, ParamType, Socket, const_name};

/// A WGSL float literal.
fn lit(v: f32) -> String {
    if v.is_finite() {
        format!("{v:?}")
    } else {
        "0.0".into()
    }
}

fn vec4(v: [f32; 4]) -> String {
    format!(
        "vec4<f32>({}, {}, {}, {})",
        lit(v[0]),
        lit(v[1]),
        lit(v[2]),
        lit(v[3])
    )
}

pub fn generate(def: &Def) -> String {
    let mut s = String::from("// ---- generated ----\n");
    for p in &def.params {
        let o = p.offset;
        let id = &p.id;
        let v = |i: usize| format!("values[{}u]", o + i);
        let _ = match &p.ty {
            ParamType::Float { .. } => writeln!(s, "fn p_{id}() -> f32 {{ return {}; }}", v(0)),
            ParamType::Int { .. } => {
                writeln!(s, "fn p_{id}() -> i32 {{ return i32(round({})); }}", v(0))
            }
            ParamType::Bool => writeln!(s, "fn p_{id}() -> bool {{ return {} > 0.5; }}", v(0)),
            ParamType::Enum(options) => {
                // Dynamic lists (`@parts`) have no fixed options to name.
                for (i, option) in options
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| !o.starts_with('@'))
                {
                    let _ = writeln!(
                        s,
                        "const {}_{}: i32 = {i};",
                        const_name(id),
                        const_name(option)
                    );
                }
                writeln!(s, "fn p_{id}() -> i32 {{ return i32(round({})); }}", v(0))
            }
            ParamType::Vector { n, .. } => {
                let parts: Vec<String> = (0..*n).map(v).collect();
                writeln!(
                    s,
                    "fn p_{id}() -> vec{n}<f32> {{ return vec{n}<f32>({}); }}",
                    parts.join(", ")
                )
            }
            ParamType::Color => {
                let parts: Vec<String> = (0..4).map(v).collect();
                writeln!(
                    s,
                    "fn p_{id}() -> vec4<f32> {{ return vec4<f32>({}); }}",
                    parts.join(", ")
                )
            }
            ParamType::Gradient => writeln!(
                s,
                "fn gradient_{id}(t: f32, ease: bool) -> vec4<f32> {{ return gradient_eval({o}u, t, ease); }}\nfn gradient_step_{id}(t: f32) -> vec4<f32> {{ return gradient_step({o}u, t); }}"
            ),
            ParamType::GradientControls => writeln!(
                s,
                "fn gradient_{id}(t: f32, ease: bool) -> vec4<f32> {{ return gradient_eval_stride({o}u, t, ease, 6u); }}\nfn gradient_step_{id}(t: f32) -> vec4<f32> {{ return gradient_step_stride({o}u, t, 6u); }}\nfn gradient_flat_{id}(t: f32) -> vec4<f32> {{ return gradient_flat({o}u, t); }}\nfn gradient_smooth_{id}(s: f32) {{ gradient_smooth_build({o}u, s); }}"
            ),
            ParamType::Curve | ParamType::CurvePoints { .. } => {
                writeln!(
                    s,
                    "const CURVE_{}_OFFSET: u32 = {o}u;\nfn curve_{id}(x: f32) -> f32 {{ return curve_eval({o}u, x); }}",
                    const_name(id)
                )
            }
            // Strings live on the host: file paths and text are rasterised there, code is
            // linked in by `Engine::compile`.
            ParamType::Text(_) => Ok(()),
        };
    }
    for (i, input) in def.inputs.iter().enumerate() {
        let id = &input.id;
        let mut d = vec4(input.default);
        let mut g = lit(input.default[0]);
        if let Some(legacy) = input.legacy_default {
            d = format!(
                "select({}, {d}, (info.domain.w & {}u) != 0u)",
                vec4(legacy),
                crate::gpu::UPDATED_INPUT_DEFAULTS
            );
            g = format!(
                "select({}, {g}, (info.domain.w & {}u) != 0u)",
                lit(legacy[0]),
                crate::gpu::UPDATED_INPUT_DEFAULTS
            );
        }
        let _ = writeln!(
            s,
            "fn has_{id}() -> bool {{ return info.inputs[{i}].x != 0u; }}"
        );
        if input.socket == Socket::Material {
            let _ = writeln!(
                s,
                "fn material_{id}(p: vec2<i32>) -> Material {{ if !has_{id}() {{ return material_default(); }} return material_fetch(input{i}, {i}u, p); }}"
            );
            let _ = writeln!(
                s,
                "fn sample_material_{id}(uv: vec2<f32>) -> Material {{ if !has_{id}() {{ return material_default(); }} return material_sample(input{i}, {i}u, uv); }}"
            );
            continue;
        }
        let _ = writeln!(
            s,
            "fn in_{id}(p: vec2<i32>) -> vec4<f32> {{ if !has_{id}() {{ return {d}; }} return fetch(input{i}, {i}u, p, 0u); }}"
        );
        let _ = writeln!(
            s,
            "fn gray_{id}(p: vec2<i32>) -> f32 {{ if !has_{id}() {{ return {g}; }} return to_gray(fetch(input{i}, {i}u, p, 0u), {i}u); }}"
        );
        let _ = writeln!(
            s,
            "fn sample_{id}(uv: vec2<f32>) -> vec4<f32> {{ if !has_{id}() {{ return {d}; }} return textureSampleLevel(input{i}, samp, uv, i32(info.inputs[{i}].y), 0.0); }}"
        );
        let _ = writeln!(
            s,
            "fn size_{id}() -> vec2<i32> {{ return vec2<i32>(textureDimensions(input{i})); }}"
        );
        let _ = writeln!(
            s,
            "fn sample_gray_{id}(uv: vec2<f32>) -> f32 {{ if !has_{id}() {{ return {g}; }} return to_gray(sample_{id}(uv), {i}u); }}"
        );
    }
    s.push_str("struct Out {\n");
    for o in &def.outputs {
        let ty = match o.socket {
            Socket::Gray => "f32",
            Socket::Color => "vec4<f32>",
            Socket::Material => "Material",
        };
        let _ = writeln!(s, "    out_{}: {ty},", o.id);
    }
    s.push_str("}\n");
    s.push_str(
        "@compute @workgroup_size(8, 8)\nfn main(@builtin(global_invocation_id) id: vec3<u32>) {\n    \
         if id.x >= info.size.x || id.y >= info.size.y {\n        return;\n    }\n    \
         let p = vec2<i32>(id.xy);\n    let o = eval(p);\n    if info.size.z + 1u == info.size.w {\n",
    );
    for o in &def.outputs {
        let store = match o.socket {
            Socket::Gray => "store_gray",
            Socket::Color => "store_color",
            Socket::Material => "store_material",
        };
        let _ = writeln!(s, "        {store}(p, {}u, o.out_{});", o.layer, o.id);
    }
    s.push_str("    }\n}\n");
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn generates_accessors() {
        let src = "//! input: input Color default=0.5 primary\n//! output: output Color\n\
                   //! param: mode enum(Copy|Add Sub)\n//! param: map gradient\n";
        let def = Def::parse("nodes/a/x.wesl", src).unwrap();
        let code = generate(&def);
        assert!(code.contains("const MODE_ADD_SUB: i32 = 1;"));
        assert!(code.contains(
            "fn gradient_map(t: f32, ease: bool) -> vec4<f32> { return gradient_eval(1u, t, ease); }"
        ));
        assert!(code.contains("return vec4<f32>(0.5, 0.5, 0.5, 1.0);"));
        assert!(code.contains("store_color(p, 0u, o.out_output);"));
    }
}
