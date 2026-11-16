//! Loads a view library (15.8) and calls its `ui` function.

use std::ffi::c_void;
use std::path::Path;

use patina_sdk::glue::{self, Reporter};

use crate::Edit;
use crate::library::{Sink, collect, open, symbol};
use crate::load::Value;
use crate::project::GraphFile;

type VersionFn = unsafe extern "C" fn() -> u32;
type HostFn = unsafe extern "C" fn(glue::ShareFn, *const u8, usize);
type NewFn = unsafe extern "C" fn() -> *mut c_void;
type UiFn = unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *const u8, usize, *const Reporter) -> bool;
type FreeFn = unsafe extern "C" fn(*mut c_void);

/// One instance of a view: the functions of its library and its state. Each instance has its own
/// state (15.8).
pub struct View {
    state: *mut c_void,
    new: NewFn,
    ui: UiFn,
    free: FreeFn,
    sink: Sink,
}

/// The result of one call of `View::ui`.
#[derive(Debug, Default)]
pub struct Frame {
    /// The widget lines (4.3).
    pub tree: String,
    /// The writes to the graph, as edits for the edit path of the editor (15.8 rule 2).
    pub edits: Vec<Edit>,
    /// The nodes to select in the graph view, when the view selected nodes.
    pub select: Option<Vec<String>>,
}

impl View {
    /// Loads the view library at `path` from `Project::build_view`. The view shares the values of
    /// `patina_sdk::shared` with the clusters of the project `root`, for example the GPU context
    /// (10.2). The state is made on the first call of `ui`.
    pub fn load(path: &Path, root: &Path) -> Result<View, String> {
        let fail = |text: String| format!("{}: {text}", path.display());
        let library = open(path).map_err(|e| fail(e.to_string()))?;
        let missing = |e: libloading::Error| fail(e.to_string());
        // SAFETY: the `#[view]` attribute writes these symbols with these signatures.
        let view = unsafe {
            let version: VersionFn = symbol(&library, b"patina_abi_version\0").map_err(missing)?;
            if version() != glue::ABI_VERSION {
                return Err(fail("the view has another ABI version. Build it again".to_owned()));
            }
            let host: HostFn = symbol(&library, b"patina_view_host\0").map_err(missing)?;
            // The same root text as for the clusters (library.rs), so the keys of shared values match.
            let root = root.to_string_lossy();
            host(glue::share, root.as_ptr(), root.len());
            let new: NewFn = symbol(&library, b"patina_view_new\0").map_err(missing)?;
            let ui: UiFn = symbol(&library, b"patina_view_ui\0").map_err(missing)?;
            let free: FreeFn = symbol(&library, b"patina_view_free\0").map_err(missing)?;
            View { state: std::ptr::null_mut(), new, ui, free, sink: Sink::default() }
        };
        // Rust libraries do not unload safely, so the library stays loaded (8.5).
        std::mem::forget(library);
        Ok(view)
    }

    /// Calls `ui` in mode `mode` with the user input `events` and the graph lines of `snapshot`.
    /// After an error, the next call makes a new state.
    pub fn ui(&mut self, mode: u32, events: &str, graph: &str) -> Result<Frame, String> {
        if self.state.is_null() {
            // SAFETY: the function came from the loaded view.
            self.state = unsafe { (self.new)() };
            if self.state.is_null() {
                return Err("the init function of the view panicked".to_owned());
            }
        }
        self.sink.clear();
        let rep = Reporter { ctx: (&raw mut self.sink).cast(), report: collect, control: std::ptr::null() };
        // SAFETY: the state came from `new` of this library and is live. The texts and the reporter
        // are valid during the call.
        let ok = unsafe { (self.ui)(self.state, mode, events.as_ptr(), events.len(), graph.as_ptr(), graph.len(), &rep) };
        let mut frame = Frame::default();
        let mut error = None;
        for (_, kind, text) in self.sink.iter() {
            match kind {
                glue::UI => frame.tree = text.to_owned(),
                glue::EDIT => match select(text) {
                    Some(ids) => frame.select = Some(ids),
                    None => frame.edits.extend(edit(text)),
                },
                glue::ERROR | glue::PANIC => error = Some(text.to_owned()),
                _ => {}
            }
        }
        if ok {
            return Ok(frame);
        }
        self.free_state();
        Err(error.unwrap_or_else(|| "the view failed".to_owned()))
    }

    fn free_state(&mut self) {
        let state = std::mem::replace(&mut self.state, std::ptr::null_mut());
        if !state.is_null() {
            // SAFETY: `state` came from `new` of this library and is not used again.
            unsafe { (self.free)(state) }
        }
    }
}

impl Drop for View {
    fn drop(&mut self) {
        self.free_state();
    }
}

/// The graph lines that a view reads (`patina_sdk::Graph`): the nodes, the parameters and the wires
/// of `graph`, the live values of the last run and the `selected` nodes of the graph view.
pub fn snapshot(graph: &GraphFile, values: &[Value], selected: &[String]) -> String {
    let clean = |text: &str| text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>();
    let mut lines = String::new();
    for node in &graph.node {
        let folder = node.folder.as_deref().unwrap_or_default();
        lines.push_str(&format!("node\t{}\t{folder}\n", node.id));
        for (key, text) in &node.params {
            lines.push_str(&format!("param\t{}\t{key}\t{}\n", node.id, clean(text)));
        }
    }
    for wire in &graph.wire {
        lines.push_str(&format!("wire\t{}\t{}\n", wire.from, wire.to));
    }
    for value in values {
        let Some((id, key)) = value.from.split_once('.') else { continue };
        lines.push_str(&format!("out\t{id}\t{key}\t{}\n", clean(&value.text)));
    }
    for id in selected {
        lines.push_str(&format!("selected\t{}\n", clean(id)));
    }
    lines
}

/// The IDs of a select line of a view (`patina_sdk::Graph::select`). None for another line.
fn select(line: &str) -> Option<Vec<String>> {
    let ids = line.strip_prefix("select")?;
    (ids.is_empty() || ids.starts_with('\t')).then(|| ids.split('\t').filter(|id| !id.is_empty()).map(str::to_owned).collect())
}

/// The edit of a write line of a view (`patina_sdk::Graph`). None for a bad line.
fn edit(line: &str) -> Option<Edit> {
    let mut fields = line.split('\t');
    let mut text = || fields.next().map(str::to_owned);
    let edit = match text()?.as_str() {
        "set" => Edit::SetParam { id: text()?, key: text()?, text: Some(text()?) },
        "add" => Edit::AddNode { id: text()?, folder: text()?, builtin: false, pos: [text()?.parse().ok()?, text()?.parse().ok()?] },
        "remove" => Edit::RemoveNode { id: text()? },
        "connect" => Edit::Connect { from: text()?, to: text()? },
        "disconnect" => Edit::Disconnect { from: text()?, to: text()? },
        _ => return None,
    };
    text().is_none().then_some(edit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_and_edit_lines() {
        let mut graph: GraphFile = toml::from_str("format = 1\n[[node]]\nid = \"a\"\nfolder = \"number\"\nparams = { value = \"2\" }\n").unwrap();
        graph.node[0].params.insert("text".to_owned(), "x\ty".to_owned());
        graph.wire = toml::from_str::<GraphFile>("format = 1\n[[wire]]\nfrom = \"a.out\"\nto = \"b.in\"\n").unwrap().wire;
        let values = [Value { from: "a.out".to_owned(), text: "2.0".to_owned(), lanes: None }];
        let lines = "node\ta\tnumber\nparam\ta\ttext\tx y\nparam\ta\tvalue\t2\nwire\ta.out\tb.in\nout\ta\tout\t2.0\nselected\ta\n";
        assert_eq!(snapshot(&graph, &values, &["a".to_owned()]), lines);
        assert_eq!(select("select\ta\tb"), Some(vec!["a".to_owned(), "b".to_owned()]));
        assert_eq!(select("select"), Some(Vec::new()));
        assert_eq!(select("selected\ta"), None);
        let set = edit("set\ta\tvalue\t3 4");
        assert!(matches!(set, Some(Edit::SetParam { id, key, text: Some(text) }) if id == "a" && key == "value" && text == "3 4"));
        assert!(matches!(edit("add\tb\tnodes/show\t1\t-2.5"), Some(Edit::AddNode { id, folder, builtin: false, pos: [1.0, -2.5] }) if id == "b" && folder == "nodes/show"));
        assert!(matches!(edit("remove\tb"), Some(Edit::RemoveNode { id }) if id == "b"));
        assert!(matches!(edit("connect\ta.out\tb.in"), Some(Edit::Connect { from, to }) if from == "a.out" && to == "b.in"));
        assert!(matches!(edit("disconnect\ta.out\tb.in"), Some(Edit::Disconnect { .. })));
        for bad in ["add\ta", "add\tb\tnodes/show\tx\t1", "set\ta\tvalue", "remove\tb\tc", "move\tb", ""] {
            assert!(edit(bad).is_none(), "{bad}");
        }
    }
}
