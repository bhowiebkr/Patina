//! Checks for the whole repository. `cargo test --workspace` runs them.
//!
//! The rules come from the "File length" section of `AGENTS.md`, and from task B5 of
//! `docs/workbenches-plan.md`: a workbench names no path outside its own folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The maximum number of lines in one source file. A longer file needs a refactor.
pub const MAX_LINES: usize = 1000;

/// The extensions of source files: Rust, the editor frontend, shaders and scripts.
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "svelte", "vue", "css", "scss", "html", "wgsl",
    "wesl", "glsl", "hlsl", "py", "sh", "ps1",
];

/// Folders with build output, downloads or dependencies, at any depth.
const SKIPPED_FOLDERS: &[&str] = &["target", "node_modules", "dist"];

/// Folders at the root that `.gitignore` excludes.
const SKIPPED_ROOT_FOLDERS: &[&str] = &["toolchain", "projects", "artifacts"];

/// One source file and its number of lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// The path, relative to the root of the search.
    pub path: PathBuf,
    /// The number of lines, with comments and blank lines.
    pub lines: usize,
}

/// The root folder of the repository.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("tools/repo-checks is two folders below the root")
        .to_path_buf()
}

/// Finds each source file below `root` and counts its lines.
/// Hidden folders, the skipped folders and the `build` folder of a Patina project are not read.
pub fn source_files(root: &Path) -> io::Result<Vec<SourceFile>> {
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if entry.file_type()?.is_dir() {
                let at_root = folder == root;
                let skipped = name.starts_with('.')
                    || SKIPPED_FOLDERS.contains(&name.as_ref())
                    || (at_root && SKIPPED_ROOT_FOLDERS.contains(&name.as_ref()))
                    || (name == "build" && folder.join("patina.toml").is_file());
                if !skipped {
                    folders.push(path);
                }
            } else if is_source(&path) {
                let bytes = fs::read(&path)?;
                let lines = String::from_utf8_lossy(&bytes).lines().count();
                let path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                files.push(SourceFile { path, lines });
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// Returns the files with more than `max` lines, the longest first.
pub fn too_long(files: &[SourceFile], max: usize) -> Vec<&SourceFile> {
    let mut long: Vec<&SourceFile> = files.iter().filter(|f| f.lines > max).collect();
    long.sort_by_key(|f| std::cmp::Reverse(f.lines));
    long
}

fn is_source(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SOURCE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Finds the relative paths in the text files of a workbench that go outside its folder, so the
/// workbench can move out of the repository. Each item is `file: path`. Build output is not read.
pub fn outside_paths(workbench: &Path) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    let mut folders = vec![workbench.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder)? {
            let path = entry?.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if path.is_dir() {
                if !name.starts_with('.') && !SKIPPED_FOLDERS.contains(&name.as_ref()) && name != "build" {
                    folders.push(path);
                }
                continue;
            }
            // A binary file is not text, so it names no path.
            let Ok(text) = fs::read_to_string(&path) else { continue };
            let rel = path.strip_prefix(workbench).unwrap_or(&path);
            let depth = rel.components().count() - 1;
            for (at, _) in text.match_indices("../") {
                if text[..at].ends_with('.') || text[..at].ends_with('/') {
                    continue;
                }
                let rest = &text[at..];
                let ups = (rest.len() - rest.trim_start_matches("../").len()) / 3;
                if ups > depth {
                    let end = rest.find(|c: char| c.is_whitespace() || "\"'`)>],".contains(c)).unwrap_or(rest.len());
                    found.push(format!("{}: {}", rel.to_string_lossy().replace('\\', "/"), &rest[..end]));
                }
            }
        }
    }
    found.sort();
    Ok(found)
}
