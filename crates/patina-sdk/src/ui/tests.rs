//! The tests of the node GUI widgets (spec 4.3).

use super::*;

#[test]
fn widgets_read_their_events_and_write_their_lines() {
    let (mut on, mut level) = (true, 3_i32);
    let mut ui = Ui::new(1, "0 1\n2 99\nbad\n3 0");
    assert_eq!(ui.mode(), Mode::Large);
    assert!(ui.button("Re\tset").clicked());
    assert!(!ui.toggle("On", &mut on).changed());
    assert!(ui.slider("Level", &mut level, 0..=10).changed());
    assert!(ui.toggle("Off", &mut on).changed());
    assert_eq!((on, level), (false, 10));
    assert_eq!(ui.tree(), "button\tRe set\ntoggle\tOn\t1\nslider\tLevel\t3\t0\t10\ntoggle\tOff\t1");
}

#[test]
fn fields_and_dropdowns_take_their_values() {
    let (mut count, mut name, mut pick) = (1_u32, String::from("a"), 0);
    let mut ui = Ui::new(0, "0 7\n1 new name\n2 5\n3 x");
    assert!(ui.number("Count", &mut count).changed());
    assert!(ui.text_field("Name", &mut name).changed());
    assert!(!ui.dropdown("Wave", &mut pick, &["Sine", "Saw"]).changed());
    assert!(!ui.number("Bad", &mut count).changed());
    assert_eq!((count, name.as_str(), pick), (7, "new name", 0));
    assert_eq!(ui.tree(), "number\tCount\t1\ntext\tName\ta\ndropdown\tWave\t0\tSine\tSaw\nnumber\tBad\t7");
}

#[test]
fn units_tabs_and_sections_write_their_lines() {
    let (mut gain, mut tab, mut open, mut shut) = (1.5_f32, 0, false, true);
    // Index 2 opens the first section, so its label is index 3 and its end is 4. Index 5 closes the next one.
    let mut ui = Ui::new(0, "0 -3\n1 1\n2 1\n5 0\n8 1");
    assert!(ui.number_unit("Gain", &mut gain, -0.5, "d\tB").changed());
    assert!(ui.tabs("Pages", &mut tab, &["Main", "More"]).changed());
    let mut inner = false;
    assert!(ui.section("More", &mut open, |ui| ui.label("in")).changed());
    ui.section("Shut", &mut shut, |ui| {
        ui.toggle("hidden", &mut inner);
    });
    assert!(ui.button("After").clicked());
    assert_eq!((gain, tab, open, shut), (-3.0, 1, true, false));
    let lines: Vec<&str> = ui.tree().lines().collect();
    assert_eq!(lines, ["number\tGain\t1.5\t0.5\td B", "tabs\tPages\t0\tMain\tMore", "section\tMore\t1", "label\tin", "end\t", "section\tShut\t0", "", "end\t", "button\tAfter"]);
}

#[test]
fn axes_a_scatter_plot_and_a_histogram_write_their_lines() {
    let mut points = vec![[0.5, 0.5]];
    let mut ui = Ui::new(0, "");
    ui.plot_axes("Response", &[1.0, 2.0], -1.0..=2.0, 20.0..=20000.0, ["Hz", "d\tB"]);
    ui.scatter("Dots", &[[1.0, 2.0], [3.0, 4.0]], 0.0..=4.0, 0.0..=5.0, ["s", ""]);
    ui.curve_histogram("Levels", &mut points, &[1.0, 2.0, 3.0]);
    let lines: Vec<_> = ui.tree().lines().collect();
    assert_eq!(lines, ["plot\tResponse\t-1\t2\t1 2\t20\t20000\tHz\td B", "scatter\tDots\t0\t5\t1 2 3 4\t0\t4\ts\t", "curve\tLevels\t1 2 3\t0.5 0.5"]);

    // A long histogram keeps its total, and a long scatter plot sends points at equal steps.
    let (bars, dots) = (vec![1.0; 1000], vec![[0.0, 0.0]; 3000]);
    let mut ui = Ui::new(0, "");
    ui.curve_histogram("Long", &mut points, &bars);
    ui.scatter("Long", &dots, 0.0..=1.0, 0.0..=1.0, ["", ""]);
    let lines: Vec<Vec<&str>> = ui.tree().lines().map(|l| l.split('\t').collect()).collect();
    let total: f32 = lines[0][2].split(' ').map(|v| v.parse::<f32>().unwrap()).sum();
    assert_eq!((lines[0][2].split(' ').count(), total), (MAX_BINS, 1000.0));
    assert_eq!(lines[1][4].split(' ').count(), 2 * 1000);
}

#[test]
fn an_icon_button_and_a_progress_bar_write_their_lines() {
    let mut ui = Ui::new(0, "0 1");
    assert!(ui.icon_button("rotate-left", "Rotate\tleft").clicked());
    ui.progress("Frames", 1.5);
    ui.progress("Bad", f32::NAN);
    assert_eq!(ui.tree(), "icon\tRotate left\trotate-left\nprogress\tFrames\t1\nprogress\tBad\t0");
}

#[test]
fn a_trim_event_sets_a_sorted_range_in_the_limits() {
    let (mut trim, mut bad) = ([0.2_f32, 0.8], [f32::NAN, 0.5]);
    let mut ui = Ui::new(0, "0 0.9 -1\n1 x 0.5");
    assert!(ui.waveform_trim("Wave", &[0.5], None, &mut trim).changed());
    assert!(!ui.waveform_trim("Bad", &[], Some(0.5), &mut bad).changed());
    assert_eq!(trim, [0.0, 0.9]);
    assert_eq!(ui.tree().lines().collect::<Vec<_>>(), ["waveform\tWave\t\t0.5 0.5\t0 0.9", "waveform\tBad\t0.5\t\t0 1"]);
}

#[test]
fn a_canvas_reads_the_pointer_and_writes_its_shapes() {
    struct Dot;
    impl Widget for Dot {
        fn show(&mut self, ui: &mut Ui) -> Response {
            let mut canvas = ui.canvas("Dot", [40.0, 20.0]);
            let down = canvas.pointer().iter().any(|p| matches!(p, Pointer::Down(_)));
            canvas.circle([5.0, 5.0], 2.5, [255, 0, 16], true);
            canvas.text([1.0, 15.0], [0, 0, 0], "a\tb");
            Response { clicked: down, changed: false }
        }
    }
    let mut ui = Ui::new(0, "1 down 1 2\n1 move x 2\n1 up 3 4\n0 1");
    ui.label("Top");
    assert!(ui.add(&mut Dot).clicked());
    let mut canvas = ui.canvas("Grid", [8.0, 8.0]);
    assert!(canvas.pointer().is_empty());
    canvas.line([0.0, 0.0], [8.0, 8.0], [1, 2, 3]);
    assert_eq!(ui.canvas("Again", [1.0, 1.0]).pointer(), &[] as &[Pointer]);
    let lines: Vec<_> = ui.tree().lines().collect();
    assert_eq!(lines[1], "canvas\tDot\t40\t20\tcircle 5 5 2.5 #ff0010 1\ttext 1 15 #000000 a b");
    assert_eq!(lines[2], "canvas\tGrid\t8\t8\tline 0 0 8 8 #010203");
    let mut ui = Ui::new(0, "0 down 1 2\n0 move 3 4\n0 up 5 6");
    assert_eq!(ui.canvas("Drag", [9.0, 9.0]).pointer(), [Pointer::Down([1.0, 2.0]), Pointer::Move([3.0, 4.0]), Pointer::Up([5.0, 6.0])]);
}

#[test]
fn a_free_layout_places_pins_widgets_and_paths() {
    let mut ui = Ui::new(0, "");
    ui.canvas("Knob", [48.0, 48.0]).path("M 0 0 L 48 0\tZ", [255, 255, 255], false);
    ui.pin("out", [48.0, 24.0]);
    ui.place([4.0, 2.5]);
    ui.label("Go");
    assert_eq!(ui.tree(), "canvas\tKnob\t48\t48\tpath #ffffff 0 M 0 0 L 48 0 Z\npin\tout\t48 24\nplace\t\t4 2.5\nlabel\tGo");
}

#[test]
fn a_canvas_takes_keys_only_when_it_asks_for_them() {
    let mut ui = Ui::new(0, "0 key down ArrowUp\n0 down 1 2\n0 key up \n0 key up ArrowUp\n1 key down Space");
    let mut pad = ui.canvas("Pad", [8.0, 8.0]);
    assert_eq!(pad.keys(), [Key::Down("ArrowUp".into()), Key::Up("ArrowUp".into())]);
    assert_eq!(pad.pointer(), [Pointer::Down([1.0, 2.0])]);
    pad.rect([0.0, 0.0], [1.0, 1.0], [0, 0, 0], true);
    ui.canvas("Plain", [8.0, 8.0]);
    assert_eq!(ui.tree(), "canvas\tPad\t8\t8\tkeys\trect 0 0 1 1 #000000 1\ncanvas\tPlain\t8\t8");
}

#[test]
fn a_viewport_reads_its_place_and_input() {
    let mut ui = Ui::new(0, "0 place 7 800 600 10 20 300 200\n0 down 1 2\n0 wheel 40\n0 key down KeyW\n0 key up \n0 wheel -15\n0 key up KeyW\n0 place 7 900 600 0 0 1\n0 wheel x\n1 place 9 1 1 0 0 1 1");
    let viewport = ui.viewport("Scene", 240.0);
    assert_eq!((viewport.window, viewport.window_size, viewport.rect), (7, [800, 600], [10, 20, 300, 200]));
    assert_eq!((viewport.pointer, viewport.wheel), (vec![Pointer::Down([1.0, 2.0])], 25.0));
    assert_eq!(viewport.keys, [Key::Down("KeyW".into()), Key::Up("KeyW".into())]);
    assert_eq!(ui.tree(), "viewport\tScene\t240");
    assert_eq!(Ui::new(0, "").viewport("Empty", 1.0), Viewport::default());
}

#[test]
fn value_widgets_write_their_data_and_take_their_edits() {
    let (mut color, mut row, mut points) = ([255, 0, 16], None, vec![[0.5, 0.5], [0.0, 0.0]]);
    let mut ui = Ui::new(0, "0 #00ff80\n2 1\n3 move 0 2 -1\n4 #zz0000\n5 9\n6 add 0.5");
    assert!(ui.color("Tint", &mut color).changed());
    ui.plot("Level", &[0.0, 1.5, f32::NAN], 0.0..=2.0);
    assert!(ui.table("Notes", &["Note", "Hz"], &["A", "440", "C"], &mut row).changed());
    assert!(ui.curve("Shape", &mut points).changed());
    assert!(!ui.color("Bad", &mut color).changed());
    assert!(!ui.table("Empty", &[], &["x"], &mut row).changed());
    assert!(!ui.curve("Bad", &mut points).changed());
    ui.waveform("Wave", &[0.0, -0.5, 1.0, f32::NAN], Some(2.0));
    ui.image("Dot", [1, 1], &[1, 2, 3, 255]);
    ui.image("Bad", [2, 1], &[0; 4]);
    assert_eq!((color, row, points), ([0, 255, 128], Some(1), vec![[0.0, 0.0], [1.0, 0.0]]));
    let lines: Vec<_> = ui.tree().lines().collect();
    assert_eq!(lines[..6], ["color\tTint\t#ff0010", "plot\tLevel\t0\t2\t0 1.5 NaN", "table\tNotes\t2\t\tNote\tHz\tA\t440\tC", "curve\tShape\t\t0.5 0.5\t0 0", "color\tBad\t#00ff80", "table\tEmpty\t0\t1\tx"]);
    assert_eq!(lines[7..], ["waveform\tWave\t1\t0 0 -0.5 -0.5 1 1 inf -inf", "image\tDot\t1\t1\tAQID/w==", "image\tBad\t0\t0\t"]);

    // A long plot line keeps its peak, and a long waveform has a fixed number of columns.
    let long: Vec<f32> = (0..3000).map(|i| if i == 1500 { 9.0 } else { 0.0 }).collect();
    let mut ui = Ui::new(0, "");
    ui.plot("Long", &long, 0.0..=9.0);
    ui.waveform("Long", &long, None);
    let lines: Vec<Vec<_>> = ui.tree().lines().map(|l| l.rsplit('\t').next().unwrap().split(' ').collect()).collect();
    assert_eq!((lines[0].len(), lines[0].contains(&"9")), (MAX_PLOT, true));
    assert_eq!((lines[1].len(), lines[1].contains(&"9")), (2 * WAVE_COLUMNS, true));
    assert!(ui.tree().contains("waveform\tLong\t\t0 0"));
}

#[test]
fn a_color_ramp_sorts_its_stops_and_keeps_the_selected_stop_on_a_move() {
    let mut ramp = ColorRamp { stops: vec![(1.0, [255, 255, 255]), (0.0, [0, 0, 0])], blend: Blend::Linear };
    let mut selected = 5;
    let mut ui = Ui::new(0, "0 move 0 0.75");
    assert!(ui.color_ramp("Ramp", &mut ramp, &mut selected).changed());
    assert_eq!(ui.tree(), "ramp\tRamp\tlinear\t1\t0 #000000\t1 #ffffff");
    assert_eq!(ramp.stops, vec![(0.75, [0, 0, 0]), (1.0, [255, 255, 255])]);
    assert_eq!(selected, 0);
    assert_eq!(ramp.color(0.875), [128, 128, 128]);
    for (event, stops, at) in [
        ("0 add 0.875", 3, 1),
        ("0 color 1 #ff8000", 2, 1),
        ("0 remove 1", 1, 0),
        ("0 select 1", 2, 1),
    ] {
        let mut copy = ramp.clone();
        let mut ui = Ui::new(0, event);
        assert!(ui.color_ramp("", &mut copy, &mut selected).changed(), "{event}");
        assert_eq!((copy.stops.len(), selected), (stops, at), "{event}");
    }
    let mut ui = Ui::new(0, "0 blend constant");
    ui.color_ramp("", &mut ramp, &mut selected);
    assert_eq!(ramp.color(0.99), [0, 0, 0]);
}

#[test]
fn a_bad_ramp_event_changes_nothing_and_a_ramp_keeps_one_stop() {
    let one = ColorRamp { stops: vec![(0.5, [1, 2, 3])], blend: Blend::Ease };
    for event in ["0 remove 0", "0 move 3 0.5", "0 move 0 NaN", "0 color 0 #ff80", "0 blend smooth", "0 select x", "0 jump 0"] {
        let (mut ramp, mut selected) = (one.clone(), 0);
        let mut ui = Ui::new(0, event);
        assert!(!ui.color_ramp("", &mut ramp, &mut selected).changed(), "{event}");
        assert_eq!(ramp, one, "{event}");
    }
    assert_eq!(ColorRamp::default().color(0.5), [0, 0, 0]);
    assert_eq!(one.color(0.0), [1, 2, 3]);
}

#[test]
fn a_layer_stack_moves_hides_and_sets_the_amount_of_a_layer() {
    let layer = |name: &str, amount| Layer { name: name.into(), color: [0, 128, 255], visible: true, amount };
    let mut layers = vec![layer("Edge\tpolish", Some(0.35)), layer("Verdigris", Some(0.62)), layer("Copper", None)];
    let mut selected = 9;
    let mut ui = Ui::new(0, "0 move 0 2");
    assert!(ui.layer_stack("Layers", &mut layers, &mut selected).changed());
    assert_eq!(ui.tree(), "layers\tLayers\t2\t1 0.35 #0080ff Edge polish\t1 0.62 #0080ff Verdigris\t1 - #0080ff Copper");
    let names: Vec<_> = layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!((names, selected), (vec!["Verdigris", "Copper", "Edge\tpolish"], 2));
    for (event, to) in [("0 move 2 0", ["Edge\tpolish", "Verdigris", "Copper"]), ("0 select 1", ["Verdigris", "Copper", "Edge\tpolish"])] {
        let mut copy = layers.clone();
        let mut ui = Ui::new(0, event);
        assert!(ui.layer_stack("", &mut copy, &mut selected).changed(), "{event}");
        assert_eq!(copy.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), to, "{event}");
    }
    assert_eq!(selected, 1);
    let mut ui = Ui::new(0, "0 visible 0 0\n1 amount 0 7");
    ui.layer_stack("", &mut layers, &mut selected);
    ui.layer_stack("", &mut layers, &mut selected);
    assert_eq!((layers[0].visible, layers[0].amount, selected), (false, Some(1.0), 1));
}

#[test]
fn a_bad_layer_event_changes_nothing() {
    let first = vec![Layer { name: "Base".into(), color: [1, 2, 3], visible: true, amount: None }];
    for event in ["0 amount 0 0.5", "0 move 0 1", "0 visible 0 2", "0 amount 0 NaN", "0 select 1", "0 select", "0 hide 0"] {
        let (mut layers, mut selected) = (first.clone(), 0);
        let mut ui = Ui::new(0, event);
        assert!(!ui.layer_stack("", &mut layers, &mut selected).changed(), "{event}");
        assert_eq!(layers, first, "{event}");
    }
}

#[test]
fn a_file_path_writes_its_kind_and_takes_the_new_path() {
    let mut path = String::from("out/a.png");
    let mut ui = Ui::new(0, "0 out/b c.png");
    assert!(ui.file_path("Path", &mut path, PathKind::Save).changed());
    assert_eq!(ui.tree(), "path\tPath\tsave\tout/a.png");
    assert_eq!(path, "out/b c.png");
    assert_eq!(crate::project_path("/abs/x"), std::path::PathBuf::from("/abs/x"));
}

#[test]
fn an_asset_picker_writes_its_extensions_and_takes_the_new_path() {
    let mut path = String::new();
    let mut ui = Ui::new(0, "0 meshes/ball.glb");
    assert!(ui.asset_picker("Source", &mut path, &["glb", "obj"]).changed());
    assert_eq!(ui.tree(), "asset\tSource\tglb obj\t");
    assert_eq!(path, "meshes/ball.glb");
}

#[test]
fn a_transform_reads_a_set_event_and_maps_points_both_ways() {
    let mut t = Transform2d::default();
    let mut ui = Ui::new(0, "0 set 10 -5 450 2 3 0 1 0");
    assert!(ui.transform("Move", [100.0, 50.0], &mut t, Some(([1, 1], &[1, 2, 3, 4]))).changed());
    assert_eq!(ui.tree(), "transform\tMove\t100 50\t0 0\t0\t1 1\t0.5 0.5\t1\t1\t1\tAQIDBA==");
    assert_eq!(t, Transform2d { offset: [10.0, -5.0], rotation: 90.0, scale: [2.0, 3.0], pivot: [0.0, 1.0], lock: false });
    // The pivot at the bottom left stays, plus the move. A turn of 90 degrees puts x down.
    let near = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3;
    assert!(near(t.apply([100.0, 50.0], [0.0, 50.0]), [10.0, 45.0]));
    assert!(near(t.apply([100.0, 50.0], [1.0, 50.0]), [10.0, 47.0]));
    for p in [[0.0, 0.0], [25.0, 10.0], [100.0, 50.0]] {
        assert!(near(t.invert([100.0, 50.0], t.apply([100.0, 50.0], p)), p), "{p:?}");
    }
    let mut ui = Ui::new(0, "0 set 0 0 -190 0 1000 -1 2 1");
    ui.transform("", [0.0, f32::NAN], &mut t, Some(([2, 2], &[0; 4])));
    assert_eq!(ui.tree(), "transform\t\t1 1\t10 -5\t90\t2 3\t0 1\t0\t0\t0\t");
    assert_eq!((t.rotation, t.scale, t.pivot, t.lock), (170.0, [0.01, 100.0], [0.0, 1.0], true));
}

#[test]
fn a_bad_transform_event_changes_nothing() {
    let start = Transform2d::default();
    for event in ["0 set 1 2 3 4 5 6 7", "0 set 1 2 NaN 1 1 0 0 1", "0 move 1 2 0 1 1 0 0 1", "0 set 0 0 0 1 1 0.5 0.5 1"] {
        let mut t = start;
        let mut ui = Ui::new(0, event);
        assert!(!ui.transform("", [10.0, 10.0], &mut t, None).changed(), "{event}");
        assert_eq!(t, start, "{event}");
    }
}

fn timeline() -> Timeline {
    let clip = Clip { name: "impact\t03".into(), color: [0x40, 0x80, 0xc0], start: 0.0, length: 1.0, offset: 0.5, source: 2.0, samples: vec![0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0, 4.0] };
    let cutoff = Automation { range: [500.0, 5000.0], unit: "Hz".into(), points: vec![[0.1, 500.0], [1.0, 2000.0]] };
    let lanes = vec![
        Lane { name: "Impact".into(), mute: false, solo: false, kind: LaneKind::Clips(vec![clip]) },
        Lane { name: "Cutoff".into(), mute: false, solo: false, kind: LaneKind::Automation(cutoff) },
    ];
    Timeline { length: 3.0, playhead: 1.25, looping: true, loop_range: [0.0, 2.4], lanes, ..Timeline::default() }
}

fn timeline_edit(t: &mut Timeline, event: &str) -> bool {
    Ui::new(0, &format!("0 {event}")).timeline("", t).changed()
}

#[test]
fn a_timeline_writes_its_lanes_and_takes_edits() {
    let mut t = timeline();
    let mut ui = Ui::new(0, "");
    assert!(!ui.timeline("Time", &mut t).changed());
    // The clip shows the samples between its trims: 0.5 s to 1.5 s of a source of 2 s.
    let clip = "0 1 0.5 2 #4080c0 4 -1 -1 2 2 -2 -2 3 3 impact 03";
    assert_eq!(ui.tree(), format!("timeline\tTime\t3 1.25 0 1 0 2.4 0.1\t\tclips 0 0 1 Impact\t{clip}\tauto 0 0 500 5000 Cutoff\tHz\t0.1 500 1 2000"));
    let clip = |t: &Timeline| match &t.lanes[0].kind {
        LaneKind::Clips(clips) => (clips[0].start, clips[0].length, clips[0].offset),
        LaneKind::Automation(_) => unreachable!(),
    };
    let points = |t: &Timeline| match &t.lanes[1].kind {
        LaneKind::Automation(a) => a.points.clone(),
        LaneKind::Clips(_) => unreachable!(),
    };
    assert!(timeline_edit(&mut t, "clip 0 0 2.5 1 0.25"));
    assert_eq!((clip(&t), t.selected), ((2.0, 1.0, 0.25), Some([0, 0])));
    // A trim cannot go past the end of the source.
    assert!(timeline_edit(&mut t, "clip 0 0 0 5 1.5"));
    assert_eq!(clip(&t), (0.0, 0.5, 1.5));
    assert!(timeline_edit(&mut t, "add 1 0.5 9000"));
    assert_eq!((points(&t), t.selected), (vec![[0.1, 500.0], [0.5, 5000.0], [1.0, 2000.0]], Some([1, 1])));
    // A point stays between its neighbors and in the value range.
    assert!(timeline_edit(&mut t, "point 1 1 2 100"));
    assert!(timeline_edit(&mut t, "remove 1 0"));
    assert_eq!((points(&t), t.selected), (vec![[1.0, 500.0], [1.0, 2000.0]], None));
    for event in ["play 1", "loop 0", "range 2 0.5", "snap 0.25", "mute 1 1", "solo 0 1", "select 1 1", "seek 9"] {
        assert!(timeline_edit(&mut t, event), "{event}");
    }
    assert_eq!((t.playing, t.looping, t.loop_range, t.snap, t.playhead, t.selected), (true, false, [0.5, 2.0], 0.25, 3.0, Some([1, 1])));
    assert!(t.lanes[1].mute && t.lanes[0].solo);
}

#[test]
fn a_bad_timeline_event_changes_nothing() {
    let bad = [
        "clip 1 0 0 1 0", "clip 0 1 0 1 0", "clip 0 0.5 0 1 0", "point 0 0 1 1", "add 0 1 1", "remove 1 5", "select 0 1", "select 2 0", "select -1 0",
        "range 1 1", "seek NaN", "mute 9 1", "play", "play 1 2", "jump 1", "play 0", "seek 1.25",
    ];
    for event in bad {
        let mut t = timeline();
        assert!(!timeline_edit(&mut t, event), "{event}");
        assert_eq!(t, timeline(), "{event}");
    }
}

#[test]
fn a_timeline_plays_loops_and_stops_at_the_end() {
    let mut t = Timeline { playing: true, looping: true, loop_range: [0.5, 1.5], playhead: 1.25, ..Timeline::default() };
    t.advance(0.5);
    assert_eq!(t.playhead, 0.75);
    t.looping = false;
    t.advance(10.0);
    assert_eq!((t.playhead, t.playing), (4.0, false));
    t.advance(1.0);
    assert_eq!(t.playhead, 4.0);
    let a = Automation { range: [0.0, 10.0], unit: String::new(), points: vec![[1.0, 2.0], [3.0, 6.0]] };
    assert_eq!([0.0, 1.0, 2.0, 3.0, 9.0].map(|time| a.value(time)), [2.0, 2.0, 4.0, 6.0, 6.0]);
    assert_eq!(Automation { points: Vec::new(), ..a }.value(1.0), 0.0);
    let lane = |mute, solo| Lane { name: String::new(), mute, solo, kind: LaneKind::Clips(Vec::new()) };
    t.lanes = vec![lane(false, false), lane(false, true), lane(true, true)];
    assert_eq!([0, 1, 2, 3].map(|i| t.audible(i)), [false, true, false, false]);
}
