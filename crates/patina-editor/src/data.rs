//! The data folder of Patina (spec 13.3): the lists of recent and trusted projects (11, 15.10) and
//! the layout of each project (15.2).

use std::fs;
use std::path::{Path, PathBuf};

/// The number of projects in the recent list.
const RECENT_LIMIT: usize = 10;
const RECENT: &str = "recent.json";
const TRUSTED: &str = "trusted.json";
const LAYOUTS: &str = "layouts";
/// A layout is a few kilobytes. A larger text is an error of the page.
const LAYOUT_LIMIT: usize = 64 * 1024;

pub struct Data {
    pub dir: PathBuf,
    /// The default parent folder of new projects.
    pub projects: PathBuf,
}

impl Data {
    /// Finds the data folder: the `--portable` folder, then the folder of a portable edition,
    /// then the folder of the user.
    pub fn find(portable: Option<PathBuf>) -> Result<Data, String> {
        let exe_dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_owned));
        let (dir, projects) = match (portable, exe_dir) {
            (Some(dir), _) => (dir.join("data"), dir.join("projects")),
            (None, Some(exe)) if exe.join("portable").is_file() => (exe.join("data"), exe.join("projects")),
            (None, _) => {
                let base = dirs::data_local_dir().ok_or("the folder for local data is not known")?;
                let name = if cfg!(target_os = "linux") { "patina" } else { "Patina" };
                let documents = dirs::document_dir().unwrap_or_else(|| base.clone());
                (base.join(name).join("data"), documents.join("Patina"))
            }
        };
        let dir = std::path::absolute(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        fs::create_dir_all(&dir).map_err(|e| format!("Patina cannot write to {}: {e}", dir.display()))?;
        Ok(Data { dir, projects })
    }

    /// The recent projects, the newest first.
    pub fn recent(&self) -> Vec<PathBuf> {
        self.read_list(RECENT)
    }

    pub fn add_recent(&self, root: &Path) -> Result<(), String> {
        let mut list = self.recent();
        list.retain(|path| !same_path(path, root));
        list.insert(0, root.to_owned());
        list.truncate(RECENT_LIMIT);
        self.write_list(RECENT, &list)
    }

    pub fn is_trusted(&self, root: &Path) -> bool {
        self.read_list(TRUSTED).iter().any(|path| same_path(path, root))
    }

    pub fn trust(&self, root: &Path) -> Result<(), String> {
        let mut list = self.read_list(TRUSTED);
        if !list.iter().any(|path| same_path(path, root)) {
            list.push(root.to_owned());
        }
        self.write_list(TRUSTED, &list)
    }

    /// The saved layout of a project, or none.
    pub fn layout(&self, id: &str) -> Result<Option<String>, String> {
        let path = self.layout_path(id)?;
        match fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save_layout(&self, id: &str, text: &str) -> Result<(), String> {
        if text.len() > LAYOUT_LIMIT {
            return Err(format!("the layout has more than {LAYOUT_LIMIT} bytes"));
        }
        serde_json::from_str::<serde::de::IgnoredAny>(text).map_err(|e| format!("the layout is not JSON: {e}"))?;
        let path = self.layout_path(id)?;
        let dir = self.dir.join(LAYOUTS);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        write_file(&path, text.as_bytes())
    }

    /// The ID comes from `patina.toml`, so it must be a safe file name. Hex digits are never a
    /// reserved name of Windows, for example `CON`.
    fn layout_path(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("Patina saves no layout for the project ID {id:?}. Use 1 to 64 hex digits."));
        }
        Ok(self.dir.join(LAYOUTS).join(format!("{id}.json")))
    }

    /// A missing or bad list is empty. Then no project is trusted, which is the safe side.
    fn read_list(&self, name: &str) -> Vec<PathBuf> {
        let bytes = fs::read(self.dir.join(name)).unwrap_or_default();
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    fn write_list(&self, name: &str, list: &[PathBuf]) -> Result<(), String> {
        let text = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
        write_file(&self.dir.join(name), &text)
    }
}

/// Writes a temporary file, then renames it, so that a crash never leaves half a file.
fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("json.tmp");
    let io = |e: std::io::Error| format!("{}: {e}", path.display());
    fs::write(&temp, bytes).map_err(io)?;
    fs::rename(&temp, path).map_err(io)
}

/// Windows paths do not have a case, so `C:\A` and `c:\a` are one folder.
fn same_path(a: &Path, b: &Path) -> bool {
    let key = |path: &Path| path.components().map(|c| c.as_os_str().to_ascii_lowercase()).collect::<Vec<_>>();
    if cfg!(windows) { key(a) == key(b) } else { a == b }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lists_keep_the_newest_project_first_and_trust_once() {
        let dir = std::env::temp_dir().join(format!("patina-data-{}", std::process::id()));
        let data = Data::find(Some(dir.clone())).unwrap();
        let path = |n: usize| dir.join(format!("p{n}"));
        for n in 0..12 {
            data.add_recent(&path(n)).unwrap();
        }
        data.add_recent(&path(5)).unwrap();
        let recent = data.recent();
        assert_eq!(recent.len(), RECENT_LIMIT);
        assert_eq!((&recent[0], &recent[1]), (&path(5), &path(11)));
        assert!(!data.is_trusted(&path(1)));
        data.trust(&path(1)).unwrap();
        data.trust(&path(1)).unwrap();
        let trusted = data.read_list(TRUSTED);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(trusted, [path(1)]);
        assert!(same_path(Path::new("/a/b/"), Path::new("/a/b")));
        assert_eq!(same_path(Path::new("/A/b"), Path::new("/a/b")), cfg!(windows));
    }

    #[test]
    fn layouts_need_a_hex_id_and_json() {
        let dir = std::env::temp_dir().join(format!("patina-layout-{}", std::process::id()));
        let data = Data::find(Some(dir.clone())).unwrap();
        let id = "3f9c2a7d41e0b85c";
        let before = data.layout(id);
        data.save_layout(id, r#"{"version":1}"#).unwrap();
        let after = data.layout(id);
        let bad_ids = ["", "con", "../3f9c", "3f9c.json", &"a".repeat(65)].map(|bad| data.save_layout(bad, "{}").is_err());
        let not_json = data.save_layout(id, "{");
        let large = data.save_layout(id, &format!("\"{}\"", "a".repeat(LAYOUT_LIMIT)));
        let kept = data.layout(id);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(before, Ok(None));
        assert_eq!(after, Ok(Some(r#"{"version":1}"#.to_owned())));
        assert_eq!(bad_ids, [true; 5]);
        assert!(not_json.is_err() && large.is_err());
        assert_eq!(kept, after);
    }
}
