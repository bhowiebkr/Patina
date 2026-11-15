//! The Patina engine: reads a project, builds its graph into a cluster and runs it (spec 3, 8).

mod cargo;
mod generate;
mod load;
mod node;
mod plan;
pub mod project;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

pub use cargo::Toolchain;
pub use load::{Cluster, Outcome, Report, ReportKind};

#[derive(Debug)]
pub enum Error {
    Io(PathBuf, io::Error),
    /// A project file does not read.
    File(PathBuf, String),
    /// The graph file has these problems.
    Graph(PathBuf, Vec<String>),
    Build(String),
    Load(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Error::File(path, text) => write!(f, "{}: {text}", path.display()),
            Error::Graph(path, problems) => {
                write!(f, "{} has problems:", path.display())?;
                problems.iter().try_for_each(|p| write!(f, "\n  {p}"))
            }
            Error::Build(text) => write!(f, "the build failed: {text}"),
            Error::Load(text) => write!(f, "the cluster did not load: {text}"),
        }
    }
}

impl std::error::Error for Error {}

/// A project with a checked main graph.
pub struct Project {
    root: PathBuf,
    rust: String,
    plan: plan::Plan,
}

impl Project {
    /// Reads `patina.toml` and `graphs/main.toml` of the project folder and checks the graph.
    pub fn open(path: &Path) -> Result<Project, Error> {
        // The path must be absolute, because cargo reads the target folder from a variable.
        let root = std::path::absolute(path).map_err(|e| Error::Io(path.to_owned(), e))?;
        let file = project::read(&root.join("patina.toml"), |f: &project::ProjectFile| f.format)?;
        let graph_path = root.join("graphs").join("main.toml");
        let graph = project::read(&graph_path, |g: &project::GraphFile| g.format)?;
        let plan = plan::check(&root, &graph).map_err(|problems| Error::Graph(graph_path, problems))?;
        Ok(Project { root, rust: file.rust, plan })
    }

    /// The Rust version that the project builds with (6.4).
    pub fn rust(&self) -> &str {
        &self.rust
    }

    /// Builds the cluster of the main graph and loads it.
    pub fn build(&self, toolchain: &Toolchain) -> Result<Cluster, Error> {
        if toolchain.release != self.rust {
            return Err(Error::Build(format!(
                "the project needs Rust {}, but the toolchain is Rust {}",
                self.rust, toolchain.release
            )));
        }
        // shortcut: loads the library from the cargo target folder, which Windows locks while it
        // is loaded. The hash names of 8.5 come with the layers at milestone 6.
        let library = cargo::build(&self.root, toolchain, &self.plan)?;
        let names = self.plan.steps.iter().map(|s| (s.id.clone(), self.plan.nodes[s.node].0.clone())).collect();
        Cluster::load(&library, names)
    }

    /// Sets each parameter that has a text in the graph file (4.8).
    pub fn set_params(&self, cluster: &mut Cluster) -> Outcome {
        let mut all = Outcome { ok: true, reports: Vec::new() };
        for (index, slot) in self.plan.slots.iter().enumerate() {
            let Some(text) = &slot.text else { continue };
            let mut outcome = cluster.set(index, text);
            for report in &mut outcome.reports {
                report.text = format!("input `{}`: {}", slot.key, report.text);
            }
            all.ok &= outcome.ok;
            all.reports.append(&mut outcome.reports);
        }
        all
    }
}
