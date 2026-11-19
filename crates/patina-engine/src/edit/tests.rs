//! The tests of the edits and their undo history.

use super::*;
use crate::project::Shape;

const A: &str = "000000000000000a";
const B: &str = "000000000000000b";
const C: &str = "000000000000000c";

fn add(id: &str) -> Edit {
    Edit::AddNode { id: id.to_owned(), folder: "nodes/number".to_owned(), builtin: false, pos: [0.0, 0.0] }
}

fn no_code(_: &Disk) -> Result<Disk, String> {
    unreachable!("these edits change no code")
}

fn connect(from: &str, to: &str) -> Edit {
    Edit::Connect { from: from.to_owned(), to: to.to_owned() }
}

#[test]
fn edits_undo_and_redo() {
    let mut history = History::new(toml::from_str("format = 1").unwrap());
    history.apply(&add(A)).unwrap();
    history.apply(&add(B)).unwrap();
    assert!(history.apply(&add(B)).is_err(), "two nodes cannot have one ID");
    history.apply(&add(C)).unwrap();
    history.apply(&connect("000000000000000a.out", "000000000000000b.value")).unwrap();
    history.apply(&connect("000000000000000c.out", "000000000000000b.value")).unwrap();
    assert_eq!(history.graph.wire.len(), 1, "a second wire to one input replaces the first");
    // A cycle of wires is kept, as for a latch of two NOR gates. The plan check decides it (9.2).
    history.apply(&connect("000000000000000b.out", "000000000000000c.value")).unwrap();
    assert_eq!(history.graph.wire.len(), 2);
    assert!(history.undo(no_code).unwrap());
    let param = Edit::SetParam { id: A.to_owned(), key: "value".to_owned(), text: Some("2".to_owned()) };
    history.apply(&param).unwrap();
    assert!(history.apply(&param).is_err(), "an edit that changes nothing is not kept");
    history.apply(&Edit::RemoveNode { id: B.to_owned() }).unwrap();
    assert!(history.graph.wire.is_empty(), "the wires of a removed node go too");
    assert!(history.dirty());

    assert!(history.undo(no_code).unwrap());
    assert_eq!(history.graph.node.len(), 3);
    assert_eq!(history.graph.wire[0].from, "000000000000000c.out");
    assert!(history.redo(no_code).unwrap());
    assert_eq!(history.graph.node.len(), 2);
    assert!(!history.redo(no_code).unwrap());
    while history.undo(no_code).unwrap() {}
    assert!(!history.dirty(), "undo of all edits gives the saved graph");
    history.apply(&add(A)).unwrap();
    assert!(history.apply(&Edit::MoveNode { id: A.to_owned(), pos: [f64::NAN, 0.0] }).is_err());

    let group = Edit::Group { edits: vec![add(B), Edit::MoveNode { id: B.to_owned(), pos: [5.0, 5.0] }] };
    history.apply(&group).unwrap();
    assert!(history.undo(no_code).unwrap());
    assert_eq!(history.graph.node.len(), 1, "undo removes all edits of a group");
    let bad = Edit::Group { edits: vec![add(B), Edit::RemoveNode { id: "000000000000000c".to_owned() }] };
    assert!(history.apply(&bad).is_err());
    assert_eq!(history.graph.node.len(), 1, "a group with a bad edit changes nothing");
    let code = Edit::SetCode { path: "nodes/number/src/lib.rs".to_owned(), text: String::new() };
    assert!(history.apply(&Edit::Group { edits: vec![code] }).is_err());
}

#[test]
fn order_wires_edit() {
    let mut history = History::new(toml::from_str("format = 1").unwrap());
    history.apply(&add(A)).unwrap();
    history.apply(&add(B)).unwrap();
    let on = |before: &str, after: &str| Edit::ConnectOrder { before: before.to_owned(), after: after.to_owned() };
    let off = |before: &str, after: &str| Edit::DisconnectOrder { before: before.to_owned(), after: after.to_owned() };
    history.apply(&on(A, B)).unwrap();
    assert!(history.apply(&on(A, B)).is_err(), "no second order wire between the same nodes");
    assert!(history.apply(&on(A, A)).is_err(), "no order wire from a node to itself");
    assert!(history.apply(&on(A, C)).is_err(), "no order wire to a node that does not exist");
    let err = history.apply(&on(B, A)).unwrap_err();
    assert!(err.ends_with("nodes: 000000000000000a, 000000000000000b"), "{err}");
    assert!(history.apply(&connect("000000000000000b.out", "000000000000000a.value")).is_err(), "a wire sees the order wires");
    assert!(history.apply(&off(B, A)).is_err());
    history.apply(&off(A, B)).unwrap();
    assert!(history.graph.order.is_empty());
    assert!(history.undo(no_code).unwrap());
    assert_eq!(history.graph.order, [Order { before: A.to_owned(), after: B.to_owned() }]);
    history.apply(&Edit::RemoveNode { id: B.to_owned() }).unwrap();
    assert!(history.graph.order.is_empty(), "the order wires of a removed node go too");
}

#[test]
fn public_pins_edit_and_follow_the_nodes() {
    let mut history = History::new(toml::from_str("format = 1").unwrap());
    history.apply(&add(A)).unwrap();
    history.apply(&add(B)).unwrap();
    let pin = |key: &str, input| Edit::AddPin { key: key.to_owned(), name: "x".to_owned(), input, pos: [0.0, 0.0] };
    let to = |key: &str, end: &str| Edit::ConnectPin { key: key.to_owned(), end: end.to_owned() };
    let off = |key: &str, end: &str| Edit::DisconnectPin { key: key.to_owned(), end: end.to_owned() };
    history.apply(&pin("0000000a", true)).unwrap();
    history.apply(&pin("0000000b", false)).unwrap();
    assert!(history.apply(&pin("0000000a", false)).is_err(), "two pins cannot have one key");
    assert!(history.apply(&pin("0000000G", true)).is_err());
    assert!(history.apply(&Edit::RenamePin { key: "0000000a".to_owned(), name: " ".to_owned() }).is_err());
    let order = |key: &str, index| Edit::OrderPin { key: key.to_owned(), index };
    history.apply(&pin("0000000c", true)).unwrap();
    history.apply(&order("0000000c", 0)).unwrap();
    assert_eq!(history.graph.input.iter().map(|p| p.key.as_str()).collect::<Vec<_>>(), ["0000000c", "0000000a"]);
    assert!(history.apply(&order("0000000c", 2)).is_err(), "the inputs have no third place");
    assert!(history.apply(&order("0000000b", 1)).is_err(), "the outputs have no second place");
    assert!(history.undo(no_code).unwrap());
    assert_eq!(history.graph.input[0].key, "0000000a");
    assert!(history.undo(no_code).unwrap());
    history.apply(&connect("000000000000000a.out", "000000000000000b.value")).unwrap();
    history.apply(&to("0000000a", "000000000000000b.value")).unwrap();
    assert!(history.graph.wire.is_empty(), "the pin replaces the wire to the input");
    history.apply(&connect("000000000000000a.out", "000000000000000b.value")).unwrap();
    assert!(history.graph.input[0].to.is_empty(), "the wire replaces the pin");
    history.apply(&to("0000000a", "000000000000000a.value")).unwrap();
    history.apply(&to("0000000b", "000000000000000b.out")).unwrap();
    assert!(history.apply(&to("0000000b", "000000000000000c.out")).is_err(), "the graph has no node c");
    history.apply(&Edit::MovePin { key: "0000000b".to_owned(), pos: [9.0, 9.0] }).unwrap();
    history.apply(&Edit::RemoveNode { id: B.to_owned() }).unwrap();
    assert_eq!(history.graph.output[0].from, "", "the output of a removed node goes");
    assert!(history.apply(&off("0000000b", "000000000000000b.out")).is_err());
    history.apply(&off("0000000a", "000000000000000a.value")).unwrap();
    history.apply(&Edit::RemovePin { key: "0000000a".to_owned() }).unwrap();
    assert_eq!((history.graph.input.len(), history.graph.output[0].pos), (0, [9.0, 9.0]));
    let text = toml::to_string(&history.graph).unwrap();
    assert!(!text.contains("from"), "an output with no source has no from: {text}");
    assert_eq!(toml::from_str::<GraphFile>(&text).unwrap(), history.graph);
}

#[test]
fn a_built_in_node_has_no_folder() {
    let mut history = History::new(toml::from_str("format = 1").unwrap());
    let builtin = |folder: &str| Edit::AddNode { id: A.to_owned(), folder: folder.to_owned(), builtin: true, pos: [0.0, 0.0] };
    assert!(history.apply(&builtin("nodes/number")).is_err());
    history.apply(&builtin("delay")).unwrap();
    let node = &history.graph.node[0];
    assert_eq!((node.folder.as_deref(), node.builtin.as_deref()), (None, Some("delay")));
    let use_of = |folder: &str| Edit::AddNode { id: B.to_owned(), folder: folder.to_owned(), builtin: false, pos: [0.0, 0.0] };
    assert!(history.apply(&use_of("graphs/../main")).is_err());
    history.apply(&use_of("graphs/half")).unwrap();
    assert_eq!((history.graph.node[1].folder.as_deref(), history.graph.node[1].subgraph.as_deref()), (None, Some("half")));
}

#[test]
fn route_points_undo_and_save() {
    let mut history = History::new(toml::from_str("format = 1").unwrap());
    history.apply(&add(A)).unwrap();
    history.apply(&add(B)).unwrap();
    history.apply(&connect("000000000000000a.out", "000000000000000b.value")).unwrap();
    let route = |to: &str, points: Vec<[f64; 2]>| Edit::SetRoute { to: to.to_owned(), points };
    history.apply(&route("000000000000000b.value", vec![[10.0, 20.0], [30.0, 20.0]])).unwrap();
    let text = toml::to_string(&history.graph).unwrap();
    assert!(text.contains("points = [[10.0, 20.0], [30.0, 20.0]]"), "{text}");
    history.apply(&Edit::MoveNode { id: A.to_owned(), pos: [50.0, 50.0] }).unwrap();
    assert_eq!(history.graph.wire[0].points.len(), 2, "a move keeps the route points");
    assert!(history.apply(&route("000000000000000b.value", vec![[0.0, 0.0]; MAX_ROUTE_POINTS + 1])).is_err());
    assert!(history.apply(&route("000000000000000b.value", vec![[f64::INFINITY, 0.0]])).is_err());
    assert!(history.apply(&route("000000000000000a.value", Vec::new())).is_err(), "no wire goes to this pin");
    assert!(history.undo(no_code).unwrap() && history.undo(no_code).unwrap());
    assert!(history.graph.wire[0].points.is_empty(), "undo removes the route points");

    let look: Edit = serde_json::from_str(r#"{"kind":"SetLook","to":"000000000000000b.value","style":"orthogonal","corners":"sharp"}"#).unwrap();
    history.apply(&look).unwrap();
    let text = toml::to_string(&history.graph).unwrap();
    assert!(text.contains("style = \"orthogonal\"
corners = \"sharp\""), "{text}");
    assert!(serde_json::from_str::<Edit>(r#"{"kind":"SetLook","to":"x","style":"zigzag"}"#).is_err());
    assert!(history.undo(no_code).unwrap());
    assert_eq!(history.graph.wire[0].style, None, "undo gives the graph default again");
    history.apply(&serde_json::from_str(r#"{"kind":"SetGraphLook","style":"curved"}"#).unwrap()).unwrap();
    let text = toml::to_string(&history.graph).unwrap();
    assert!(text.starts_with("format = 1\nstyle = \"curved\"\n"), "{text}");
    history.apply(&Edit::SetGraphLook { style: Some(Style::Automatic) }).unwrap();
    assert_eq!(history.graph.style, None, "the automatic route is the default, so the file leaves it out");
    assert!(history.undo(no_code).unwrap());
    assert_eq!(history.graph.style, Some(Style::Curved));
    assert!(history.undo(no_code).unwrap());
    assert_eq!(history.graph.style, None, "undo gives the automatic route again");

    history.apply(&serde_json::from_str(r#"{"kind":"SetSymbol","symbol":{"shape":"chip","label":" CPU "}}"#).unwrap()).unwrap();
    assert_eq!(history.graph.symbol.as_ref().and_then(|s| s.label.as_deref()), Some("CPU"));
    let or = |label: &str| Edit::SetSymbol { symbol: Some(Symbol { shape: Shape::Or, label: Some(label.to_owned()), color: None, outline: Vec::new() }) };
    history.apply(&or(" ")).unwrap();
    assert_eq!(history.graph.symbol, Some(Symbol { shape: Shape::Or, label: None, color: None, outline: Vec::new() }), "a blank label shows the name");
    assert!(history.apply(&or(&"x".repeat(33))).is_err());
    // A custom shape has an outline of 3 to 32 corners in 0..=480, and only it has one (4.3).
    let custom = |color: &str, outline: &str| serde_json::from_str::<Edit>(&format!(r#"{{"kind":"SetSymbol","symbol":{{"shape":"custom","color":"{color}","outline":{outline}}}}}"#)).unwrap();
    history.apply(&custom("#AABBCC", "[[0,0],[60,0],[60,40],[0,40]]")).unwrap();
    assert_eq!(history.graph.symbol.as_ref().and_then(|s| s.color.as_deref()), Some("#aabbcc"));
    for (color, outline) in [("red", "[[0,0],[1,0],[0,1]]"), ("#000000", "[[0,0],[1,0]]"), ("#000000", "[[0,0],[1,0],[0,481]]")] {
        assert!(history.apply(&custom(color, outline)).is_err(), "{color} {outline}");
    }
    assert!(history.apply(&serde_json::from_str(r#"{"kind":"SetSymbol","symbol":{"shape":"or","outline":[[0,0],[1,0],[0,1]]}}"#).unwrap()).is_err());
    assert!(history.undo(no_code).unwrap());
    assert!(history.undo(no_code).unwrap());
    assert_eq!(history.graph.symbol.as_ref().map(|s| s.shape), Some(Shape::Chip));
    history.apply(&Edit::SetSymbol { symbol: None }).unwrap();
    assert!(!toml::to_string(&history.graph).unwrap().contains("symbol"));
    assert!(history.undo(no_code).unwrap() && history.undo(no_code).unwrap());
    assert_eq!(history.graph.symbol, None);

    let name = |name: Option<&str>| Edit::NameWire { to: "000000000000000b.value".to_owned(), name: name.map(str::to_owned) };
    history.apply(&name(Some(" addr "))).unwrap();
    assert_eq!(history.graph.wire[0].name.as_deref(), Some("addr"));
    assert!(toml::to_string(&history.graph).unwrap().contains("name = \"addr\""));
    assert!(history.apply(&name(Some(&"x".repeat(33)))).is_err());
    history.apply(&name(Some("  "))).unwrap();
    assert_eq!(history.graph.wire[0].name, None, "a blank name gives the default name");
}

#[test]
fn the_writes_of_one_gesture_are_one_step_of_undo() {
    let mut history = History::new(toml::from_str("format = 1").unwrap());
    history.apply(&add(A)).unwrap();
    let set = |v: &str| Edit::SetParam { id: A.to_owned(), key: "value".to_owned(), text: Some(v.to_owned()) };
    for v in ["1", "2", "3"] {
        history.apply_gesture(&set(v), 7).unwrap();
    }
    history.apply_gesture(&set("4"), 8).unwrap();
    assert!(history.undo(no_code).unwrap());
    assert_eq!(history.graph.node[0].params.get("value").map(String::as_str), Some("3"));
    history.apply_gesture(&set("5"), 8).unwrap();
    assert!(history.undo(no_code).unwrap(), "an undo ends the gesture, so the write after it is a new step");
    assert_eq!(history.graph.node[0].params.get("value").map(String::as_str), Some("3"));
    assert!(history.undo(no_code).unwrap());
    assert!(history.graph.node[0].params.is_empty(), "one undo removes all writes of a gesture");
    history.apply_gesture(&set("1"), 9).unwrap();
    history.apply_gesture(&Edit::SetParam { id: A.to_owned(), key: "value".to_owned(), text: None }, 9).unwrap();
    assert!(history.undo(no_code).unwrap(), "a gesture that ends at its start leaves no step");
    assert!(history.graph.node.is_empty());
}

#[test]
fn code_edits_undo_and_redo_through_the_writer() {
    let mut history = History::new(toml::from_str("format = 1").unwrap());
    let mut file = "new".to_owned();
    history.push_disk(Disk::Code { path: "nodes/a/src/lib.rs".to_owned(), text: "old".to_owned() });
    let mut write = |disk: &Disk| {
        let Disk::Code { path, text } = disk else { unreachable!() };
        assert_eq!(path, "nodes/a/src/lib.rs");
        Ok(Disk::Code { path: path.clone(), text: std::mem::replace(&mut file, text.clone()) })
    };
    assert!(history.undo(&mut write).unwrap());
    assert!(history.redo(&mut write).unwrap());
    assert!(history.undo(&mut write).unwrap());
    assert!(!history.dirty(), "a code edit does not change the graph");
    assert!(history.redo(|_| Err("the disk is full".to_owned())).is_err());
    assert!(history.can_redo(), "a step that does not write stays");
    assert_eq!(file, "old");
}

#[test]
fn a_folder_that_undo_cannot_remove_leaves_the_history() {
    let mut history = History::new(toml::from_str("format = 1").unwrap());
    history.push_disk(Disk::Folder { path: "nodes/a".to_owned(), made: true });
    assert_eq!(history.undo_folder(), Some("nodes/a"));
    let flip = |disk: &Disk| {
        let Disk::Folder { path, made } = disk else { unreachable!() };
        Ok(Disk::Folder { path: path.clone(), made: !made })
    };
    assert!(history.undo(flip).unwrap());
    assert_eq!(history.undo_folder(), None);
    assert!(history.redo(flip).unwrap());
    assert!(history.undo(|_| Err("the folder has other files".to_owned())).is_err());
    assert!(!history.can_undo() && !history.can_redo(), "the step goes");
}
