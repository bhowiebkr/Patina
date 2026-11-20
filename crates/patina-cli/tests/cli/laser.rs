//! The laser workbench runs without a camera or a CNC controller.
use crate::common::copy;
use patina_engine::{
    Cluster, Control, Mode, Project, Toolchain,
    view::{View, snapshot},
};
use std::{fs, path::Path, time::Instant};

const SURVEY: &str = "1a5e000000000018";

fn click(cluster: &mut Cluster, control: &Control, id: &str, label: &str) -> Vec<(String, String)> {
    let tree = cluster.ui(control, id, 0, "").0.unwrap();
    let index = tree
        .lines()
        .position(|l| l == format!("button\t{label}"))
        .unwrap_or_else(|| panic!("Missing button {label}: {tree}"));
    let (_, out) = cluster.ui(control, id, 0, &format!("{index} 1"));
    assert!(out.ok, "{:?}", out.reports);
    out.params
}

#[test]
fn laser_survey_runs_in_edit_bake_and_slot_with_a_shared_data_table() {
    let root = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workbenches/laser-level"), "laser-level");
    let toolchain = Toolchain::detect().unwrap();
    let control = Control::default();
    for (mode, slot, name) in [(Mode::Edit, false, "edit"), (Mode::Bake, false, "bake"), (Mode::Edit, true, "slot")] {
        let mut project = Project::open(&root).unwrap();
        project.open_graph("automated").unwrap();
        let library = project.build_view(&toolchain, "survey-data", mode).unwrap();
        let mut view = View::load(&library, &root).unwrap();
        let mut checked = project.check().unwrap();
        if slot {
            checked.set_slot(Some("survey")).unwrap();
        }
        let mut cluster = checked.build(&toolchain, mode).unwrap();
        let mut graph = checked.graph().clone();
        configure(&mut graph, name);
        assert!(cluster.set_params(&graph).ok);
        assert!(cluster.run(&control).ok);
        for _ in 0..3 {
            let out = cluster.frame(&control);
            assert!(out.ok, "{:?}", out.reports);
        }
        for node in &graph.node {
            let (tree, out) = cluster.ui(&control, &node.id, 0, "");
            assert!(out.ok, "{:?}", out.reports);
            fs::write(root.join(format!("build/gui-{}.txt", node.id)), tree.unwrap()).unwrap();
            assert!(cluster.ui(&control, &node.id, 1, "").1.ok);
        }
        let params = click(&mut cluster, &control, SURVEY, "Start scan");
        assert!(params.iter().any(|(key, _)| key == "request"));
        graph.node.iter_mut().find(|n| n.id == SURVEY).unwrap().params.extend(params);
        assert!(cluster.set_params(&graph).ok);
        let start = Instant::now();
        let mut completed = false;
        let mut count = 0;
        for _ in 0..600 {
            let out = cluster.frame(&control);
            count += 1;
            assert!(out.ok, "{:?}", out.reports);
            if out.values.iter().any(|v| v.from == format!("{SURVEY}.out") && v.text.contains("scan complete")) {
                completed = true;
                break;
            }
        }
        assert!(
            completed,
            "{name}: scan did not finish: {}",
            cluster
                .ui(&control, SURVEY, 0, "")
                .0
                .unwrap()
                .lines()
                .filter(|l| l.starts_with("label\t") || l.starts_with("progress\t"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        eprintln!(
            "laser {name}: {count} synthetic frames / {:.2} ms per frame",
            start.elapsed().as_secs_f64() * 1000.0 / count as f64
        );
        let session = fs::read_to_string(root.join(format!("measurements/{name}.json"))).unwrap();
        assert!(session.matches("\"station\":").count() >= 11);
        check_table(&mut cluster, &control, &mut view, &graph, &root);
        let params = click(&mut cluster, &control, "1a5e00000000001a", "Export new file");
        graph.node.iter_mut().find(|n| n.id == "1a5e00000000001a").unwrap().params.extend(params);
        assert!(cluster.set_params(&graph).ok);
        assert!(cluster.frame(&control).ok);
        let csv = fs::read_to_string(root.join(format!("{name}.csv"))).unwrap();
        assert_eq!(csv.lines().count(), 10);
        assert!(csv.contains("Uncertainty (um)"));
        let panel = cluster.ui(&control, "1a5e000000000019", 0, "").0.unwrap();
        assert!(panel.contains("PEAK TO VALLEY"));
        fs::write(root.join("build/results-complete.txt"), panel).unwrap();
    }
    let (output, _, stderr) = crate::common::run(&root, &["--frames", "3"]);
    assert!(output.status.success(), "{stderr}");
}

fn configure(graph: &mut patina_engine::project::GraphFile, name: &str) {
    for n in &mut graph.node {
        match n.folder.as_deref() {
            Some("nodes/station-plan") => {
                n.params.insert("nx".into(), "3".into());
                n.params.insert("ny".into(), "3".into());
                n.params.insert("dx".into(), "2".into());
                n.params.insert("dy".into(), "2".into());
            }
            Some("nodes/scan-controller") => {
                n.params.insert("lift".into(), "0".into());
                n.params.insert("feed".into(), "6000".into());
                n.params.insert("dwell".into(), "0.1".into());
            }
            Some("nodes/survey") => {
                n.params.insert(
                    "options".into(),
                    format!(
                        r#"{{"file":"measurements/{name}.json","rule":{{"min_s":0.2,"max_s":0.2,"target_um":null}}}}"#
                    ),
                );
            }
            Some("nodes/export") => {
                n.params.insert("path".into(), format!("{name}.csv"));
            }
            _ => {}
        }
    }
}

fn check_table(
    cluster: &mut Cluster,
    control: &Control,
    view: &mut View,
    graph: &patina_engine::project::GraphFile,
    root: &Path,
) {
    let graph_text = snapshot(graph, &[], &[], &[]);
    view.ui(0, "@size 1200 240", &graph_text, Some(&mut cluster.frame_part())).unwrap();
    assert!(cluster.frame(control).ok);
    let table = view.ui(0, "@size 1200 240", &graph_text, Some(&mut cluster.frame_part())).unwrap();
    assert!(table.tree.contains("table\tMeasured stations"), "{}", table.tree);
    fs::write(root.join("build/survey-data.txt"), &table.tree).unwrap();
    let index = table.tree.lines().position(|l| l.starts_with("table\tMeasured stations")).unwrap();
    let selected = view.ui(0, &format!("{index} 1"), &graph_text, Some(&mut cluster.frame_part())).unwrap();
    assert!(selected.edits.iter().any(|e|matches!(e,patina_engine::Edit::SetParam{id,key,text:Some(text)} if id==SURVEY&&key=="options"&&text.contains("\"station\":1"))),"{:?}",selected.edits);
}
