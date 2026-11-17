//! The data folder of Patina (spec 13.3): the lists of recent and trusted projects (11, 15.10), the
//! layout of each project (15.2) and the recovery files (15.10).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind::NotFound;
use std::path::{Component, Path, PathBuf};

use patina_engine::project::GraphFile;
use serde::{Deserialize, Serialize};

/// The number of projects in the recent list.
const RECENT_LIMIT: usize = 50;
const RECENT: &str = "recent.json";
const TRUSTED: &str = "trusted.json";
const LAYOUTS: &str = "layouts";
/// A layout is a few kilobytes. A larger text is an error of the page.
const LAYOUT_LIMIT: usize = 64 * 1024;
const RECOVERY: &str = "recovery";
/// The unsaved code is a few files of text. A larger text is an error of the page.
const RECOVERY_LIMIT: usize = 16 * 1024 * 1024;
const RECOVERY_FORMAT: u32 = 1;

/// The unsaved changes of a project in a recovery file (15.10).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recovery {
    format: u32,
    root: PathBuf,
    /// The open graph when it has changes that are not saved.
    pub graph: Option<GraphFile>,
    /// The name of a subgraph in `graph`. The main graph has no name (6.1).
    #[serde(default)]
    pub graph_name: Option<String>,
    /// The other graphs with changes that are not saved, by name. The main graph is `main`.
    #[serde(default)]
    pub others: BTreeMap<String, GraphFile>,
    /// The unsaved text of the Code tab, by path.
    pub code: BTreeMap<String, String>,
}

/// A recovery file to write. It borrows the state of the session.
#[derive(Serialize)]
struct RecoveryRef<'a> {
    format: u32,
    root: &'a Path,
    graph: Option<&'a GraphFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    graph_name: Option<&'a str>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    others: BTreeMap<&'a str, &'a GraphFile>,
    code: &'a BTreeMap<String, String>,
}

pub struct Data {
    /// The lists of projects, the layouts and the recovery files. It is the folder of the user, also
    /// for the portable edition, so each build and edition remembers the projects (13.3).
    pub dir: PathBuf,
    /// The WebView2 data of this edition.
    pub webview: PathBuf,
    /// The default parent folder of new projects.
    pub projects: PathBuf,
    /// The toolchain folder (13.3).
    pub toolchain: PathBuf,
    /// True when the data stays in the folder of a portable edition or of `--portable` (13.3).
    pub portable: bool,
}

impl Data {
    /// Finds the data folder: the `--portable` folder, then the folder of a portable edition,
    /// then the folder of the user. A `--portable` folder keeps all data, so a check does not change
    /// the lists of the user.
    pub fn find(portable: Option<PathBuf>) -> Result<Data, String> {
        let exe_dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_owned));
        let base = || dirs::data_local_dir().ok_or("the folder for local data is not known");
        let user = || Ok::<_, &str>(base()?.join(if cfg!(target_os = "linux") { "patina" } else { "Patina" }));
        let (root, projects, portable, lists) = match (portable, exe_dir) {
            (Some(dir), _) => (dir.clone(), dir.join("projects"), true, None),
            (None, Some(exe)) if exe.join("portable").is_file() => (exe.clone(), exe.join("projects"), true, Some(user()?)),
            (None, _) => {
                let documents = dirs::document_dir().map_or_else(base, Ok)?;
                (user()?, documents.join("Patina"), false, None)
            }
        };
        let absolute = |path: PathBuf| std::path::absolute(&path).map_err(|e| format!("{}: {e}", path.display()));
        let root = absolute(root)?;
        let (local, toolchain) = (root.join("data"), root.join("toolchain"));
        let dir = match lists {
            Some(user) => absolute(user)?.join("data"),
            None => local.clone(),
        };
        for folder in [&dir, &local] {
            fs::create_dir_all(folder).map_err(|e| format!("Patina cannot write to {}: {e}", folder.display()))?;
        }
        let data = Data { dir, webview: local.join("webview"), projects, toolchain, portable };
        if data.dir != local {
            data.take_old(&local);
        }
        Ok(data)
    }

    /// Moves the lists, the layouts and the recovery files of an older portable edition from its
    /// folder `old` to the folder of the user. A file that does not move stays, so nothing is lost.
    fn take_old(&self, old: &Path) {
        for name in [RECENT, TRUSTED] {
            let from = old.join(name);
            let Ok(bytes) = fs::read(&from) else { continue };
            let mut list = self.read_list(name);
            for path in serde_json::from_slice::<Vec<PathBuf>>(&bytes).unwrap_or_default() {
                if !list.iter().any(|p| same_path(p, &path)) {
                    list.push(path);
                }
            }
            if name == RECENT {
                list.truncate(RECENT_LIMIT);
            }
            match self.write_list(name, &list) {
                Ok(()) => drop(fs::remove_file(&from)),
                Err(e) => eprintln!("patina: {e}"),
            }
        }
        for folder in [LAYOUTS, RECOVERY] {
            let Ok(entries) = fs::read_dir(old.join(folder)) else { continue };
            for from in entries.flatten().map(|e| e.path()) {
                let to = self.dir.join(folder).join(from.file_name().unwrap_or_default());
                // A copy, because a rename fails from one drive to another.
                let moved = to.exists() || fs::create_dir_all(self.dir.join(folder)).and_then(|()| fs::copy(&from, &to)).is_ok();
                if moved {
                    let _ = fs::remove_file(&from);
                }
            }
        }
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

    /// The trusted project folders, in the order of trust (11).
    pub fn trusted(&self) -> Vec<PathBuf> {
        self.read_list(TRUSTED)
    }

    pub fn is_trusted(&self, root: &Path) -> bool {
        self.trusted().iter().any(|path| same_path(path, root))
    }

    pub fn trust(&self, root: &Path) -> Result<(), String> {
        let mut list = self.read_list(TRUSTED);
        if !list.iter().any(|path| same_path(path, root)) {
            list.push(root.to_owned());
        }
        self.write_list(TRUSTED, &list)
    }

    /// Takes a folder off the list of trusted projects.
    pub fn untrust(&self, root: &Path) -> Result<(), String> {
        let mut list = self.trusted();
        list.retain(|path| !same_path(path, root));
        self.write_list(TRUSTED, &list)
    }

    /// The saved layout of a project, or none.
    pub fn layout(&self, id: &str) -> Result<Option<String>, String> {
        let path = self.layout_path(id)?;
        match fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The recovery file of the project in `root`, or none.
    pub fn recovery(&self, root: &Path) -> Result<Option<Recovery>, String> {
        let path = self.recovery_path(root);
        let bad = |e: String| format!("{}: {e}", path.display());
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == NotFound => return Ok(None),
            Err(e) => return Err(bad(e.to_string())),
        };
        let file: Recovery = serde_json::from_slice(&bytes).map_err(|e| bad(e.to_string()))?;
        if file.format != RECOVERY_FORMAT {
            return Err(bad(format!("format {} is not known", file.format)));
        }
        // A file of another folder with the same hash is not for this project.
        Ok(same_path(&file.root, root).then_some(file))
    }

    /// Writes the unsaved changes of the project in `root`, or removes its recovery file when
    /// there are none. `graphs` are the graphs with unsaved changes, and `open` is the open graph.
    pub fn save_recovery(&self, root: &Path, open: &str, graphs: &[(&str, &GraphFile)], code: &BTreeMap<String, String>) -> Result<(), String> {
        if graphs.is_empty() && code.is_empty() {
            return self.remove_recovery(root);
        }
        let (mut graph, mut others) = (None, BTreeMap::new());
        for &(name, file) in graphs {
            match name == open {
                true => graph = Some(file),
                false => _ = others.insert(name, file),
            }
        }
        let graph_name = graph.and(Some(open)).filter(|name| *name != "main");
        let file = RecoveryRef { format: RECOVERY_FORMAT, root, graph, graph_name, others, code };
        let text = serde_json::to_vec(&file).map_err(|e| e.to_string())?;
        if text.len() > RECOVERY_LIMIT {
            return Err(format!("the unsaved changes have more than {RECOVERY_LIMIT} bytes"));
        }
        let dir = self.dir.join(RECOVERY);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        write_file(&self.recovery_path(root), &text)
    }

    pub fn remove_recovery(&self, root: &Path) -> Result<(), String> {
        let path = self.recovery_path(root);
        match fs::remove_file(&path) {
            Err(e) if e.kind() != NotFound => Err(format!("{}: {e}", path.display())),
            _ => Ok(()),
        }
    }

    /// The name is a hash of the folder, so two copies of a project have two files. FNV-1a gives
    /// the same name in each version of Patina.
    fn recovery_path(&self, root: &Path) -> PathBuf {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for part in path_key(root) {
            for byte in part.as_encoded_bytes().iter().chain(b"/") {
                hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        self.dir.join(RECOVERY).join(format!("{hash:016x}.json"))
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
fn path_key(path: &Path) -> Vec<OsString> {
    let part = |c: Component| if cfg!(windows) { c.as_os_str().to_ascii_lowercase() } else { c.as_os_str().to_owned() };
    path.components().map(part).collect()
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    path_key(a) == path_key(b)
}

/// `path` relative to `root` with `/`, or "." for `root` itself. A path outside `root` stays the same.
pub fn relative_path(root: &Path, path: &Path) -> String {
    let key = path_key(root);
    if !path_key(path).starts_with(&key) {
        return path.display().to_string();
    }
    let rest: Vec<_> = path.components().skip(key.len()).map(|c| c.as_os_str().to_string_lossy()).collect();
    if rest.is_empty() { ".".to_owned() } else { rest.join("/") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lists_keep_the_newest_project_first_and_trust_once() {
        let dir = std::env::temp_dir().join(format!("patina-data-{}", std::process::id()));
        let data = Data::find(Some(dir.clone())).unwrap();
        let path = |n: usize| dir.join(format!("p{n}"));
        for n in 0..RECENT_LIMIT + 2 {
            data.add_recent(&path(n)).unwrap();
        }
        data.add_recent(&path(5)).unwrap();
        let recent = data.recent();
        assert_eq!(recent.len(), RECENT_LIMIT);
        assert_eq!((&recent[0], &recent[1]), (&path(5), &path(RECENT_LIMIT + 1)));
        assert!(!data.is_trusted(&path(1)));
        data.trust(&path(1)).unwrap();
        data.trust(&path(1)).unwrap();
        data.trust(&path(2)).unwrap();
        let trusted = data.trusted();
        data.untrust(&path(1).join(".")).unwrap();
        let after = data.trusted();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(trusted, [path(1), path(2)]);
        assert_eq!(after, [path(2)]);
        assert!(data.portable);
        assert!(same_path(Path::new("/a/b/"), Path::new("/a/b")));
        assert_eq!(same_path(Path::new("/A/b"), Path::new("/a/b")), cfg!(windows));
    }

    #[test]
    fn an_old_portable_folder_gives_its_lists_and_files_to_the_user_folder() {
        let dir = std::env::temp_dir().join(format!("patina-move-{}", std::process::id()));
        let (data, old) = (Data::find(Some(dir.join("user"))).unwrap(), dir.join("old"));
        let path = |n: usize| dir.join(format!("p{n}"));
        data.add_recent(&path(1)).unwrap();
        fs::create_dir_all(old.join(LAYOUTS)).unwrap();
        fs::write(old.join(RECENT), serde_json::to_vec(&[path(2), path(1)]).unwrap()).unwrap();
        fs::write(old.join(TRUSTED), b"bad").unwrap();
        fs::write(old.join(LAYOUTS).join("a.json"), b"{}").unwrap();
        data.take_old(&old);
        let (recent, trusted, layout) = (data.recent(), data.trusted(), fs::read(data.dir.join(LAYOUTS).join("a.json")));
        let left = [old.join(RECENT), old.join(TRUSTED), old.join(LAYOUTS).join("a.json")].map(|p| p.exists());
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(recent, [path(1), path(2)]);
        assert!(trusted.is_empty());
        assert_eq!(layout.unwrap(), b"{}");
        assert_eq!(left, [false; 3]);
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

    #[test]
    fn a_recovery_file_belongs_to_one_folder_and_goes_with_the_last_change() {
        let dir = std::env::temp_dir().join(format!("patina-recovery-{}", std::process::id()));
        let data = Data::find(Some(dir.clone())).unwrap();
        let (root, copy) = (dir.join("p"), dir.join("copy"));
        let graph: GraphFile = toml::from_str("format = 1\n").unwrap();
        let code = BTreeMap::from([("nodes/a/src/lib.rs".to_owned(), "fn a() {}".to_owned())]);
        let none = data.recovery(&root).map(|r| r.is_none());
        // The open graph and each other graph with unsaved changes go into the file (15.10).
        data.save_recovery(&root, "half", &[("main", &graph), ("half", &graph)], &code).unwrap();
        let found = data.recovery(&root).unwrap().map(|r| (r.graph, r.graph_name, r.others.into_keys().collect::<Vec<_>>(), r.code));
        let other = data.recovery(&copy).map(|r| r.is_none());
        let moved = fs::rename(data.recovery_path(&root), data.recovery_path(&copy));
        let wrong_root = data.recovery(&copy).map(|r| r.is_none());
        let large = data.save_recovery(&root, "main", &[], &BTreeMap::from([(String::new(), "a".repeat(RECOVERY_LIMIT))]));
        data.save_recovery(&root, "main", &[], &code).unwrap();
        data.save_recovery(&root, "main", &[], &BTreeMap::new()).unwrap();
        let removed = data.recovery(&root).map(|r| r.is_none());
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!((none, other, wrong_root, removed), (Ok(true), Ok(true), Ok(true), Ok(true)));
        assert_eq!(found, Some((Some(graph), Some("half".to_owned()), vec!["main".to_owned()], code)));
        assert!(moved.is_ok() && large.is_err());
        assert_eq!(data.recovery_path(Path::new("/A/b")) == data.recovery_path(Path::new("/a/b/")), cfg!(windows));
    }

    #[test]
    fn a_path_in_the_project_becomes_relative() {
        let root = Path::new("/p/proj");
        assert_eq!(relative_path(root, Path::new("/p/proj/out/a b.png")), "out/a b.png");
        assert_eq!(relative_path(root, Path::new("/p/proj/")), ".");
        assert_eq!(relative_path(root, Path::new("/p/projx/a.png")), Path::new("/p/projx/a.png").display().to_string());
        assert_eq!(relative_path(root, Path::new("/P/Proj/a")) == "a", cfg!(windows));
    }
}
