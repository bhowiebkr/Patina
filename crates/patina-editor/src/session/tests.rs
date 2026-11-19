use super::*;

#[test]
fn only_a_rust_file_of_a_node_goes_into_the_slot() {
    assert_eq!(slot_folder("nodes/acc/src/lib.rs").as_deref(), Some("acc"));
    assert_eq!(slot_folder("nodes/acc/Cargo.toml"), None);
    assert_eq!(slot_folder("widgets/bar/src/lib.rs"), None);
    assert_eq!(view_folder("views/map/src/lib.rs").as_deref(), Some("map"));
    assert_eq!(view_folder("nodes/map/src/lib.rs"), None);
}

#[test]
fn only_a_folder_of_the_project_panel_can_go() {
    assert_eq!(deletable_folder("nodes/switch").as_deref(), Ok("nodes/switch"));
    assert_eq!(deletable_folder("types/seg-2").as_deref(), Ok("types/seg-2"));
    assert_eq!(deletable_folder("graphs/filter").as_deref(), Ok("graphs/filter.toml"));
    for bad in ["graphs/main", "nodes/..", "nodes/a/b", "nodes/", "src/lib", "nodes", "target/debug", "nodes/a\\b"] {
        assert!(deletable_folder(bad).is_err(), "{bad}");
    }
}

#[test]
fn an_outside_change_of_a_view_closes_only_the_view() {
    let stamp = |len: u64| (None, len);
    let old: Stamps = [("views/map/src/lib.rs".to_owned(), stamp(1)), ("nodes/n/src/lib.rs".to_owned(), stamp(1))].into();
    let mut new = old.clone();
    new.insert("views/map/src/lib.rs".to_owned(), stamp(2));
    assert_eq!(outside_changes(&old, &new), (["map".to_owned()].into(), false));
    new.remove("nodes/n/src/lib.rs");
    assert!(outside_changes(&old, &new).1, "a removed file is a change");
    assert_eq!(outside_changes(&old, &old), (BTreeSet::new(), false));
}

#[test]
fn a_delete_from_outside_goes_to_the_next_command_one_time() {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().map(Result::unwrap) {
            let (name, path) = (entry.file_name(), entry.path());
            match path.is_dir() {
                true if name != "build" && name != "target" => copy(&path, &to.join(name)),
                true => {}
                false => _ = std::fs::copy(&path, to.join(name)).unwrap(),
            }
        }
    }
    let dir = std::env::temp_dir().join(format!("patina-outside-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), &dir);
    let project = Project::open(&dir).unwrap();
    let mut session = Session { project, serial: 1, trusted: true, live: false, code: BTreeMap::new(), test: None, recovery: None, recovery_error: None, checked: None, stamps: Stamps::new() };
    session.stamps = session.project.stamps();
    assert_eq!(session.take_outside(), None);
    let revision = session.project.code_revision();
    std::fs::remove_dir_all(dir.join("nodes/tape")).unwrap();
    assert_eq!(session.take_outside(), Some((BTreeSet::new(), true)));
    assert_eq!(session.project.code_revision(), revision + 1, "the Code tab reads the files again");
    assert_eq!(session.take_outside(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_a_change_that_runs_needs_the_engine() {
    let graph: GraphFile = toml::from_str(include_str!("../../../../examples/hello/graphs/main.toml")).unwrap();
    let mut moved = graph.clone();
    moved.node[0].pos = Some([500.0, 500.0]);
    assert_eq!(affects_run(&graph, &moved), None);
    let mut routed = graph.clone();
    routed.wire[0].points.push([40.0, 40.0]);
    assert_eq!(affects_run(&graph, &routed), None, "a route point changes only the drawing");
    let mut param = moved.clone();
    param.node[0].params.insert("value".to_owned(), "7".to_owned());
    assert_eq!(affects_run(&moved, &param), Some(false));
    let mut unwired = param.clone();
    unwired.wire.clear();
    assert_eq!(affects_run(&param, &unwired), Some(true));
}
