//! Subgraphs (spec 6.1, 6.4): each use of a subgraph becomes the nodes and wires of its file. A
//! node of a use gets the ID `<use ID>/<node ID>`, so two uses of one subgraph do not share IDs.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::project::{self, GraphFile, Order, Wire};
use crate::{NodeInfo, Pin, node, plan};

/// The inner ends of the public pins of one use, and the IDs of its nodes.
struct Use {
    inputs: HashMap<String, Vec<String>>,
    outputs: HashMap<String, String>,
    nodes: Vec<String>,
}

/// The output pin `<use ID>.<pin key>` of a use and the inner output that gives its value.
pub(crate) type Alias = (String, String);

/// Gives the graph with each use of a subgraph replaced by the nodes of the subgraph file, and
/// the aliases of the output pins of the uses. The collars and cables stay out, because only the
/// editor draws them. `open` is a subgraph of the editor that replaces its file.
pub(crate) fn flatten(root: &Path, graph: &GraphFile, open: Option<(&str, &GraphFile)>) -> Result<(GraphFile, Vec<Alias>), Vec<String>> {
    let mut problems = Vec::new();
    let flat = expand(root, graph, open, &mut Vec::new(), &mut problems);
    if problems.is_empty() { Ok(flat) } else { Err(problems) }
}

fn expand(root: &Path, graph: &GraphFile, open: Option<(&str, &GraphFile)>, stack: &mut Vec<String>, problems: &mut Vec<String>) -> (GraphFile, Vec<Alias>) {
    if graph.node.iter().all(|n| n.subgraph.is_none()) {
        return (graph.clone(), Vec::new());
    }
    let mut flat = GraphFile { node: Vec::new(), wire: Vec::new(), order: Vec::new(), collar: Vec::new(), cable: Vec::new(), ..graph.clone() };
    let mut uses: HashMap<&str, Use> = HashMap::new();
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
        if let Err(text) = plan::check_name(name) {
            fail(format!("the subgraph name {name:?}: {text}"));
            continue;
        }
        if stack.contains(name) {
            fail(format!("the subgraph {name} uses itself"));
            continue;
        }
        let file = match open.filter(|(open, _)| open == name) {
            Some((_, graph)) => Ok(graph.clone()),
            None => project::read(&root.join("graphs").join(format!("{name}.toml")), |g: &GraphFile| g.format),
        };
        let file = match file {
            Ok(file) => file,
            Err(e) => {
                fail(e.to_string());
                continue;
            }
        };
        stack.push(name.clone());
        let inner = expand(root, &file, open, stack, problems).0;
        stack.pop();
        let prefix = |end: &str| format!("{id}/{end}");
        let mut one = Use { inputs: HashMap::new(), outputs: HashMap::new(), nodes: Vec::new() };
        for pin in &inner.input {
            if one.inputs.insert(pin.key.clone(), pin.to.iter().map(|to| prefix(to)).collect()).is_some() {
                fail(format!("the subgraph {name} has two inputs with the key {}", pin.key));
            }
        }
        for pin in &inner.output {
            if one.outputs.insert(pin.key.clone(), prefix(&pin.from)).is_some() {
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
            let Some(targets) = one.inputs.get(key) else {
                fail(format!("the subgraph {name} has no input `{key}` for the parameter"));
                continue;
            };
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

    // The wires and the public pins that end at a use go to the inner ends of its pins.
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
        let from = source(&wire.from, problems);
        for to in targets(&wire.to, problems) {
            flat.wire.push(Wire { from: from.clone(), to, ..wire.clone() });
        }
    }
    for pin in &mut flat.input {
        pin.to = pin.to.iter().flat_map(|to| targets(to, problems)).collect();
    }
    for pin in &mut flat.output {
        pin.from = source(&pin.from, problems);
    }
    // An order wire to a use orders all the nodes of the use.
    let nodes = |id: &String| uses.get(id.as_str()).map_or_else(|| vec![id.clone()], |one| one.nodes.clone());
    for order in &graph.order {
        for before in nodes(&order.before) {
            for after in nodes(&order.after) {
                flat.order.push(Order { before: before.clone(), after });
            }
        }
    }
    let mut aliases: Vec<Alias> = uses.iter().flat_map(|(id, one)| one.outputs.iter().map(move |(key, from)| (format!("{id}.{key}"), from.clone()))).collect();
    aliases.sort();
    (flat, aliases)
}

/// The subgraph files of `graphs/`, without the main graph, for the node list of the editor.
pub(crate) fn infos(root: &Path) -> Vec<NodeInfo> {
    let mut infos = Vec::new();
    for entry in fs::read_dir(root.join("graphs")).into_iter().flatten().flatten() {
        let path = entry.path();
        let toml = path.extension().is_some_and(|e| e == "toml");
        let Some(name) = path.file_stem().and_then(|s| s.to_str()).filter(|n| toml && *n != "main") else { continue };
        let mut info = NodeInfo { name: name.to_owned(), inputs: Vec::new(), outputs: Vec::new(), error: None, builtin: false, generics: Vec::new(), full: false, subgraph: true };
        match project::read(&path, |g: &GraphFile| g.format) {
            Ok(graph) => {
                let stack = &mut vec![name.to_owned()];
                // shortcut: a pin that ends at a generic input gets the name of the type parameter, change it when the editor shows subgraphs with generic nodes.
                let mut pin = |key: &str, pin: &str, end: Option<&String>, input| Pin {
                    key: key.to_owned(),
                    ty: end.and_then(|end| end_type(root, &graph, end, input, stack)).unwrap_or_else(|| "?".to_owned()),
                    name: Some(pin.to_owned()),
                };
                info.inputs = graph.input.iter().map(|p| pin(&p.key, &p.name, p.to.first(), true)).collect();
                info.outputs = graph.output.iter().map(|p| pin(&p.key, &p.name, Some(&p.from), false)).collect();
            }
            Err(e) => info.error = Some(e.to_string()),
        }
        infos.push(info);
    }
    infos.sort_by(|a, b| a.name.cmp(&b.name));
    infos
}

/// The Rust type of the wire end `<node ID>.<pin key>` of a graph, through the uses of subgraphs.
fn end_type(root: &Path, graph: &GraphFile, end: &str, input: bool, stack: &mut Vec<String>) -> Option<String> {
    let (id, key) = end.split_once('.')?;
    let entry = graph.node.iter().find(|n| n.id == id)?;
    let node = match (&entry.folder, &entry.builtin, &entry.subgraph) {
        (Some(folder), None, None) => plan::read_node(root, folder.strip_prefix("nodes/")?).ok()?,
        (None, Some(builtin), None) if builtin == plan::DELAY => node::delay(),
        (None, None, Some(name)) if plan::check_name(name).is_ok() && !stack.contains(name) => {
            let inner = project::read(&root.join("graphs").join(format!("{name}.toml")), |g: &GraphFile| g.format).ok()?;
            let end = match input {
                true => inner.input.iter().find(|p| p.key == key)?.to.first()?.clone(),
                false => inner.output.iter().find(|p| p.key == key)?.from.clone(),
            };
            stack.push(name.clone());
            let ty = end_type(root, &inner, &end, input, stack);
            stack.pop();
            return ty;
        }
        _ => return None,
    };
    match input {
        true => node.inputs.into_iter().find(|i| i.key == key).map(|i| i.ty),
        false => node.output_keys().into_iter().zip(node.output_types).find(|(k, _)| k == key).map(|(_, ty)| ty),
    }
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

    fn graph(text: &str) -> GraphFile {
        toml::from_str(text).unwrap()
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
        let (flat, aliases) = flatten(&root, &main, None).unwrap();
        let inner = |id: &str| format!("{id}/00000000000000a1.out");
        assert_eq!(aliases, [("00000000000000d1.0000000b".to_owned(), inner("00000000000000d1")), ("00000000000000d2.0000000b".to_owned(), inner("00000000000000d2"))]);
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

        // A subgraph that uses itself and a pin that does not exist give problems.
        std::fs::write(root.join("graphs/cycle.toml"), "format = 1\n[[node]]\nid = \"00000000000000e1\"\nsubgraph = \"cycle\"\n").unwrap();
        let bad = graph("format = 1\n[[node]]\nid = \"00000000000000d1\"\nsubgraph = \"cycle\"\n[[node]]\nid = \"00000000000000d2\"\nsubgraph = \"half\"\n[[wire]]\nfrom = \"00000000000000d2.nothing\"\nto = \"00000000000000d2.0000000a\"\n");
        let problems = flatten(&root, &bad, None).unwrap_err();
        // The node list shows each subgraph with its public pins. The add folder is missing here.
        let infos = infos(&root);
        std::fs::remove_dir_all(&root).unwrap();
        let names: Vec<&str> = infos.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["cycle", "half"]);
        let input = &infos[1].inputs[0];
        assert_eq!((input.key.as_str(), input.name.as_deref(), input.ty.as_str()), ("0000000a", Some("x"), "?"));
        assert!(problems.iter().any(|p| p.contains("the subgraph cycle uses itself")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("has no output `nothing`")), "{problems:?}");
    }
}
