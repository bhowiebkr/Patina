//! The synthesizer example: real audio, animated wire values and editable instrument panels.
use crate::common::copy;
use patina_engine::{Control, Mode, Project, Toolchain};
use std::path::Path;

#[test]
fn the_synthesizer_plays_the_same_patch_in_edit_and_bake_and_has_live_controls() {
    let project = copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/synth"),
        "synth",
    );
    let opened = Project::open(&project).unwrap();
    let checked = opened.check().unwrap();
    let toolchain = Toolchain::detect().unwrap();
    let control = Control::default();
    let mut previous = Vec::new();
    for mode in [Mode::Edit, Mode::Bake] {
        let mut cluster = checked.build(&toolchain, mode).unwrap();
        assert!(cluster.set_params(checked.graph()).ok);
        assert!(cluster.run(&control).ok);
        // The first scope frame has no audio yet. The controls still publish their settings.
        assert!(cluster.frame(&control).ok);
        let mut samples = Vec::new();
        for block in 0..96 {
            let result = cluster.block(&control);
            assert!(result.ok, "{:?}", result.reports);
            assert!(
                result
                    .values
                    .iter()
                    .any(|v| v.from == "e6be000000000026.out" && !v.wave.is_empty())
            );
            samples.extend_from_slice(cluster.sound().expect("the output node plays real audio"));
            if block % 3 == 0 {
                assert!(cluster.frame(&control).ok);
            }
        }
        assert!(samples.iter().all(|v| v.is_finite() && v.abs() <= 0.7));
        let peak = samples.iter().fold(0.0_f32, |p, v| p.max(v.abs()));
        assert!(peak > 0.2, "silent patch: {peak}");
        if !previous.is_empty() {
            let max = samples
                .iter()
                .zip(&previous)
                .fold(0.0_f32, |m, (a, b): (&f32, &f32)| m.max((a - b).abs()));
            assert!(max < 0.0001, "edit and bake differ by {max}");
        }
        previous = samples;
        let id = "e6be000000000011";
        let before = cluster.ui(&control, id, 0, "").0.unwrap();
        assert!(before.contains("Timbre shape") && before.contains("SAW"));
        // The first canvas has the four shape buttons. The input becomes an undoable parameter write.
        let (after, result) = cluster.ui(&control, id, 0, "0 down 30 80\n0 up 30 80");
        assert!(result.ok && after.unwrap().contains("Timbre shape"));
        assert!(
            result
                .params
                .iter()
                .any(|(key, value)| key == "shape" && value == "0"),
            "{:?}",
            result.params
        );
        for scene in [1_u8, 2] {
            let mut graph = checked.graph().clone();
            graph
                .node
                .iter_mut()
                .find(|node| node.id == "e6be000000000030")
                .unwrap()
                .params
                .insert("scene".into(), scene.to_string());
            assert!(cluster.set_params(&graph).ok);
            assert!(cluster.frame(&control).ok);
            let mut energy = 0.0_f32;
            for block in 0..256 {
                assert!(cluster.block(&control).ok);
                let sound = cluster.sound().unwrap();
                assert!(sound.iter().all(|v| v.is_finite() && v.abs() <= 0.7));
                if block >= 64 {
                    energy += sound.iter().map(|v| v * v).sum::<f32>();
                }
                if block % 3 == 0 {
                    assert!(cluster.frame(&control).ok);
                }
            }
            assert!(energy > 1.0, "scene {scene} is silent: {energy}");
        }
        assert!(
            cluster.split().is_some(),
            "the audio graph needs its own thread"
        );
    }
}
