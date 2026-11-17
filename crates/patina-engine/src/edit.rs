//! The edits of the user and their undo history (spec 7.4).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::project::{Corners, GraphFile, NodeEntry, Order, PublicInput, PublicOutput, Style, Wire, is_id, is_key};

/// The number of edits that undo keeps. The oldest edit goes first.
const UNDO_LIMIT: usize = 1000;
/// The route points of one wire at most (6.4).
pub const MAX_ROUTE_POINTS: usize = 12;

/// One action of the user on a graph (7.4). A pin is `<node ID>.<pin key>`.
#[derive(Deserialize, Serialize, Clone, Debug)]
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
    /// Adds an order wire: node `before` runs before node `after`. It carries no data (9.2).
    ConnectOrder { before: String, after: String },
    DisconnectOrder { before: String, after: String },
    /// Replaces the route points of the wire to the input `to` (15.9). An empty list removes them.
    SetRoute { to: String, points: Vec<[f64; 2]> },
    /// Sets the path style and the corners of the wire to the input `to` (15.9). `None` uses the
    /// graph default.
    SetLook { to: String, style: Option<Style>, corners: Option<Corners> },
    /// Sets the bus name of the wire to the input `to` (15.9). `None` or a blank name gives the
    /// default name.
    NameWire { to: String, name: Option<String> },
    /// `None` removes the text, so the input gets its default value (4.8).
    SetParam { id: String, key: String, text: Option<String> },
    /// Applies the edits in order as one step of undo, for example a move of many nodes.
    Group { edits: Vec<Edit> },
    /// Replaces the text of a file in a node folder (6.2). The path is relative to the project
    /// root, for example `nodes/number/src/lib.rs`. The file must exist.
    SetCode { path: String, text: String },
    /// Adds a public pin to the open graph (6.4). The key is 8 lowercase hexadecimal characters.
    AddPin { key: String, name: String, input: bool, pos: [f64; 2] },
    /// Also removes the wires of the pin in the graph.
    RemovePin { key: String },
    MovePin { key: String, pos: [f64; 2] },
    RenamePin { key: String, name: String },
    /// Moves a public pin to the place `index` of its list, the inputs or the outputs. The order is
    /// the pin order of each use (6.4).
    OrderPin { key: String, index: usize },
    /// Connects a public input to the inner input `end`, which loses its other wire, or a public
    /// output to the inner output `end`.
    ConnectPin { key: String, end: String },
    DisconnectPin { key: String, end: String },
}

/// One entry of undo or redo: a full copy of the graph, or a change of the project files, before
/// or after an edit. Full copies keep undo and redo always correct.
enum Step {
    Graph(GraphFile),
    Disk(Disk),
}

/// A change of the project files that undo and redo reverse (6.2, 15.2). A path is relative to
/// the project root.
#[derive(Debug, PartialEq)]
pub(crate) enum Disk {
    /// The text of a code file, for example `nodes/number/src/lib.rs`.
    Code { path: String, text: String },
    /// A new crate folder, for example `nodes/number`. `made` is true when the folder exists, so
    /// the step removes it.
    Folder { path: String, made: bool },
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

    /// A new graph with no file yet. It stays dirty, so the next save writes it (15.10).
    pub fn unsaved(graph: GraphFile) -> History {
        let saved = GraphFile { format: 0, node: Vec::new(), wire: Vec::new(), order: Vec::new(), collar: Vec::new(), cable: Vec::new(), input: Vec::new(), output: Vec::new() };
        History { saved, graph, undo: Vec::new(), redo: Vec::new() }
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

    /// Records a change of the files that is written: the old text of a code file, or a new folder.
    pub fn push_disk(&mut self, disk: Disk) {
        self.push(Step::Disk(disk));
    }

    /// The folder that the next undo removes.
    pub fn undo_folder(&self) -> Option<&str> {
        match self.undo.last() {
            Some(Step::Disk(Disk::Folder { path, made: true })) => Some(path),
            _ => None,
        }
    }

    fn push(&mut self, step: Step) {
        if self.undo.len() == UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(step);
        self.redo.clear();
    }

    /// Removes the last edit. Returns false when there is no edit to remove. `write` writes a
    /// change of the files and returns the change that reverses it.
    pub fn undo(&mut self, write: impl FnOnce(&Disk) -> Result<Disk, String>) -> Result<bool, String> {
        step(&mut self.graph, &mut self.undo, &mut self.redo, write)
    }

    /// Applies the last removed edit again. Returns false when there is no such edit.
    pub fn redo(&mut self, write: impl FnOnce(&Disk) -> Result<Disk, String>) -> Result<bool, String> {
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

/// Moves the last step of `from` to `to`. A code step that does not write stays in `from`. A
/// folder step that does not write goes, so undo goes on with the edits before it.
fn step(
    graph: &mut GraphFile,
    from: &mut Vec<Step>,
    to: &mut Vec<Step>,
    write: impl FnOnce(&Disk) -> Result<Disk, String>,
) -> Result<bool, String> {
    let Some(step) = from.pop() else { return Ok(false) };
    let back = match step {
        Step::Graph(other) => Step::Graph(std::mem::replace(graph, other)),
        Step::Disk(disk) => match write(&disk) {
            Ok(back) => Step::Disk(back),
            Err(e) => {
                if let Disk::Code { .. } = disk {
                    from.push(Step::Disk(disk));
                }
                return Err(e);
            }
        },
    };
    to.push(back);
    Ok(true)
}

pub(crate) fn change(graph: &mut GraphFile, edit: &Edit) -> Result<(), String> {
    match edit {
        Edit::AddNode { id, folder, builtin, pos } => {
            if !is_id(id) {
                return Err(format!("the node ID {id:?} is not 16 lowercase hexadecimal characters"));
            }
            if graph.node.iter().any(|n| &n.id == id) {
                return Err(format!("the graph has a node with the ID {id}"));
            }
            if *builtin && crate::node::Builtin::parse(folder).is_none() {
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
            for pin in &mut graph.input {
                pin.to.retain(|t| node_of(t) != id);
            }
            for pin in graph.output.iter_mut().filter(|p| node_of(&p.from) == id) {
                pin.from.clear();
            }
        }
        Edit::MoveNode { id, pos } => {
            let index = find(graph, id)?;
            graph.node[index].pos = Some(finite(pos)?);
        }
        Edit::Connect { from, to } => {
            check_end(graph, from)?;
            check_end(graph, to)?;
            if let Some(ids) = cycle(graph, node_of(from), node_of(to)) {
                return Err(format!("the wire makes a cycle through these nodes: {}", ids.join(", ")));
            }
            free_input(graph, to);
            graph.wire.push(Wire { from: from.clone(), to: to.clone(), name: None, style: None, corners: None, points: Vec::new() });
        }
        Edit::Disconnect { from, to } => {
            let count = graph.wire.len();
            graph.wire.retain(|w| !(&w.from == from && &w.to == to));
            if graph.wire.len() == count {
                return Err(format!("no wire goes from {from} to {to}"));
            }
        }
        Edit::ConnectOrder { before, after } => {
            find(graph, before)?;
            find(graph, after)?;
            if graph.order.iter().any(|o| &o.before == before && &o.after == after) {
                return Err(format!("an order wire already goes from {before} to {after}"));
            }
            if let Some(ids) = cycle(graph, before, after) {
                return Err(format!("the order wire makes a cycle through these nodes: {}", ids.join(", ")));
            }
            graph.order.push(Order { before: before.clone(), after: after.clone() });
        }
        Edit::DisconnectOrder { before, after } => {
            let count = graph.order.len();
            graph.order.retain(|o| !(&o.before == before && &o.after == after));
            if graph.order.len() == count {
                return Err(format!("no order wire goes from {before} to {after}"));
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
        Edit::NameWire { to, name } => {
            let name = name.as_deref().map(str::trim).filter(|n| !n.is_empty());
            if name.is_some_and(|n| n.len() > 32 || n.chars().any(char::is_control)) {
                return Err("a wire name has 32 bytes at most and no control characters".to_owned());
            }
            let wire = graph.wire.iter_mut().find(|w| &w.to == to).ok_or(format!("no wire goes to {to}"))?;
            wire.name = name.map(str::to_owned);
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
        Edit::AddPin { key, name, input, pos } => {
            if !is_key(key) {
                return Err(format!("the pin key {key:?} is not 8 lowercase hexadecimal characters"));
            }
            if graph.input.iter().any(|p| &p.key == key) || graph.output.iter().any(|p| &p.key == key) {
                return Err(format!("the graph has a public pin with the key {key}"));
            }
            let (name, pos) = (pin_name(name)?, finite(pos)?);
            match input {
                true => graph.input.push(PublicInput { key: key.clone(), name, pos, to: Vec::new() }),
                false => graph.output.push(PublicOutput { key: key.clone(), name, pos, from: String::new() }),
            }
        }
        Edit::RemovePin { key } => {
            let count = graph.input.len() + graph.output.len();
            graph.input.retain(|p| &p.key != key);
            graph.output.retain(|p| &p.key != key);
            if graph.input.len() + graph.output.len() == count {
                return Err(format!("the graph has no public pin {key}"));
            }
        }
        Edit::MovePin { key, pos } => *pin_mut(graph, key)?.1 = finite(pos)?,
        Edit::RenamePin { key, name } => *pin_mut(graph, key)?.0 = pin_name(name)?,
        Edit::OrderPin { key, index } => match graph.input.iter().position(|p| &p.key == key) {
            Some(at) => move_to(&mut graph.input, at, *index)?,
            None => {
                let at = graph.output.iter().position(|p| &p.key == key).ok_or(format!("the graph has no public pin {key}"))?;
                move_to(&mut graph.output, at, *index)?;
            }
        },
        Edit::ConnectPin { key, end } => {
            check_end(graph, end)?;
            if let Some(pin) = graph.output.iter_mut().find(|p| &p.key == key) {
                pin.from.clone_from(end);
            } else {
                let index = graph.input.iter().position(|p| &p.key == key).ok_or(format!("the graph has no public pin {key}"))?;
                free_input(graph, end);
                graph.input[index].to.push(end.clone());
            }
        }
        Edit::DisconnectPin { key, end } => {
            let none = || format!("no wire goes from the public pin {key} to {end}");
            if let Some(pin) = graph.output.iter_mut().find(|p| &p.key == key) {
                if &pin.from != end {
                    return Err(none());
                }
                pin.from.clear();
            } else {
                let pin = graph.input.iter_mut().find(|p| &p.key == key).ok_or(format!("the graph has no public pin {key}"))?;
                let count = pin.to.len();
                pin.to.retain(|t| t != end);
                if pin.to.len() == count {
                    return Err(none());
                }
            }
        }
    }
    Ok(())
}

/// Checks that the wire end `<node ID>.<pin key>` names a node of the graph.
fn check_end(graph: &GraphFile, end: &str) -> Result<(), String> {
    let (id, key) = end.split_once('.').ok_or(format!("the pin {end:?} is not <node ID>.<pin key>"))?;
    find(graph, id)?;
    if key.is_empty() {
        return Err(format!("the pin {end:?} has no key"));
    }
    Ok(())
}

/// Removes the wire and the public input that go to the input `to`: an input has one source.
fn free_input(graph: &mut GraphFile, to: &str) {
    graph.wire.retain(|w| w.to != to);
    for pin in &mut graph.input {
        pin.to.retain(|t| t != to);
    }
}

/// The name and the position of the public pin `key`.
fn pin_mut<'a>(graph: &'a mut GraphFile, key: &str) -> Result<(&'a mut String, &'a mut [f64; 2]), String> {
    if let Some(i) = graph.input.iter().position(|p| p.key == key) {
        let pin = &mut graph.input[i];
        return Ok((&mut pin.name, &mut pin.pos));
    }
    let pin = graph.output.iter_mut().find(|p| p.key == key).ok_or(format!("the graph has no public pin {key}"))?;
    Ok((&mut pin.name, &mut pin.pos))
}

fn move_to<T>(list: &mut Vec<T>, from: usize, to: usize) -> Result<(), String> {
    if to >= list.len() {
        return Err(format!("the list of {} pins has no place {to}", list.len()));
    }
    let pin = list.remove(from);
    list.insert(to, pin);
    Ok(())
}

fn pin_name(name: &str) -> Result<String, String> {
    match name.trim().is_empty() || name.chars().count() > 64 {
        true => Err("a pin name has 1 to 64 characters and is not blank".to_owned()),
        false => Ok(name.to_owned()),
    }
}

fn find(graph: &GraphFile, id: &str) -> Result<usize, String> {
    graph.node.iter().position(|n| n.id == id).ok_or(format!("the graph has no node {id}"))
}

/// The nodes of the cycle that a new wire from node `from` to node `to` makes (9.2), from `to`
/// along the wires and the order wires to `from`. None when the wire makes no cycle.
pub(crate) fn cycle<'a>(graph: &'a GraphFile, from: &'a str, to: &'a str) -> Option<Vec<&'a str>> {
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

pub(crate) fn node_of(end: &str) -> &str {
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

        let name = |name: Option<&str>| Edit::NameWire { to: "000000000000000b.value".to_owned(), name: name.map(str::to_owned) };
        history.apply(&name(Some(" addr "))).unwrap();
        assert_eq!(history.graph.wire[0].name.as_deref(), Some("addr"));
        assert!(toml::to_string(&history.graph).unwrap().contains("name = \"addr\""));
        assert!(history.apply(&name(Some(&"x".repeat(33)))).is_err());
        history.apply(&name(Some("  "))).unwrap();
        assert_eq!(history.graph.wire[0].name, None, "a blank name gives the default name");
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
}
