//! **Copy** and **Paste** of nodes, also between two editors and two projects (spec 15.11). A
//! clip is TOML text with the nodes, the wires between them and each subgraph that they use. It
//! holds no code: a paste imports each missing node folder from the source project (6.2).

use std::collections::{BTreeMap, HashMap, HashSet, btree_map::Entry};
use std::hash::{BuildHasher, RandomState};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::import::{self, Choice, State};
use crate::project::check_name;
use crate::project::{FORMAT, GraphFile, NodeEntry, Style, Wire, is_id};
use crate::{Edit, MAIN, Project, Toolchain};

/// The version of the clip text.
const VERSION: u32 = 1;

/// The most subgraphs in one clip. The limit keeps the checks of an untrusted clip short.
const MAX_GRAPHS: usize = 256;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Clip {
    #[serde(rename = "patina-clip")]
    version: u32,
    /// The ID of the source project (5.3).
    project: String,
    /// The folder name of the source project, for the messages.
    name: String,
    /// The copied nodes, with the wires, orders, collars and cables between them.
    nodes: GraphFile,
    /// Each subgraph that the nodes use, also through other subgraphs, with its unsaved changes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    graphs: BTreeMap<String, GraphFile>,
}

/// The names in this project of the subgraphs of a clip, and the new subgraphs.
type Placed = (HashMap<String, String>, Vec<(String, GraphFile)>);

/// The result of a paste.
#[derive(Serialize, Debug, PartialEq)]
pub struct Pasted {
    /// The IDs of the new nodes in the open graph, in the order of the clip.
    pub ids: Vec<String>,
    pub warnings: Vec<String>,
}

/// A wire between the free end of a pin drag and the pin `key` of the first pasted node (15.3).
#[derive(Deserialize, Debug)]
pub struct Link {
    /// The pin `<node id>.<key>` where the drag started.
    pub end: String,
    /// True when `end` is an output.
    pub output: bool,
    pub key: String,
}

/// One subgraph or folder that a paste brings into this project, with its state (6.2).
#[derive(Serialize, Debug, PartialEq)]
pub struct Need {
    /// For example `graphs/half`, `nodes/blur` or `types/image`.
    pub folder: String,
    #[serde(flatten)]
    pub state: NeedState,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "state")]
pub enum NeedState {
    /// The paste copies it.
    New,
    /// This project has it with the same contents, so the paste uses it.
    Same,
    /// This project has the folder with other files. The paste uses the folder of this project.
    Keep,
    /// A different subgraph has the name, so the paste uses the subgraph `to`.
    Rename { to: String },
    /// A folder of another group has the crate name, so the paste fails.
    Clash { existing: String },
}

impl Project {
    /// The clip text of the nodes `ids` of the open graph (15.11). A wire, an order, a collar or a
    /// cable goes into the clip only when all of its nodes do.
    pub fn copy(&self, ids: &[String]) -> Result<String, String> {
        let graph = &self.history.graph;
        let has = |end: &str| end.split_once('.').is_some_and(|(id, _)| ids.iter().any(|i| i == id));
        let node: Vec<NodeEntry> = graph.node.iter().filter(|n| ids.contains(&n.id)).cloned().collect();
        if node.is_empty() {
            return Err("select the nodes to copy".to_owned());
        }
        let wire: Vec<Wire> = graph.wire.iter().filter(|w| has(&w.from) && has(&w.to)).cloned().collect();
        let copied = |ends: &[String]| !ends.is_empty() && ends.iter().all(|to| wire.iter().any(|w| &w.to == to));
        let nodes = GraphFile {
            format: FORMAT,
            style: graph.style,
            symbol: None,
            node,
            order: graph.order.iter().filter(|o| ids.contains(&o.before) && ids.contains(&o.after)).cloned().collect(),
            collar: graph.collar.iter().filter(|c| copied(&c.wires)).cloned().collect(),
            cable: graph.cable.iter().filter(|c| copied(&c.wires)).cloned().collect(),
            wire,
            input: Vec::new(),
            output: Vec::new(),
        };
        self.clip(nodes)
    }

    /// The clip of one node of the folder `nodes/<name>`, or of one use of the subgraph `name`,
    /// for a pick from a library (6.2).
    pub fn library_clip(&self, name: &str, subgraph: bool) -> Result<String, String> {
        check_name(name)?;
        if !subgraph && !self.root.join("nodes").join(name).join("Cargo.toml").is_file() {
            return Err(format!("the library has no node folder {name}"));
        }
        let entry = NodeEntry {
            id: "0000000000000001".to_owned(),
            folder: (!subgraph).then(|| format!("nodes/{name}")),
            subgraph: subgraph.then(|| name.to_owned()),
            builtin: None,
            pos: Some([0.0, 0.0]),
            domain: None,
            settings: None,
            params: BTreeMap::new(),
        };
        let nodes = GraphFile { format: FORMAT, node: vec![entry], ..GraphFile::default() };
        self.clip(nodes)
    }

    fn clip(&self, nodes: GraphFile) -> Result<String, String> {
        let mut graphs = BTreeMap::new();
        let mut next = uses(&nodes);
        while let Some(name) = next.pop() {
            if let Entry::Vacant(slot) = graphs.entry(name) {
                let graph = self.subgraph_file(slot.key())?;
                next.extend(uses(&graph));
                slot.insert(graph);
            }
        }
        let name = self.root.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
        toml::to_string(&Clip { version: VERSION, project: self.id.clone(), name, nodes, graphs }).map_err(|e| e.to_string())
    }

    /// Pastes the clip `text` into the open graph, with the top left node at `at`, as one step of
    /// undo (15.11). `source` gives the folder of the trusted project with an ID. A node folder
    /// that this project does not have comes from that project with an import and a build check
    /// (6.2). A new subgraph stays in memory until a save. The wire of `link` goes in the same step.
    pub fn paste(&mut self, text: &str, at: [f64; 2], toolchain: Option<&Toolchain>, source: impl Fn(&str) -> Option<PathBuf>, link: Option<&Link>) -> Result<Pasted, String> {
        let clip = parse(text)?;
        let order = leaves_first(&clip)?;
        let (renames, new) = self.place_graphs(&clip, &order)?;
        if self.name != MAIN && renames.values().any(|n| n.eq_ignore_ascii_case(&self.name)) {
            return Err(format!("the subgraph {} can not use itself", self.name));
        }
        let mut warnings = Vec::new();
        if clip.project != self.id {
            self.import_folders(&clip, &order, toolchain, &source, &mut warnings)?;
        }
        for (name, graph) in new {
            self.add_graph(&name, graph);
        }
        let ids = self.paste_nodes(clip.nodes, &renames, at, link, &mut warnings);
        Ok(Pasted { ids, warnings })
    }

    /// What a paste of the clip `text` brings into this project, with no change (6.2): each
    /// subgraph, then each folder. The Library panel shows it before a pick (15.2).
    pub fn paste_plan(&self, text: &str, source: impl Fn(&str) -> Option<PathBuf>) -> Result<Vec<Need>, String> {
        let clip = parse(text)?;
        let order = leaves_first(&clip)?;
        let (renames, new) = self.place_graphs(&clip, &order)?;
        let mut needs: Vec<Need> = order
            .iter()
            .map(|name| {
                let to = &renames[name];
                let state = if to != name {
                    NeedState::Rename { to: to.clone() }
                } else if new.iter().any(|(made, _)| made == to) {
                    NeedState::New
                } else {
                    NeedState::Same
                };
                Need { folder: format!("graphs/{name}"), state }
            })
            .collect();
        let folders = node_folders(&clip, &order);
        if clip.project == self.id || folders.is_empty() {
            return Ok(needs);
        }
        let other = source(&clip.project).ok_or_else(|| format!("Open the project {} in Patina and trust it.", clip.name))?;
        for item in import::plan_all(&self.root, &other, &folders)? {
            let state = match item.state {
                State::New => NeedState::New,
                State::Same => NeedState::Same,
                State::Conflict { replace: true, .. } => NeedState::Keep,
                State::Conflict { existing, replace: false } => NeedState::Clash { existing },
            };
            needs.push(Need { folder: item.folder, state });
        }
        Ok(needs)
    }

    /// The name in this project of each subgraph of the clip, and the new subgraphs. A subgraph
    /// with the same name and the same graph is used again. A different graph gets a free name
    /// `<name>-2`, `<name>-3` and so on.
    fn place_graphs(&self, clip: &Clip, order: &[String]) -> Result<Placed, String> {
        let mut renames: HashMap<String, String> = HashMap::new();
        let mut new: Vec<(String, GraphFile)> = Vec::new();
        for name in order {
            let mut graph = clip.graphs[name].clone();
            rename_uses(&mut graph, &renames);
            let mut n = 1;
            let target = loop {
                let candidate = if n == 1 { name.clone() } else { format!("{name}-{n}") };
                check_name(&candidate).map_err(|e| format!("the subgraph name {candidate:?}: {e}"))?;
                let same = match new.iter().find(|(made, _)| made.eq_ignore_ascii_case(&candidate)) {
                    Some((_, made)) => Some(*made == graph),
                    None if self.has_graph(&candidate) => Some(self.subgraph_file(&candidate).is_ok_and(|g| g == graph)),
                    None => None,
                };
                match same {
                    None => {
                        new.push((candidate.clone(), graph));
                        break candidate;
                    }
                    Some(true) => break candidate,
                    Some(false) => n += 1,
                }
            };
            renames.insert(name.clone(), target);
        }
        Ok((renames, new))
    }

    /// Imports the node folders of the clip that this project does not have from the source
    /// project, with the type and widget folders that they need (6.2). A folder that this
    /// project has with other files stays, with a warning.
    fn import_folders(&mut self, clip: &Clip, order: &[String], toolchain: Option<&Toolchain>, source: &dyn Fn(&str) -> Option<PathBuf>, warnings: &mut Vec<String>) -> Result<(), String> {
        let folders = node_folders(clip, order);
        if folders.is_empty() {
            return Ok(());
        }
        let Some(other) = source(&clip.project) else {
            if folders.iter().all(|f| self.root.join(f).join("Cargo.toml").is_file()) {
                warnings.push(format!("Patina did not find the trusted project {}, so the paste uses the node folders of this project.", clip.name));
                return Ok(());
            }
            return Err(format!("Open the project {} in Patina and trust it, then paste again. The paste copies its node folders.", clip.name));
        };
        let mut choices = BTreeMap::new();
        let mut copies = false;
        for item in import::plan_all(&self.root, &other, &folders)? {
            match item.state {
                State::New => copies = true,
                State::Same => {}
                State::Conflict { replace: true, .. } => {
                    warnings.push(format!("{} of this project is not the same as in {}. The paste uses the folder of this project.", item.folder, clip.name));
                    choices.insert(item.folder, Choice::Keep);
                }
                State::Conflict { existing, replace: false } => {
                    return Err(format!("{existing} has the crate name of {}. Use Import with another name, then paste again.", item.folder));
                }
            }
        }
        if !copies {
            return Ok(());
        }
        let toolchain = toolchain.ok_or("Patina needs the toolchain to import the node folders of the paste")?;
        self.check_toolchain(toolchain)?;
        let applied = import::apply_all(&self.root, &other, &folders, &choices)?;
        self.build_applied(toolchain, applied)
    }

    /// Adds the nodes of the clip to the open graph with new IDs, moved to `at`.
    fn paste_nodes(&mut self, clip: GraphFile, renames: &HashMap<String, String>, at: [f64; 2], link: Option<&Link>, warnings: &mut Vec<String>) -> Vec<String> {
        let mut graph = self.history.graph.clone();
        let mut taken: HashSet<String> = graph.node.iter().map(|n| n.id.clone()).collect();
        let state = RandomState::new();
        let mut count = 0u64;
        let mut ids: HashMap<String, String> = HashMap::new();
        for entry in &clip.node {
            let id = loop {
                count += 1;
                let id = format!("{:016x}", state.hash_one(count));
                if taken.insert(id.clone()) {
                    break id;
                }
            };
            ids.insert(entry.id.clone(), id);
        }
        let corner = clip.node.iter().filter_map(|n| n.pos).reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1])]).unwrap_or(at);
        let shift = |p: [f64; 2]| [p[0] + at[0] - corner[0], p[1] + at[1] - corner[1]];
        // The parse checked that each end names a node of the clip.
        let end = |end: &String| end.split_once('.').map_or_else(String::new, |(id, key)| format!("{}.{key}", ids[id]));
        let new: Vec<String> = clip.node.iter().map(|n| ids[&n.id].clone()).collect();
        for mut entry in clip.node {
            entry.id = ids[&entry.id].clone();
            entry.pos = entry.pos.map(shift);
            rename_use(&mut entry, renames);
            graph.node.push(entry);
        }
        for mut wire in clip.wire {
            (wire.from, wire.to) = (end(&wire.from), end(&wire.to));
            wire.points = wire.points.into_iter().map(shift).collect();
            wire.style = Style::moved(wire.style, clip.style, graph.style);
            graph.wire.push(wire);
        }
        for mut order in clip.order {
            (order.before, order.after) = (ids[&order.before].clone(), ids[&order.after].clone());
            graph.order.push(order);
        }
        for mut collar in clip.collar {
            collar.pos = shift(collar.pos);
            collar.wires = collar.wires.iter().map(end).collect();
            graph.collar.push(collar);
        }
        for mut cable in clip.cable {
            cable.wires = cable.wires.iter().map(end).collect();
            // A cable name of the graph gets a free name `<name>-2`, `<name>-3` and so on.
            let (base, mut n) = (cable.name.clone(), 1);
            while graph.cable.iter().any(|c| c.name == cable.name) {
                n += 1;
                cable.name = format!("{base}-{n}");
            }
            graph.cable.push(cable);
        }
        if let (Some(link), Some(first)) = (link, new.first()) {
            let pin = format!("{first}.{}", link.key);
            let (from, to) = if link.output { (link.end.clone(), pin) } else { (pin, link.end.clone()) };
            // The folders are already in, so a wire that fails is a warning and not an error.
            if let Err(e) = crate::edit::change(&mut graph, &Edit::Connect { from, to }) {
                warnings.push(format!("the wire to {} did not connect: {e}", link.end));
            }
        }
        self.history.replace(graph);
        new
    }
}

/// The node folders of the clip and of its subgraphs, each one time.
fn node_folders(clip: &Clip, order: &[String]) -> Vec<String> {
    let graphs = order.iter().map(|name| &clip.graphs[name]);
    let mut folders: Vec<String> = Vec::new();
    for entry in std::iter::once(&clip.nodes).chain(graphs).flat_map(|g| &g.node) {
        if let Some(folder) = &entry.folder
            && !folders.contains(folder)
        {
            folders.push(folder.clone());
        }
    }
    folders
}

/// Reads and checks a clip. The text comes from the clipboard, so it is not trusted: a folder
/// must be a node folder and each ID and name must be valid.
fn parse(text: &str) -> Result<Clip, String> {
    if !text.trim_start().starts_with("patina-clip") {
        return Err("The clipboard has no Patina nodes.".to_owned());
    }
    let clip: Clip = toml::from_str(text).map_err(|e| format!("the clip is not valid: {e}"))?;
    if clip.version != VERSION {
        return Err(format!("the clip has version {}, but this Patina reads version {VERSION}", clip.version));
    }
    if clip.graphs.len() > MAX_GRAPHS {
        return Err(format!("the clip has more than {MAX_GRAPHS} subgraphs"));
    }
    for name in clip.graphs.keys() {
        check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
        if name.eq_ignore_ascii_case(MAIN) {
            return Err("the main graph is not a subgraph".to_owned());
        }
    }
    for graph in std::iter::once(&clip.nodes).chain(clip.graphs.values()) {
        if graph.format != FORMAT {
            return Err(format!("the clip has graph format {}, but this Patina reads format {FORMAT}", graph.format));
        }
        for entry in &graph.node {
            if let Some(folder) = &entry.folder {
                let name = folder.strip_prefix("nodes/").ok_or_else(|| format!("the folder {folder:?} is not in nodes/"))?;
                check_name(name).map_err(|e| format!("the folder {folder:?}: {e}"))?;
            }
            if let Some(name) = &entry.subgraph {
                check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
            }
        }
    }
    let nodes = &clip.nodes;
    let mut ids = HashSet::new();
    if nodes.node.is_empty() || !nodes.input.is_empty() || !nodes.output.is_empty() {
        return Err("the clip has no nodes, or it has public pins".to_owned());
    }
    for entry in &nodes.node {
        if !is_id(&entry.id) || !ids.insert(entry.id.as_str()) {
            return Err(format!("the clip has a bad or repeated node ID {:?}", entry.id));
        }
    }
    let known = |end: &str| end.split_once('.').is_some_and(|(id, _)| ids.contains(id));
    let wired = |to: &String| nodes.wire.iter().any(|w| &w.to == to);
    let good = nodes.wire.iter().all(|w| known(&w.from) && known(&w.to))
        && nodes.order.iter().all(|o| ids.contains(o.before.as_str()) && ids.contains(o.after.as_str()))
        && nodes.collar.iter().all(|c| c.wires.iter().all(wired))
        && nodes.cable.iter().all(|c| c.wires.iter().all(wired));
    if !good {
        return Err("the clip has a wire, an order, a collar or a cable of a node that it does not have".to_owned());
    }
    Ok(clip)
}

/// The subgraphs that the nodes of the clip use, also through other subgraphs. Each subgraph
/// comes after the subgraphs that it uses.
fn leaves_first(clip: &Clip) -> Result<Vec<String>, String> {
    fn visit(name: &str, clip: &Clip, done: &mut Vec<String>, stack: &mut Vec<String>) -> Result<(), String> {
        if done.iter().any(|d| d == name) {
            return Ok(());
        }
        if stack.iter().any(|s| s == name) {
            return Err(format!("the subgraph {name} uses itself"));
        }
        let graph = clip.graphs.get(name).ok_or_else(|| format!("the clip has no subgraph {name}"))?;
        stack.push(name.to_owned());
        for used in uses(graph) {
            visit(&used, clip, done, stack)?;
        }
        stack.pop();
        done.push(name.to_owned());
        Ok(())
    }
    let mut done = Vec::new();
    for name in uses(&clip.nodes) {
        visit(&name, clip, &mut done, &mut Vec::new())?;
    }
    Ok(done)
}

/// The subgraph names of the uses in a graph.
fn uses(graph: &GraphFile) -> Vec<String> {
    graph.node.iter().filter_map(|n| n.subgraph.clone()).collect()
}

fn rename_uses(graph: &mut GraphFile, renames: &HashMap<String, String>) {
    for entry in &mut graph.node {
        rename_use(entry, renames);
    }
}

fn rename_use(entry: &mut NodeEntry, renames: &HashMap<String, String>) {
    if let Some(name) = &mut entry.subgraph
        && let Some(to) = renames.get(name.as_str())
    {
        name.clone_from(to);
    }
}

#[cfg(test)]
mod tests;
