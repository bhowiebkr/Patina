use super::*;

fn event(tree: &str, prefix: &str, value: &str) -> String {
    format!(
        "{} {value}",
        tree.lines()
            .position(|s| s.starts_with(prefix))
            .expect(prefix)
    )
}

#[test]
fn a_pattern_cell_edits_the_selected_sequence_and_respects_wires() {
    let mut view = Patterns::default();
    let lines = "node\ta\tnodes/sequence\nnode\tb\tnodes/sequence\nselected\tb\nparam\tb\tpattern\t0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0";
    let mut g = Graph::new(lines);
    let mut ui = Ui::new(2, "@size 900 190");
    view.ui(&mut ui, &mut g);
    let press = event(ui.tree(), "canvas\tPhrase steps", "down 50 25");
    view.ui(&mut Ui::new(2, &format!("@size 900 190\n{press}")), &mut g);
    assert_eq!(
        g.edits().collect::<Vec<_>>(),
        ["set\tb\tpattern\t1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0"]
    );
    let mut g = Graph::new(&format!("{lines}\nwire\tx.out\tb.pattern"));
    view.ui(&mut Ui::new(2, &format!("@size 900 190\n{press}")), &mut g);
    assert_eq!(g.edits().count(), 0);
}

#[test]
fn a_route_connects_real_pins_without_replacing_an_existing_source() {
    let mut v = Modulation::default();
    let mut g = Graph::new("node\ta\tnodes/lfo\nnode\tb\tnodes/svf");
    let mut ui = Ui::new(2, "");
    v.ui(&mut ui, &mut g);
    let open = event(ui.tree(), "section\tConnect modulation", "1");
    let mut ui = Ui::new(2, &open);
    v.ui(&mut ui, &mut g);
    let mut ui = Ui::new(2, "");
    v.ui(&mut ui, &mut g);
    let click = event(ui.tree(), "button\tConnect route", "1");
    v.ui(&mut Ui::new(2, &click), &mut g);
    assert_eq!(g.edits().collect::<Vec<_>>(), ["connect\ta.out\tb.cv"]);
    let mut g = Graph::new("node\ta\tnodes/lfo\nnode\tb\tnodes/svf\nwire\tx.out\tb.cv");
    let mut ui = Ui::new(2, "");
    v.ui(&mut ui, &mut g);
    assert!(!ui.tree().contains("button\tConnect route"));
}

#[test]
fn the_learn_view_locates_a_node_without_changing_sound() {
    let mut v = Learn::default();
    let mut g = Graph::new(&format!("node\t{}\tnodes/sequence", parameters::SEQUENCE));
    let mut ui = Ui::new(2, "@size 220 400");
    v.ui(&mut ui, &mut g);
    assert_eq!(g.edits().count(), 0);
    v.ui(
        &mut Ui::new(2, &event(ui.tree(), "button\tLocate node", "1")),
        &mut g,
    );
    assert_eq!(
        g.edits().collect::<Vec<_>>(),
        [format!("select\t{}", parameters::SEQUENCE)]
    );
}
