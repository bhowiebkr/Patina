//! The data folder of Patina (spec 13.3) and its lists of recent and trusted projects (11, 15.10).

use std::fs;
use std::path::{Path, PathBuf};

/// The number of projects in the recent list.
const RECENT_LIMIT: usize = 10;
const RECENT: &str = "recent.json";
const TRUSTED: &str = "trusted.json";

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

    /// A missing or bad list is empty. Then no project is trusted, which is the safe side.
    fn read_list(&self, name: &str) -> Vec<PathBuf> {
        let bytes = fs::read(self.dir.join(name)).unwrap_or_default();
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    fn write_list(&self, name: &str, list: &[PathBuf]) -> Result<(), String> {
        let path = self.dir.join(name);
        let temp = path.with_extension("json.tmp");
        let text = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
        let io = |e: std::io::Error| format!("{}: {e}", path.display());
        fs::write(&temp, text).map_err(io)?;
        fs::rename(&temp, &path).map_err(io)
    }
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
}
