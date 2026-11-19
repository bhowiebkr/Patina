//! The collapse of nodes into a new subgraph and the expand of a use (spec 6.1).

use std::collections::HashMap;

use crate::edit::{cycle, node_of, used};
use crate::project::{self, Collar, GraphFile, Group, NodeEntry, Note, Order, PublicInput, PublicOutput, Style, Wire};

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
    if !project::is_id(use_id) || used(graph, use_id) {
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
    let mut inner = GraphFile { format: graph.format, style: graph.style, node: moved, ..GraphFile::default() };
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

    // An order wire of the use orders each node of the subgraph, so it can make a cycle that the
    // graph did not have. A cycle of data wires can run, and the plan check decides it (9.2).
    for o in outer.order.iter().filter(|o| o.before == use_id || o.after == use_id) {
        if let Some(path) = cycle(&outer, &o.before, &o.after) {
            return Err(format!("the subgraph makes a cycle with an order wire through these nodes: {}", path.join(", ")));
        }
    }
    Ok((outer, inner))
}

/// Puts the nodes of the subgraph `inner` in place of the use `use_id` of `graph` (6.1), the
/// reverse of a collapse. An inner node, group or note keeps its ID, unless the graph has the ID:
/// then `new_id` gives one. The wires of the public pins go to their inner ends, and a parameter of the use goes
/// to the inner inputs of its pin.
pub(crate) fn expand_use(graph: &GraphFile, use_id: &str, inner: &GraphFile, mut new_id: impl FnMut() -> String) -> Result<GraphFile, String> {
    let Some(entry) = graph.node.iter().find(|n| n.id == use_id && n.subgraph.is_some()) else {
        return Err(format!("the graph has no subgraph use {use_id}"));
    };
    let mut given: Vec<String> = Vec::new();
    let mut fresh = |old: &str| {
        let mut id = old.to_owned();
        while (id != use_id && used(graph, &id)) || given.contains(&id) {
            id = new_id();
        }
        given.push(id.clone());
        id
    };
    let ids: HashMap<&str, String> = inner.node.iter().map(|n| (n.id.as_str(), fresh(&n.id))).collect();
    let end = |e: &str| match e.split_once('.') {
        Some((id, key)) => format!("{}.{key}", ids.get(id).map_or(id, |v| v.as_str())),
        None => ids.get(e).cloned().unwrap_or_else(|| e.to_owned()),
    };
    let pin = |e: &str| e.split_once('.').filter(|(id, _)| *id == use_id).map(|(_, key)| key.to_owned());
    let targets = |key: &str| inner.input.iter().find(|p| p.key == key).map(|p| p.to.iter().map(|t| end(t)).collect::<Vec<_>>()).unwrap_or_default();
    let source = |key: &str| inner.output.iter().find(|p| p.key == key).map(|p| end(&p.from));

    // The inner nodes move with the use: their top left corner goes to the position of the use.
    let at = entry.pos.unwrap_or_default();
    let corner = match inner.node.is_empty() {
        true => at,
        false => inner.node.iter().map(|n| n.pos.unwrap_or_default()).fold([f64::MAX, f64::MAX], |c, p| [c[0].min(p[0]), c[1].min(p[1])]),
    };
    let shift = |p: [f64; 2]| [p[0] - corner[0] + at[0], p[1] - corner[1] + at[1]];
    let mut out = GraphFile { node: Vec::new(), wire: Vec::new(), order: Vec::new(), ..graph.clone() };
    // The groups and the notes of the subgraph keep their places at its nodes (15.12).
    out.group.extend(inner.group.iter().map(|g| Group { id: fresh(&g.id), pos: shift(g.pos), ..g.clone() }));
    out.note.extend(inner.note.iter().map(|n| Note { id: fresh(&n.id), pos: shift(n.pos), ..n.clone() }));
    for node in &graph.node {
        if node.id != use_id {
            out.node.push(node.clone());
            continue;
        }
        for n in &inner.node {
            let pos = n.pos.map(shift);
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
    out.wire.extend(inner.wire.iter().map(|w| Wire { from: end(&w.from), to: end(&w.to), style: Style::moved(w.style, inner.style, graph.style), ..w.clone() }));
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
    use super::super::tests::graph;
    use super::*;

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

        // a1 and c1 around b1 make a cycle through the use and its order wire to b2. An empty
        // selection does nothing.
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

        // A collapse keeps the groups in the outer graph. An expand adds the groups and the notes of
        // the subgraph at the place of its nodes, with a new ID for an ID that the graph has.
        let group = Group { id: "00000000000000f1".to_owned(), title: "Grade".to_owned(), color: project::GroupColor::Teal, pos: [90.0, -10.0], size: [320.0, 200.0] };
        let mut with = main.clone();
        with.group.push(group.clone());
        let (outer, mut inner) = collapse(&with, &ids, "00000000000000d1", "part", || "00000001".to_owned()).unwrap();
        assert_eq!((outer.group.len(), inner.group.len()), (1, 0));
        inner.group.push(group);
        inner.note.push(Note { id: "00000000000000f2".to_owned(), pos: [100.0, 300.0], width: 200.0, text: "hi".to_owned() });
        let back = expand_use(&outer, "00000000000000d1", &inner, || "00000000000000e2".to_owned()).unwrap();
        let groups: Vec<(&str, [f64; 2])> = back.group.iter().map(|g| (g.id.as_str(), g.pos)).collect();
        assert_eq!(groups, [("00000000000000f1", [90.0, -10.0]), ("00000000000000e2", [90.0, -10.0])]);
        assert_eq!((back.note[0].id.as_str(), back.note[0].pos), ("00000000000000f2", [100.0, 300.0]));
    }
}
