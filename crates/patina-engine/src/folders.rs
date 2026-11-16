//! The node, widget and view folders of a project (6.2): new folders, their start code, and safe
//! reads and writes of their code files.

use std::fs;
use std::path::{Path, PathBuf};

use crate::{Error, plan};

/// Makes the crate folder `<group>/<name>` with the start code in `src/lib.rs`. No other folder of the
/// project can have the same crate name (6.2).
pub(crate) fn new_folder(root: &Path, group: &str, name: &str) -> Result<(), String> {
    plan::check_name(name)?;
    let code = start_code(group, name);
    let key = plan::name_key(name);
    for group in ["nodes", "views", "widgets", "connections", "types"] {
        for entry in fs::read_dir(root.join(group)).into_iter().flatten().flatten() {
            let other = entry.file_name().to_string_lossy().into_owned();
            if plan::name_key(&other) == key {
                return Err(format!("the folder {group}/{other} has this name"));
            }
        }
    }
    let parent = root.join(group);
    fs::create_dir_all(&parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    // create_dir fails when the folder exists, so the files of another folder stay.
    let dir = parent.join(name);
    fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // A view compiles as its own library (15.8).
    let lib = if group == "views" { "[lib]\ncrate-type = [\"cdylib\"]\n\n" } else { "" };
    let manifest = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n{lib}\
         [dependencies]\npatina-sdk = {{ workspace = true }}\n"
    );
    let written = write_file(&dir.join("Cargo.toml"), &manifest).and_then(|()| write_file(&dir.join("src/lib.rs"), &code));
    if let Err(e) = written {
        let _ = fs::remove_dir_all(&dir);
        return Err(e.to_string());
    }
    Ok(())
}

/// The start code of a node, widget or view folder.
fn start_code(group: &str, name: &str) -> String {
    let ty: String = name.split(['-', '_']).filter(|p| !p.is_empty()).map(|p| p[..1].to_ascii_uppercase() + &p[1..]).collect();
    match group {
        "widgets" => WIDGET_CODE.replace("NAME", &ty),
        "views" => VIEW_CODE.replace("NAME", &ty),
        _ => format!(
            "use patina_sdk::node;

/// Gives the value of the input. Write the work of the node here.
             #[node]
pub fn {}(value: f64) -> f64 {{
    value
}}
",
            plan::name_key(name)
        ),
    }
}

pub(crate) fn write_file(path: &Path, text: &str) -> Result<(), Error> {
    let parent = path.parent().expect("a project file is in a folder");
    fs::create_dir_all(parent).map_err(|e| Error::Io(parent.to_owned(), e))?;
    fs::write(path, text).map_err(|e| Error::Io(path.to_owned(), e))
}

/// The start code of a widget folder. `NAME` is the type name.
const WIDGET_CODE: &str = "/// A bar that the user drags to set a value from 0 to 1. Write the drawing and the input of the
/// widget here. A node keeps the widget in its state and shows it with `ui.add(&mut self.bar)`.
pub struct NAME {
    pub value: f32,
}

impl patina_sdk::Widget for NAME {
    fn show(&mut self, ui: &mut patina_sdk::Ui) -> patina_sdk::Response {
        const SIZE: [f32; 2] = [120.0, 16.0];
        let mut canvas = ui.canvas(\"\", SIZE);
        let mut changed = false;
        for pointer in canvas.pointer() {
            if let patina_sdk::Pointer::Down(at) | patina_sdk::Pointer::Move(at) = pointer {
                self.value = (at[0] / SIZE[0]).clamp(0.0, 1.0);
                changed = true;
            }
        }
        canvas.rect([0.0, 0.0], [SIZE[0] * self.value, SIZE[1]], [200, 140, 60], true);
        patina_sdk::Response::new(false, changed)
    }
}
";

/// The start code of a view folder (15.8). `NAME` is the type name.
const VIEW_CODE: &str = "use patina_sdk::{Graph, Ui, view};

/// A panel that shows each node of the open graph and its live outputs. Write the panel here.
#[view]
pub struct NAME {
    pub shown: u64,
}

impl NAME {
    pub fn init() -> Self {
        NAME { shown: 0 }
    }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        self.shown += 1;
        for id in graph.nodes() {
            let folder = graph.folder(id).unwrap_or(\"-\");
            ui.label(&format!(\"{id} ({folder})\"));
        }
    }
}
";

/// Checks a node, widget or view folder as `nodes/<name>`, `widgets/<name>` or `views/<name>`
/// (6.2). The page sends it.
pub(crate) fn code_folder(folder: &str) -> Result<(), String> {
    let (group, name) = folder.split_once('/').ok_or_else(|| format!("{folder:?} is not a node, widget or view folder"))?;
    if !["nodes", "widgets", "views"].contains(&group) {
        return Err(format!("{folder:?} is not a node, widget or view folder"));
    }
    plan::check_name(name)
}

/// The full path of an existing file in a node or widget folder, from a path like
/// `nodes/number/src/lib.rs` (6.2). The page sends the path, so it must stay in the folder.
pub(crate) fn code_file(root: &Path, path: &str) -> Result<PathBuf, String> {
    let bad = || format!("{path:?} is not a file in a node or widget folder");
    let mut parts = path.split('/');
    let folder = [parts.next().ok_or_else(bad)?, parts.next().ok_or_else(bad)?].join("/");
    code_folder(&folder).map_err(|_| bad())?;
    let mut file = root.join(&folder);
    let mut depth = 0;
    for part in parts {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
            return Err(bad());
        }
        file.push(part);
        depth += 1;
    }
    if depth == 0 || !file.is_file() {
        return Err(format!("the project has no file {path}"));
    }
    Ok(file)
}

/// Writes the text to a code file and returns the old text. A temporary file and a rename keep
/// the old text when the write fails.
pub(crate) fn swap_code(root: &Path, path: &str, text: &str) -> Result<String, String> {
    let file = code_file(root, path)?;
    let old = fs::read_to_string(&file).map_err(|e| format!("{path}: {e}"))?;
    if old != text {
        let temp = file.with_extension("patina-tmp");
        fs::write(&temp, text).map_err(|e| format!("{}: {e}", temp.display()))?;
        fs::rename(&temp, &file).map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(old)
}

pub(crate) fn list_files(dir: &Path, relative: &str, files: &mut Vec<String>) -> Result<(), Error> {
    for entry in fs::read_dir(dir).map_err(|e| Error::Io(dir.to_owned(), e))? {
        let entry = entry.map_err(|e| Error::Io(dir.to_owned(), e))?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        // The file type does not follow links, so a link loop cannot make the walk endless.
        let Ok(kind) = entry.file_type() else { continue };
        if name.starts_with('.') || (kind.is_dir() && name == "target") {
            continue;
        }
        let path = format!("{relative}/{name}");
        if kind.is_dir() {
            list_files(&entry.path(), &path, files)?;
        } else if kind.is_file() {
            files.push(path);
        }
    }
    Ok(())
}
