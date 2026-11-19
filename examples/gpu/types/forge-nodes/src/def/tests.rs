use super::*;

#[test]
fn empty_curve_defaults_keep_the_legacy_identity_layout() {
    let header = "//! defaults: 4\n//! output: output Color\n//! param: curve curve = empty \"Curve\" legacy=(0,0,1,1)\n";
    let def = Def::parse("curve.wesl", header).unwrap();
    let param = &def.params[0];
    assert_eq!(param.default, [0.]);
    assert_eq!(param.pack(None)[0], 0.);
    assert_eq!(
        param.legacy_default,
        Some(curve_values(&[[0., 0.], [1., 1.]]))
    );
    assert!(Def::parse("curve.wesl", &header.replace("= empty", "= empty 0")).is_err());
}

#[test]
fn defaults_revision_is_positive_and_defaults_to_one_for_saved_sources() {
    let header = "//! output: output Gray\n";
    assert_eq!(Def::parse("test.wesl", header).unwrap().defaults_version, 1);
    assert_eq!(
        Def::parse("test.wesl", &format!("{header}//! defaults: 2"))
            .unwrap()
            .defaults_version,
        2
    );
    for value in ["0", "-1", "bad", "1.5", "4294967296"] {
        let err = Def::parse("test.wesl", &format!("{header}//! defaults: {value}")).unwrap_err();
        assert!(err.message.contains("defaults revision"));
    }
}

#[test]
fn input_defaults_can_advance_without_resetting_source_curve_parameters() {
    let source = "//! output: o Color\n//! input-defaults: 5\n//! defaults: 4";
    let def = Def::parse("curve.wesl", source).unwrap();
    assert_eq!(def.defaults_version, 4);
    assert_eq!(def.input_defaults_version, Some(5));
    assert_eq!(
        Def::parse("old.wesl", LEVELS)
            .unwrap()
            .input_defaults_version,
        None
    );
    for value in ["0", "-1", "bad", "1.5", "4294967296"] {
        assert!(
            Def::parse(
                "bad.wesl",
                &source.replace("input-defaults: 5", &format!("input-defaults: {value}"))
            )
            .is_err()
        );
    }
}

const LEVELS: &str = r#"//! name: Levels
//! category: Adjustment
//! description: Remaps the
//! input range.
//! input: input Color "Input" default=(0.5,0.5,0.5,1) primary
//! input: mask Gray default=1
//! output: output Color
//! param: levelinlow float(0,1) = 0 "In low"
//! param: mode enum(Copy|Add|Add Sub) = add_sub
//! param: offset float2(-1,1) = 0.25 0.5 "Offset"
//! param: tint color = 1 0.5 0
//! param: map gradient
//! param: shape curve = 0 0 0.5 0.8 1 1
//! param: steps int(1,8) = 3
//! passes: steps+1
//! stats
fn eval(p: vec2<i32>) -> Out { return Out(); }
"#;

#[test]
fn parses_header() {
    let d = Def::parse("nodes/adjust/levels.wesl", LEVELS).unwrap();
    assert_eq!(d.id, "levels");
    assert_eq!(d.category, "Adjustment");
    assert_eq!(d.description, "Remaps the input range.");
    assert_eq!(d.inputs.len(), 2);
    assert!(d.inputs[0].primary);
    assert_eq!(d.inputs[0].default, [0.5, 0.5, 0.5, 1.0]);
    assert_eq!(d.inputs[1].default, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(d.inputs[1].label, "Mask");
    assert_eq!(d.params[0].label, "In low");
    assert_eq!(d.params[1].default, vec![2.0]);
    assert_eq!(d.params[2].default, vec![0.25, 0.5]);
    assert_eq!(d.params[3].default, vec![1.0, 0.5, 0.0, 1.0]);
    assert_eq!(d.params[4].default[0], 2.0);
    assert_eq!(d.params[5].default[0], 3.0);
    assert_eq!(d.params[3].offset, 4);
    assert!(d.stats && d.multipass());
    let mut values = vec![0.0; d.slots()];
    assert_eq!(d.pass_count(1024, &values), 1);
    values[d.params[6].offset] = 3.0;
    assert_eq!(d.pass_count(1024, &values), 4);
    let scaled = Def::parse("s.wesl", "//! output: o Gray\n//! passes: 3*log2+2").unwrap();
    assert_eq!(scaled.pass_count(1024, &[]), 32);
    let sum = Def::parse(
        "s.wesl",
        "//! output: o Gray
//! param: n int(0,9) = 4
//! passes: n+log2+2",
    )
    .unwrap();
    assert_eq!(sum.pass_count(1024, &[4.0]), 16);
}

#[test]
fn soft_float_bounds_preserve_sdk_values_without_changing_packing() {
    let source = "//! output: o Gray\n//! param: intensity float(0,100) = 1 \"Intensity\" soft";
    let def = Def::parse("normal.wesl", source).unwrap();
    let param = &def.params[0];
    assert!(param.soft_range);
    assert_eq!(param.pack(Some(&[200.])), [200.]);
    assert_eq!(param.pack(None), [1.]);
    assert_eq!(def.slots(), 1);
    assert!(!Def::parse("legacy.wesl", LEVELS).unwrap().params[0].soft_range);
    assert!(
        Def::parse(
            "x.wesl",
            "//! output: o Gray\n//! param: n int(0,3) = 1 \"N\" soft"
        )
        .is_err()
    );
}

#[test]
fn versioned_parameter_and_input_defaults_parse_independently() {
    let def = Def::parse("normal.wesl", "//! output: o Color\n//! input: input1 Gray default=0 legacy=0.5 primary\n//! param: mode enum(DirectX|OpenGL) = directx \"Format\" legacy=opengl\n//! param: alpha bool = true \"Alpha\" legacy=false").unwrap();
    assert_eq!(def.inputs[0].default, [0., 0., 0., 1.]);
    assert_eq!(def.inputs[0].legacy_default, Some([0.5, 0.5, 0.5, 1.]));
    assert_eq!(def.params[0].default, [0.]);
    assert_eq!(def.params[0].legacy_default, Some(vec![1.]));
    assert_eq!(def.params[1].default, [1.]);
    assert_eq!(def.params[1].legacy_default, Some(vec![0.]));
    assert_eq!(def.slots(), 2);
    let soft = Def::parse(
        "mode.wesl",
        "//! output: o Gray\n//! param: mode enum(Hard|Soft) = hard \"Mode\" legacy=soft",
    )
    .unwrap();
    assert!(!soft.params[0].soft_range);
    assert_eq!(soft.params[0].legacy_default, Some(vec![1.]));
    assert!(
        Def::parse(
            "mode.wesl",
            "//! output: o Gray\n//! param: mode enum(Hard|Soft) = soft"
        )
        .is_ok()
    );
}

#[test]
fn rejects_mistakes() {
    let bad = |s: &str| Def::parse("x.wesl", s).unwrap_err().to_string();
    assert!(bad("//! output: a Gray\n//! output: a Gray").contains("twice"));
    assert!(bad("//! output: a Pink").contains("socket"));
    assert!(bad("//! colour: x").contains("unknown key"));
    assert!(bad("//! param: a float").contains("output"));
    assert!(bad("//! output: o Gray\n//! param: m enum(A|B) = c").contains("option"));
}

#[test]
fn gradient_stops_sort() {
    let mut v = vec![2.0, 0.8, 1.0, 1.0, 1.0, 1.0, 0.2, 0.0, 0.0, 0.0, 1.0];
    sort_stops(&mut v);
    assert_eq!(v[1], 0.2);
    assert_eq!(v[6], 0.8);
}

#[test]
fn dense_gradient_preserves_all_256_compiled_keys() {
    let stops = (0..256)
        .map(|i| format!("{} {} 0 0 1", i as f32 / 255., i as f32 / 255.))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!("//! output: output Color\n//! param: gradient gradient = {stops}");
    let def = Def::parse("gradient.wesl", &source).unwrap();
    assert_eq!(def.params[0].default.len(), 1 + 256 * 5);
    assert_eq!(def.params[0].default[0], 256.);
    assert_eq!(&def.params[0].default[1 + 255 * 5..], &[1., 1., 0., 0., 1.]);
    assert!(def.slots() >= def.params[0].default.len());
}

#[test]
fn curve_capacity_preserves_legacy_offsets_and_dense_handles() {
    let legacy = Def::parse(
        "old.wesl",
        "//! output: o Color\n//! param: a curve\n//! param: b curve",
    )
    .unwrap();
    assert_eq!(legacy.params[1].offset, 97);
    let current = Def::parse(
        "new.wesl",
        "//! output: o Color\n//! param: a curve(256) = empty\n//! param: b curve(256) = empty",
    )
    .unwrap();
    assert_eq!(current.params[1].offset, 1537);
    assert_eq!(current.params[0].ty.curve_capacity(), Some(256));
    let mut points = vec![256.];
    points.extend((0..1536).map(|i| i as f32 / 1536.));
    assert_eq!(current.params[0].pack(Some(&points)), points);
    assert_eq!(current.params[0].pack(None)[0], 0.);
    for ty in [
        "curve(0)",
        "curve(1)",
        "curve(257)",
        "curve(2,4)",
        "curve(x)",
    ] {
        assert!(
            Def::parse(
                "bad.wesl",
                &format!("//! output: o Color\n//! param: a {ty}")
            )
            .is_err()
        );
    }
}
