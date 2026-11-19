//! Reads the node entries and the wires of a graph for the check (spec 6.4, 9.2).

use std::collections::HashMap;
use std::path::Path;

use crate::node::{Builtin, Domain, Node};
use crate::project::{GraphFile, NodeEntry, is_id, read_node};

/// Edges of the run order, each from one entry to another.
type Edges = Vec<(usize, usize)>;

pub(super) struct Entry {
    pub(super) id: String,
    pub(super) node: usize,
    /// The wired source of each input: the entry and the output index.
    pub(super) wired: Vec<Option<(usize, usize)>>,
    /// The domain that the node declares or that the user selected (9.1).
    pub(super) domain: Option<Domain>,
}

/// The nodes of a graph that the check read. An entry is None when its node has a problem.
pub(super) struct Read<'a> {
    pub(super) nodes: Vec<(String, Node)>,
    pub(super) entries: Vec<Option<Entry>>,
    pub(super) builtins: Vec<(usize, Builtin)>,
    by_id: HashMap<&'a str, usize>,
    /// The node index of each folder name, or None when the folder has a problem.
    by_name: HashMap<String, Option<usize>>,
}

/// True when the node with index `node` is the delay (9.2).
pub(super) fn is_delay(builtins: &[(usize, Builtin)], node: usize) -> bool {
    builtins.contains(&(node, Builtin::Delay))
}

/// True when the node with index `node` is a trigger (9.4).
pub(super) fn fires(builtins: &[(usize, Builtin)], node: usize) -> bool {
    builtins.iter().any(|&(i, b)| i == node && b.fires())
}

impl<'a> Read<'a> {
    /// Reads each node entry of `graph` and the node folders that they use.
    pub(super) fn new(root: &Path, graph: &'a GraphFile, problems: &mut Vec<String>) -> Self {
        let mut read = Read { nodes: Vec::new(), entries: Vec::new(), builtins: Vec::new(), by_id: HashMap::new(), by_name: HashMap::new() };
        for (index, entry) in graph.node.iter().enumerate() {
            let entry = read.entry(root, index, entry, problems);
            read.entries.push(entry);
        }
        read
    }

    /// Reads the node entry `index` of the graph. Gives None when the entry has a problem.
    fn entry(&mut self, root: &Path, index: usize, entry: &'a NodeEntry, problems: &mut Vec<String>) -> Option<Entry> {
        let id = entry.id.as_str();
        let fail = |text: &str| format!("node {id}: {text}");
        // A node of a subgraph use has the ID path `<use ID>/<node ID>` (6.4).
        if !id.split('/').all(is_id) {
            problems.push(fail("the ID must be 16 lowercase hexadecimal characters"));
            return None;
        }
        if self.by_id.insert(id, index).is_some() {
            problems.push(fail("two nodes have this ID"));
            return None;
        }
        if entry.settings.is_some() {
            problems.push(fail("node settings (4.3) are not ready yet, so remove the settings of the node"));
        }
        let node_index = match self.node_index(root, entry) {
            Ok(i) => i,
            Err(problem) => {
                problems.extend(problem);
                return None;
            }
        };
        let folder = entry.folder.as_deref().or(entry.builtin.as_deref()).unwrap_or_default();
        // A parameter with no input is a lost parameter: the run ignores it and the editor shows it.
        let node = &self.nodes[node_index].1;
        let chosen = entry.domain.as_deref().map(Domain::parse).transpose().unwrap_or_else(|text| {
            problems.push(fail(&text));
            None
        });
        if let (Some(chosen), Some(declared)) = (chosen, node.domain)
            && chosen != declared
        {
            problems.push(fail(&format!("{folder} declares the {} domain", declared.name())));
        }
        let wired = vec![None; node.inputs.len()];
        Some(Entry { id: id.to_owned(), node: node_index, wired, domain: node.domain.or(chosen) })
    }

    /// Finds or reads the node of `entry`. The error is the problem, or None when an earlier entry
    /// with the same folder reported it.
    fn node_index(&mut self, root: &Path, entry: &NodeEntry) -> Result<usize, Option<String>> {
        let id = &entry.id;
        let fail = |text: String| Err(Some(format!("node {id}: {text}")));
        match (&entry.folder, &entry.builtin) {
            (None, Some(name)) => {
                let Some(builtin) = Builtin::parse(name) else {
                    return fail(format!("Patina has no built-in node {name:?} (spec 9.2)"));
                };
                if let Some(&(i, _)) = self.builtins.iter().find(|(_, b)| *b == builtin) {
                    return Ok(i);
                }
                self.nodes.push((builtin.name().to_owned(), builtin.node()));
                self.builtins.push((self.nodes.len() - 1, builtin));
                Ok(self.nodes.len() - 1)
            }
            (Some(_), Some(_)) => fail("give the node a folder or a built-in node, not both".to_owned()),
            (None, None) => fail("the node has no folder".to_owned()),
            (Some(folder), None) => {
                let Some(name) = folder.strip_prefix("nodes/") else {
                    return fail(format!("the folder {folder:?} is not in nodes/"));
                };
                let nodes = &mut self.nodes;
                let mut problem = None;
                let slot = self.by_name.entry(name.to_owned()).or_insert_with(|| match read_node(root, name) {
                    Ok(node) => {
                        nodes.push((name.to_owned(), node));
                        Some(nodes.len() - 1)
                    }
                    Err(text) => {
                        problem = Some(format!("{folder}: {text}"));
                        None
                    }
                });
                slot.ok_or(problem)
            }
        }
    }

    /// Records the source of each wired input. Gives the edges of the run order apart: the data
    /// wires, except the input wire of a delay, and the order wires.
    pub(super) fn wires(&mut self, graph: &GraphFile, problems: &mut Vec<String>) -> (Edges, Edges) {
        let (mut edges, mut orders) = (Vec::new(), Vec::new());
        for wire in &graph.wire {
            let from = self.find_pin(&wire.from, Node::output_keys);
            let to = self.find_pin(&wire.to, |n| n.inputs.iter().map(|i| i.key.clone()).collect());
            let ((from_entry, output), (to_entry, input)) = match (from, to) {
                (Ok(from), Ok(to)) => (from, to),
                (from, to) => {
                    let ends = [(&wire.from, from.err()), (&wire.to, to.err())];
                    for (end, text) in ends.into_iter().filter_map(|(e, t)| Some((e, t?))) {
                        problems.push(format!("wire end {end:?}: {text}"));
                    }
                    continue;
                }
            };
            let entry = self.entries[to_entry].as_mut().expect("find_pin gives a checked entry");
            let node = entry.node;
            if entry.wired[input].replace((from_entry, output)).is_some() {
                problems.push(format!("the input {:?} has more than one wire", wire.to));
            }
            if fires(&self.builtins, node) {
                problems.push(format!("the input {:?} is a setting of the trigger. Set it in its parameter field (spec 9.4)", wire.to));
            }
            // The delay gives the value of the previous frame, so its input wire makes no cycle (9.2).
            if !is_delay(&self.builtins, node) {
                edges.push((from_entry, to_entry));
            }
        }
        for order in &graph.order {
            match (self.by_id.get(order.before.as_str()), self.by_id.get(order.after.as_str())) {
                (Some(&before), Some(&after)) => orders.push((before, after)),
                _ => problems.push(format!("the order wire {} to {} names no node", order.before, order.after)),
            }
        }
        (edges, orders)
    }

    /// Finds the entry and the pin index of a wire end `<node ID>.<pin key>`.
    fn find_pin(&self, end: &str, keys: impl Fn(&Node) -> Vec<String>) -> Result<(usize, usize), String> {
        let (id, key) = end.split_once('.').ok_or("write the end as <node ID>.<pin key>")?;
        let &index = self.by_id.get(id).ok_or("no node has this ID")?;
        let entry = self.entries[index].as_ref().ok_or("the node has an error")?;
        let (name, node) = &self.nodes[entry.node];
        let pin = keys(node).iter().position(|k| k == key);
        pin.map(|pin| (index, pin)).ok_or_else(|| format!("nodes/{name} has no such pin"))
    }
}
