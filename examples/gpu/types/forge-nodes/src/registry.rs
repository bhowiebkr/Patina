//! Every node definition and shared module, read from `kernels/` beside the crate when it
//! exists (edits there reload live) or from the copies embedded at build time. Kernels and
//! modules are `.wesl`; `thumb.wgsl` and `stamp.wgsl` are plain WGSL.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use web_time::Instant;

use crate::def::{Def, DefError};
use crate::link::{self, Modules, PRELUDE};

include!(concat!(env!("OUT_DIR"), "/kernels.rs"));

type SourceFiles = BTreeMap<String, String>;

const THUMB: &str = "thumb.wgsl";
const STAMP: &str = "stamp.wgsl";
const POLL: Duration = Duration::from_millis(500);

/// Menu order of the node categories; others follow alphabetically.
const ORDER: &[&str] = &[
    "Core",
    "Atomic",
    "Material",
    "Mesh",
    "Mesh Masks",
    "Mesh Weathering",
    "Adjustment",
    "Blending",
    "Blur",
    "Channel",
    "Effects",
    "Normal Map",
    "Tiling",
    "Transforms",
    "Noise",
    "Pattern",
    "Scan Processing",
    "HDRI",
];

/// What a poll found changed.
#[derive(Default, Debug)]
pub struct Changes {
    /// `prelude.wesl` and `lib/` files added, edited or removed: their importers recompile.
    pub modules: Vec<String>,
    /// A module's declared names changed, so any kernel's imports may: every kernel
    /// recompiles.
    pub names: bool,
    /// Ids of definitions added, edited or removed.
    pub defs: Vec<String>,
}

impl Changes {
    pub fn any(&self) -> bool {
        !self.modules.is_empty() || !self.defs.is_empty()
    }
}

pub struct Registry {
    linked: BTreeMap<String, String>,
    dir: Option<PathBuf>,
    files: BTreeMap<String, String>,
    modules: Modules,
    scoped_modules: BTreeMap<String, Modules>,
    stamps: BTreeMap<String, SystemTime>,
    defs: BTreeMap<String, Def>,
    errors: Vec<DefError>,
    polled: Instant,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/kernels"));
        let dir = (!forge_gpu::PACKAGED && dir.join(PRELUDE).is_file()).then_some(dir);
        let mut registry = Self {
            linked: BTreeMap::new(),
            dir,
            files: BTreeMap::new(),
            modules: Modules::default(),
            scoped_modules: BTreeMap::new(),
            stamps: BTreeMap::new(),
            defs: BTreeMap::new(),
            errors: Vec::new(),
            polled: Instant::now(),
        };
        let (files, stamps) = registry.read();
        registry.stamps = stamps;
        registry.apply(files);
        registry
    }

    /// An immutable material-local registry; never reads the source tree or polls disk.
    pub fn from_files(files: BTreeMap<String, String>) -> Self {
        let mut registry = Self {
            linked: BTreeMap::new(),
            dir: None,
            files: BTreeMap::new(),
            modules: Modules::default(),
            scoped_modules: BTreeMap::new(),
            stamps: BTreeMap::new(),
            defs: BTreeMap::new(),
            errors: Vec::new(),
            polled: Instant::now(),
        };
        registry.apply(files);
        registry
    }

    pub fn embedded() -> Self {
        Self::from_files(
            KERNELS
                .iter()
                .map(|(p, s)| (p.to_string(), s.to_string()))
                .collect(),
        )
    }

    /// Add available nodes without changing any pinned source or linked implementation.
    pub fn enable_catalog(&mut self) -> Result<(), String> {
        let mut files: BTreeMap<_, _> = KERNELS
            .iter()
            .map(|(p, s)| (p.to_string(), s.to_string()))
            .collect();
        files.extend(self.files.clone());
        let candidate = Self::from_files(files);
        if !candidate.errors.is_empty() {
            return Err(candidate
                .errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"));
        }
        self.files = candidate.files;
        self.modules = candidate.modules;
        self.scoped_modules = candidate.scoped_modules;
        self.defs = candidate.defs;
        Ok(())
    }

    pub fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }

    pub fn set_linked(&mut self, linked: BTreeMap<String, String>) {
        self.linked = linked;
    }

    pub fn linked(&self, kind: &str, code: &str) -> Option<&str> {
        self.linked
            .get(&format!("{kind}\n{code}"))
            .map(String::as_str)
    }
    pub fn linked_sources(&self) -> &BTreeMap<String, String> {
        &self.linked
    }
    /// Validate external pinned programs before a host changes its working document.
    pub fn validate_pinned(&self) -> Result<(), String> {
        for (key, source) in &self.linked {
            crate::link::naga_check(source).map_err(|(error, _)| {
                format!("Pinned {}: {error}", key.split('\n').next().unwrap_or(key))
            })?;
        }
        Ok(())
    }

    /// Pins both the editable WESL and the linked WGSL of every kernel used by this graph.
    pub fn package_sources(
        &self,
        graph: &crate::Graph,
    ) -> Result<(SourceFiles, SourceFiles), String> {
        let mut files: BTreeMap<String, String> = self
            .files
            .iter()
            .filter(|(p, _)| !p.starts_with("nodes/"))
            .map(|(p, s)| (p.clone(), s.clone()))
            .collect();
        let mut linked = BTreeMap::new();
        for node in &graph.nodes {
            let def = self
                .get(&node.kind)
                .ok_or_else(|| format!("Unknown node {}", node.kind))?;
            files.insert(def.path.clone(), def.source.clone());
            let code = def
                .code_param()
                .map_or_else(String::new, |p| node.text(self, &p.id));
            let key = format!("{}\n{code}", node.kind);
            if linked.contains_key(&key) {
                continue;
            }
            if let Some(shader) = self.linked(&node.kind, &code) {
                link::naga_check(shader)
                    .map_err(|(message, _)| format!("{}: {message}", def.path))?;
                linked.insert(key, shader.to_owned());
                continue;
            }
            let shader = link::link(
                self.modules_for(&node.kind),
                &def.path,
                &def.source,
                &code,
                &crate::generate(def),
            )?;
            if let Some(error) = shader.explain(&self.modules, &def.path) {
                return Err(error);
            }
            linked.insert(key, shader.wgsl);
        }
        Ok((files, linked))
    }

    /// The source files (path under `kernels/`, text) and their modification times.
    fn read(&self) -> (BTreeMap<String, String>, BTreeMap<String, SystemTime>) {
        let Some(dir) = &self.dir else {
            let files = KERNELS
                .iter()
                .map(|(p, s)| ((*p).to_owned(), (*s).to_owned()))
                .collect();
            return (files, BTreeMap::new());
        };
        let mut paths = Vec::new();
        walk(dir, &mut paths);
        let mut files = BTreeMap::new();
        let mut stamps = BTreeMap::new();
        for path in paths {
            let Ok(rel) = path.strip_prefix(dir) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            if let Ok(text) = std::fs::read_to_string(&path) {
                files.insert(rel.clone(), text);
            }
            if let Ok(time) = std::fs::metadata(&path).and_then(|m| m.modified()) {
                stamps.insert(rel, time);
            }
        }
        (files, stamps)
    }

    /// Re-parses every definition; returns what changed.
    fn apply(&mut self, files: BTreeMap<String, String>) -> Changes {
        let (modules, mut errors) = Modules::new(&files);
        // Recipe-local library revisions are durable source files, outside the
        // common linker namespace. Alias kinds select their own original modules.
        let mut scopes: BTreeMap<String, SourceFiles> = BTreeMap::new();
        for (path, source) in &files {
            if let Some((scope, module)) =
                path.strip_prefix("pinned/").and_then(|p| p.split_once('/'))
                && link::is_module(module)
            {
                scopes
                    .entry(scope.into())
                    .or_default()
                    .insert(module.into(), source.clone());
            }
        }
        let mut scoped_modules = BTreeMap::new();
        for (scope, sources) in scopes {
            let (local, local_errors) = Modules::new(&sources);
            errors.extend(local_errors.into_iter().map(|mut error| {
                error.file = format!("pinned/{scope}/{}", error.file);
                error
            }));
            scoped_modules.insert(scope, local);
        }
        let mut changed: Vec<String> = files
            .iter()
            .filter(|(p, t)| link::is_module(p) && self.files.get(*p) != Some(*t))
            .map(|(p, _)| p.clone())
            .collect();
        changed.extend(
            self.files
                .keys()
                .filter(|p| link::is_module(p) && !files.contains_key(*p))
                .cloned(),
        );
        let mut changes = Changes {
            names: !modules.same_names(&self.modules),
            modules: changed,
            defs: Vec::new(),
        };
        let mut defs = BTreeMap::new();
        for (path, text) in files
            .iter()
            .filter(|(p, _)| p.starts_with("nodes/") && p.ends_with(".wesl"))
        {
            match Def::parse(path, text) {
                Ok(def) => {
                    if let Some(other) = defs.get(&def.id).map(|d: &Def| d.path.clone()) {
                        errors.push(DefError {
                            file: path.clone(),
                            line: 1,
                            message: format!("id `{}` is also {other}", def.id),
                        });
                    } else {
                        defs.insert(def.id.clone(), def);
                    }
                }
                Err(e) => errors.push(e),
            }
        }
        for (id, def) in &defs {
            if self.defs.get(id).is_none_or(|old| old.source != def.source) {
                changes.defs.push(id.clone());
            }
        }
        changes.defs.extend(
            self.defs
                .keys()
                .filter(|id| !defs.contains_key(*id))
                .cloned(),
        );
        self.files = files;
        self.modules = modules;
        self.scoped_modules = scoped_modules;
        self.defs = defs;
        self.errors = errors;
        changes
    }

    /// Checks the kernel folder for edits (at most every half second).
    pub fn poll(&mut self) -> Changes {
        if self.dir.is_none() || self.polled.elapsed() < POLL {
            return Changes::default();
        }
        self.polled = Instant::now();
        let (files, stamps) = self.read();
        if stamps == self.stamps {
            return Changes::default();
        }
        self.stamps = stamps;
        self.apply(files)
    }

    pub fn get(&self, id: &str) -> Option<&Def> {
        self.defs.get(id)
    }

    pub fn defs(&self) -> impl Iterator<Item = &Def> {
        self.defs.values()
    }

    /// Definition files that failed to parse.
    pub fn errors(&self) -> &[DefError] {
        &self.errors
    }

    /// Categories in menu order.
    pub fn categories(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.defs.values().map(|d| d.category.as_str()).collect();
        out.sort_by_key(|c| (ORDER.iter().position(|o| o == c).unwrap_or(ORDER.len()), *c));
        out.dedup();
        out
    }

    /// Definitions in `category`, sorted by name.
    pub fn in_category<'a>(&'a self, category: &'a str) -> impl Iterator<Item = &'a Def> + 'a {
        let mut defs: Vec<&Def> = self
            .defs
            .values()
            .filter(|d| d.category == category)
            .collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs.into_iter()
    }

    /// Validate a recipe prelude against the shared GPU data layout, preserving its helpers.
    pub fn accepts_prelude(&self, incoming: &Self) -> Result<(), String> {
        match (self.files.get(PRELUDE), incoming.files.get(PRELUDE)) {
            (Some(current), Some(next)) if current != next => {
                link::compatible_prelude(current, next)
                    .map_err(|error| format!("Conflicting shared GPU interface: {error}"))
            }
            _ => Ok(()),
        }
    }

    /// Immutable recipe aliases compile new code variants against their own libraries.
    pub fn modules_for(&self, kind: &str) -> &Modules {
        kind.split_once("__")
            .and_then(|(scope, _)| self.scoped_modules.get(scope))
            .unwrap_or(&self.modules)
    }

    /// `prelude.wesl` and the common `lib/` modules kernels link against.
    pub fn modules(&self) -> &Modules {
        &self.modules
    }

    pub fn thumb(&self) -> &str {
        self.files.get(THUMB).map_or("", String::as_str)
    }

    /// The paint brush kernel.
    pub fn stamp(&self) -> &str {
        self.files.get(STAMP).map_or("", String::as_str)
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "wesl" || e == "wgsl") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_definition_parses() {
        let registry = Registry::new();
        let errors: Vec<String> = registry.errors().iter().map(|e| e.to_string()).collect();
        assert_eq!(errors, Vec::<String>::new());
        assert!(registry.get("output").is_some());
        assert!(registry.modules().files().count() > 1 && !registry.thumb().is_empty());
    }
}
