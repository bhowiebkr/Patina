//! The hybrid arrangement through actual generated libraries and the view edit path.
use crate::common::copy;
use patina_engine::view::{View, snapshot};
use patina_engine::{Control, Edit, Mode, Project, Toolchain};
use std::{fs, path::Path};

const ID: &str = "e6bf000000000001";
const SCORE: &str = r#"{"version":1,"bpm":120,"length":4,"looping":false,"loop_range":[0,4],"tracks":[{"id":1,"name":"Hybrid test","instrument":"Lead","sample":"","root_note":60,"gain":0.6,"pan":0,"mute":false,"solo":false,"attack":0.004,"release":0.05,"cutoff":18000,"drive":1,"delay_mix":0,"delay_beats":0.75,"automation":[],"clips":[{"id":1,"name":"Stereo WAV","start":0,"length":1,"fade_in":0.005,"fade_out":0.005,"content":{"kind":"Audio","path":"types/synth-dsp/test.wav","offset_seconds":0,"rate":1}},{"id":2,"name":"Notes","start":0.5,"length":1,"fade_in":0,"fade_out":0,"content":{"kind":"Notes","period":1,"offset":0,"notes":[{"start":0,"length":0.75,"key":62,"velocity":0.8},{"start":0,"length":0.5,"key":69,"velocity":0.6}]}}]}]}"#;

#[test]
fn the_song_plays_stereo_wav_and_polyphonic_notes_in_edit_and_bake_and_saves_view_edits() {
    let path = copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workbenches/synth"),
        "song",
    );
    fs::write(path.join("types/synth-dsp/test.wav"), wav()).unwrap();
    let mut project = Project::open_top(&path, "song").unwrap();
    project
        .apply(&Edit::SetParam {
            id: ID.into(),
            key: "score".into(),
            text: Some(SCORE.into()),
        })
        .unwrap();
    project.save().unwrap();
    let toolchain = Toolchain::detect().unwrap();
    let control = Control::default();
    let checked = project.check().unwrap();
    let mut reference = Vec::new();
    for mode in [Mode::Edit, Mode::Bake] {
        let mut cluster = checked.build(&toolchain, mode).unwrap();
        assert!(cluster.set_params(checked.graph()).ok);
        assert!(cluster.run(&control).ok && cluster.frame(&control).ok);
        let mut samples = Vec::new();
        for block in 0..160 {
            let result = cluster.block(&control);
            assert!(result.ok, "{:?}", result.reports);
            samples.extend_from_slice(cluster.sound().unwrap());
            if block % 3 == 0 {
                assert!(cluster.frame(&control).ok);
            }
        }
        assert!(samples.iter().all(|v| v.is_finite() && v.abs() <= 0.95));
        assert!(samples.iter().any(|v| v.abs() > 0.05));
        assert!(
            samples
                .as_chunks::<2>()
                .0
                .iter()
                .any(|p| (p[0] - p[1]).abs() > 0.1)
        );
        if !reference.is_empty() {
            let difference = samples
                .iter()
                .zip(&reference)
                .fold(0.0_f32, |max, (a, b): (&f32, &f32)| max.max((a - b).abs()));
            assert!(difference < 0.00001, "edit/bake difference: {difference}");
        }
        reference = samples;
        assert!(cluster.split().is_some());
    }
    let library = project
        .build_view(&toolchain, "arrange", Mode::Edit)
        .unwrap();
    let mut view = View::load(&library, &path).unwrap();
    let before = snapshot(project.graph(), &[], &[], &[]);
    let first = view.ui(2, "@size 1100 620", &before, None).unwrap();
    let index = first
        .tree
        .lines()
        .position(|s| s.starts_with("timeline\tArrangement"))
        .unwrap();
    let frame = view
        .ui(
            2,
            &format!("@size 1100 620\n{index} mute 0 1"),
            &before,
            None,
        )
        .unwrap();
    assert_eq!(frame.edits.len(), 1);
    project.apply(&Edit::Group { edits: frame.edits }).unwrap();
    project.save().unwrap();
    let saved = Project::open_top(&path, "song").unwrap();
    let after = snapshot(saved.graph(), &[], &[], &[]);
    assert!(after.contains("\"mute\":true"));
    project.undo(&mut Default::default()).unwrap();
    assert_eq!(snapshot(project.graph(), &[], &[], &[]), before);
}

fn wav() -> Vec<u8> {
    let frames = 22050_u32;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((frames * 8 + 36).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(3_u16.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(44100_u32.to_le_bytes());
    bytes.extend(352800_u32.to_le_bytes());
    bytes.extend(8_u16.to_le_bytes());
    bytes.extend(32_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((frames * 8).to_le_bytes());
    for i in 0..frames {
        let value = (i as f32 * std::f32::consts::TAU * 440.0 / 44100.0).sin() * 0.3;
        bytes.extend(value.to_le_bytes());
        bytes.extend((-value * 0.5).to_le_bytes());
    }
    bytes
}

#[test]
fn the_sandstorm_excerpt_renders_and_its_lead_controls_save_and_undo() {
    let path = copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workbenches/synth"),
        "sandstorm",
    );
    let mut project = Project::open_top(&path, "sandstorm").unwrap();
    let toolchain = Toolchain::detect().unwrap();
    let mut checked = project.check().unwrap();
    let control = Control::default();
    let mut reference = Vec::new();
    for (mode, slot) in [(Mode::Edit, false), (Mode::Edit, true), (Mode::Bake, false)] {
        checked.set_slot(slot.then_some("spectrogram")).unwrap();
        let mut cluster = checked.build(&toolchain, mode).unwrap();
        assert!(cluster.set_params(checked.graph()).ok);
        assert!(cluster.run(&control).ok && cluster.frame(&control).ok);
        let mut samples = Vec::new();
        for block in 0..1875 {
            assert!(cluster.block(&control).ok);
            samples.extend_from_slice(cluster.sound().unwrap());
            if block % 3 == 2 {
                assert!(cluster.frame(&control).ok);
            }
        }
        check_spectrogram(&mut cluster, &control);
        assert!(samples.iter().all(|v| v.is_finite() && v.abs() <= 0.95));
        let peak = samples.iter().fold(0.0_f32, |peak, v| peak.max(v.abs()));
        assert!(peak > 0.2 && peak < 0.94, "peak {peak}");
        let end = &samples[samples.len() - 48000..];
        assert!(end.iter().all(|v| v.abs() < 0.001));
        if !reference.is_empty() {
            assert!(
                samples
                    .iter()
                    .zip(&reference)
                    .all(|(a, b): (&f32, &f32)| { (a - b).abs() < 0.00001 })
            );
        }
        reference = samples;
    }
    let library = project
        .build_view(&toolchain, "arrange", Mode::Edit)
        .unwrap();
    let mut view = View::load(&library, &path).unwrap();
    let before = snapshot(project.graph(), &[], &[], &[]);
    let page = view.ui(2, "@size 1100 620", &before, None).unwrap();
    let tabs = page
        .tree
        .lines()
        .position(|s| s.starts_with("tabs\t"))
        .unwrap();
    let mixer = view
        .ui(2, &format!("@size 1100 620\n{tabs} 2"), &before, None)
        .unwrap();
    let fifth = mixer
        .tree
        .lines()
        .position(|s| s.starts_with("slider\tFifth level"))
        .unwrap();
    let edit = view
        .ui(2, &format!("@size 1100 620\n{fifth} 0.25"), &before, None)
        .unwrap();
    assert_eq!(edit.edits.len(), 1);
    project.apply(&Edit::Group { edits: edit.edits }).unwrap();
    project.save().unwrap();
    let saved = Project::open_top(&path, "sandstorm").unwrap();
    assert!(snapshot(saved.graph(), &[], &[], &[]).contains("\"fifth\":0.25"));
    project.undo(&mut Default::default()).unwrap();
    assert_eq!(snapshot(project.graph(), &[], &[], &[]), before);

    let sound = view
        .ui(2, &format!("@size 1100 620\n{tabs} 4"), &before, None)
        .unwrap();
    let edge = sound
        .tree
        .lines()
        .position(|s| s.starts_with("slider\tSource edge"))
        .unwrap();
    let edit = view
        .ui(2, &format!("@size 1100 620\n{edge} 7.5"), &before, None)
        .unwrap();
    assert_eq!(edit.edits.len(), 1);
    project.apply(&Edit::Group { edits: edit.edits }).unwrap();
    project.save().unwrap();
    let saved = Project::open_top(&path, "sandstorm").unwrap();
    assert!(snapshot(saved.graph(), &[], &[], &[]).contains("\"edge\":7.5"));
    project.undo(&mut Default::default()).unwrap();
    assert_eq!(snapshot(project.graph(), &[], &[], &[]), before);
}

fn check_spectrogram(cluster: &mut patina_engine::Cluster, control: &Control) {
    let id = "e6c0000000000022";
    let tree = cluster.ui(control, id, 0, "").0.unwrap();
    assert!(tree.contains("LIVE / continuous audio capture"));
    let image = |tree: &str| {
        tree.lines()
            .find(|line| line.starts_with("image\t"))
            .unwrap()
            .to_owned()
    };
    let live_image = image(&tree);
    let freeze = tree
        .lines()
        .position(|line| line.starts_with("toggle\tFreeze"))
        .unwrap();
    let clear = tree
        .lines()
        .position(|line| line.starts_with("button\tClear"))
        .unwrap();
    cluster.ui(control, id, 0, &format!("{freeze} 1"));
    for _ in 0..48 {
        assert!(cluster.block(control).ok);
        assert!(cluster.frame(control).ok);
    }
    let tree = cluster.ui(control, id, 0, "").0.unwrap();
    assert!(tree.contains("FROZEN /"));
    assert_eq!(image(&tree), live_image);
    let tree = cluster.ui(control, id, 0, &format!("{clear} 1")).0.unwrap();
    assert_ne!(image(&tree), live_image);
}
