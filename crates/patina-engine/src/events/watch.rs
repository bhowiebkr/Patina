//! The scans of a `file_change` trigger (spec 9.4).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The time and the size of each file that a scan found.
type Scan = BTreeMap<PathBuf, (Option<SystemTime>, u64)>;

/// The files of one `file_change` trigger at the last scan.
pub(super) struct Watch {
    path: PathBuf,
    seen: Scan,
}

impl Watch {
    /// Takes the first scan of `path`: a file, or the files directly in a folder.
    pub(super) fn new(path: PathBuf) -> Watch {
        let seen = scan(&path);
        Watch { path, seen }
    }

    /// Scans again. Returns the path of each file that changed, came or went since the last scan.
    pub(super) fn changes(&mut self) -> Vec<String> {
        let now = scan(&self.path);
        let changed = now.iter().filter(|&(path, file)| self.seen.get(path) != Some(file)).map(|(path, _)| path);
        let gone = self.seen.keys().filter(|path| !now.contains_key(*path));
        let mut paths: Vec<String> = changed.chain(gone).map(|path| path.display().to_string()).collect();
        paths.sort_unstable();
        self.seen = now;
        paths
    }
}

// shortcut: a poll of the folder, change it to the change events of the system when folders get large.
fn scan(path: &Path) -> Scan {
    let file = |path: &Path| fs::metadata(path).ok().filter(|m| m.is_file()).map(|m| (m.modified().ok(), m.len()));
    match fs::read_dir(path) {
        Ok(entries) => entries.flatten().filter_map(|e| file(&e.path()).map(|f| (e.path(), f))).collect(),
        Err(_) => file(path).map(|f| (path.to_path_buf(), f)).into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_watch_finds_changed_new_and_removed_files() {
        let folder = std::env::temp_dir().join(format!("patina-watch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(folder.join("inner")).unwrap();
        let (a, b) = (folder.join("a.txt"), folder.join("b.txt"));
        fs::write(&a, "1").unwrap();
        let mut watch = Watch::new(folder.clone());
        let mut file = Watch::new(a.clone());
        assert!(watch.changes().is_empty());
        fs::write(&a, "12").unwrap();
        fs::write(&b, "1").unwrap();
        fs::write(folder.join("inner/c.txt"), "1").unwrap();
        let name = |p: &Path| p.display().to_string();
        assert_eq!(watch.changes(), [name(&a), name(&b)]);
        assert_eq!(file.changes(), [name(&a)]);
        fs::remove_file(&b).unwrap();
        assert_eq!(watch.changes(), [name(&b)]);
        assert!(file.changes().is_empty());
        fs::remove_dir_all(&folder).unwrap();
    }
}
