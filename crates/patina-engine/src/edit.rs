//! The edits of the user and their undo history (spec 7.4).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Deserialize;

use crate::project::{Corners, GraphFile, NodeEntry, Style, Wire, is_id};

/// The number of edits that undo keeps. The oldest edit goes first.
const UNDO_LIMIT: usize = 1000;
/// The route points of one wire at most (6.4).
pub const MAX_ROUTE_POINTS: usize = 12;

/// One action of the user on a graph (7.4). A pin is `<node ID>.<pin key>`.
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Edit {
    /// `folder` is the name of a built-in node when `builtin` is true (9.2).
    AddNode {
        id: String,
        folder: String,
        #[serde(default)]
        builtin: bool,
        pos: [f64; 2],
    },
    /// Also removes the wires and the order wires of the node.
    RemoveNode { id: String },
    MoveNode { id: String, pos: [f64; 2] },
    /// Replaces the wire that goes to the same input (6.4).
    Connect { from: String, to: String },
    Disconnect { from: String, to: String },
    /// Replaces the route points of the wire to the input `to` (15.9). An empty list removes them.
    SetRoute { to: String, points: Vec<[f64; 2]> },
    /// Sets the path style and the corners of the wire to the input `to` (15.9). `None` uses the
    /// graph default.
    SetLook { to: String, style: Option<Style>, corners: Option<Corners> },
    /// `None` removes the text, so the input gets its default value (4.8).
    SetParam { id: String, key: String, text: Option<String> },
    /// Applies the edits in order as one step of undo, for example a move of many nodes.
    Group { edits: Vec<Edit> },
    /// Replaces the text of a file in a node folder (6.2). The path is relative to the project
    /// root, for example `nodes/number/src/lib.rs`. The file must exist.
    SetCode { path: String, text: String },
}

/// One entry of undo or redo: a full copy of the graph, or the text of a code file, before or
/// after an edit. Full copies keep undo and redo always correct.
enum Step {
    Graph(GraphFile),
    Code { path: String, text: String },
}

pub(crate) struct History {
    pub graph: GraphFile,
    saved: GraphFile,
    undo: Vec<Step>,
    redo: Vec<Step>,
}

impl History {
    pub fn new(graph: GraphFile) -> History {
        History { saved: graph.clone(), graph, undo: Vec::new(), redo: Vec::new() }
    }

    /// Applies the edit. An edit that is not valid or changes nothing does not go into the history.
    pub fn apply(&mut self, edit: &Edit) -> Result<(), String> {
        let mut next = self.graph.clone();
        change(&mut next, edit)?;
        if next == self.graph {
            return Err("the edit changes nothing".to_owned());
        }
        self.replace(next);
        Ok(())
    }

    /// Replaces the graph as one step of undo, for example with the graph of a recovery file
    /// (15.10). The same graph changes nothing.
    pub fn replace(&mut self, next: GraphFile) {
        if next != self.graph {
            let old = std::mem::replace(&mut self.graph, next);
            self.push(Step::Graph(old));
        }
    }

    /// Records a code edit that is written. `text` is the old text of the file.
    pub fn push_code(&mut self, path: String, text: String) {
        self.push(Step::Code { path, text });
    }

    fn push(&mut self, step: Step) {
        if self.undo.len() == UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(step);
        self.redo.clear();
    }

    /// Removes the last edit. Returns false when there is no edit to remove. `write` writes a
    /// code file and returns its old text.
    pub fn undo(&mut self, write: impl FnOnce(&str, &str) -> Result<String, String>) -> Result<bool, String> {
        step(&mut self.graph, &mut self.undo, &mut self.redo, write)
    }

    /// Applies the last removed edit again. Returns false when there is no such edit.
    pub fn redo(&mut self, write: impl FnOnce(&str, &str) -> Result<String, String>) -> Result<bool, String> {
        step(&mut self.graph, &mut self.redo, &mut self.undo, write)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// True when the graph is not the same as the saved graph.
    pub fn dirty(&self) -> bool {
        self.graph != self.saved
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.graph.clone();
    }
}

/// Moves the last step of `from` to `to`. A step that does not write stays in `from`.
fn step(
    graph: &mut GraphFile,
    from: &mut Vec<Step>,
    to: &mut Vec<Step>,
    write: impl FnOnce(&str, &str) -> Result<String, String>,
) -> Result<bool, String> {
    let Some(step) = from.pop() else { return Ok(false) };
    let back = match step {
        Step::Graph(other) => Step::Graph(std::mem::replace(graph, other)),
        Step::Code { path, text } => match write(&path, &text) {
            Ok(old) => Step::Code { path, text: old },
            Err(e) => {
                from.push(Step::Code { path, text });
                return Err(e);
            }
        },
    };
    to.push(back);
    Ok(true)
}

fn change(graph: &mut GraphFile, edit: &Edit) -> Result<(), String> {
    match edit {
        Edit::AddNode { id, folder, builtin, pos } => {
            if !is_id(id) {
                return Err(format!("the node ID {id:?} is not 16 lowercase hexadecimal characters"));
            }
            if graph.node.iter().any(|n| &n.id == id) {
                return Err(format!("the graph has a node with the ID {id}"));
            }
            if *builtin && folder != crate::plan::DELAY {
                return Err(format!("Patina has no built-in node {folder:?} (spec 9.2)"));
            }
            // The folder `graphs/<name>` adds a use of a subgraph (6.1).
            let subgraph = folder.strip_prefix("graphs/").filter(|_| !builtin);
            if let Some(name) = subgraph {
                crate::plan::check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
            }
            graph.node.push(NodeEntry {
                id: id.clone(),
                folder: (!builtin && subgraph.is_none()).then(|| folder.clone()),
                subgraph: subgraph.map(str::to_owned),
                builtin: builtin.then(|| folder.clone()),
                pos: Some(finite(pos)?),
                domain: None,
                settings: None,
                params: Default::default(),
            });
        }
        Edit::RemoveNode { id } => {
            let index = find(graph, id)?;
            graph.node.remove(index);
            graph.wire.retain(|w| node_of(&w.from) != id && node_of(&w.to) != id);
            graph.order.retain(|o| &o.before != id && &o.after != id);
        }
        Edit::MoveNode { id, pos } => {
            let index = find(graph, id)?;
            graph.node[index].pos = Some(finite(pos)?);
        }
        Edit::Connect { from, to } => {
            for end in [from, to] {
                let (id, key) = end.split_once('.').ok_or(format!("the pin {end:?} is not <node ID>.<pin key>"))?;
                find(graph, id)?;
                if key.is_empty() {
                    return Err(format!("the pin {end:?} has no key"));
                }
            }
            if let Some(ids) = cycle(graph, node_of(from), node_of(to)) {
                return Err(format!("the wire makes a cycle through these nodes: {}", ids.join(", ")));
            }
            graph.wire.retain(|w| &w.to != to);
            graph.wire.push(Wire { from: from.clone(), to: to.clone(), style: None, corners: None, points: Vec::new() });
        }
        Edit::Disconnect { from, to } => {
            let count = graph.wire.len();
            graph.wire.retain(|w| !(&w.from == from && &w.to == to));
            if graph.wire.len() == count {
                return Err(format!("no wire goes from {from} to {to}"));
            }
        }
        Edit::SetRoute { to, points } => {
            if points.len() > MAX_ROUTE_POINTS {
                return Err(format!("a wire has {MAX_ROUTE_POINTS} route points at most"));
            }
            for p in points {
                finite(p)?;
            }
            let wire = graph.wire.iter_mut().find(|w| &w.to == to).ok_or(format!("no wire goes to {to}"))?;
            wire.points.clone_from(points);
        }
        Edit::SetLook { to, style, corners } => {
            let wire = graph.wire.iter_mut().find(|w| &w.to == to).ok_or(format!("no wire goes to {to}"))?;
            (wire.style, wire.corners) = (*style, *corners);
        }
        Edit::SetParam { id, key, text } => {
            let index = find(graph, id)?;
            let params = &mut graph.node[index].params;
            match text {
                Some(text) => params.insert(key.clone(), text.clone()),
                None => params.remove(key),
            };
        }
        Edit::Group { edits } => {
            for edit in edits {
                change(graph, edit)?;
            }
        }
        // `Project::apply` writes a code edit. Only a group gets one here.
        Edit::SetCode { .. } => return Err("a group cannot hold a code edit".to_owned()),
    }
    Ok(())
}

fn find(graph: &GraphFile, id: &str) -> Result<usize, String> {
    graph.node.iter().position(|n| n.id == id).ok_or(format!("the graph has no node {id}"))
}

/// The nodes of the cycle that a new wire from node `from` to node `to` makes (9.2), from `to`
/// along the wires and the order wires to `from`. None when the wire makes no cycle.
fn cycle<'a>(graph: &'a GraphFile, from: &'a str, to: &'a str) -> Option<Vec<&'a str>> {
    let links: Vec<(&str, &str)> = graph
        .wire
        .iter()
        .map(|w| (node_of(&w.from), node_of(&w.to)))
        .chain(graph.order.iter().map(|o| (o.before.as_str(), o.after.as_str())))
        .collect();
    let mut back = BTreeMap::new();
    let mut seen = BTreeSet::from([to]);
    let mut queue = VecDeque::from([to]);
    while let Some(node) = queue.pop_front() {
        if node == from {
            let mut path = vec![node];
            while let Some(&prev) = back.get(path[path.len() - 1]) {
                path.push(prev);
            }
            path.reverse();
            return Some(path);
        }
        for &(a, b) in &links {
            if a == node && seen.insert(b) {
                back.insert(b, node);
                queue.push_back(b);
            }
        }
    }
    None
}

fn node_of(end: &str) -> &str {
    end.split_once('.').map_or(end, |(id, _)| id)
}

fn finite(pos: &[f64; 2]) -> Result<[f64; 2], String> {
    if pos.iter().all(|v| v.is_finite()) { Ok(*pos) } else { Err("the position is not a finite number".to_owned()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "000000000000000a";
    const B: &str = "000000000000000b";
    const C: &str = "000000000000000c";

    fn add(id: &str) -> Edit {
        Edit::AddNode { id: id.to_owned(), folder: "nodes/number".to_owned(), builtin: false, pos: [0.0, 0.0] }
    }

    fn no_code(_: &str, _: &str) -> Result<String, String> {
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
        let err = history.apply(&connect("000000000000000b.out", "000000000000000c.value")).unwrap_err();
        assert!(err.ends_with("nodes: 000000000000000c, 000000000000000b"), "{err}");
        assert!(history.apply(&connect("000000000000000b.out", "000000000000000b.value")).is_err());
        assert_eq!(history.graph.wire.len(), 1, "a wire that makes a cycle changes nothing");
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
    }

    #[test]
    fn code_edits_undo_and_redo_through_the_writer() {
        let mut history = History::new(toml::from_str("format = 1").unwrap());
        let mut file = "new".to_owned();
        history.push_code("nodes/a/src/lib.rs".to_owned(), "old".to_owned());
        let mut write = |path: &str, text: &str| {
            assert_eq!(path, "nodes/a/src/lib.rs");
            Ok(std::mem::replace(&mut file, text.to_owned()))
        };
        assert!(history.undo(&mut write).unwrap());
        assert!(history.redo(&mut write).unwrap());
        assert!(history.undo(&mut write).unwrap());
        assert!(!history.dirty(), "a code edit does not change the graph");
        assert!(history.redo(|_, _| Err("the disk is full".to_owned())).is_err());
        assert!(history.can_redo(), "a step that does not write stays");
        assert_eq!(file, "old");
    }
}
