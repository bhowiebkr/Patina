//! The node, widget, view, connection and type folders of a project (6.2): new folders, their
//! start code, and safe reads and writes of their code files.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::edit::Disk;
use crate::{Error, project};

/// Makes the crate folder `<group>/<name>` with the start code in `src/lib.rs`. No other folder of the
/// project can have the same crate name (6.2).
pub(crate) fn new_folder(root: &Path, group: &str, name: &str) -> Result<(), String> {
    project::check_name(name)?;
    let key = project::name_key(name);
    for group in ["nodes", "views", "widgets", "connections", "types"] {
        for entry in fs::read_dir(root.join(group)).into_iter().flatten().flatten() {
            let other = entry.file_name().to_string_lossy().into_owned();
            if project::name_key(&other) == key {
                return Err(format!("the folder {group}/{other} has this name"));
            }
        }
    }
    let parent = root.join(group);
    fs::create_dir_all(&parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    // create_dir fails when the folder exists, so the files of another folder stay.
    let dir = parent.join(name);
    fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let written = start_files(group, name).iter().try_for_each(|(file, text)| write_file(&dir.join(file), text));
    if let Err(e) = written {
        let _ = fs::remove_dir_all(&dir);
        return Err(e.to_string());
    }
    Ok(())
}

/// The files of a new crate folder with their start text.
fn start_files(group: &str, name: &str) -> [(&'static str, String); 2] {
    // A view compiles as its own library (15.8). A type crate is plain Rust (5.1).
    let lib = if group == "views" { "[lib]\ncrate-type = [\"cdylib\"]\n\n" } else { "" };
    let sdk = if group == "types" { "" } else { "patina-sdk = { workspace = true }\n" };
    let manifest = format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n{lib}[dependencies]\n{sdk}");
    [("Cargo.toml", manifest), ("src/lib.rs", start_code(group, name))]
}

/// True when the unsaved code of a new folder is not its start code, so undo keeps the folder and
/// its step (15.2). `code` has the unsaved text of each code file.
pub(crate) fn unsaved_in(folder: &str, code: &BTreeMap<String, String>) -> bool {
    let Some((group, name)) = folder.split_once('/') else { return false };
    let start = start_files(group, name);
    code.iter()
        .filter_map(|(path, text)| Some((path.strip_prefix(folder)?.strip_prefix('/')?, text)))
        .any(|(file, text)| !start.iter().any(|(f, s)| *f == file && s == text))
}

/// Removes a new crate folder for undo (15.2). A folder with other files or changed files stays,
/// because they are work of the user. A file counts with its unsaved text in `code`, and the
/// unsaved text of the folder goes with it.
fn remove_new_folder(root: &Path, group: &str, name: &str, code: &mut BTreeMap<String, String>) -> Result<(), String> {
    let dir = root.join(group).join(name);
    let folder = format!("{group}/{name}/");
    let names = |dir: &Path| {
        let mut names: Vec<String> = fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    };
    let same = start_files(group, name).iter().all(|(file, text)| match code.get(&format!("{folder}{file}")) {
        Some(unsaved) => unsaved == text,
        None => fs::read_to_string(dir.join(file)).is_ok_and(|old| old == *text),
    });
    if !same || names(&dir) != ["Cargo.toml", "src"] || names(&dir.join("src")) != ["lib.rs"] {
        return Err(format!("Undo kept the folder {group}/{name}, because it has changes. Delete it in the file manager."));
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    code.retain(|path, _| !path.starts_with(&folder));
    Ok(())
}

/// Removes or makes a new folder for undo or redo, and returns the step that reverses it.
pub(crate) fn flip_folder(root: &Path, path: &str, made: bool, code: &mut BTreeMap<String, String>) -> Result<Disk, String> {
    let (group, name) = path.split_once('/').ok_or_else(|| format!("{path} is not a crate folder"))?;
    if made { remove_new_folder(root, group, name, code)? } else { new_folder(root, group, name)? }
    Ok(Disk::Folder { path: path.to_owned(), made: !made })
}

/// The start code of a node, widget, view, connection or type folder.
fn start_code(group: &str, name: &str) -> String {
    let ty: String = name.split(['-', '_']).filter(|p| !p.is_empty()).map(|p| p[..1].to_ascii_uppercase() + &p[1..]).collect();
    match group {
        "widgets" => WIDGET_CODE.replace("NAME", &ty),
        "views" => VIEW_CODE.replace("NAME", &ty),
        "connections" => CONNECTION_CODE.replace("NAME", &project::name_key(name)),
        "types" => TYPE_CODE.replace("NAME", &ty),
        _ => format!(
            "use patina_sdk::node;

/// Gives the value of the input. Write the work of the node here.
#[node]
pub fn {}(value: f64) -> f64 {{
    value
}}
",
            project::name_key(name)
        ),
    }
}

pub(crate) fn write_file(path: &Path, text: &str) -> Result<(), Error> {
    let parent = path.parent().expect("a project file is in a folder");
    fs::create_dir_all(parent).map_err(|e| Error::Io(parent.to_owned(), e))?;
    fs::write(path, text).map_err(|e| Error::Io(path.to_owned(), e))
}

/// The start code of a connection folder (6.3). `NAME` is the crate name.
const CONNECTION_CODE: &str = "use patina_sdk::{Lanes, connection};

/// Draws each wire of the type `u16` as a ribbon of 16 lanes, bit 15 at the top. Change the type,
/// the lane names, the label and the color here.
#[connection(lanes = 16, label = \"NAME\", color = \"teal\")]
pub fn NAME(value: &u16) -> Lanes {
    Lanes::bits(16, u64::from(*value))
}
";

/// The start code of a type folder (5.1). `NAME` is the type name.
const TYPE_CODE: &str = "/// A value that goes on a wire between two nodes. Write its fields here. A node or a connection
/// uses it after **Add crate** adds this folder to its Cargo.toml.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NAME(pub u8);
";

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

/// Checks a code folder as `<group>/<name>`, for example `nodes/number` or `types/segments` (6.2).
/// The page sends it.
pub(crate) fn code_folder(folder: &str) -> Result<(), String> {
    let bad = || format!("{folder:?} is not a node, widget, view, connection or type folder");
    let (group, name) = folder.split_once('/').ok_or_else(bad)?;
    if !["nodes", "widgets", "views", "connections", "types"].contains(&group) {
        return Err(bad());
    }
    project::check_name(name)
}

/// The full path of an existing file in a code folder, from a path like `nodes/number/src/lib.rs`
/// (6.2). The page sends the path, so it must stay in the folder.
pub(crate) fn code_file(root: &Path, path: &str) -> Result<PathBuf, String> {
    let bad = || format!("{path:?} is not a file in a code folder");
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

/// The text of a code file on disk, for example `nodes/number/src/lib.rs`.
pub(crate) fn read_code(root: &Path, path: &str) -> Result<String, String> {
    fs::read_to_string(code_file(root, path)?).map_err(|e| format!("{path}: {e}"))
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{Edit, Project};

    #[test]
    fn undo_removes_a_new_folder_only_while_it_has_its_start_files() {
        let root = std::env::temp_dir().join(format!("patina-folder-undo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut project = Project::create(&root, "1.94.0").unwrap();
        let (dir, lib) = (root.join("views/panel"), "views/panel/src/lib.rs");
        project.new_view("panel").unwrap();
        project.apply(&Edit::SetCode { path: lib.to_owned(), text: "// mine\n".to_owned() }).unwrap();
        assert_eq!(project.undo_folder(), None, "the code edit comes first");
        let mut unsaved = BTreeMap::new();
        let start = project.undo(&mut unsaved).unwrap().unwrap().text;
        assert_eq!(project.undo_folder(), Some("views/panel"));
        unsaved.insert(lib.to_owned(), "// draft\n".to_owned());
        assert!(project.undo(&mut unsaved).unwrap_err().contains("Save or undo"), "unsaved changes keep the folder");
        unsaved.insert(lib.to_owned(), start);
        project.undo(&mut unsaved).unwrap();
        assert!(!dir.exists() && unsaved.is_empty(), "undo removes the new folder and its unsaved start code");
        assert_eq!(project.last_code(), Some("views/panel/Cargo.toml"));
        project.redo(&mut unsaved).unwrap();
        assert_eq!(project.redo(&mut unsaved).unwrap().unwrap().text, "// mine\n", "redo makes the folder and gives the code");
        assert!(unsaved[lib] == "// mine\n" && project.read_code(lib).unwrap() != "// mine\n", "redo writes no code file");

        std::fs::write(dir.join("notes.txt"), "mine").unwrap();
        project.undo(&mut unsaved).unwrap();
        let kept = project.undo(&mut unsaved).unwrap_err();
        assert!(kept.contains("views/panel") && dir.join("notes.txt").is_file(), "{kept}");
        assert!(!project.can_undo(), "the folder step goes, so undo goes on with the edits before it");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
