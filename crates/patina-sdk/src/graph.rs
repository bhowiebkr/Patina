//! The `Graph` handle of a view (spec 15.8).

use std::fmt::Display;
use std::hash::{BuildHasher, RandomState};
use std::str::FromStr;

use crate::glue::Reader;

/// The access of a view to the graph (15.8). The engine gives a copy of the graph and of the live
/// values before each call of `ui`. A write applies on the next run, through the edit path of the
/// editor, so undo removes it.
///
/// ```
/// let mut graph = patina_sdk::Graph::new("node\tclock\tclock\nout\tclock\tframe\t42\nparam\tclock\trate\t60\nwire\tclock.frame\tshow.value\nselected\tclock\nerror\tshow\tno value");
/// assert_eq!(graph.nodes().collect::<Vec<_>>(), ["clock"]);
/// assert_eq!(graph.output::<u64>("clock", "frame"), Some(42));
/// assert_eq!(graph.outputs("clock").collect::<Vec<_>>(), ["frame"]);
/// assert_eq!(graph.param("clock", "rate"), Some("60"));
/// assert_eq!(graph.wires().collect::<Vec<_>>(), [("clock.frame", "show.value")]);
/// assert_eq!(graph.selected().collect::<Vec<_>>(), ["clock"]);
/// assert_eq!((graph.error("show"), graph.error("clock")), (Some("no value"), None));
/// graph.set_param("clock", "rate", 30);
/// let id = graph.add_node("nodes/show", [10.0, 20.5]);
/// graph.connect("clock.frame", &format!("{id}.value"));
/// graph.disconnect("clock.frame", "show.value");
/// graph.remove_node("show");
/// graph.select(["clock", "show"]);
/// graph.fire("tick", "a\tb");
/// let edits = graph.edits().collect::<Vec<_>>();
/// assert_eq!(edits[0], "set\tclock\trate\t30");
/// assert_eq!(edits[1], format!("add\t{id}\tnodes/show\t10\t20.5"));
/// assert_eq!(edits[2..5], [format!("connect\tclock.frame\t{id}.value"), "disconnect\tclock.frame\tshow.value".into(), "remove\tshow".into()]);
/// assert_eq!(edits[5], "select\tclock\tshow");
/// assert_eq!(edits[6], "fire\ttick\ta b");
/// assert!(id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit()));
/// ```
pub struct Graph {
    /// One line for each item, with tabs between the fields: `node <id> <folder>`,
    /// `param <id> <key> <text>`, `out <id> <output> <text>`, `wire <from> <to>`, `selected <id>` and
    /// `error <id> <text>`.
    lines: String,
    /// One line for each write: `set <id> <key> <text>`, `add <id> <folder> <x> <y>`,
    /// `remove <id>`, `connect <from> <to>`, `disconnect <from> <to>`, `select <id>...` and
    /// `fire <id> <text>`.
    edits: Vec<String>,
    /// The engine side of `value`, during a call of `ui`.
    reader: Option<Reader>,
}

impl Graph {
    /// A handle on the copy `lines` of the graph. The engine makes this text.
    pub fn new(lines: &str) -> Graph {
        Graph { lines: lines.to_owned(), edits: Vec::new(), reader: None }
    }

    /// A handle that also reads the typed outputs, for `glue::view_ui`.
    pub(crate) fn with_reader(lines: &str, reader: Option<Reader>) -> Graph {
        Graph { reader, ..Graph::new(lines) }
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

    /// The live value of the output `key` of node `id` after the last run, as its type `T` from
    /// the type crate of the output (15.8 rule 3). `T` must have `Clone` and `Send`. None when
    /// the type is not `T`, when the output has no wire and is not of a batch node, and before the
    /// second run after the first call: the first call asks the engine to keep a copy of the value.
    pub fn value<T: 'static>(&self, id: &str, key: &str) -> Option<&T> {
        // SAFETY: the reader comes from `view_ui` and is valid during the call of `ui`, and the
        // result borrows `self`, which lives only during that call.
        self.reader.and_then(|reader| unsafe { reader.value(id, key) })
    }

    /// The key of each output of node `id` that has a live value after the last run.
    pub fn outputs<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a str> {
        self.items("out").filter(move |[n, _, _]| *n == id).map(|[_, key, _]| key)
    }

    /// The live value of an output, read from its text. Use it for numbers and `bool`. The text of
    /// a `String` has quotes.
    pub fn output<T: FromStr>(&self, id: &str, key: &str) -> Option<T> {
        self.output_text(id, key)?.parse().ok()
    }

    /// Each wire of the open graph, as `(from, to)`: from `<node>.<output>` to `<node>.<input>` (6.4).
    pub fn wires(&self) -> impl Iterator<Item = (&str, &str)> {
        self.items("wire").map(|[from, to, _]| (from, to))
    }

    /// The error of node `id` in the last run (4.7), for example the text of its `Err`. None when
    /// the node had no error.
    pub fn error(&self, id: &str) -> Option<&str> {
        self.items("error").find(|[n, _, _]| *n == id).map(|[_, text, _]| text)
    }

    /// The ID of each selected node of the graph view.
    pub fn selected(&self) -> impl Iterator<Item = &str> {
        self.items("selected").map(|[id, _, _]| id)
    }

    /// Selects the nodes `ids` in the graph view, and no other node. A selection is not an edit, so
    /// undo does not remove it. The last call of a `ui` call wins.
    pub fn select<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        self.write(&std::iter::once("select").chain(ids).collect::<Vec<_>>());
    }

    /// Sets the parameter `key` of node `id` from the text of `value` (6.4). It applies on the
    /// next run of the clock domain of the node, and undo removes it.
    pub fn set_param(&mut self, id: &str, key: &str, value: impl Display) {
        self.write(&["set", id, key, &value.to_string()]);
    }

    /// Adds a node of the node folder `folder`, for example `nodes/number`, at `pos` in the graph
    /// view (7.4). It returns the ID of the new node, for `connect` in the same call.
    pub fn add_node(&mut self, folder: &str, pos: [f64; 2]) -> String {
        let id = format!("{:016x}", RandomState::new().hash_one(self.edits.len()));
        self.write(&["add", &id, folder, &pos[0].to_string(), &pos[1].to_string()]);
        id
    }

    /// Removes node `id` with its wires (7.4).
    pub fn remove_node(&mut self, id: &str) {
        self.write(&["remove", id]);
    }

    /// Connects the output `from` to the input `to`, for example `a1.out` to `b2.value`. The input
    /// loses its other wire (6.4).
    pub fn connect(&mut self, from: &str, to: &str) {
        self.write(&["connect", from, to]);
    }

    /// Removes the wire from `from` to `to`.
    pub fn disconnect(&mut self, from: &str, to: &str) {
        self.write(&["disconnect", from, to]);
    }

    /// Fires the `trigger` node `id` of the live run with `text` (9.4). A control character in the
    /// text becomes a space. When the graph is not live, no write of the call applies, and the
    /// panel shows the error. A fire is not an edit, so undo does not remove it.
    pub fn fire(&mut self, id: &str, text: &str) {
        self.write(&["fire", id, text]);
    }

    /// Adds a write line. A control character in a field becomes a space, so it cannot split the line.
    fn write(&mut self, fields: &[&str]) {
        let clean = |field: &&str| field.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>();
        self.edits.push(fields.iter().map(clean).collect::<Vec<_>>().join("\t"));
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
