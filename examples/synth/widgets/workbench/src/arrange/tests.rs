use super::*;

fn event(tree: &str, prefix: &str, input: &str) -> String {
    format!(
        "{} {input}",
        tree.lines()
            .position(|l| l.starts_with(prefix))
            .expect(prefix)
    )
}

fn draw(view: &mut Arrange, graph: &mut Graph) -> String {
    let mut ui = Ui::new(2, "@size 1100 620");
    view.ui(&mut ui, graph);
    ui.tree().into()
}

#[test]
fn timeline_edits_the_saved_score_and_refuses_connected_score_inputs() {
    let mut view = Arrange::default();
    let mut graph = Graph::new("node\tsong\tnodes/arrangement");
    let tree = draw(&mut view, &mut graph);
    assert!(tree.contains("Arrangement / bars and beats"));
    assert_eq!(graph.edits().count(), 0);
    let move_clip = event(&tree, "timeline\tArrangement", "clip 0 0 36 28 0");
    view.ui(&mut Ui::new(2, &move_clip), &mut graph);
    let edits: Vec<_> = graph.edits().collect();
    assert_eq!(edits.len(), 1);
    let score = Score::parse(edits[0].strip_prefix("set\tsong\tscore\t").unwrap()).unwrap();
    assert_eq!(
        (
            score.tracks[0].clips[0].start,
            score.tracks[0].clips[0].length
        ),
        (36.0, 28.0)
    );
    let mut wired = Graph::new("node\tsong\tnodes/arrangement\nwire\tx.out\tsong.score");
    view.ui(&mut Ui::new(2, &move_clip), &mut wired);
    assert_eq!(wired.edits().count(), 0);
}

#[test]
fn adding_a_track_adds_its_player_clock_wires_and_master_route() {
    let mut view = Arrange::default();
    let mut graph = Graph::new(
        "node\tsong\tnodes/arrangement\nnode\tclock\tnodes/song-clock\nnode\tmaster\tnodes/stereo-output\nwire\tsong.out\tclock.score\nwire\tmix.out\tmaster.sound",
    );
    let tree = draw(&mut view, &mut graph);
    let click = event(&tree, "button\tAdd track", "1");
    view.ui(&mut Ui::new(2, &click), &mut graph);
    let edits: Vec<_> = graph.edits().collect();
    assert!(edits.iter().any(|e| e.contains("nodes/song-track")));
    assert!(edits.iter().any(|e| e.starts_with("connect\tclock.out")));
    assert!(edits.contains(&"disconnect\tmix.out\tmaster.sound"));
    let score = Score::parse(
        edits
            .iter()
            .find_map(|e| e.strip_prefix("set\tsong\tscore\t"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(score.tracks.len(), 7);
}

#[test]
fn removing_a_track_bypasses_its_mixer_and_preserves_the_other_source() {
    let mut graph = Graph::new(
        "node\tsong\tnodes/arrangement\nnode\tt\tnodes/song-track\nparam\tt\ttrack\t7\nwire\tsong.out\tt.score\nnode\tm\tnodes/stereo-mix\nwire\ta.out\tm.a\nwire\tt.out\tm.b\nwire\tm.out\tmaster.sound",
    );
    routing::remove(&mut graph, "song", 7).unwrap();
    assert_eq!(
        graph.edits().collect::<Vec<_>>(),
        ["remove\tt", "remove\tm", "connect\ta.out\tmaster.sound"]
    );
}

#[test]
fn a_piano_drag_commits_one_note_edit_at_release() {
    let mut piano = piano::Piano::default();
    let mut notes = vec![Note {
        start: 0.0,
        length: 0.5,
        key: 60,
        velocity: 0.8,
    }];
    let mut selected = 0;
    let mut ui = Ui::new(2, "@size 800 600");
    assert!(!piano.draw(&mut ui, &mut notes, 4.0, &mut selected, 0.25));
    let index = ui
        .tree()
        .lines()
        .position(|l| l.starts_with("canvas\tChromatic piano"))
        .unwrap();
    assert!(!piano.draw(
        &mut Ui::new(2, &format!("@size 800 600\n{index} down 400 80")),
        &mut notes,
        4.0,
        &mut selected,
        0.25
    ));
    assert_eq!(notes.len(), 1);
    assert!(piano.draw(
        &mut Ui::new(2, &format!("@size 800 600\n{index} up 400 80")),
        &mut notes,
        4.0,
        &mut selected,
        0.25
    ));
    assert_eq!(notes.len(), 2);
    assert!(notes[1].start >= 1.0 && notes[1].key < 128);
}

#[test]
fn playback_rate_keeps_the_source_segment_and_extends_the_song() {
    let mut score = demo();
    score.tracks[0].clips[0].content = Content::Audio {
        path: "recording.wav".into(),
        offset_seconds: 0.0,
        rate: 1.0,
    };
    score.tracks[0].clips[0].start = 112.0;
    score.tracks[0].clips[0].length = 16.0;
    let text = score.text();
    let mut graph = Graph::new(&format!(
        "node\tsong\tnodes/arrangement\nparam\tsong\tscore\t{text}"
    ));
    let mut view = Arrange::default();
    let tree = draw(&mut view, &mut graph);
    let input = event(&tree, "number\tPlayback rate", "0.5");
    view.ui(&mut Ui::new(2, &input), &mut graph);
    let edits: Vec<_> = graph.edits().collect();
    let score = Score::parse(edits[0].strip_prefix("set\tsong\tscore\t").unwrap()).unwrap();
    assert_eq!(score.tracks[0].clips[0].length, 32.0);
    assert_eq!(score.length, 144.0);
}

#[test]
fn the_piano_octave_control_uses_the_same_octaves_as_the_keys() {
    let mut view = Arrange::default();
    let mut graph = Graph::new("node\tsong\tnodes/arrangement");
    draw(&mut view, &mut graph);
    view.page = 1;
    let tree = draw(&mut view, &mut graph);
    assert!(tree.contains("slider\tLowest octave\t4\t-1\t8"));
    let input = event(&tree, "slider\tLowest octave", "8");
    view.ui(&mut Ui::new(2, &input), &mut graph);
    let tree = draw(&mut view, &mut graph);
    assert!(tree.contains("C8") && tree.contains("C9"));
    assert!(!tree.contains("C10"));
}

#[test]
fn an_invalid_edit_does_not_poison_the_next_track_edit() {
    let mut view = Arrange::default();
    let mut graph = Graph::new("node\tsong\tnodes/arrangement");
    let tree = draw(&mut view, &mut graph);
    let input = event(&tree, "text\tClip name", &"x".repeat(81));
    view.ui(&mut Ui::new(2, &input), &mut graph);
    assert_eq!(graph.edits().count(), 0);
    let tree = draw(&mut view, &mut graph);
    let input = event(&tree, "button\tAdd note clip", "1");
    view.ui(&mut Ui::new(2, &input), &mut graph);
    assert_eq!(graph.edits().count(), 1);
}

#[test]
fn automation_lane_mute_and_solo_control_the_same_track_as_the_mixer() {
    for (action, muted, solo) in [("mute 0 1", true, false), ("solo 0 1", false, true)] {
        let mut view = Arrange {
            page: 3,
            ..Default::default()
        };
        let mut graph = Graph::new("node\tsong\tnodes/arrangement");
        let tree = draw(&mut view, &mut graph);
        let input = event(&tree, "timeline\tTrack automation", action);
        view.ui(&mut Ui::new(2, &input), &mut graph);
        let edits: Vec<_> = graph.edits().collect();
        let score = Score::parse(edits[0].strip_prefix("set\tsong\tscore\t").unwrap()).unwrap();
        assert_eq!((score.tracks[0].mute, score.tracks[0].solo), (muted, solo));
    }
}
