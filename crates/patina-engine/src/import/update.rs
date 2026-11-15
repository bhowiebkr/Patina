//! **Update** of an imported folder (spec 6.2).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    Applied, absolute, digest, files, hash, inside, manifest, member, move_folder, path_deps, read_project, remove, renamed_manifest, undo_moves,
    users, work, write_files,
};
use crate::project::{self, ProjectFile};
use crate::{Project, Toolchain};

/// The differences between an imported folder and its folder in the other project.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Changes {
    /// The folder in the other project changed after the import or the last update.
    pub other: bool,
    /// The copy in this project changed after the import or the last update.
    pub this: bool,
    pub files: Vec<FileChange>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub state: FileState,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub enum FileState {
    OnlyOther,
    OnlyThis,
    Different,
}

/// The version of a file that an update keeps.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    Other,
    This,
}

impl Project {
    /// Updates the imported `folder` from the project at `other` (6.2). Patina builds the changed
    /// folders first. A failed build puts the project back as it was.
    pub fn update(&mut self, toolchain: &Toolchain, other: &Path, folder: &str, picks: &BTreeMap<String, Pick>) -> Result<(), String> {
        self.check_toolchain(toolchain)?;
        let applied = update(&self.root, other, folder, picks)?;
        self.build_applied(toolchain, applied)
    }
}

/// The ID of the project that the imported `folder` came from.
pub fn source_id(root: &Path, folder: &str) -> Result<String, String> {
    let file = read_project(root)?;
    file.imports.get(folder).map(|r| r.project.clone()).ok_or_else(|| format!("{folder} is not an imported folder"))
}

/// The differences between the imported `folder` and its folder in the project at `other`.
pub fn changes(root: &Path, other: &Path, folder: &str) -> Result<Changes, String> {
    Ok(compare(&absolute(root)?, &absolute(other)?, folder)?.changes)
}

/// Writes the new version of the imported `folder` with the picks of the user. When the copy did
/// not change, each file comes from the other project. An error leaves the project as it was.
pub fn update(root: &Path, other: &Path, folder: &str, picks: &BTreeMap<String, Pick>) -> Result<Applied, String> {
    let (root, other) = (absolute(root)?, absolute(other)?);
    let Compare { mut file, record, mut theirs, mut ours, changes, other_hash } = compare(&root, &other, folder)?;
    if !changes.other {
        return Err("the folder in the other project did not change".to_owned());
    }
    for change in &changes.files {
        let pick = match changes.this {
            true => *picks.get(&change.path).ok_or_else(|| format!("pick the version of {}", change.path))?,
            false => Pick::Other,
        };
        if pick == Pick::Other {
            match theirs.remove(&change.path) {
                Some(bytes) => ours.insert(change.path.clone(), bytes),
                None => ours.remove(&change.path),
            };
        }
    }
    let path = root.join("patina.toml");
    let old = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let work = work(&root);
    remove(&work)?;
    let staged = work.join("new").join(folder);
    let copy = Some(digest(ours.iter()));
    file.imports.insert(folder.to_owned(), project::Import { hash: other_hash, copy, ..record });
    let mut moves = Vec::new();
    let moved = write_files(&staged, ours)
        .and_then(|()| move_folder(&root.join(folder), &work.join("old").join(folder), &mut moves))
        .and_then(|()| move_folder(&staged, &root.join(folder), &mut moves))
        .and_then(|()| project::write(&path, &file).map_err(|e| e.to_string()));
    if let Err(e) = moved {
        let _ = undo_moves(&moves);
        let _ = remove(&work);
        return Err(e);
    }
    let mut packages: Vec<String> = member(folder).into_iter().chain(users(&root, folder).iter().filter_map(|u| member(u))).collect();
    packages.sort();
    Ok(Applied { root, moves, project: old, packages })
}

struct Compare {
    file: ProjectFile,
    record: project::Import,
    /// The files of the other project, with the crate names of this project.
    theirs: BTreeMap<String, Vec<u8>>,
    ours: BTreeMap<String, Vec<u8>>,
    changes: Changes,
    other_hash: String,
}

fn compare(root: &Path, other: &Path, folder: &str) -> Result<Compare, String> {
    let (file, source) = (read_project(root)?, read_project(other)?);
    let record = file.imports.get(folder).cloned().ok_or_else(|| format!("{folder} is not an imported folder"))?;
    if record.project != source.id {
        return Err(format!("{folder} did not come from this project"));
    }
    let (group, _) = folder.split_once('/').expect("a record names group/name");
    let from = format!("{group}/{}", record.folder);
    let dir = other.join(&from);
    if !dir.join("Cargo.toml").is_file() {
        return Err(format!("the other project has no folder {from}"));
    }
    // The folders of this project with the copies of the other project, by their folder there.
    let mut renames = BTreeMap::new();
    for (key, r) in file.imports.iter().filter(|(_, r)| r.project == record.project) {
        let (g, _) = key.split_once('/').expect("a record names group/name");
        renames.insert(format!("{g}/{}", r.folder), key.clone());
    }
    renames.insert(from.clone(), folder.to_owned());
    for (_, path) in path_deps(&dir)? {
        let dep = inside(other, &dir.join(&path))
            .filter(|d| d.starts_with("types/") || d.starts_with("widgets/"))
            .ok_or_else(|| format!("{from} depends on {path}, which is not a type or widget folder of its project"))?;
        if !root.join(renames.get(&dep).unwrap_or(&dep)).join("Cargo.toml").is_file() {
            return Err(format!("{from} depends on {dep} now. Import {dep} first"));
        }
    }
    let mut theirs: BTreeMap<String, Vec<u8>> = files(&dir)?.into_iter().collect();
    if let Some(text) = renamed_manifest(manifest(&dir)?, &dir, &from, folder, other, &renames)? {
        theirs.insert("Cargo.toml".to_owned(), text.into_bytes());
    }
    let ours: BTreeMap<String, Vec<u8>> = files(&root.join(folder))?.into_iter().collect();
    let other_hash = hash(&dir)?;
    let mut paths: Vec<&String> = theirs.keys().chain(ours.keys()).collect();
    paths.sort();
    paths.dedup();
    let files = paths.into_iter().filter_map(|p| {
        let state = match (theirs.get(p), ours.get(p)) {
            (Some(a), Some(b)) if a == b => return None,
            (Some(_), None) => FileState::OnlyOther,
            (None, Some(_)) => FileState::OnlyThis,
            _ => FileState::Different,
        };
        Some(FileChange { path: p.clone(), state })
    });
    let changes = Changes {
        other: other_hash != record.hash,
        this: record.copy.as_deref() != Some(digest(ours.iter()).as_str()),
        files: files.collect(),
    };
    Ok(Compare { file, record, theirs, ours, changes, other_hash })
}

#[cfg(test)]
mod tests {
    use super::super::{Choice, apply};
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
    fn an_update_finds_the_changes_of_each_side_and_keeps_the_picks() {
        let base = std::env::temp_dir().join(format!("patina-update-{}", std::process::id()));
        let (root, other) = (base.join("this"), base.join("other"));
        let _ = fs::remove_dir_all(&base);
        let image = "image = { path = \"../../types/image\" }\n";
        write(&other, &[
            ("patina.toml", "format = 1\nid = \"aaaaaaaaaaaaaaaa\"\nrust = \"1.94.0\"\n"),
            ("nodes/blur/Cargo.toml", &manifest("blur", image)),
            ("nodes/blur/src/lib.rs", "// 1"),
            ("types/image/Cargo.toml", &manifest("image", "")),
        ]);
        write(&root, &[("patina.toml", "format = 1\nid = \"bbbbbbbbbbbbbbbb\"\nrust = \"1.94.0\"\n"), ("types/image/Cargo.toml", "")]);
        let choices = BTreeMap::from([("types/image".to_owned(), Choice::Rename { name: "image2".into() })]);
        apply(&root, &other, "nodes/blur", &choices).unwrap().finish().unwrap();
        let none = Changes { other: false, this: false, files: vec![] };
        assert_eq!(changes(&root, &other, "nodes/blur").unwrap(), none);
        assert_eq!(changes(&root, &other, "types/image2").unwrap(), none);
        assert_eq!(source_id(&root, "nodes/blur").unwrap(), "aaaaaaaaaaaaaaaa");

        let lib = root.join("nodes/blur/src/lib.rs");
        let picks = BTreeMap::new();
        assert!(update(&root, &other, "nodes/blur", &picks).unwrap_err().contains("did not change"));
        write(&other, &[("nodes/blur/src/lib.rs", "// 2"), ("nodes/blur/new.txt", "n")]);
        let found = changes(&root, &other, "nodes/blur").unwrap();
        assert!(found.other && !found.this);
        assert_eq!(found.files.iter().map(|f| f.state).collect::<Vec<_>>(), [FileState::OnlyOther, FileState::Different]);
        let applied = update(&root, &other, "nodes/blur", &picks).unwrap();
        assert_eq!(applied.packages, ["blur"]);
        assert_eq!(fs::read_to_string(&lib).unwrap(), "// 2");
        applied.undo().unwrap();
        assert_eq!(fs::read_to_string(&lib).unwrap(), "// 1");
        update(&root, &other, "nodes/blur", &picks).unwrap().finish().unwrap();
        assert_eq!(changes(&root, &other, "nodes/blur").unwrap(), none);

        write(&root, &[("nodes/blur/src/lib.rs", "// mine")]);
        write(&other, &[("nodes/blur/src/lib.rs", "// 3")]);
        assert!(changes(&root, &other, "nodes/blur").unwrap().this);
        assert!(update(&root, &other, "nodes/blur", &picks).unwrap_err().contains("pick"));
        let picks = BTreeMap::from([("src/lib.rs".to_owned(), Pick::This)]);
        update(&root, &other, "nodes/blur", &picks).unwrap().finish().unwrap();
        assert_eq!(fs::read_to_string(&lib).unwrap(), "// mine");
        assert!(!changes(&root, &other, "nodes/blur").unwrap().other);

        write(&other, &[("nodes/blur/Cargo.toml", &manifest("blur", "extra = { path = \"../../types/extra\" }\n"))]);
        assert!(changes(&root, &other, "nodes/blur").unwrap_err().contains("Import types/extra first"));
        let _ = fs::remove_dir_all(&base);
    }
}
