//! Subgraphs (spec 6.1, 6.4): each use of a subgraph becomes the nodes and wires of its file. A
//! node of a use gets the ID `<use ID>/<node ID>`, so two uses of one subgraph do not share IDs.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::rc::Rc;

use crate::node::{self, Node};
use crate::project::{self, GraphFile, Order, Wire};
use crate::{NodeInfo, Pin};

mod collapse;

pub(crate) use collapse::{collapse, expand_use};

/// The inner ends of the public pins of one use, and the IDs of its nodes.
struct Use {
    inputs: HashMap<String, Vec<String>>,
    outputs: HashMap<String, String>,
    nodes: Vec<String>,
}

/// The output pin `<use ID>.<pin key>` of a use and the inner output that gives its value.
pub(crate) type Alias = (String, String);

/// The subgraphs of the editor, by name, with their unsaved changes.
pub(crate) type Edits<'a> = &'a [(&'a str, &'a GraphFile)];

/// The subgraph files and node folders of one call. Each is read one time, because a large graph
/// has many uses of one subgraph. A graph of `edits` replaces its file.
struct Files<'a> {
    root: &'a Path,
    edits: Edits<'a>,
    graphs: HashMap<String, Result<Rc<GraphFile>, String>>,
    nodes: HashMap<String, Option<Rc<Node>>>,
}

impl<'a> Files<'a> {
    fn new(root: &'a Path, edits: Edits<'a>) -> Self {
        Files { root, edits, graphs: HashMap::new(), nodes: HashMap::new() }
    }

    /// The subgraph `name`: its graph in `edits`, or else its file.
    fn graph(&mut self, name: &str) -> Result<Rc<GraphFile>, String> {
        if let Some(found) = self.graphs.get(name) {
            return found.clone();
        }
        let found = match self.edits.iter().find(|(edit, _)| *edit == name) {
            Some((_, graph)) => Ok(Rc::new((*graph).clone())),
            None => project::read(&self.root.join("graphs").join(format!("{name}.toml")), |g: &GraphFile| g.format).map(Rc::new).map_err(|e| e.to_string()),
        };
        self.graphs.insert(name.to_owned(), found.clone());
        found
    }

    /// The pins of the node folder `nodes/<name>`, or `None` if it does not read.
    fn node(&mut self, name: &str) -> Option<Rc<Node>> {
        let root = self.root;
        self.nodes.entry(name.to_owned()).or_insert_with(|| project::read_node(root, name).ok().map(Rc::new)).clone()
    }
}

/// Gives the graph with each use of a subgraph replaced by the nodes of the subgraph file, and
/// the aliases of the output pins of the uses. The collars and cables stay out, because only the
/// editor draws them. Each graph of `edits` is a subgraph of the editor that replaces its file.
pub(crate) fn flatten(root: &Path, graph: &GraphFile, edits: Edits) -> Result<(GraphFile, Vec<Alias>), Vec<String>> {
    let mut problems = Vec::new();
    let (flat, mut aliases) = expand(&mut Files::new(root, edits), graph, &mut Vec::new(), &mut problems);
    // An inner public input pin can get its value from an outer public input pin.
    let map: HashMap<String, String> = aliases.iter().cloned().collect();
    for (_, from) in &mut aliases {
        while let Some(next) = map.get(from.as_str()) {
            from.clone_from(next);
        }
    }
    if problems.is_empty() { Ok((flat, aliases)) } else { Err(problems) }
}

fn expand(files: &mut Files, graph: &GraphFile, stack: &mut Vec<String>, problems: &mut Vec<String>) -> (GraphFile, Vec<Alias>) {
    if graph.node.iter().all(|n| n.subgraph.is_none()) {
        return (graph.clone(), Vec::new());
    }
    let mut flat = GraphFile { node: Vec::new(), wire: Vec::new(), order: Vec::new(), collar: Vec::new(), cable: Vec::new(), ..graph.clone() };
    let mut uses: HashMap<&str, Use> = HashMap::new();
    let mut nested: Vec<Alias> = Vec::new();
    let mut errors = Vec::new();
    for entry in &graph.node {
        let Some(name) = &entry.subgraph else {
            flat.node.push(entry.clone());
            continue;
        };
        let id = &entry.id;
        let mut fail = |text: String| errors.push(format!("node {id}: {text}"));
        if entry.folder.is_some() || entry.builtin.is_some() || entry.domain.is_some() || entry.settings.is_some() {
            fail("a use of a subgraph has no folder, built-in node, domain or settings".to_owned());
            continue;
        }
        if let Err(text) = project::check_name(name) {
            fail(format!("the subgraph name {name:?}: {text}"));
            continue;
        }
        if stack.contains(name) {
            fail(format!("the subgraph {name} uses itself"));
            continue;
        }
        let file = match files.graph(name) {
            Ok(file) if crate::is_top(name, &file) => {
                fail(format!("the graph {name} is a top graph, not a subgraph"));
                continue;
            }
            Ok(file) => file,
            Err(e) => {
                fail(e);
                continue;
            }
        };
        stack.push(name.clone());
        let (inner, inner_aliases) = expand(files, &file, stack, problems);
        stack.pop();
        let prefix = |end: &str| format!("{id}/{end}");
        // The editor shows the values inside a use, so the uses inside it keep their aliases (15.9).
        nested.extend(inner_aliases.iter().map(|(outer, from)| (prefix(outer), prefix(from))));
        // The keys come from a file, and a wire end splits at the dot (6.4).
        if let Some(bad) = inner.input.iter().map(|p| &p.key).chain(inner.output.iter().map(|p| &p.key)).find(|k| !project::is_key(k)) {
            fail(format!("the subgraph {name} has the pin key {bad:?}, not 8 lowercase hexadecimal characters"));
        }
        let mut one = Use { inputs: HashMap::new(), outputs: HashMap::new(), nodes: Vec::new() };
        for pin in &inner.input {
            if one.inputs.insert(pin.key.clone(), pin.to.iter().map(|to| prefix(to)).collect()).is_some() {
                fail(format!("the subgraph {name} has two inputs with the key {}", pin.key));
            }
        }
        for pin in &inner.output {
            let from = if pin.from.is_empty() { String::new() } else { prefix(&pin.from) };
            if one.outputs.insert(pin.key.clone(), from).is_some() {
                fail(format!("the subgraph {name} has two outputs with the key {}", pin.key));
            }
        }
        let first = flat.node.len();
        for node in inner.node {
            one.nodes.push(prefix(&node.id));
            flat.node.push(project::NodeEntry { id: prefix(&node.id), ..node });
        }
        flat.wire.extend(inner.wire.into_iter().map(|w| Wire { from: prefix(&w.from), to: prefix(&w.to), ..w }));
        flat.order.extend(inner.order.into_iter().map(|o| Order { before: prefix(&o.before), after: prefix(&o.after) }));
        // A parameter of the use goes to each inner input of its public input pin.
        for (key, text) in &entry.params {
            // A parameter with no public input is a lost parameter, which the run ignores.
            let Some(targets) = one.inputs.get(key) else { continue };
            for target in targets {
                let (node, input) = target.split_once('.').unwrap_or((target, ""));
                if let Some(node) = flat.node[first..].iter_mut().find(|n| n.id == node) {
                    node.params.insert(input.to_owned(), text.clone());
                }
            }
        }
        uses.insert(id, one);
    }
    problems.append(&mut errors);
    rewire(graph, &uses, &mut flat, &mut nested, problems);
    let mut aliases: Vec<Alias> = nested.into_iter().chain(uses.iter().flat_map(|(id, one)| one.outputs.iter().filter(|(_, from)| !from.is_empty()).map(move |(key, from)| (format!("{id}.{key}"), from.clone())))).collect();
    aliases.sort();
    (flat, aliases)
}

/// Moves the wires and the public pins that end at a use to the inner ends of its pins. An order
/// wire to a use orders all the nodes of the use.
fn rewire(graph: &GraphFile, uses: &HashMap<&str, Use>, flat: &mut GraphFile, nested: &mut Vec<Alias>, problems: &mut Vec<String>) {
    let source = |end: &str, problems: &mut Vec<String>| match end.split_once('.').and_then(|(id, key)| Some((uses.get(id)?, id, key))) {
        Some((one, id, key)) => one.outputs.get(key).cloned().unwrap_or_else(|| {
            problems.push(format!("wire end {end:?}: the subgraph use {id} has no output `{key}`"));
            end.to_owned()
        }),
        None => end.to_owned(),
    };
    let targets = |end: &str, problems: &mut Vec<String>| match end.split_once('.').and_then(|(id, key)| Some((uses.get(id)?, id, key))) {
        Some((one, id, key)) => one.inputs.get(key).cloned().unwrap_or_else(|| {
            problems.push(format!("wire end {end:?}: the subgraph use {id} has no input `{key}`"));
            Vec::new()
        }),
        None => vec![end.to_owned()],
    };
    for wire in &graph.wire {
        // A wire from an output of a use that is not connected inside gives no value.
        let from = source(&wire.from, problems);
        if from.is_empty() {
            continue;
        }
        // The public input pin inside a use shows the value of the wire to the use (15.9).
        if let Some((id, key)) = wire.to.split_once('.').filter(|(id, _)| uses.contains_key(id)) {
            nested.push((format!("{id}/in:{key}.pin"), from.clone()));
        }
        for to in targets(&wire.to, problems) {
            flat.wire.push(Wire { from: from.clone(), to, ..wire.clone() });
        }
    }
    for pin in &mut flat.input {
        for (id, key) in pin.to.iter().filter_map(|to| to.split_once('.')).filter(|(id, _)| uses.contains_key(id)) {
            nested.push((format!("{id}/in:{key}.pin"), format!("in:{}.pin", pin.key)));
        }
        pin.to = pin.to.iter().flat_map(|to| targets(to, problems)).collect();
    }
    for pin in &mut flat.output {
        pin.from = source(&pin.from, problems);
    }
    let nodes = |id: &String| uses.get(id.as_str()).map_or_else(|| vec![id.clone()], |one| one.nodes.clone());
    for order in &graph.order {
        for before in nodes(&order.before) {
            for after in nodes(&order.after) {
                flat.order.push(Order { before: before.clone(), after });
            }
        }
    }
}

/// The subgraph files of `graphs/`, without the main graph, for the node list of the editor.
/// A subgraph of `edits` gives its pins from its edits, which are not saved yet. The first list
/// has the names of the top graphs (6.1), `main` first.
pub(crate) fn infos(root: &Path, edits: Edits) -> (Vec<String>, Vec<NodeInfo>) {
    // A new subgraph of a paste or a collapse has no file before a save (15.10, 15.11).
    let mut names: BTreeSet<String> = edits.iter().map(|(name, _)| (*name).to_owned()).collect();
    for entry in fs::read_dir(root.join("graphs")).into_iter().flatten().flatten() {
        let path = entry.path();
        let toml = path.extension().is_some_and(|e| e == "toml");
        names.extend(path.file_stem().and_then(|s| s.to_str()).filter(|n| toml && *n != "main").map(str::to_owned));
    }
    let (mut tops, mut infos) = (vec![crate::MAIN.to_owned()], Vec::new());
    let files = &mut Files::new(root, edits);
    for name in &names {
        if files.graph(name).is_ok_and(|g| g.top) {
            tops.push(name.clone());
            continue;
        }
        let mut info = NodeInfo { name: name.clone(), inputs: Vec::new(), outputs: Vec::new(), error: None, builtin: false, generics: Vec::new(), full: false, free: false, color: None, symbol: None, subgraph: true, kind: "subgraph", doc: None };
        match files.graph(name) {
            Ok(graph) => {
                info.symbol.clone_from(&graph.symbol);
                let stack = &mut vec![name.clone()];
                let mut generics = Vec::new();
                // A pin that ends at a generic pin is generic too, so the palette offers each type (4.5).
                let mut pin = |key: &str, pin: &str, end: Option<&String>, input| {
                    let (ty, of_node) = end.and_then(|end| end_type(files, &graph, end, input, stack)).unwrap_or_else(|| ("?".to_owned(), Vec::new()));
                    if of_node.contains(&ty) && !generics.contains(&ty) {
                        generics.push(ty.clone());
                    }
                    Pin { key: key.to_owned(), ty, name: Some(pin.to_owned()) }
                };
                info.inputs = graph.input.iter().map(|p| pin(&p.key, &p.name, p.to.first(), true)).collect();
                info.outputs = graph.output.iter().map(|p| pin(&p.key, &p.name, Some(&p.from), false)).collect();
                info.generics = generics;
            }
            Err(e) => info.error = Some(e),
        }
        infos.push(info);
    }
    (tops, infos)
}

/// The use path (6.4) of each use of the subgraph `name` in `graph`, also inside other uses (15.9).
pub(crate) fn use_paths(root: &Path, graph: &GraphFile, edits: Edits, name: &str) -> Vec<String> {
    let mut paths = Vec::new();
    find_uses(&mut Files::new(root, edits), graph, name, ("", &mut Vec::new()), &mut paths);
    paths
}

fn find_uses(files: &mut Files, graph: &GraphFile, name: &str, (at, stack): (&str, &mut Vec<String>), paths: &mut Vec<String>) {
    for entry in &graph.node {
        let Some(sub) = &entry.subgraph else { continue };
        let path = if at.is_empty() { entry.id.clone() } else { format!("{at}/{}", entry.id) };
        if sub == name {
            paths.push(path);
        } else if !stack.contains(sub)
            && project::check_name(sub).is_ok()
            && let Ok(inner) = files.graph(sub)
        {
            stack.push(sub.clone());
            find_uses(files, &inner, name, (&path, stack), paths);
            stack.pop();
        }
    }
}

/// The value type of the wire end `<node ID>.<pin key>` of a graph, through the uses of subgraphs,
/// and the type parameters of its node. An input `&bool` gives `bool`.
fn end_type(files: &mut Files, graph: &GraphFile, end: &str, input: bool, stack: &mut Vec<String>) -> Option<(String, Vec<String>)> {
    let (id, key) = end.split_once('.')?;
    let entry = graph.node.iter().find(|n| n.id == id)?;
    let node = match (&entry.folder, &entry.builtin, &entry.subgraph) {
        (Some(folder), None, None) => files.node(folder.strip_prefix("nodes/")?)?,
        (None, Some(builtin), None) => Rc::new(node::Builtin::parse(builtin)?.node()),
        (None, None, Some(name)) if project::check_name(name).is_ok() && !stack.contains(name) => {
            let inner = files.graph(name).ok()?;
            let end = match input {
                true => inner.input.iter().find(|p| p.key == key)?.to.first()?.clone(),
                false => inner.output.iter().find(|p| p.key == key)?.from.clone(),
            };
            stack.push(name.clone());
            let ty = end_type(files, &inner, &end, input, stack);
            stack.pop();
            return ty;
        }
        _ => return None,
    };
    let ty = match input {
        true => node.inputs.iter().find(|i| i.key == key).map(|i| i.value.clone()),
        false => node.output_keys().into_iter().zip(&node.output_types).find(|(k, _)| k == key).map(|(_, ty)| ty.clone()),
    }?;
    // A generic output, as of a delay, has the type of the wire into an input of the same type (4.5).
    let seen = format!("#{end}");
    if !input && node.generics.contains(&ty) && !stack.contains(&seen) {
        let to = node.inputs.iter().find(|i| i.value == ty).map(|i| format!("{id}.{}", i.key));
        if let Some(from) = to.and_then(|to| graph.wire.iter().find(|w| w.to == to)).map(|w| w.from.clone()) {
            stack.push(seen);
            let found = end_type(files, graph, &from, false, stack);
            stack.pop();
            if let Some((found, of)) = found.filter(|(found, of)| !of.contains(found)) {
                return Some((found, of));
            }
        }
    }
    Some((ty, node.generics.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: &str = r#"format = 1

[[node]]
id = "00000000000000a1"
folder = "nodes/add"

[node.params]
b = "1"

[[input]]
key = "0000000a"
name = "x"
pos = [0.0, 0.0]
to = ["00000000000000a1.a", "00000000000000a1.b"]

[[output]]
key = "0000000b"
name = "sum"
pos = [0.0, 0.0]
from = "00000000000000a1.out"
"#;

    pub(super) fn graph(text: &str) -> GraphFile {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn the_use_paths_go_through_the_uses_inside_uses() {
        let uses = |names: &[(&str, &str)]| {
            let nodes: String = names.iter().map(|(id, sub)| format!("[[node]]
id = \"{id}\"
subgraph = \"{sub}\"
")).collect();
            graph(&format!("format = 1
{nodes}"))
        };
        let main = uses(&[("d1", "outer"), ("d2", "half"), ("d3", "loop")]);
        let (outer, looped) = (uses(&[("e1", "half"), ("e2", "half")]), uses(&[("f1", "loop")]));
        let edits: [(&str, &GraphFile); 3] = [("outer", &outer), ("half", &graph(HALF)), ("loop", &looped)];
        let root = Path::new("no-such-folder");
        assert_eq!(use_paths(root, &main, &edits, "half"), ["d1/e1", "d1/e2", "d2"]);
        assert_eq!(use_paths(root, &main, &edits, "outer"), ["d1"]);
        assert_eq!(use_paths(root, &main, &edits, "loop"), ["d3"], "a loop of uses ends");
        assert!(use_paths(root, &main, &edits, "none").is_empty());
    }

    #[test]
    fn each_use_gets_its_own_nodes_and_the_wires_go_to_the_inner_pins() {
        let root = std::env::temp_dir().join(format!("patina-subgraph-{}", std::process::id()));
        std::fs::create_dir_all(root.join("graphs")).unwrap();
        std::fs::write(root.join("graphs/half.toml"), HALF).unwrap();
        // Use u1 feeds use u2. Use u2 has a parameter for its input.
        let main = graph(
            r#"format = 1
[[node]]
id = "00000000000000c1"
folder = "nodes/number"
[[node]]
id = "00000000000000d1"
subgraph = "half"
[[node]]
id = "00000000000000d2"
subgraph = "half"
[node.params]
0000000a = "7"
[[node]]
id = "00000000000000c2"
folder = "nodes/show"
[[wire]]
from = "00000000000000c1.out"
to = "00000000000000d1.0000000a"
[[wire]]
from = "00000000000000d1.0000000b"
to = "00000000000000c2.value"
[[order]]
before = "00000000000000d1"
after = "00000000000000c2"
"#,
        );
        let (flat, aliases) = flatten(&root, &main, &[]).unwrap();
        let inner = |id: &str| format!("{id}/00000000000000a1.out");
        let pin = ("00000000000000d1/in:0000000a.pin".to_owned(), "00000000000000c1.out".to_owned());
        assert_eq!(aliases, [("00000000000000d1.0000000b".to_owned(), inner("00000000000000d1")), pin, ("00000000000000d2.0000000b".to_owned(), inner("00000000000000d2"))]);
        let ids: Vec<&str> = flat.node.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["00000000000000c1", "00000000000000d1/00000000000000a1", "00000000000000d2/00000000000000a1", "00000000000000c2"]);
        let wires: Vec<(&str, &str)> = flat.wire.iter().map(|w| (w.from.as_str(), w.to.as_str())).collect();
        assert_eq!(
            wires,
            [
                ("00000000000000c1.out", "00000000000000d1/00000000000000a1.a"),
                ("00000000000000c1.out", "00000000000000d1/00000000000000a1.b"),
                ("00000000000000d1/00000000000000a1.out", "00000000000000c2.value"),
            ]
        );
        assert_eq!(flat.node[1].params["b"], "1");
        assert_eq!((flat.node[2].params["a"].as_str(), flat.node[2].params["b"].as_str()), ("7", "7"));
        assert_eq!((flat.order[0].before.as_str(), flat.order[0].after.as_str()), ("00000000000000d1/00000000000000a1", "00000000000000c2"));

        // A use inside a use keeps its alias, with the path of the outer use.
        let twice = "format = 1
[[node]]
id = \"00000000000000e1\"
subgraph = \"half\"
[[output]]
key = \"0000000d\"
name = \"y\"
pos = [0.0, 0.0]
from = \"00000000000000e1.0000000b\"
[[input]]
key = \"0000000e\"
name = \"x\"
pos = [0.0, 0.0]
to = [\"00000000000000e1.0000000a\"]
";
        std::fs::write(root.join("graphs/twice.toml"), twice).unwrap();
        let (_, aliases) = flatten(&root, &graph("format = 1
[[node]]
id = \"c1\"
folder = \"nodes/number\"
[[node]]
id = \"f1\"
subgraph = \"twice\"
[[wire]]
from = \"c1.out\"
to = \"f1.0000000e\"
"), &[]).unwrap();
        let deep = "f1/00000000000000e1/00000000000000a1.out".to_owned();
        let at = |pin: &str| (pin.to_owned(), "c1.out".to_owned());
        assert_eq!(aliases, [("f1.0000000d".to_owned(), deep.clone()), ("f1/00000000000000e1.0000000b".to_owned(), deep), at("f1/00000000000000e1/in:0000000a.pin"), at("f1/in:0000000e.pin")]);
        std::fs::remove_file(root.join("graphs/twice.toml")).unwrap();

        // A wire from an output that is not connected inside gives no value.
        std::fs::write(root.join("graphs/open.toml"), "format = 1\n[[output]]\nkey = \"0000000c\"\nname = \"y\"\npos = [0.0, 0.0]\n").unwrap();
        let open = graph("format = 1\n[[node]]\nid = \"00000000000000d1\"\nsubgraph = \"open\"\n[[node]]\nid = \"00000000000000c2\"\nfolder = \"nodes/show\"\n[[wire]]\nfrom = \"00000000000000d1.0000000c\"\nto = \"00000000000000c2.value\"\n");
        let (flat, aliases) = flatten(&root, &open, &[]).unwrap();
        assert!(flat.wire.is_empty() && aliases.is_empty(), "{:?}", flat.wire);

        // A subgraph that uses itself, a pin that does not exist and a bad pin key give problems.
        std::fs::write(root.join("graphs/cycle.toml"), "format = 1\n[[node]]\nid = \"00000000000000e1\"\nsubgraph = \"cycle\"\n").unwrap();
        std::fs::write(root.join("graphs/dotted.toml"), "format = 1\n[[output]]\nkey = \"a.b\"\nname = \"y\"\npos = [0.0, 0.0]\n").unwrap();
        let bad = graph("format = 1\n[[node]]\nid = \"00000000000000d1\"\nsubgraph = \"cycle\"\n[[node]]\nid = \"00000000000000d2\"\nsubgraph = \"half\"\n[[node]]\nid = \"00000000000000d3\"\nsubgraph = \"dotted\"\n[[wire]]\nfrom = \"00000000000000d2.nothing\"\nto = \"00000000000000d2.0000000a\"\n");
        let problems = flatten(&root, &bad, &[]).unwrap_err();
        std::fs::remove_file(root.join("graphs/dotted.toml")).unwrap();
        // The node list shows each subgraph with its public pins. The add folder is missing here.
        let infos = infos(&root, &[]).1;
        std::fs::remove_dir_all(&root).unwrap();
        let names: Vec<&str> = infos.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["cycle", "half", "open"]);
        let input = &infos[1].inputs[0];
        assert_eq!((input.key.as_str(), input.name.as_deref(), input.ty.as_str()), ("0000000a", Some("x"), "?"));
        assert!(problems.iter().any(|p| p.contains("the subgraph cycle uses itself")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("has no output `nothing`")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("the subgraph dotted has the pin key \"a.b\"")), "{problems:?}");
    }

    #[test]
    fn a_pin_of_a_subgraph_gets_the_value_type_and_stays_generic() {
        let root = std::env::temp_dir().join(format!("patina-subgraph-types-{}", std::process::id()));
        for (name, code) in [("gate", "pub fn gate(a: &bool) -> bool { *a }"), ("pick", "pub fn pick<T: Copy>(a: T) -> T { a }")] {
            std::fs::create_dir_all(root.join("nodes").join(name).join("src")).unwrap();
            std::fs::write(root.join("nodes").join(name).join("src/lib.rs"), format!("#[patina_sdk::node]
{code}")).unwrap();
            std::fs::write(root.join("nodes").join(name).join("Cargo.toml"), format!("[package]
name = \"{name}\"
")).unwrap();
        }
        std::fs::create_dir_all(root.join("graphs")).unwrap();
        let pins = "[[node]]
id = \"00000000000000a1\"
folder = \"nodes/gate\"
[[node]]
id = \"00000000000000a2\"
folder = \"nodes/pick\"
[[node]]
id = \"00000000000000a3\"
builtin = \"delay\"
[[wire]]
from = \"00000000000000a1.out\"
to = \"00000000000000a3.value\"
[[output]]
key = \"0000000c\"
name = \"q\"
pos = [0.0, 0.0]
from = \"00000000000000a3.out\"
[[output]]
key = \"0000000d\"
name = \"p\"
pos = [0.0, 0.0]
from = \"00000000000000a2.out\"
                    [[input]]
key = \"0000000a\"
name = \"a\"
pos = [0.0, 0.0]
to = [\"00000000000000a1.a\"]
[[input]]
key = \"0000000b\"
name = \"b\"
pos = [0.0, 0.0]
to = [\"00000000000000a2.a\"]
";
        std::fs::write(root.join("graphs/both.toml"), format!("format = 1
{pins}")).unwrap();
        // The open subgraph gives its pins from its edits, not from its file.
        let mut open = graph(&std::fs::read_to_string(root.join("graphs/both.toml")).unwrap());
        open.input.pop();
        let edited = infos(&root, &[("both", &open)]).1;
        let infos = infos(&root, &[]).1;
        std::fs::remove_dir_all(&root).unwrap();
        let pins: Vec<(&str, &str)> = edited[0].inputs.iter().map(|p| (p.key.as_str(), p.ty.as_str())).collect();
        assert_eq!(pins, [("0000000a", "bool")]);
        let types: Vec<&str> = infos[0].inputs.iter().map(|p| p.ty.as_str()).collect();
        assert_eq!((types, &infos[0].generics[..]), (vec!["bool", "T"], &["T".to_owned()][..]));
        // A delay output takes the type of its input wire. An output of a generic node stays generic.
        let outs: Vec<&str> = infos[0].outputs.iter().map(|p| p.ty.as_str()).collect();
        assert_eq!(outs, ["bool", "T"]);
    }
}
