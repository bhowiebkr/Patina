//! Subgraphs (spec 6.1, 6.4): each use of a subgraph becomes the nodes and wires of its file. A
//! node of a use gets the ID `<use ID>/<node ID>`, so two uses of one subgraph do not share IDs.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::edit::{cycle, node_of};
use crate::project::{self, Collar, GraphFile, NodeEntry, Order, PublicInput, PublicOutput, Wire};
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
    let (flat, mut aliases) = expand(root, graph, open, &mut Vec::new(), &mut problems);
    // An inner public input pin can get its value from an outer public input pin.
    let map: HashMap<String, String> = aliases.iter().cloned().collect();
    for (_, from) in &mut aliases {
        while let Some(next) = map.get(from.as_str()) {
            from.clone_from(next);
        }
    }
    if problems.is_empty() { Ok((flat, aliases)) } else { Err(problems) }
}

fn expand(root: &Path, graph: &GraphFile, open: Option<(&str, &GraphFile)>, stack: &mut Vec<String>, problems: &mut Vec<String>) -> (GraphFile, Vec<Alias>) {
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
        let (inner, inner_aliases) = expand(root, &file, open, stack, problems);
        stack.pop();
        let prefix = |end: &str| format!("{id}/{end}");
        // The editor shows the values inside a use, so the uses inside it keep their aliases (15.9).
        nested.extend(inner_aliases.iter().map(|(outer, from)| (prefix(outer), prefix(from))));
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
    // An order wire to a use orders all the nodes of the use.
    let nodes = |id: &String| uses.get(id.as_str()).map_or_else(|| vec![id.clone()], |one| one.nodes.clone());
    for order in &graph.order {
        for before in nodes(&order.before) {
            for after in nodes(&order.after) {
                flat.order.push(Order { before: before.clone(), after });
            }
        }
    }
    let mut aliases: Vec<Alias> = nested.into_iter().chain(uses.iter().flat_map(|(id, one)| one.outputs.iter().filter(|(_, from)| !from.is_empty()).map(move |(key, from)| (format!("{id}.{key}"), from.clone())))).collect();
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
        let mut info = NodeInfo { name: name.to_owned(), inputs: Vec::new(), outputs: Vec::new(), error: None, builtin: false, generics: Vec::new(), full: false, color: None, subgraph: true };
        match project::read(&path, |g: &GraphFile| g.format) {
            Ok(graph) => {
                let stack = &mut vec![name.to_owned()];
                let mut generics = Vec::new();
                // A pin that ends at a generic pin is generic too, so the palette offers each type (4.5).
                let mut pin = |key: &str, pin: &str, end: Option<&String>, input| {
                    let (ty, of_node) = end.and_then(|end| end_type(root, &graph, end, input, stack)).unwrap_or_else(|| ("?".to_owned(), Vec::new()));
                    if of_node.contains(&ty) && !generics.contains(&ty) {
                        generics.push(ty.clone());
                    }
                    Pin { key: key.to_owned(), ty, name: Some(pin.to_owned()) }
                };
                info.inputs = graph.input.iter().map(|p| pin(&p.key, &p.name, p.to.first(), true)).collect();
                info.outputs = graph.output.iter().map(|p| pin(&p.key, &p.name, Some(&p.from), false)).collect();
                info.generics = generics;
            }
            Err(e) => info.error = Some(e.to_string()),
        }
        infos.push(info);
    }
    infos.sort_by(|a, b| a.name.cmp(&b.name));
    infos
}

/// The value type of the wire end `<node ID>.<pin key>` of a graph, through the uses of subgraphs,
/// and the type parameters of its node. An input `&bool` gives `bool`.
fn end_type(root: &Path, graph: &GraphFile, end: &str, input: bool, stack: &mut Vec<String>) -> Option<(String, Vec<String>)> {
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
    let ty = match input {
        true => node.inputs.iter().find(|i| i.key == key).map(|i| i.value.clone()),
        false => node.output_keys().into_iter().zip(node.output_types).find(|(k, _)| k == key).map(|(_, ty)| ty),
    }?;
    // A generic output, as of a delay, has the type of the wire into an input of the same type (4.5).
    let seen = format!("#{end}");
    if !input && node.generics.contains(&ty) && !stack.contains(&seen) {
        let to = node.inputs.iter().find(|i| i.value == ty).map(|i| format!("{id}.{}", i.key));
        if let Some(from) = to.and_then(|to| graph.wire.iter().find(|w| w.to == to)).map(|w| w.from.clone()) {
            stack.push(seen);
            let found = end_type(root, graph, &from, false, stack);
            stack.pop();
            if let Some((found, of)) = found.filter(|(found, of)| !of.contains(found)) {
                return Some((found, of));
            }
        }
    }
    Some((ty, node.generics))
}

/// Moves the nodes `ids` of `graph` into a new subgraph `name` (6.1). Gives the outer graph, with
/// the use `use_id` in place of the nodes, and the subgraph. `key` gives each new pin key. Each
/// outer pin that feeds the nodes and each inner output that feeds the outer graph gets a public pin.
pub(crate) fn collapse(graph: &GraphFile, ids: &[String], use_id: &str, name: &str, mut key: impl FnMut() -> String) -> Result<(GraphFile, GraphFile), String> {
    if ids.is_empty() {
        return Err("select one or more nodes".to_owned());
    }
    if let Some(id) = ids.iter().find(|id| !graph.node.iter().any(|n| &n.id == *id)) {
        return Err(format!("the graph has no node {id}"));
    }
    if !project::is_id(use_id) || graph.node.iter().any(|n| n.id == use_id) {
        return Err(format!("{use_id:?} is not a new node ID"));
    }
    let inside = |end: &str| ids.iter().any(|id| id == node_of(end));
    let (moved, kept): (Vec<NodeEntry>, Vec<NodeEntry>) = graph.node.iter().cloned().partition(|n| inside(&n.id));
    let at: Vec<[f64; 2]> = moved.iter().map(|n| n.pos.unwrap_or_default()).collect();
    let (left, top) = at.iter().fold((f64::MAX, f64::MAX), |(x, y), p| (x.min(p[0]), y.min(p[1])));
    let right = at.iter().fold(f64::MIN, |x, p| x.max(p[0]));
    let use_entry = NodeEntry { id: use_id.to_owned(), folder: None, subgraph: Some(name.to_owned()), builtin: None, pos: Some([left, top]), domain: None, settings: None, params: Default::default() };
    let mut outer = GraphFile { node: kept, wire: Vec::new(), order: Vec::new(), input: Vec::new(), output: Vec::new(), ..graph.clone() };
    outer.node.push(use_entry);
    let mut inner = GraphFile { format: graph.format, node: moved, wire: Vec::new(), order: Vec::new(), collar: Vec::new(), cable: Vec::new(), input: Vec::new(), output: Vec::new() };
    let mut names = Vec::new();
    let pin = |end: &str| end.split_once('.').map_or(end, |(_, key)| key).to_owned();

    // One public input for each outer source: an output pin, or `#<key>` for a public input of
    // the graph. Gives the key of a new input.
    let mut sources: Vec<String> = Vec::new();
    let mut input = |source: String, to: &str, inner: &mut GraphFile, names: &mut Vec<String>| {
        if let Some(i) = sources.iter().position(|s| *s == source) {
            inner.input[i].to.push(to.to_owned());
            return None;
        }
        let k = key();
        let pos = [left - 240.0, top + 60.0 * sources.len() as f64];
        inner.input.push(PublicInput { key: k.clone(), name: unique(names, &pin(to)), pos, to: vec![to.to_owned()] });
        sources.push(source);
        Some(k)
    };
    for w in &graph.wire {
        match (inside(&w.from), inside(&w.to)) {
            (true, true) => inner.wire.push(w.clone()),
            (false, false) => outer.wire.push(w.clone()),
            (false, true) => {
                if let Some(k) = input(w.from.clone(), &w.to, &mut inner, &mut names) {
                    outer.wire.push(Wire { from: w.from.clone(), to: format!("{use_id}.{k}"), name: w.name.clone(), style: w.style, corners: w.corners, points: Vec::new() });
                }
            }
            (true, false) => {}
        }
    }
    for p in &graph.input {
        let mut to = Vec::new();
        for t in &p.to {
            if !inside(t) {
                to.push(t.clone());
            } else if let Some(k) = input(format!("#{}", p.key), t, &mut inner, &mut names) {
                to.push(format!("{use_id}.{k}"));
            }
        }
        outer.input.push(PublicInput { to, ..p.clone() });
    }

    // One public output for each inner output that feeds the outer graph.
    let mut output = |from: &str, inner: &mut GraphFile, names: &mut Vec<String>| {
        if let Some(p) = inner.output.iter().find(|p| p.from == from) {
            return format!("{use_id}.{}", p.key);
        }
        let k = key();
        let pos = [right + 280.0, top + 60.0 * inner.output.len() as f64];
        inner.output.push(PublicOutput { key: k.clone(), name: unique(names, &pin(from)), pos, from: from.to_owned() });
        format!("{use_id}.{k}")
    };
    for w in graph.wire.iter().filter(|w| inside(&w.from) && !inside(&w.to)) {
        outer.wire.push(Wire { from: output(&w.from, &mut inner, &mut names), points: Vec::new(), ..w.clone() });
    }
    for p in &graph.output {
        let from = if inside(&p.from) { output(&p.from, &mut inner, &mut names) } else { p.from.clone() };
        outer.output.push(PublicOutput { from, ..p.clone() });
    }

    for o in &graph.order {
        if inside(&o.before) && inside(&o.after) {
            inner.order.push(o.clone());
            continue;
        }
        let map = |id: &String| if inside(id) { use_id.to_owned() } else { id.clone() };
        let order = Order { before: map(&o.before), after: map(&o.after) };
        if !outer.order.contains(&order) {
            outer.order.push(order);
        }
    }
    // The collars and the cables keep only the wires to outer inputs.
    let keep = |wires: &[String]| wires.iter().filter(|w| !inside(w)).cloned().collect::<Vec<_>>();
    outer.collar = graph.collar.iter().map(|c| Collar { wires: keep(&c.wires), ..c.clone() }).filter(|c| !c.wires.is_empty()).collect();
    outer.cable = graph.cable.iter().map(|c| project::Cable { wires: keep(&c.wires), ..c.clone() }).filter(|c| !c.wires.is_empty()).collect();

    let next = outer.wire.iter().filter(|w| node_of(&w.from) == use_id).map(|w| node_of(&w.to));
    for after in next.chain(outer.order.iter().filter(|o| o.before == use_id).map(|o| o.after.as_str())) {
        if let Some(path) = cycle(&outer, use_id, after) {
            return Err(format!("the subgraph makes a cycle through these nodes: {}", path.join(", ")));
        }
    }
    Ok((outer, inner))
}

/// Puts the nodes of the subgraph `inner` in place of the use `use_id` of `graph` (6.1), the
/// reverse of a collapse. An inner node keeps its ID, unless the graph has the ID: then `new_id`
/// gives one. The wires of the public pins go to their inner ends, and a parameter of the use goes
/// to the inner inputs of its pin.
pub(crate) fn expand_use(graph: &GraphFile, use_id: &str, inner: &GraphFile, mut new_id: impl FnMut() -> String) -> Result<GraphFile, String> {
    let Some(entry) = graph.node.iter().find(|n| n.id == use_id && n.subgraph.is_some()) else {
        return Err(format!("the graph has no subgraph use {use_id}"));
    };
    let mut ids = HashMap::new();
    for node in &inner.node {
        let mut id = node.id.clone();
        while graph.node.iter().any(|n| n.id == id && n.id != use_id) || ids.values().any(|v| *v == id) {
            id = new_id();
        }
        ids.insert(node.id.as_str(), id);
    }
    let end = |e: &str| match e.split_once('.') {
        Some((id, key)) => format!("{}.{key}", ids.get(id).map_or(id, |v| v.as_str())),
        None => ids.get(e).cloned().unwrap_or_else(|| e.to_owned()),
    };
    let pin = |e: &str| e.split_once('.').filter(|(id, _)| *id == use_id).map(|(_, key)| key.to_owned());
    let targets = |key: &str| inner.input.iter().find(|p| p.key == key).map(|p| p.to.iter().map(|t| end(t)).collect::<Vec<_>>()).unwrap_or_default();
    let source = |key: &str| inner.output.iter().find(|p| p.key == key).map(|p| end(&p.from));

    // The inner nodes move with the use: their top left corner goes to the position of the use.
    let at = entry.pos.unwrap_or_default();
    let corner = inner.node.iter().map(|n| n.pos.unwrap_or_default()).fold([f64::MAX, f64::MAX], |c, p| [c[0].min(p[0]), c[1].min(p[1])]);
    let mut out = GraphFile { node: Vec::new(), wire: Vec::new(), order: Vec::new(), ..graph.clone() };
    for node in &graph.node {
        if node.id != use_id {
            out.node.push(node.clone());
            continue;
        }
        for n in &inner.node {
            let pos = n.pos.map(|p| [p[0] - corner[0] + at[0], p[1] - corner[1] + at[1]]);
            let mut params = n.params.clone();
            let id = &ids[n.id.as_str()];
            for (key, text) in &entry.params {
                for t in targets(key) {
                    if let Some(input) = t.strip_prefix(id.as_str()).and_then(|rest| rest.strip_prefix('.')) {
                        params.insert(input.to_owned(), text.clone());
                    }
                }
            }
            out.node.push(NodeEntry { id: id.clone(), pos, params, ..n.clone() });
        }
    }
    out.wire.extend(inner.wire.iter().map(|w| Wire { from: end(&w.from), to: end(&w.to), ..w.clone() }));
    for w in &graph.wire {
        let from = match pin(&w.from) {
            Some(key) => source(&key).ok_or(format!("the subgraph has no output {key}"))?,
            None => w.from.clone(),
        };
        if from.is_empty() {
            continue;
        }
        let to = pin(&w.to).map_or_else(|| vec![w.to.clone()], |key| targets(&key));
        out.wire.extend(to.into_iter().map(|to| Wire { from: from.clone(), to, ..w.clone() }));
    }
    for p in &mut out.input {
        p.to = p.to.iter().flat_map(|t| pin(t).map_or_else(|| vec![t.clone()], |key| targets(&key))).collect();
    }
    for p in &mut out.output {
        if let Some(key) = pin(&p.from) {
            p.from = source(&key).ok_or(format!("the subgraph has no output {key}"))?;
        }
    }
    // An order wire of the use orders each inner node.
    let nodes = |id: &String| -> Vec<String> { if id == use_id { inner.node.iter().map(|n| ids[n.id.as_str()].clone()).collect() } else { vec![id.clone()] } };
    let mut orders: Vec<Order> = inner.order.iter().map(|o| Order { before: end(&o.before), after: end(&o.after) }).collect();
    for o in &graph.order {
        for before in nodes(&o.before) {
            for after in nodes(&o.after) {
                orders.push(Order { before: before.clone(), after });
            }
        }
    }
    for order in orders {
        if !out.order.contains(&order) {
            out.order.push(order);
        }
    }
    // The collars and the cables drop the wires to the use.
    let keep = |wires: &[String]| wires.iter().filter(|w| pin(w).is_none()).cloned().collect::<Vec<_>>();
    out.collar = graph.collar.iter().map(|c| Collar { wires: keep(&c.wires), ..c.clone() }).filter(|c| !c.wires.is_empty()).collect();
    out.cable = graph.cable.iter().map(|c| project::Cable { wires: keep(&c.wires), ..c.clone() }).filter(|c| !c.wires.is_empty()).collect();
    Ok(out)
}

/// Gives `base`, or `base_2`, `base_3` and so on when the name is in `names`, and adds it to `names`.
fn unique(names: &mut Vec<String>, base: &str) -> String {
    let mut name = base.to_owned();
    for i in 2.. {
        if !names.contains(&name) {
            break;
        }
        name = format!("{base}_{i}");
    }
    names.push(name.clone());
    name
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
"), None).unwrap();
        let deep = "f1/00000000000000e1/00000000000000a1.out".to_owned();
        let at = |pin: &str| (pin.to_owned(), "c1.out".to_owned());
        assert_eq!(aliases, [("f1.0000000d".to_owned(), deep.clone()), ("f1/00000000000000e1.0000000b".to_owned(), deep), at("f1/00000000000000e1/in:0000000a.pin"), at("f1/in:0000000e.pin")]);
        std::fs::remove_file(root.join("graphs/twice.toml")).unwrap();

        // A wire from an output that is not connected inside gives no value.
        std::fs::write(root.join("graphs/open.toml"), "format = 1\n[[output]]\nkey = \"0000000c\"\nname = \"y\"\npos = [0.0, 0.0]\n").unwrap();
        let open = graph("format = 1\n[[node]]\nid = \"00000000000000d1\"\nsubgraph = \"open\"\n[[node]]\nid = \"00000000000000c2\"\nfolder = \"nodes/show\"\n[[wire]]\nfrom = \"00000000000000d1.0000000c\"\nto = \"00000000000000c2.value\"\n");
        let (flat, aliases) = flatten(&root, &open, None).unwrap();
        assert!(flat.wire.is_empty() && aliases.is_empty(), "{:?}", flat.wire);

        // A subgraph that uses itself and a pin that does not exist give problems.
        std::fs::write(root.join("graphs/cycle.toml"), "format = 1\n[[node]]\nid = \"00000000000000e1\"\nsubgraph = \"cycle\"\n").unwrap();
        let bad = graph("format = 1\n[[node]]\nid = \"00000000000000d1\"\nsubgraph = \"cycle\"\n[[node]]\nid = \"00000000000000d2\"\nsubgraph = \"half\"\n[[wire]]\nfrom = \"00000000000000d2.nothing\"\nto = \"00000000000000d2.0000000a\"\n");
        let problems = flatten(&root, &bad, None).unwrap_err();
        // The node list shows each subgraph with its public pins. The add folder is missing here.
        let infos = infos(&root);
        std::fs::remove_dir_all(&root).unwrap();
        let names: Vec<&str> = infos.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["cycle", "half", "open"]);
        let input = &infos[1].inputs[0];
        assert_eq!((input.key.as_str(), input.name.as_deref(), input.ty.as_str()), ("0000000a", Some("x"), "?"));
        assert!(problems.iter().any(|p| p.contains("the subgraph cycle uses itself")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("has no output `nothing`")), "{problems:?}");
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
        let infos = infos(&root);
        std::fs::remove_dir_all(&root).unwrap();
        let types: Vec<&str> = infos[0].inputs.iter().map(|p| p.ty.as_str()).collect();
        assert_eq!((types, &infos[0].generics[..]), (vec!["bool", "T"], &["T".to_owned()][..]));
        // A delay output takes the type of its input wire. An output of a generic node stays generic.
        let outs: Vec<&str> = infos[0].outputs.iter().map(|p| p.ty.as_str()).collect();
        assert_eq!(outs, ["bool", "T"]);
    }

    #[test]
    fn a_collapse_gives_public_pins_for_the_wires_that_cross() {
        // a1 feeds both inputs of b1, b1 feeds b2, b2 feeds c1 and the public output.
        let main = graph(
            r#"format = 1
[[node]]
id = "00000000000000a1"
[[node]]
id = "00000000000000b1"
pos = [100.0, 50.0]
[[node]]
id = "00000000000000b2"
pos = [300.0, 0.0]
[[node]]
id = "00000000000000c1"
[[wire]]
from = "00000000000000a1.out"
to = "00000000000000b1.a"
[[wire]]
from = "00000000000000a1.out"
to = "00000000000000b1.b"
[[wire]]
from = "00000000000000b1.out"
to = "00000000000000b2.a"
[[wire]]
from = "00000000000000b2.out"
to = "00000000000000c1.value"
[[output]]
key = "0000000f"
name = "total"
pos = [0.0, 0.0]
from = "00000000000000b2.out"
[[order]]
before = "00000000000000a1"
after = "00000000000000b2"
"#,
        );
        let mut n = 0;
        let key = || {
            n += 1;
            format!("{n:08x}")
        };
        let ids = ["00000000000000b1".to_owned(), "00000000000000b2".to_owned()];
        let (outer, inner) = collapse(&main, &ids, "00000000000000d1", "part", key).unwrap();
        let wires = |g: &GraphFile| g.wire.iter().map(|w| format!("{} {}", w.from, w.to)).collect::<Vec<_>>();
        assert_eq!(wires(&inner), ["00000000000000b1.out 00000000000000b2.a"]);
        assert_eq!(wires(&outer), ["00000000000000a1.out 00000000000000d1.00000001", "00000000000000d1.00000002 00000000000000c1.value"]);
        assert_eq!((inner.input[0].name.as_str(), inner.input[0].to.len(), inner.input[0].pos), ("a", 2, [-140.0, 0.0]));
        assert_eq!((inner.output.len(), inner.output[0].name.as_str()), (1, "out"));
        assert_eq!(outer.output[0].from, "00000000000000d1.00000002");
        assert_eq!((outer.order[0].before.as_str(), outer.order[0].after.as_str()), ("00000000000000a1", "00000000000000d1"));
        let use_entry = outer.node.last().unwrap();
        assert_eq!((use_entry.subgraph.as_deref(), use_entry.pos), (Some("part"), Some([100.0, 0.0])));

        // a1 and c1 around b1 make a cycle through the use. An empty selection does nothing.
        let ends = ["00000000000000a1".to_owned(), "00000000000000c1".to_owned()];
        let mut main = main;
        main.wire.push(Wire { from: "00000000000000c1.out".to_owned(), to: "00000000000000b1.c".to_owned(), name: None, style: None, corners: None, points: Vec::new() });
        assert!(collapse(&main, &ends, "00000000000000d1", "part", String::new).unwrap_err().contains("cycle"));
        assert!(collapse(&main, &[], "00000000000000d1", "part", String::new).is_err());
        assert!(collapse(&main, &ids, "00000000000000a1", "part", String::new).is_err(), "the use ID is new");

        // An expand gives the nodes and the wires again, at the same positions.
        main.wire.pop();
        let (outer, inner) = collapse(&main, &ids, "00000000000000d1", "part", || "00000001".to_owned()).unwrap();
        let back = expand_use(&outer, "00000000000000d1", &inner, String::new).unwrap();
        let sorted = |g: &GraphFile| {
            let mut list = wires(g);
            list.sort();
            list
        };
        assert_eq!(sorted(&back), sorted(&main));
        assert_eq!(back.node.iter().map(|n| (n.id.as_str(), n.pos)).collect::<Vec<_>>()[2..], [("00000000000000b1", Some([100.0, 50.0])), ("00000000000000b2", Some([300.0, 0.0]))]);
        assert_eq!(back.output[0].from, "00000000000000b2.out");
        assert!(back.order.contains(&main.order[0]));

        // An inner ID that the graph has gets a new ID. A parameter of the use goes to the inner inputs.
        let mut taken = outer.clone();
        taken.node.push(NodeEntry { id: "00000000000000b1".to_owned(), ..main.node[0].clone() });
        taken.node[2].params.insert("00000001".to_owned(), "5".to_owned());
        let back = expand_use(&taken, "00000000000000d1", &inner, || "00000000000000e1".to_owned()).unwrap();
        assert!(wires(&back).contains(&"00000000000000e1.out 00000000000000b2.a".to_owned()), "{:?}", wires(&back));
        let e1 = back.node.iter().find(|n| n.id == "00000000000000e1").unwrap();
        assert_eq!((e1.params["a"].as_str(), e1.params["b"].as_str()), ("5", "5"));
        assert!(expand_use(&outer, "00000000000000a1", &inner, String::new).is_err(), "a1 is not a use");
    }
}
