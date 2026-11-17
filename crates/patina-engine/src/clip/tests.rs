use std::fs;

use super::*;
use crate::Edit;

const HALF: &str = "format = 1
[[node]]
id = \"00000000000000a1\"
folder = \"nodes/number\"
";

const MAIN_GRAPH: &str = "format = 1
[[node]]
id = \"000000000000000a\"
folder = \"nodes/number\"
pos = [10.0, 20.0]
[[node]]
id = \"000000000000000b\"
folder = \"nodes/show\"
pos = [100.0, 20.0]
[[node]]
id = \"000000000000000c\"
subgraph = \"half\"
pos = [50.0, 80.0]
[[node]]
id = \"000000000000000d\"
folder = \"nodes/show\"
[[wire]]
from = \"000000000000000a.out\"
to = \"000000000000000b.value\"
points = [[40.0, 30.0]]
[[wire]]
from = \"000000000000000a.out\"
to = \"000000000000000d.value\"
[[order]]
before = \"000000000000000a\"
after = \"000000000000000b\"
";

fn project(name: &str, half: &str) -> (std::path::PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("patina-clip-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    Project::create(&root, "1.94.0").unwrap();
    fs::write(root.join("graphs/half.toml"), half).unwrap();
    fs::write(root.join("graphs/main.toml"), MAIN_GRAPH).unwrap();
    let project = Project::open(&root).unwrap();
    (root, project)
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| format!("{id:0>16}")).collect()
}

fn node<'a>(project: &'a Project, id: &str) -> &'a NodeEntry {
    project.graph().node.iter().find(|n| n.id == id).unwrap()
}

#[test]
fn a_paste_in_the_same_project_gives_new_ids_at_the_place() {
    let (root, mut project) = project("same", HALF);
    assert!(project.copy(&[]).is_err() && project.copy(&ids(&["f"])).is_err(), "nothing to copy");
    let text = project.copy(&ids(&["a", "b", "c"])).unwrap();
    assert!(text.starts_with("patina-clip") && text.contains("[graphs.half]"), "{text}");
    let pasted = project.paste(&text, [200.0, 300.0], None, |_| None).unwrap();
    assert_eq!((pasted.ids.len(), pasted.warnings.len()), (3, 0));
    let [a, b, c] = [0, 1, 2].map(|i| node(&project, &pasted.ids[i]).clone());
    assert_eq!((a.pos, b.pos, c.pos), (Some([200.0, 300.0]), Some([290.0, 300.0]), Some([240.0, 360.0])));
    assert_eq!((a.folder.as_deref(), c.subgraph.as_deref()), (Some("nodes/number"), Some("half")));
    let graph = project.graph();
    // Only the wire between two copied nodes goes with them.
    let wire = graph.wire.last().unwrap();
    assert_eq!((graph.wire.len(), &wire.from, &wire.to), (3, &format!("{}.out", a.id), &format!("{}.value", b.id)));
    assert_eq!(wire.points, [[230.0, 310.0]]);
    let order = graph.order.last().unwrap();
    assert_eq!((graph.order.len(), &order.before, &order.after), (2, &a.id, &b.id));
    assert_eq!(project.unsaved().map(|g| g.0).collect::<Vec<_>>(), ["main"], "the paste uses the same subgraph");
    assert!(project.paste("hello", [0.0, 0.0], None, |_| None).unwrap_err().contains("no Patina nodes"));
    project.undo().unwrap();
    assert_eq!(project.graph().node.len(), 4, "one undo removes the paste");

    // A node of a library is a clip of one node.
    let lib = project.library_clip("half", true).unwrap();
    assert!(project.library_clip("missing", false).is_err() && project.library_clip("../x", true).is_err());
    let pasted = project.paste(&lib, [5.0, 6.0], None, |_| None).unwrap();
    let used = node(&project, &pasted.ids[0]);
    assert_eq!((used.subgraph.as_deref(), used.pos), (Some("half"), Some([5.0, 6.0])));
    let one = project.library_clip("number", false).unwrap();
    assert!(project.paste(&one, [0.0, 0.0], None, |_| None).is_ok());

    // A subgraph can not use itself.
    project.open_graph("half").unwrap();
    assert!(project.paste(&text, [0.0, 0.0], None, |_| None).unwrap_err().contains("can not use itself"));
    assert_eq!(project.graph().node.len(), 1, "a failed paste changes nothing");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_paste_into_another_project_renames_a_different_subgraph_and_imports_the_folders() {
    let (source, mut from) = project("from", HALF);
    let (root, mut project) = project("into", "format = 1\n");
    let text = from.copy(&ids(&["a", "c"])).unwrap();
    let id = from.id.clone();
    let find = |want: &str| (want == id).then(|| source.clone());
    let pasted = project.paste(&text, [0.0, 0.0], None, find).unwrap();
    assert_eq!(pasted.warnings, Vec::<String>::new(), "the node folders are the same");
    assert_eq!(node(&project, &pasted.ids[1]).subgraph.as_deref(), Some("half-2"));
    assert!(!root.join("graphs/half-2.toml").exists(), "only a save writes a graph");
    assert_eq!(project.subgraph_file("half-2").unwrap(), crate::project::read(&source.join("graphs/half.toml"), |g: &GraphFile| g.format).unwrap());
    project.save().unwrap();
    assert!(root.join("graphs/half-2.toml").is_file());

    // The second paste uses the subgraph of the first paste.
    let again = project.paste(&text, [0.0, 0.0], None, find).unwrap();
    assert_eq!(node(&project, &again.ids[1]).subgraph.as_deref(), Some("half-2"));

    // A folder that this project does not have needs the source project and the toolchain.
    from.new_node("extra").unwrap();
    from.apply(&Edit::AddNode { id: "00000000000000e1".to_owned(), folder: "nodes/extra".to_owned(), builtin: false, pos: [0.0, 0.0] }).unwrap();
    let text = from.copy(&ids(&["e1"])).unwrap();
    let count = project.graph().node.len();
    assert!(project.paste(&text, [0.0, 0.0], None, |_| None).unwrap_err().contains("Open the project"));
    assert!(project.paste(&text, [0.0, 0.0], None, find).unwrap_err().contains("toolchain"));
    assert!(!root.join("nodes/extra").exists() && project.graph().node.len() == count, "a failed paste changes nothing");
    // A folder with other files stays, with a warning.
    let show = source.join("nodes/show/src/lib.rs");
    fs::write(&show, fs::read_to_string(&show).unwrap() + "// other\n").unwrap();
    let text = from.copy(&ids(&["b"])).unwrap();
    assert_eq!(project.paste(&text, [0.0, 0.0], None, find).unwrap().warnings.len(), 1);
    // Without the source project, the folders of this project are used.
    assert_eq!(project.paste(&text, [0.0, 0.0], None, |_| None).unwrap().warnings.len(), 1);
    for dir in [&source, &root] {
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn a_bad_clip_is_refused() {
    let (root, project) = project("bad", HALF);
    let text = project.copy(&ids(&["a", "b", "c"])).unwrap();
    let bad = |from: &str, to: &str| {
        let changed = text.replace(from, to);
        assert_ne!(changed, text, "{from}");
        parse(&changed).and_then(|clip| leaves_first(&clip)).err().unwrap_or_else(|| panic!("{from} to {to}"))
    };
    assert!(bad("nodes/number", "nodes/../x").contains("nodes/"));
    assert!(bad("nodes/number", "graphs/x").contains("nodes/"));
    assert!(bad("to = \"000000000000000b.value\"", "to = \"0000000000000009.value\"").contains("does not have"));
    assert!(bad("id = \"000000000000000b\"", "id = \"000000000000000a\"").contains("repeated"));
    assert!(bad("patina-clip = 1", "patina-clip = 2").contains("version"));
    assert!(bad("graphs.half", "graphs.main").contains("main"));
    assert!(bad("graphs.half", "graphs.other").contains("no subgraph half"));
    assert!(bad("subgraph = \"half\"", "subgraph = \"Con\"").contains("Con"));
    // A subgraph that uses itself.
    let looped = format!("{text}\n[[graphs.half.node]]\nid = \"00000000000000f1\"\nsubgraph = \"half\"\n");
    assert!(parse(&looped).and_then(|clip| leaves_first(&clip)).unwrap_err().contains("uses itself"));
    fs::remove_dir_all(&root).unwrap();
}
