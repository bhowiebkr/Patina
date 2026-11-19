use super::*;

fn graph(extra: &str) -> Graph {
    let mut lines = String::new();
    for (id, folder) in [
        (CONSOLE, "console"),
        (SEQUENCE, "sequence"),
        (COLOR, "color"),
        (TIMBRE, "timbre"),
        (CONTOUR, "contour"),
        (SPACE, "space"),
        (OUTPUT, "output"),
    ] {
        lines.push_str(&format!("node\t{id}\tnodes/{folder}\n"));
    }
    lines.push_str(extra);
    Graph::new(&lines)
}

fn widget(tree: &str, prefix: &str) -> usize {
    tree.lines()
        .position(|line| line.starts_with(prefix))
        .unwrap_or_else(|| panic!("missing {prefix} in {tree}"))
}

#[test]
fn the_xy_pad_writes_both_real_parameters_and_rereads_external_changes() {
    let mut view = Perform::init();
    let mut g = graph("");
    let mut ui = Ui::new(2, "");
    view.ui(&mut ui, &mut g);
    assert_eq!(g.edits().count(), 0);
    let pad = widget(ui.tree(), "canvas\tFilter XY");
    view.ui(
        &mut Ui::new(2, &format!("{pad} down 272 80\n{pad} up 272 80")),
        &mut g,
    );
    let edits: Vec<_> = g.edits().collect();
    assert_eq!(edits.len(), 2, "{edits:?}");
    let cutoff: f32 = edits[0].rsplit('\t').next().unwrap().parse().unwrap();
    assert!((cutoff - (60.0_f32 * 16000.0).sqrt()).abs() < 0.01);
    assert!(edits[1].ends_with("resonance\t0.475"));
    let mut restored = graph(&format!(
        "param\t{COLOR}\tcutoff\t800\nparam\t{COLOR}\tresonance\t0.1"
    ));
    let mut ui = Ui::new(2, "");
    view.ui(&mut ui, &mut restored);
    assert!(ui.tree().contains("number\tCUTOFF\t800\t"));
    assert_eq!(
        restored.edits().count(),
        0,
        "reading a value must not write it back"
    );
}

#[test]
fn exact_input_clamps_and_reset_and_selection_reach_the_correct_node() {
    let mut view = Perform::init();
    view.precise = 4;
    let mut g = graph(&format!("param\t{COLOR}\tcutoff\t800\n"));
    let mut ui = Ui::new(2, "");
    view.ui(&mut ui, &mut g);
    let field = widget(ui.tree(), "number\tCUTOFF");
    let reset = widget(ui.tree(), "button\tReset this control");
    let select = widget(ui.tree(), "button\tSelect its node");
    view.ui(&mut Ui::new(2, &format!("{field} 99000")), &mut g);
    assert_eq!(
        g.edits().next().unwrap(),
        format!("set\t{COLOR}\tcutoff\t16000")
    );
    let mut g = graph(&format!("param\t{COLOR}\tcutoff\t800\n"));
    view.ui(&mut Ui::new(2, &format!("{reset} 1\n{select} 1")), &mut g);
    assert_eq!(
        g.edits().collect::<Vec<_>>(),
        [
            format!("set\t{COLOR}\tcutoff\t1600"),
            format!("select\t{COLOR}")
        ]
    );
}

#[test]
fn missing_or_connected_controls_never_write_a_hidden_override() {
    let mut view = Perform::init();
    let mut missing = Graph::new("");
    let mut ui = Ui::new(2, "0 2\n8 1");
    view.ui(&mut ui, &mut missing);
    assert!(ui.tree().contains("Open Ember's main graph"));
    assert_eq!(missing.edits().count(), 0);
    let mut connected = graph(&format!("wire\tx.out\t{COLOR}.cutoff\n"));
    CUTOFF.write(&mut connected, 500.0);
    assert_eq!(connected.edits().count(), 0);
    let mut ui = Ui::new(2, "");
    view.ui(&mut ui, &mut connected);
    assert!(!ui.tree().contains("canvas\tFilter XY"));
    let mut valid = graph("");
    assert_eq!(
        BPM.read(&valid),
        40.0,
        "an absent f32 input starts at zero, then the node clamps it"
    );
    assert_eq!(MASTER.read(&valid), 0.0);
    CUTOFF.write(&mut valid, f32::NAN);
    assert_eq!(valid.edits().count(), 0);
}

#[test]
fn scene_and_transport_controls_use_saved_graph_parameters() {
    let mut view = Perform::init();
    let mut g = graph("");
    let mut ui = Ui::new(2, "");
    view.ui(&mut ui, &mut g);
    let scene = widget(ui.tree(), "tabs\tScene");
    let play = widget(ui.tree(), "toggle\tPhrase playing");
    let audio = widget(ui.tree(), "toggle\tAudio output enabled");
    view.ui(
        &mut Ui::new(2, &format!("{scene} 2\n{play} 0\n{audio} 0")),
        &mut g,
    );
    assert_eq!(
        g.edits().collect::<Vec<_>>(),
        [
            format!("set\t{CONSOLE}\tscene\t2"),
            format!("set\t{SEQUENCE}\tplaying\tfalse"),
            format!("set\t{OUTPUT}\taudible\tfalse")
        ]
    );
}

#[test]
fn exact_readouts_hide_binary_float_noise_without_writing_the_graph() {
    let mut view = Perform::init();
    let mut g = graph(&format!("param\t{CONSOLE}\tlevel\t0.65\n"));
    let mut ui = Ui::new(2, "");
    view.ui(&mut ui, &mut g);
    assert!(ui.tree().contains("number\tMASTER\t0.65\t"));
    assert_eq!(g.edits().count(), 0);
}
