//! The `Graph` handle of a view (spec 15.8).

use std::fmt::Display;
use std::str::FromStr;

/// The access of a view to the graph (15.8). The engine gives a copy of the graph and of the live
/// values before each call of `ui`. A write applies on the next run, through the edit path of the
/// editor, so undo removes it.
///
/// ```
/// let mut graph = patina_sdk::Graph::new("node\tclock\tclock\nout\tclock\tframe\t42\nparam\tclock\trate\t60");
/// assert_eq!(graph.nodes().collect::<Vec<_>>(), ["clock"]);
/// assert_eq!(graph.output::<u64>("clock", "frame"), Some(42));
/// assert_eq!(graph.param("clock", "rate"), Some("60"));
/// graph.set_param("clock", "rate", 30);
/// assert_eq!(graph.edits().collect::<Vec<_>>(), ["set\tclock\trate\t30"]);
/// ```
pub struct Graph {
    /// One line for each item, with tabs between the fields: `node <id> <folder>`,
    /// `param <id> <key> <text>` and `out <id> <output> <text>`.
    lines: String,
    /// One line for each write: `set <id> <key> <text>`.
    edits: Vec<String>,
}

impl Graph {
    /// A handle on the copy `lines` of the graph. The engine makes this text.
    pub fn new(lines: &str) -> Graph {
        Graph { lines: lines.to_owned(), edits: Vec::new() }
    }

    /// The ID of each node of the open graph, in the order of the graph file.
    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.items("node").map(|[id, _, _]| id)
    }

    /// The node folder of node `id`, for example `nodes/number`. None for a subgraph or a built-in node.
    pub fn folder(&self, id: &str) -> Option<&str> {
        self.items("node").find(|[n, _, _]| *n == id).map(|[_, folder, _]| folder).filter(|f| !f.is_empty())
    }

    /// The text of the parameter `key` of node `id` (6.4). None when the input has its default.
    pub fn param(&self, id: &str, key: &str) -> Option<&str> {
        self.find("param", id, key)
    }

    /// The text of the live value of the output `key` of node `id` after the last run (15.9).
    /// It is the `Debug` text or the `Viewer` text of the value (5.3).
    pub fn output_text(&self, id: &str, key: &str) -> Option<&str> {
        self.find("out", id, key)
    }

    /// The live value of an output, read from its text. Use it for numbers and `bool`. The text of
    /// a `String` has quotes.
    pub fn output<T: FromStr>(&self, id: &str, key: &str) -> Option<T> {
        self.output_text(id, key)?.parse().ok()
    }

    /// Sets the parameter `key` of node `id` from the text of `value` (6.4). It applies on the
    /// next run of the clock domain of the node, and undo removes it.
    pub fn set_param(&mut self, id: &str, key: &str, value: impl Display) {
        let clean = |text: String| text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>();
        self.edits.push(format!("set\t{}\t{}\t{}", clean(id.to_owned()), clean(key.to_owned()), clean(value.to_string())));
    }

    /// The writes of this call, for the engine.
    pub fn edits(&self) -> impl Iterator<Item = &str> {
        self.edits.iter().map(String::as_str)
    }

    fn find(&self, kind: &str, id: &str, key: &str) -> Option<&str> {
        self.items(kind).find(|[n, k, _]| *n == id && *k == key).map(|[_, _, text]| text)
    }

    /// The fields of each line of `kind`. A missing field is empty.
    fn items(&self, kind: &str) -> impl Iterator<Item = [&str; 3]> {
        self.lines.lines().filter_map(move |line| {
            let mut fields = line.strip_prefix(kind)?.strip_prefix('\t')?.splitn(3, '\t');
            Some([fields.next()?, fields.next().unwrap_or(""), fields.next().unwrap_or("")])
        })
    }
}
