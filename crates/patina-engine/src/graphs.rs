//! The graphs of a project: the open graph, the subgraphs in memory, the undo history and the
//! save (spec 6.1, 15.10).

use std::collections::HashSet;
use std::hash::{BuildHasher, RandomState};

use crate::edit::Disk;
use crate::folders::{self, swap_code};
use crate::project::{self, GraphFile};
use crate::{Edit, Error, MAIN, Project, edit, graph_path, plan, subgraph};

impl Project {
    /// The open graph.
    pub fn graph(&self) -> &GraphFile {
        &self.history.graph
    }

    /// The name of the open graph: `main` or the name of a subgraph.
    pub fn graph_name(&self) -> &str {
        &self.name
    }

    /// Opens the main graph or a subgraph for edits (6.1). The other graphs keep their unsaved
    /// changes and their undo history in memory. Only a save writes them (15.10).
    pub fn open_graph(&mut self, name: &str) -> Result<(), String> {
        if name == self.name {
            return Ok(());
        }
        if name != MAIN {
            plan::check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
        }
        let history = match self.parked.remove(name) {
            Some(history) => history,
            None => edit::History::new(project::read(&graph_path(&self.root, name), |g: &GraphFile| g.format).map_err(|e| e.to_string())?),
        };
        let old = std::mem::replace(&mut self.history, history);
        self.parked.insert(std::mem::replace(&mut self.name, name.to_owned()), old);
        Ok(())
    }

    /// The file of the subgraph `name`, with its unsaved edits, to show its nodes in place in the
    /// graph that uses it (15.3).
    pub fn subgraph_file(&self, name: &str) -> Result<GraphFile, String> {
        if name == MAIN {
            return Err("the main graph is not a subgraph".to_owned());
        }
        plan::check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
        if name == self.name {
            return Ok(self.history.graph.clone());
        }
        match self.parked.get(name) {
            Some(history) => Ok(history.graph.clone()),
            None => project::read(&graph_path(&self.root, name), |g: &GraphFile| g.format).map_err(|e| e.to_string()),
        }
    }

    /// True when the graph `name` has a file or is in memory. The case of the letters is not
    /// important, as for the files on Windows.
    pub(crate) fn has_graph(&self, name: &str) -> bool {
        let mut open = std::iter::once(&self.name).chain(self.parked.keys());
        name.eq_ignore_ascii_case(MAIN) || open.any(|n| n.eq_ignore_ascii_case(name)) || graph_path(&self.root, name).exists()
    }

    /// Adds the new subgraph `name` in memory. Only a save writes its file (15.10).
    pub(crate) fn add_graph(&mut self, name: &str, graph: GraphFile) {
        self.parked.insert(name.to_owned(), edit::History::unsaved(graph));
    }

    /// Moves the nodes `ids` of the open graph into the new subgraph `name`, with the use `use_id`
    /// in their place, as one step of undo (6.1). The new subgraph stays in memory until a save.
    /// An undo keeps the subgraph for a redo.
    pub fn collapse(&mut self, name: &str, ids: &[String], use_id: &str) -> Result<(), String> {
        plan::check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
        if self.has_graph(name) {
            return Err(format!("the graph {name} exists"));
        }
        let state = RandomState::new();
        let mut count = 0u32;
        let key = || {
            count += 1;
            format!("{:08x}", state.hash_one(count) as u32)
        };
        let (outer, inner) = subgraph::collapse(&self.history.graph, ids, use_id, name, key)?;
        self.add_graph(name, inner);
        self.history.replace(outer);
        Ok(())
    }

    /// Puts the nodes of each subgraph use `ids` of the open graph in place of the use, as one step
    /// of undo (6.1). The subgraph files stay.
    pub fn expand(&mut self, ids: &[String]) -> Result<(), String> {
        let state = RandomState::new();
        let mut count = 0u32;
        let mut new_id = || {
            count += 1;
            format!("{:016x}", state.hash_one(count))
        };
        let mut graph = self.history.graph.clone();
        for id in ids {
            let name = graph.node.iter().find(|n| &n.id == id).and_then(|n| n.subgraph.clone()).ok_or(format!("the node {id} is not a use of a subgraph"))?;
            let inner = self.subgraph_file(&name)?;
            graph = subgraph::expand_use(&graph, id, &inner, &mut new_id)?;
        }
        self.history.replace(graph);
        Ok(())
    }

    /// Applies an edit of the user (7.4). A code edit writes its file at once, because cargo
    /// builds from the files.
    pub fn apply(&mut self, edit: &Edit) -> Result<(), String> {
        let Edit::SetCode { path, text } = edit else { return self.history.apply(edit) };
        let old = swap_code(&self.root, path, text)?;
        if old == *text {
            return Err("the edit changes nothing".to_owned());
        }
        self.history.push_disk(Disk::Code { path: path.clone(), text: old });
        self.code_revision += 1;
        self.last_code = Some(path.clone());
        Ok(())
    }

    /// Replaces the open graph as one step of undo, for example with the graph of a recovery file
    /// (15.10). The graph is not saved.
    pub fn restore_graph(&mut self, graph: GraphFile) {
        self.history.replace(graph);
    }

    /// Removes the last edit. Returns false when there is no edit to remove.
    pub fn undo(&mut self) -> Result<bool, String> {
        self.step(true)
    }

    /// Applies the last removed edit again. Returns false when there is no such edit.
    pub fn redo(&mut self) -> Result<bool, String> {
        self.step(false)
    }

    fn step(&mut self, undo: bool) -> Result<bool, String> {
        let root = &self.root;
        let mut wrote = None;
        let write = |disk: &Disk| {
            // A new or removed folder changes the crates, as a change of its Cargo.toml does.
            wrote = Some(match disk {
                Disk::Code { path, .. } => path.clone(),
                Disk::Folder { path, .. } => format!("{path}/Cargo.toml"),
            });
            folders::write_disk(root, disk)
        };
        let done = if undo { self.history.undo(write) } else { self.history.redo(write) };
        if let (Some(path), true) = (wrote, done.is_ok()) {
            self.code_revision += 1;
            self.last_code = Some(path);
        }
        done
    }

    /// Changes when an edit, undo or redo writes a code file, or when undo or redo removes or makes
    /// a folder. Each step writes one file or one folder.
    pub fn code_revision(&self) -> u64 {
        self.code_revision
    }

    /// The path of the last code file that an edit, undo or redo wrote.
    pub fn last_code(&self) -> Option<&str> {
        self.last_code.as_deref()
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// True when a graph has changes that are not saved.
    pub fn dirty(&self) -> bool {
        self.unsaved().next().is_some()
    }

    /// The name and the graph of each graph with changes that are not saved, the open graph first.
    /// A new subgraph that nothing uses is not in the list (15.10).
    pub fn unsaved(&self) -> impl Iterator<Item = (&str, &GraphFile)> {
        let unused = self.unused_new();
        let parked = self.parked.iter().map(|(name, h)| (name.as_str(), h));
        std::iter::once((self.name.as_str(), &self.history)).chain(parked).filter(move |(name, h)| h.dirty() && !unused.contains(name)).map(|(name, h)| (name, &h.graph))
    }

    /// The new subgraphs of a paste or a collapse with no file, no edits of their own and no use in
    /// the other graphs in memory, for example after an undo. They stay in memory for a redo (15.10).
    fn unused_new(&self) -> HashSet<&str> {
        let mut unused: HashSet<&str> = self.parked.iter().filter(|(_, h)| h.is_new() && !h.has_steps()).map(|(name, _)| name.as_str()).collect();
        loop {
            // A use in an unused new subgraph does not count, so repeat until nothing changes.
            let graphs = std::iter::once((self.name.as_str(), &self.history.graph)).chain(self.parked.iter().map(|(name, h)| (name.as_str(), &h.graph)));
            let used: HashSet<&str> = graphs.filter(|(name, _)| !unused.contains(name)).flat_map(|(_, g)| g.node.iter().filter_map(|n| n.subgraph.as_deref())).collect();
            let count = unused.len();
            unused.retain(|name| !used.contains(name));
            if unused.len() == count {
                return unused;
            }
        }
    }

    /// Writes each graph with unsaved changes to its file in `graphs/`. The undo history stays
    /// (15.10).
    pub fn save(&mut self) -> Result<(), Error> {
        let unused: HashSet<String> = self.unused_new().into_iter().map(str::to_owned).collect();
        let parked = self.parked.iter_mut().map(|(name, h)| (name.as_str(), h));
        for (name, history) in std::iter::once((self.name.as_str(), &mut self.history)).chain(parked) {
            if history.dirty() && !unused.contains(name) {
                project::write(&graph_path(&self.root, name), &history.graph)?;
                history.mark_saved();
            }
        }
        Ok(())
    }

    /// The use path (6.4) of each use of the open subgraph in the main graph, also inside other
    /// uses (15.9). The main graph has none.
    pub fn use_paths(&self) -> Vec<String> {
        let Some(main) = self.parked.get(MAIN) else { return Vec::new() };
        subgraph::use_paths(&self.root, &main.graph, &self.edits(), &self.name)
    }

    /// The subgraphs that the editor has in memory, with their unsaved changes. They replace their
    /// files in a check (6.1).
    pub(crate) fn edits(&self) -> Vec<(&str, &GraphFile)> {
        let parked = self.parked.iter().map(|(name, h)| (name.as_str(), &h.graph));
        std::iter::once((self.name.as_str(), &self.history.graph)).chain(parked).filter(|(name, _)| *name != MAIN).collect()
    }
}
