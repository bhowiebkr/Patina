//! Declared, typed subgraph boundaries. Implementations remain ordinary pinned WESL nodes.
use crate::{Graph, Node, NodeId, Socket};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt::Write;

pub const PREFIX: &str = "forge_subgraph_";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Interface {
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Port {
    pub id: String,
    pub label: String,
    pub socket: Socket,
    pub node: NodeId,
}

pub fn reference(node: &Node) -> Option<&str> {
    if node.kind == "material_instance" {
        node.text.get("reference").map(String::as_str)
    } else {
        node.kind.strip_prefix(PREFIX)
    }
}

pub fn port_kind(socket: Socket) -> &'static str {
    match socket {
        Socket::Gray => "graph_port_gray",
        Socket::Color => "graph_port_color",
        Socket::Material => "graph_port_material",
    }
}
fn type_name(socket: Socket) -> &'static str {
    match socket {
        Socket::Gray => "Gray",
        Socket::Color => "Color",
        Socket::Material => "Material",
    }
}
fn label(text: &str) -> String {
    text.chars()
        .map(|c| {
            if matches!(c, '\n' | '\r' | '"') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

impl Interface {
    pub fn validate(&self, graph: &Graph) -> Result<(), String> {
        if self.outputs.is_empty() || self.inputs.len() + self.outputs.len() > 16 {
            return Err(
                "A subgraph needs an output and supports at most 16 total interface ports".into(),
            );
        }
        let mut ids = BTreeSet::new();
        let mut nodes = BTreeSet::new();
        for port in self.inputs.iter().chain(&self.outputs) {
            if port.id.is_empty()
                || port.id.len() > 64
                || !port
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                || !port.id.starts_with(['i', 'o'])
                || port.label.trim().is_empty()
                || port.label.len() > 256
                || !ids.insert(&port.id)
                || !nodes.insert(port.node)
                || graph
                    .node(port.node)
                    .is_none_or(|n| n.kind != port_kind(port.socket))
            {
                return Err("Invalid or duplicate subgraph interface port".into());
            }
        }
        Ok(())
    }

    pub fn contains(&self, node: NodeId) -> bool {
        self.inputs
            .iter()
            .chain(&self.outputs)
            .any(|p| p.node == node)
    }

    /// A wrapper keeps visible inputs for wiring and hidden inputs fed by the flattened graph.
    pub fn source(&self, name: &str) -> String {
        let mut source = format!(
            "//! name: {}\n//! category: Subgraphs\n//! description: Reusable graph with declared inputs and outputs.\n",
            label(name)
        );
        for p in &self.inputs {
            let _ = writeln!(
                source,
                "//! input: {} {} \"{}\"",
                p.id,
                type_name(p.socket),
                label(&p.label)
            );
        }
        for p in &self.outputs {
            let _ = writeln!(
                source,
                "//! input: result_{} {} hidden",
                p.id,
                type_name(p.socket)
            );
            let _ = writeln!(
                source,
                "//! output: {} {} \"{}\"",
                p.id,
                type_name(p.socket),
                label(&p.label)
            );
        }
        let values: Vec<_> = self
            .outputs
            .iter()
            .map(|p| {
                let accessor = match p.socket {
                    Socket::Gray => "gray",
                    Socket::Color => "in",
                    Socket::Material => "material",
                };
                format!("{accessor}_result_{}(p)", p.id)
            })
            .collect();
        let _ = writeln!(
            source,
            "\nfn eval(p: vec2<i32>) -> Out {{ return Out({}); }}",
            values.join(", ")
        );
        source
    }
}
