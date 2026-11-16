//! **Import** of a folder from another project (spec 6.2).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::plan::{check_name, name_key};
use crate::project::{self, ProjectFile};
use crate::{Project, Toolchain, cargo};

mod update;
pub use update::{Changes, FileChange, FileState, Pick, Side, changes, source_id, update};

/// The folder groups that are crates (6.2).
const GROUPS: [&str; 5] = ["nodes", "views", "widgets", "connections", "types"];

/// The groups that are members of the workspace, so a build can name their crates (8.1).
const MEMBERS: [&str; 3] = ["nodes", "widgets", "connections"];

/// One folder that an import copies, with its state in this project.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Item {
    /// The folder, for example `types/image`. It has the same path in the two projects.
    pub folder: String,
    pub state: State,
    /// The folders of this project that depend on the folder that a replace changes.
    pub users: Vec<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind")]
pub enum State {
    /// This project has no folder with the crate name.
    New,
    /// This project has the folder with the same files. The import keeps it.
    Same,
    /// This project has a folder with the crate name and other files. `replace` is false when
    /// that folder is in another group, so only a rename can solve it.
    Conflict { existing: String, replace: bool },
}

/// The choice of the user for a conflict.
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum Choice {
    Replace,
    Keep,
    Rename { name: String },
}

/// The folders of the project at `other` that an import can copy.
pub fn folders(other: &Path) -> Vec<String> {
    let mut folders = Vec::new();
    for group in GROUPS {
        for entry in fs::read_dir(other.join(group)).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if check_name(&name).is_ok() && entry.path().join("Cargo.toml").is_file() {
                folders.push(format!("{group}/{name}"));
            }
        }
    }
    folders.sort();
    folders
}

/// The folders that an import of `folder` from the project at `other` copies: the folder, then
/// the type and widget folders that it depends on.
pub fn plan(root: &Path, other: &Path, folder: &str) -> Result<Vec<Item>, String> {
    let (root, other) = (absolute(root)?, absolute(other)?);
    let folders = closure(&other, folder)?;
    folders.iter().map(|f| item(&root, &other, f)).collect()
}

/// An import that is in the project. The caller builds the project, then calls
/// [`Applied::finish`], or [`Applied::undo`] when the build fails.
#[derive(Debug)]
pub struct Applied {
    root: PathBuf,
    /// Each folder move `(from, to)`, in order.
    moves: Vec<(PathBuf, PathBuf)>,
    project: String,
    /// The crates that the build check compiles.
    pub packages: Vec<String>,
}

impl Project {
    /// Imports `folder` from the project at `other` with the choices of the user (6.2). Patina
    /// builds the changed folders first. A failed build puts the project back as it was.
    pub fn import(&mut self, toolchain: &Toolchain, other: &Path, folder: &str, choices: &BTreeMap<String, Choice>) -> Result<(), String> {
        self.check_toolchain(toolchain)?;
        let applied = apply(&self.root, other, folder, choices)?;
        self.build_applied(toolchain, applied)
    }

    /// The imported folders of the project (6.4), for example `nodes/blur`.
    pub fn imports(&self) -> Vec<String> {
        read_project(&self.root).map(|f| f.imports.into_keys().collect()).unwrap_or_default()
    }

    fn check_toolchain(&self, toolchain: &Toolchain) -> Result<(), String> {
        match toolchain.release == self.rust {
            true => Ok(()),
            false => Err(format!("the project needs Rust {}, but the toolchain is Rust {}", self.rust, toolchain.release)),
        }
    }

    /// Builds the changed crates of an import or an update, and undoes it when the build fails.
    fn build_applied(&mut self, toolchain: &Toolchain, applied: Applied) -> Result<(), String> {
        if let Err(e) = cargo::build_packages(&self.root, toolchain, &applied.packages) {
            return applied.undo().and(Err(e.to_string()));
        }
        // The copied folders can change the code of each node, so the run builds again.
        self.code_revision += 1;
        self.last_code = None;
        applied.finish()
    }
}

fn read_project(root: &Path) -> Result<ProjectFile, String> {
    project::read(&root.join("patina.toml"), |f: &ProjectFile| f.format).map_err(|e| e.to_string())
}

impl Applied {
    /// Removes the backups of the replaced folders.
    pub fn finish(self) -> Result<(), String> {
        remove(&work(&self.root))
    }

    /// Puts the project back as it was before the import.
    pub fn undo(self) -> Result<(), String> {
        undo_moves(&self.moves)?;
        let file = self.root.join("patina.toml");
        fs::write(&file, &self.project).map_err(|e| format!("{}: {e}", file.display()))?;
        remove(&work(&self.root))
    }
}

/// Copies the folders of an import into the project at `root` with the choices of the user, and
/// records each copy in `patina.toml` (6.4). An error leaves the project as it was.
pub fn apply(root: &Path, other: &Path, folder: &str, choices: &BTreeMap<String, Choice>) -> Result<Applied, String> {
    let (root, other) = (absolute(root)?, absolute(other)?);
    let (mut file, source) = (read_project(&root)?, read_project(&other)?);
    if file.id == source.id {
        return Err("the folder is in this project already".to_owned());
    }
    let items = plan(&root, &other, folder)?;
    let copies = targets(&root, &items, choices)?;
    let path = root.join("patina.toml");
    let old = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let work = work(&root);
    remove(&work)?;
    let mut packages = Vec::new();
    let mut moves = Vec::new();
    let copied = (|| {
        for (from, to) in &copies {
            let staged = work.join("new").join(to);
            copy(&other.join(from), &staged)?;
            if let Some(text) = renamed_manifest(manifest(&staged)?, &other.join(from), from, to, &other, &copies)? {
                let path = staged.join("Cargo.toml");
                fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            if let Some(item) = items.iter().find(|i| &i.folder == to && matches!(i.state, State::Conflict { .. })) {
                move_folder(&root.join(to), &work.join("old").join(to), &mut moves)?;
                packages.extend(item.users.iter().filter_map(|u| member(u)));
            }
            move_folder(&staged, &root.join(to), &mut moves)?;
            packages.extend(member(to));
            let copied = files(&root.join(to))?;
            let (hash, copy) = (hash(&other.join(from))?, Some(digest(copied.iter().map(|(p, b)| (p, b)))));
            let files = Some(file_hashes(copied.iter().map(|(p, b)| (p, b))));
            file.imports.insert(to.clone(), project::Import { project: source.id.clone(), folder: crate_name(from), hash, copy, files });
        }
        Ok::<(), String>(())
    })();
    // project::write keeps the old file when it fails, so only the folders go back.
    if let Err(e) = copied.and_then(|()| project::write(&path, &file).map_err(|e| e.to_string())) {
        let _ = undo_moves(&moves);
        let _ = remove(&work);
        return Err(e);
    }
    packages.sort();
    packages.dedup();
    Ok(Applied { root, moves, project: old, packages })
}

/// The target folder of each folder that the import copies, from the choices of the user.
fn targets(root: &Path, items: &[Item], choices: &BTreeMap<String, Choice>) -> Result<BTreeMap<String, String>, String> {
    let mut targets = BTreeMap::new();
    let mut names: Vec<String> = Vec::new();
    for item in items {
        let f = &item.folder;
        let (group, _) = f.split_once('/').expect("a checked folder has a group");
        let target = match (&item.state, choices.get(f)) {
            (State::New, _) => Some(f.clone()),
            (State::Same, _) => None,
            (State::Conflict { .. }, None) => return Err(format!("pick replace, keep or rename for {f}")),
            (State::Conflict { replace: true, .. }, Some(Choice::Replace)) => Some(f.clone()),
            (State::Conflict { replace: true, .. }, Some(Choice::Keep)) => None,
            (State::Conflict { existing, .. }, Some(Choice::Replace | Choice::Keep)) => {
                return Err(format!("{existing} has the crate name of {f}. Import {f} with another name"));
            }
            (State::Conflict { .. }, Some(Choice::Rename { name })) => {
                check_name(name)?;
                if let Some(other) = clash(root, name) {
                    return Err(format!("the folder {other} has the name {name}"));
                }
                Some(format!("{group}/{name}"))
            }
        };
        let Some(to) = target else { continue };
        let key = name_key(&crate_name(&to));
        if names.contains(&key) {
            return Err(format!("two imported folders have the name of {to}"));
        }
        names.push(key);
        targets.insert(f.clone(), to);
    }
    Ok(targets)
}

/// The folder and the type and widget folders that it depends on, without repeats.
fn closure(other: &Path, folder: &str) -> Result<Vec<String>, String> {
    check_folder(folder)?;
    let mut found = vec![folder.to_owned()];
    let mut next = 0;
    while next < found.len() {
        let dir = other.join(&found[next]);
        if !dir.join("Cargo.toml").is_file() {
            return Err(format!("the other project has no folder {}", found[next]));
        }
        for (_, path) in path_deps(&dir)? {
            let dep = inside(other, &dir.join(&path))
                .filter(|d| d.starts_with("types/") || d.starts_with("widgets/"))
                .ok_or_else(|| format!("{} depends on {path}, which is not a type or widget folder of its project", found[next]))?;
            check_folder(&dep)?;
            if !found.contains(&dep) {
                found.push(dep);
            }
        }
        next += 1;
    }
    Ok(found)
}

fn item(root: &Path, other: &Path, folder: &str) -> Result<Item, String> {
    let name = folder.split_once('/').map_or(folder, |(_, n)| n);
    let here = root.join(folder);
    let (state, users) = if here.is_dir() {
        if files(&here)? == files(&other.join(folder))? {
            (State::Same, Vec::new())
        } else {
            (State::Conflict { existing: folder.to_owned(), replace: true }, users(root, folder))
        }
    } else if let Some(existing) = clash(root, name) {
        (State::Conflict { existing, replace: false }, Vec::new())
    } else {
        (State::New, Vec::new())
    };
    Ok(Item { folder: folder.to_owned(), state, users })
}

/// The folders of the project at `root` that depend on `folder`.
fn users(root: &Path, folder: &str) -> Vec<String> {
    let mut users = Vec::new();
    for group in GROUPS {
        for entry in fs::read_dir(root.join(group)).into_iter().flatten().flatten() {
            let user = format!("{group}/{}", entry.file_name().to_string_lossy());
            let deps = path_deps(&entry.path()).unwrap_or_default();
            if user != folder && deps.iter().any(|(_, p)| inside(root, &entry.path().join(p)).as_deref() == Some(folder)) {
                users.push(user);
            }
        }
    }
    users.sort();
    users
}

/// The folder of the project at `root` with the crate name `name`, in any group.
fn clash(root: &Path, name: &str) -> Option<String> {
    let key = name_key(name);
    GROUPS.iter().find_map(|group| {
        let entries = fs::read_dir(root.join(group)).into_iter().flatten().flatten();
        entries.map(|e| e.file_name().to_string_lossy().into_owned()).find(|n| name_key(n) == key).map(|n| format!("{group}/{n}"))
    })
}

fn check_folder(folder: &str) -> Result<(), String> {
    match folder.split_once('/') {
        Some((group, name)) if GROUPS.contains(&group) => check_name(name),
        _ => Err(format!("{folder:?} is not a node, view, widget, connection or type folder")),
    }
}

/// The crate of a folder when the build can name it.
fn member(folder: &str) -> Option<String> {
    let (group, name) = folder.split_once('/')?;
    MEMBERS.contains(&group).then(|| name.to_owned())
}

/// The key and the path of each path dependency in the `Cargo.toml` of `dir`.
fn path_deps(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let manifest = manifest(dir)?;
    let mut deps = Vec::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        for (key, value) in manifest.get(section).and_then(toml::Value::as_table).into_iter().flatten() {
            if let Some(path) = value.get("path").and_then(toml::Value::as_str) {
                deps.push((key.clone(), path.to_owned()));
            }
        }
    }
    Ok(deps)
}

fn manifest(dir: &Path) -> Result<toml::Table, String> {
    let path = dir.join("Cargo.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.parse().map_err(|e| format!("{}: {e}", path.display()))
}

/// The `Cargo.toml` of the copy of `from` (at `source`) for the renamed folders, or None when no
/// name changes. A renamed crate gets its new name, and a dependency on it gets the new path and
/// the `package` key.
/// shortcut: the new Cargo.toml loses its comments and key order, change it if users miss them.
fn renamed_manifest(mut manifest: toml::Table, source: &Path, from: &str, to: &str, other: &Path, renames: &BTreeMap<String, String>) -> Result<Option<String>, String> {
    let mut changed = false;
    if from != to {
        let package = manifest.get_mut("package").and_then(toml::Value::as_table_mut).ok_or("the Cargo.toml has no [package]")?;
        package.insert("name".to_owned(), crate_name(to).into());
        changed = true;
    }
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        for (_, value) in manifest.get_mut(section).and_then(toml::Value::as_table_mut).into_iter().flatten() {
            let Some(dep) = value.as_table_mut() else { continue };
            let Some(path) = dep.get("path").and_then(toml::Value::as_str) else { continue };
            let Some(target) = inside(other, &source.join(path)).and_then(|f| renames.get(&f).filter(|t| **t != f)) else { continue };
            dep.insert("path".to_owned(), format!("../../{target}").into());
            dep.insert("package".to_owned(), crate_name(target).into());
            changed = true;
        }
    }
    match changed {
        true => toml::to_string(&manifest).map(Some).map_err(|e| e.to_string()),
        false => Ok(None),
    }
}

fn crate_name(folder: &str) -> String {
    folder.split_once('/').map_or(folder, |(_, n)| n).to_owned()
}

/// The folder `group/name` of the project at `root` that holds `path`, with `..` resolved.
fn inside(root: &Path, path: &Path) -> Option<String> {
    let mut parts: Vec<Component> = Vec::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::CurDir => {}
            _ => parts.push(part),
        }
    }
    let rest: PathBuf = parts.iter().collect::<PathBuf>().strip_prefix(root).ok()?.to_owned();
    let names: Vec<&str> = rest.components().map(|c| c.as_os_str().to_str()).collect::<Option<_>>()?;
    (names.len() == 2).then(|| names.join("/"))
}

/// The relative paths with `/` and the bytes of the files of a folder, in sorted order. Hidden
/// files, links and `target` folders are not part of a folder (6.2).
pub(crate) fn files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut paths = Vec::new();
    crate::list_files(dir, ".", &mut paths).map_err(|e| e.to_string())?;
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let file = dir.join(&p);
            let bytes = fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            Ok((p.trim_start_matches("./").to_owned(), bytes))
        })
        .collect()
}

/// The import hash of a folder (6.4): SHA-256 of each path, a zero byte, the length and the bytes.
pub fn hash(dir: &Path) -> Result<String, String> {
    Ok(digest(files(dir)?.iter().map(|(p, b)| (p, b))))
}

/// The import hash of files in the sorted order of their paths.
fn digest<'a>(files: impl Iterator<Item = (&'a String, &'a Vec<u8>)>) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("sha256:{}", hasher.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>())
}

/// The hash of each file, with the method of the import hash for one file (6.4).
fn file_hashes<'a>(files: impl Iterator<Item = (&'a String, &'a Vec<u8>)>) -> BTreeMap<String, String> {
    files.map(|(p, b)| (p.clone(), digest(std::iter::once((p, b))))).collect()
}

pub(crate) fn copy(from: &Path, to: &Path) -> Result<(), String> {
    write_files(to, files(from)?)
}

fn write_files(to: &Path, files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Result<(), String> {
    for (path, bytes) in files {
        let file = to.join(&path);
        let parent = file.parent().expect("a file is in a folder");
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        fs::write(&file, bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    Ok(())
}

fn move_folder(from: &Path, to: &Path, moves: &mut Vec<(PathBuf, PathBuf)>) -> Result<(), String> {
    let parent = to.parent().expect("a folder has a parent");
    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    fs::rename(from, to).map_err(|e| format!("{}: {e}", from.display()))?;
    moves.push((from.to_owned(), to.to_owned()));
    Ok(())
}

fn undo_moves(moves: &[(PathBuf, PathBuf)]) -> Result<(), String> {
    for (from, to) in moves.iter().rev() {
        fs::rename(to, from).map_err(|e| format!("{}: {e}", to.display()))?;
    }
    Ok(())
}

/// The work folder of an import: `new` holds the copies, `old` the replaced folders.
fn work(root: &Path) -> PathBuf {
    root.join("build").join("import")
}

fn remove(dir: &Path) -> Result<(), String> {
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", dir.display())),
        _ => Ok(()),
    }
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, files: &[(&str, &str)]) {
        for (path, text) in files {
            let file = root.join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }
    }

    fn manifest(name: &str, deps: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[dependencies]\n{deps}")
    }

    #[test]
    fn an_import_copies_the_dependencies_renames_and_goes_back() {
        let base = std::env::temp_dir().join(format!("patina-import-{}", std::process::id()));
        let (root, other) = (base.join("this"), base.join("other"));
        let _ = fs::remove_dir_all(&base);
        let image = "image = { path = \"../../types/image\" }\n";
        write(&other, &[
            ("patina.toml", "format = 1\nid = \"aaaaaaaaaaaaaaaa\"\nrust = \"1.94.0\"\n"),
            ("nodes/blur/Cargo.toml", &manifest("blur", image)),
            ("nodes/blur/src/lib.rs", "// blur"),
            ("nodes/blur/target/x", "build output"),
            ("types/image/Cargo.toml", &manifest("image", "")),
            ("types/image/src/lib.rs", "// A"),
            ("nodes/bad/Cargo.toml", &manifest("bad", "blur = { path = \"../blur\" }\n")),
        ]);
        let project = "format = 1\nid = \"bbbbbbbbbbbbbbbb\"\nrust = \"1.94.0\"\n";
        write(&root, &[
            ("patina.toml", project),
            ("types/image/Cargo.toml", &manifest("image", "")),
            ("types/image/src/lib.rs", "// B"),
            ("nodes/user/Cargo.toml", &manifest("user", image)),
        ]);
        assert_eq!(folders(&other), ["nodes/bad", "nodes/blur", "types/image"]);
        assert!(plan(&root, &other, "nodes/bad").unwrap_err().contains("not a type or widget folder"));
        assert_eq!(inside(&root, &root.join("nodes/x/../../../y/z")), None);
        let items = plan(&root, &other, "nodes/blur").unwrap();
        let conflict = State::Conflict { existing: "types/image".into(), replace: true };
        assert_eq!(items[0], Item { folder: "nodes/blur".into(), state: State::New, users: vec![] });
        assert_eq!(items[1], Item { folder: "types/image".into(), state: conflict, users: vec!["nodes/user".into()] });

        let mut choices = BTreeMap::new();
        assert!(apply(&root, &other, "nodes/blur", &choices).is_err());
        choices.insert("types/image".to_owned(), Choice::Rename { name: "image2".into() });
        let applied = apply(&root, &other, "nodes/blur", &choices).unwrap();
        assert_eq!(applied.packages, ["blur"]);
        assert!(!root.join("nodes/blur/target").exists());
        let blur = super::manifest(&root.join("nodes/blur")).unwrap();
        assert_eq!(blur["dependencies"]["image"]["package"].as_str(), Some("image2"));
        assert_eq!(blur["dependencies"]["image"]["path"].as_str(), Some("../../types/image2"));
        assert_eq!(super::manifest(&root.join("types/image2")).unwrap()["package"]["name"].as_str(), Some("image2"));
        assert_eq!(fs::read_to_string(root.join("types/image/src/lib.rs")).unwrap(), "// B");
        let file: ProjectFile = project::read(&root.join("patina.toml"), |f: &ProjectFile| f.format).unwrap();
        assert_eq!(file.imports.keys().collect::<Vec<_>>(), ["nodes/blur", "types/image2"]);
        assert_eq!((file.imports["types/image2"].folder.as_str(), file.imports["types/image2"].hash.len()), ("image", 71));
        let blur_files = file.imports["nodes/blur"].files.as_ref().unwrap();
        assert_eq!(blur_files.keys().collect::<Vec<_>>(), ["Cargo.toml", "src/lib.rs"]);
        assert_eq!(blur_files["src/lib.rs"], digest(std::iter::once((&"src/lib.rs".to_owned(), &b"// blur".to_vec()))));
        applied.undo().unwrap();
        assert!(!root.join("nodes/blur").exists() && !root.join("types/image2").exists());
        assert_eq!(fs::read_to_string(root.join("patina.toml")).unwrap(), project);

        choices.insert("types/image".to_owned(), Choice::Replace);
        let applied = apply(&root, &other, "nodes/blur", &choices).unwrap();
        assert_eq!(applied.packages, ["blur", "user"]);
        assert_eq!(fs::read_to_string(root.join("types/image/src/lib.rs")).unwrap(), "// A");
        assert_eq!(plan(&root, &other, "types/image").unwrap()[0].state, State::Same);
        applied.undo().unwrap();
        assert_eq!(fs::read_to_string(root.join("types/image/src/lib.rs")).unwrap(), "// B");
        let _ = fs::remove_dir_all(&base);
    }
}
