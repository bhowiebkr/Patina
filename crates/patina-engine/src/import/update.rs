//! **Update** of an imported folder (spec 6.2).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    Applied, absolute, check_folder, digest, file_hashes, files, hash, inside, member, move_folder, path_deps, read_project, remove, renamed_manifest,
    undo_moves, users, work, write_files,
};
use crate::plan::check_name;
use crate::project::{self, ProjectFile};
use crate::{Project, Toolchain};

/// The differences between an imported folder and its folder in the other project.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Changes {
    /// The folder in the other project changed after the import or the last update.
    pub other: bool,
    pub files: Vec<FileChange>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub state: FileState,
    /// The side that changed the file after the import or the last update.
    pub side: Side,
}

/// The side that changed a file. An update takes the version of that side, and the user picks
/// the version when the two sides changed the file.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Other,
    This,
    Both,
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

/// Writes the new version of the imported `folder`. A file takes the version of the side that
/// changed it, or the pick of the user when the two sides changed it. An error leaves the project
/// as it was.
pub fn update(root: &Path, other: &Path, folder: &str, picks: &BTreeMap<String, Pick>) -> Result<Applied, String> {
    let (root, other) = (absolute(root)?, absolute(other)?);
    let Compare { mut file, record, mut theirs, mut ours, changes, other_hash, their_files } = compare(&root, &other, folder)?;
    if !changes.other {
        return Err("the folder in the other project did not change".to_owned());
    }
    for change in &changes.files {
        let pick = match change.side {
            Side::Other => Pick::Other,
            Side::This => Pick::This,
            Side::Both => *picks.get(&change.path).ok_or_else(|| format!("pick the version of {}", change.path))?,
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
    file.imports.insert(folder.to_owned(), project::Import { hash: other_hash, copy, files: Some(their_files), ..record });
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
    /// The hash of each file of `theirs`, for the record.
    their_files: BTreeMap<String, String>,
}

fn compare(root: &Path, other: &Path, folder: &str) -> Result<Compare, String> {
    let (file, source) = (read_project(root)?, read_project(other)?);
    let record = file.imports.get(folder).cloned().ok_or_else(|| format!("{folder} is not an imported folder"))?;
    if record.project != source.id {
        return Err(format!("{folder} did not come from this project"));
    }
    let group = record_group(folder, &record)?;
    let from = format!("{group}/{}", record.folder);
    let dir = other.join(&from);
    if !dir.join("Cargo.toml").is_file() {
        return Err(format!("the other project has no folder {from}"));
    }
    // The folders of this project with the copies of the other project, by their folder there.
    let mut renames = BTreeMap::new();
    for (key, r) in file.imports.iter().filter(|(_, r)| r.project == record.project) {
        let g = record_group(key, r)?;
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
    if let Some(text) = renamed_manifest(&dir, &dir, &from, folder, other, &renames)? {
        theirs.insert("Cargo.toml".to_owned(), text.into_bytes());
    }
    let ours: BTreeMap<String, Vec<u8>> = files(&root.join(folder))?.into_iter().collect();
    let other_hash = hash(&dir)?;
    let (their_files, our_files) = (file_hashes(theirs.iter()), file_hashes(ours.iter()));
    // A record from before the file hashes knows only if the copy changed.
    let copy_changed = record.copy.as_deref() != Some(digest(ours.iter()).as_str());
    let mut paths: Vec<&String> = their_files.keys().chain(our_files.keys()).collect();
    paths.sort();
    paths.dedup();
    let files = paths.into_iter().filter_map(|p| {
        let (a, b) = (their_files.get(p), our_files.get(p));
        let state = match (a, b) {
            (Some(a), Some(b)) if a == b => return None,
            (Some(_), None) => FileState::OnlyOther,
            (None, Some(_)) => FileState::OnlyThis,
            _ => FileState::Different,
        };
        let (other, this) = match &record.files {
            Some(old) => (a != old.get(p), b != old.get(p)),
            None => (true, copy_changed),
        };
        let side = match (other, this) {
            (true, false) => Side::Other,
            (false, true) => Side::This,
            _ => Side::Both,
        };
        Some(FileChange { path: p.clone(), state, side })
    });
    let changes = Changes { other: other_hash != record.hash, files: files.collect() };
    Ok(Compare { file, record, theirs, ours, changes, other_hash, their_files })
}

/// The group of the import record `key`, after a check of its names. A person can edit
/// patina.toml, so a bad record must give an error and no path outside the project.
fn record_group<'a>(key: &'a str, record: &project::Import) -> Result<&'a str, String> {
    check_folder(key).and_then(|()| check_name(&record.folder)).map_err(|e| format!("patina.toml: the import record {key}: {e}"))?;
    Ok(key.split_once('/').map_or(key, |(group, _)| group))
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
    fn a_bad_import_record_gives_an_error() {
        let base = std::env::temp_dir().join(format!("patina-update-bad-{}", std::process::id()));
        let (root, other) = (base.join("this"), base.join("other"));
        let _ = fs::remove_dir_all(&base);
        let record = |key: &str, folder: &str| format!("[imports.\"{key}\"]
project = \"aaaaaaaaaaaaaaaa\"
folder = \"{folder}\"
hash = \"x\"
");
        write(&other, &[("patina.toml", "format = 1
id = \"aaaaaaaaaaaaaaaa\"
rust = \"1.94.0\"
")]);
        let project = format!("format = 1
id = \"bbbbbbbbbbbbbbbb\"
rust = \"1.94.0\"

{}{}", record("blur", "blur"), record("nodes/up", "../up"));
        write(&root, &[("patina.toml", &project)]);
        for key in ["blur", "nodes/up"] {
            let e = changes(&root, &other, key).unwrap_err();
            assert!(e.contains(&format!("the import record {key}:")), "{e}");
        }
        fs::remove_dir_all(&base).unwrap();
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
        let none = Changes { other: false, files: vec![] };
        fn sides(found: &Changes) -> Vec<(&str, FileState, Side)> {
            found.files.iter().map(|f| (f.path.as_str(), f.state, f.side)).collect()
        }
        assert_eq!(changes(&root, &other, "nodes/blur").unwrap(), none);
        assert_eq!(changes(&root, &other, "types/image2").unwrap(), none);
        assert_eq!(source_id(&root, "nodes/blur").unwrap(), "aaaaaaaaaaaaaaaa");

        let lib = root.join("nodes/blur/src/lib.rs");
        let picks = BTreeMap::new();
        assert!(update(&root, &other, "nodes/blur", &picks).unwrap_err().contains("did not change"));
        write(&other, &[("nodes/blur/src/lib.rs", "// 2"), ("nodes/blur/new.txt", "n")]);
        let found = changes(&root, &other, "nodes/blur").unwrap();
        assert!(found.other);
        assert_eq!(sides(&found), [("new.txt", FileState::OnlyOther, Side::Other), ("src/lib.rs", FileState::Different, Side::Other)]);
        let applied = update(&root, &other, "nodes/blur", &picks).unwrap();
        assert_eq!(applied.packages, ["blur"]);
        assert_eq!(fs::read_to_string(&lib).unwrap(), "// 2");
        applied.undo().unwrap();
        assert_eq!(fs::read_to_string(&lib).unwrap(), "// 1");
        update(&root, &other, "nodes/blur", &picks).unwrap().finish().unwrap();
        assert_eq!(changes(&root, &other, "nodes/blur").unwrap(), none);

        // Each side changed one file, and the two sides changed src/lib.rs.
        write(&root, &[("nodes/blur/src/lib.rs", "// mine"), ("nodes/blur/mine.txt", "m")]);
        write(&other, &[("nodes/blur/src/lib.rs", "// 3"), ("nodes/blur/new.txt", "n2")]);
        let found = changes(&root, &other, "nodes/blur").unwrap();
        assert_eq!(sides(&found), [
            ("mine.txt", FileState::OnlyThis, Side::This),
            ("new.txt", FileState::Different, Side::Other),
            ("src/lib.rs", FileState::Different, Side::Both),
        ]);
        assert!(update(&root, &other, "nodes/blur", &picks).unwrap_err().contains("pick the version of src/lib.rs"));
        let picks = BTreeMap::from([("src/lib.rs".to_owned(), Pick::This)]);
        update(&root, &other, "nodes/blur", &picks).unwrap().finish().unwrap();
        assert_eq!(fs::read_to_string(&lib).unwrap(), "// mine");
        assert_eq!(fs::read_to_string(root.join("nodes/blur/new.txt")).unwrap(), "n2");
        assert_eq!(fs::read_to_string(root.join("nodes/blur/mine.txt")).unwrap(), "m");
        assert!(!changes(&root, &other, "nodes/blur").unwrap().other);
        // The kept file stays different from the other project, so its next change needs a pick.
        write(&other, &[("nodes/blur/src/lib.rs", "// 4")]);
        assert_eq!(sides(&changes(&root, &other, "nodes/blur").unwrap())[1], ("src/lib.rs", FileState::Different, Side::Both));
        // A record from before the file hashes takes each file from the other project, or asks
        // for each file when the copy changed.
        let path = root.join("patina.toml");
        let text = fs::read_to_string(&path).unwrap();
        let old = text.split("[imports.\"nodes/blur\".files]").next().unwrap().to_owned() + &text[text.find("[imports.\"types").unwrap()..];
        fs::write(&path, old).unwrap();
        let all = |side| sides(&changes(&root, &other, "nodes/blur").unwrap()).iter().all(|f| f.2 == side);
        assert!(all(Side::Other));
        write(&root, &[("nodes/blur/mine.txt", "m2")]);
        assert!(all(Side::Both));
        fs::write(&path, text).unwrap();

        write(&other, &[("nodes/blur/Cargo.toml", &manifest("blur", "extra = { path = \"../../types/extra\" }\n"))]);
        assert!(changes(&root, &other, "nodes/blur").unwrap_err().contains("Import types/extra first"));
        let _ = fs::remove_dir_all(&base);
    }
}
