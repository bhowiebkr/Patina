//! Checks for the whole repository. `cargo test --workspace` runs them.
//!
//! The rules come from the "File length" section of `AGENTS.md`.

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
/// Hidden folders and the skipped folders are not read.
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
                    || (at_root && SKIPPED_ROOT_FOLDERS.contains(&name.as_ref()));
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
