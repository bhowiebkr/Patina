//! The edits of the user and their undo history (spec 7.4).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::project::{Corners, GraphFile, Group, NodeEntry, Note, Order, PublicInput, PublicOutput, Shape, Style, Symbol, Wire, is_id, is_key};

mod groups;
pub(crate) use groups::{check_group, check_note, used};

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
    /// Sets the path style of the wires with no style of their own (15.9). `None` or `Automatic`
    /// gives the automatic route.
    SetGraphLook { style: Option<Style> },
    /// Sets the symbol of each use of the open subgraph (4.3). `None` shows the box.
    SetSymbol { symbol: Option<Symbol> },
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
    /// Adds the group, or replaces the group with the same ID (15.12).
    SetGroup { group: Group },
    RemoveGroup { id: String },
    /// Adds the note, or replaces the note with the same ID (15.12).
    SetNote { note: Note },
    RemoveNote { id: String },
}

/// One entry of undo or redo: a full copy of the graph, or a change of the project files, before
/// or after an edit. Full copies keep undo and redo always correct.
enum Step {
    Graph(Box<GraphFile>),
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
    /// The gesture of the last step of undo, while no other step came after it (4.3).
    gesture: Option<u64>,
}

impl History {
    pub fn new(graph: GraphFile) -> History {
        History { saved: graph.clone(), graph, undo: Vec::new(), redo: Vec::new(), gesture: None }
    }

    /// A new graph with no file yet. It stays dirty, so the next save writes it (15.10).
    pub fn unsaved(graph: GraphFile) -> History {
        let saved = GraphFile::default();
        History { saved, graph, undo: Vec::new(), redo: Vec::new(), gesture: None }
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

    /// Applies a parameter write of a node GUI (4.3). The writes of one gesture, for example the
    /// moves of a canvas drag, go into one step of undo. A gesture that ends at its start removes it.
    pub fn apply_gesture(&mut self, edit: &Edit, gesture: u64) -> Result<(), String> {
        if self.gesture != Some(gesture) {
            self.apply(edit)?;
            self.gesture = Some(gesture);
            return Ok(());
        }
        let mut next = self.graph.clone();
        change(&mut next, edit)?;
        if next == self.graph {
            return Err("the edit changes nothing".to_owned());
        }
        if matches!(self.undo.last(), Some(Step::Graph(old)) if **old == next) {
            self.undo.pop();
            self.gesture = None;
        }
        self.graph = next;
        Ok(())
    }

    /// Replaces the graph as one step of undo, for example with the graph of a recovery file
    /// (15.10). The same graph changes nothing.
    pub fn replace(&mut self, next: GraphFile) {
        if next != self.graph {
            let old = std::mem::replace(&mut self.graph, next);
            self.push(Step::Graph(Box::new(old)));
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
        self.gesture = None;
    }

    /// Removes the last edit. Returns false when there is no edit to remove. `write` applies a
    /// change of the files or of the unsaved code and returns the change that reverses it.
    pub fn undo(&mut self, write: impl FnOnce(&Disk) -> Result<Disk, String>) -> Result<bool, String> {
        self.gesture = None;
        step(&mut self.graph, &mut self.undo, &mut self.redo, write)
    }

    /// Applies the last removed edit again. Returns false when there is no such edit.
    pub fn redo(&mut self, write: impl FnOnce(&Disk) -> Result<Disk, String>) -> Result<bool, String> {
        self.gesture = None;
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

    /// True for a graph of `unsaved` that no save wrote yet.
    pub fn is_new(&self) -> bool {
        self.saved.format == 0
    }

    /// True when the graph has edits of its own: undo or redo has a step.
    pub fn has_steps(&self) -> bool {
        !self.undo.is_empty() || !self.redo.is_empty()
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
        Step::Graph(other) => Step::Graph(Box::new(std::mem::replace(graph, *other))),
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
        Edit::AddNode { id, folder, builtin, pos } => add_node(graph, id, folder, *builtin, pos)?,
        Edit::RemoveNode { id } => remove_node(graph, id)?,
        Edit::MoveNode { id, pos } => {
            let index = find(graph, id)?;
            graph.node[index].pos = Some(finite(pos)?);
        }
        Edit::Connect { from, to } => {
            check_end(graph, from)?;
            check_end(graph, to)?;
            free_input(graph, to);
            graph.wire.push(Wire { from: from.clone(), to: to.clone(), name: None, style: None, corners: None, points: Vec::new() });
            // A cycle of gates can run, and the plan check decides each other cycle (9.2). An order
            // wire in a cycle can never run.
            if let Some(ids) = graph.order.iter().find_map(|o| cycle(graph, &o.before, &o.after)) {
                return Err(format!("the wire makes a cycle with an order wire through these nodes: {}", ids.join(", ")));
            }
        }
        Edit::Disconnect { from, to } => {
            let count = graph.wire.len();
            graph.wire.retain(|w| !(&w.from == from && &w.to == to));
            if graph.wire.len() == count {
                return Err(format!("no wire goes from {from} to {to}"));
            }
        }
        Edit::ConnectOrder { before, after } => connect_order(graph, before, after)?,
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
        Edit::SetGraphLook { style } => {
            graph.style = style.filter(|s| *s != Style::Automatic);
        }
        Edit::SetSymbol { symbol } => graph.symbol = symbol_of(symbol)?,
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
        Edit::AddPin { key, name, input, pos } => add_pin(graph, key, name, *input, pos)?,
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
        Edit::ConnectPin { key, end } => connect_pin(graph, key, end)?,
        Edit::DisconnectPin { key, end } => disconnect_pin(graph, key, end)?,
        Edit::SetGroup { group } => groups::set_group(graph, group)?,
        Edit::RemoveGroup { id } => groups::remove_group(graph, id)?,
        Edit::SetNote { note } => groups::set_note(graph, note)?,
        Edit::RemoveNote { id } => groups::remove_note(graph, id)?,
    }
    Ok(())
}

/// Checks a symbol for `Edit::SetSymbol`. A blank label shows the graph name (4.3).
fn symbol_of(symbol: &Option<Symbol>) -> Result<Option<Symbol>, String> {
    let mut symbol = symbol.clone();
    if let Some(s) = &mut symbol {
        s.label = s.label.as_deref().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned);
        if s.label.as_deref().is_some_and(|l| l.len() > 32 || l.chars().any(char::is_control)) {
            return Err("a symbol label has 32 bytes at most and no control characters".to_owned());
        }
        if let Some(c) = &mut s.color {
            if !c.strip_prefix('#').is_some_and(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit())) {
                return Err(format!("the symbol color {c:?} is not \"#rrggbb\""));
            }
            c.make_ascii_lowercase();
        }
        let custom = s.shape == Shape::Custom;
        if custom != !s.outline.is_empty() || (custom && !(3..=32).contains(&s.outline.len())) {
            return Err("only the custom shape has an outline, of 3 to 32 corners".to_owned());
        }
        if s.outline.iter().flatten().any(|v| !(0.0..=480.0).contains(v)) {
            return Err("each corner of a symbol outline is from 0 to 480 pixels".to_owned());
        }
    }
    Ok(symbol)
}

/// Adds a node, a built-in node or a use of a subgraph.
fn add_node(graph: &mut GraphFile, id: &str, folder: &str, builtin: bool, pos: &[f64; 2]) -> Result<(), String> {
    if !is_id(id) {
        return Err(format!("the node ID {id:?} is not 16 lowercase hexadecimal characters"));
    }
    if used(graph, id) {
        return Err(format!("the graph has an item with the ID {id}"));
    }
    if builtin && crate::node::Builtin::parse(folder).is_none() {
        return Err(format!("Patina has no built-in node {folder:?} (spec 9.2)"));
    }
    // The folder `graphs/<name>` adds a use of a subgraph (6.1).
    let subgraph = folder.strip_prefix("graphs/").filter(|_| !builtin);
    if let Some(name) = subgraph {
        crate::project::check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
    }
    graph.node.push(NodeEntry {
        id: id.to_owned(),
        folder: (!builtin && subgraph.is_none()).then(|| folder.to_owned()),
        subgraph: subgraph.map(str::to_owned),
        builtin: builtin.then(|| folder.to_owned()),
        pos: Some(finite(pos)?),
        domain: None,
        settings: None,
        params: Default::default(),
    });
    Ok(())
}

/// Removes a node with its wires, its order wires and the wires of its public pins.
fn remove_node(graph: &mut GraphFile, id: &str) -> Result<(), String> {
    let index = find(graph, id)?;
    graph.node.remove(index);
    graph.wire.retain(|w| node_of(&w.from) != id && node_of(&w.to) != id);
    graph.order.retain(|o| o.before != id && o.after != id);
    for pin in &mut graph.input {
        pin.to.retain(|t| node_of(t) != id);
    }
    for pin in graph.output.iter_mut().filter(|p| node_of(&p.from) == id) {
        pin.from.clear();
    }
    Ok(())
}

/// Adds an order wire (6.4).
fn connect_order(graph: &mut GraphFile, before: &str, after: &str) -> Result<(), String> {
    find(graph, before)?;
    find(graph, after)?;
    if graph.order.iter().any(|o| o.before == before && o.after == after) {
        return Err(format!("an order wire already goes from {before} to {after}"));
    }
    if let Some(ids) = cycle(graph, before, after) {
        return Err(format!("the order wire makes a cycle through these nodes: {}", ids.join(", ")));
    }
    graph.order.push(Order { before: before.to_owned(), after: after.to_owned() });
    Ok(())
}

/// Adds a public pin to the graph (6.1).
fn add_pin(graph: &mut GraphFile, key: &str, name: &str, input: bool, pos: &[f64; 2]) -> Result<(), String> {
    if !is_key(key) {
        return Err(format!("the pin key {key:?} is not 8 lowercase hexadecimal characters"));
    }
    if graph.input.iter().any(|p| p.key == key) || graph.output.iter().any(|p| p.key == key) {
        return Err(format!("the graph has a public pin with the key {key}"));
    }
    let (name, pos) = (pin_name(name)?, finite(pos)?);
    match input {
        true => graph.input.push(PublicInput { key: key.to_owned(), name, pos, to: Vec::new() }),
        false => graph.output.push(PublicOutput { key: key.to_owned(), name, pos, from: String::new() }),
    }
    Ok(())
}

/// Connects a public pin to the node pin `end`.
fn connect_pin(graph: &mut GraphFile, key: &str, end: &str) -> Result<(), String> {
    check_end(graph, end)?;
    if let Some(pin) = graph.output.iter_mut().find(|p| p.key == key) {
        end.clone_into(&mut pin.from);
    } else {
        let index = graph.input.iter().position(|p| p.key == key).ok_or(format!("the graph has no public pin {key}"))?;
        free_input(graph, end);
        graph.input[index].to.push(end.to_owned());
    }
    Ok(())
}

/// Removes the wire between a public pin and the node pin `end`.
fn disconnect_pin(graph: &mut GraphFile, key: &str, end: &str) -> Result<(), String> {
    let none = || format!("no wire goes from the public pin {key} to {end}");
    if let Some(pin) = graph.output.iter_mut().find(|p| p.key == key) {
        if pin.from != end {
            return Err(none());
        }
        pin.from.clear();
    } else {
        let pin = graph.input.iter_mut().find(|p| p.key == key).ok_or(format!("the graph has no public pin {key}"))?;
        let count = pin.to.len();
        pin.to.retain(|t| t != end);
        if pin.to.len() == count {
            return Err(none());
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
mod tests;
