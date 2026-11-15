//! The harness of the milestone 0 prototype "hot reload" (spec 8.5 and 19).
//!
//! It uses the user project and the loader of the prototype "Rust types across clusters". It
//! adds what a reload needs:
//!
//! - A `build/` folder with content-hash file names. A changed build gets a new name. A build
//!   with no change gets the old name, so the loaded library stays in use (spec 8.5 and 8.6).
//! - A set of loaded clusters. The harness loads each file one time and never unloads it.
//! - The memory of the process, the size of a folder, and the library that holds an address.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use types_across_clusters::abi::KeyHasher;
use types_across_clusters::{BuildOutput, Cluster, Project, pe_info};

fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

/// The libraries of one build in `build/`.
pub struct Install {
    pub types: PathBuf,
    pub cluster_a: PathBuf,
    pub cluster_b: PathBuf,
    /// The files that this install added to `build/`.
    pub new_files: Vec<PathBuf>,
}

/// Copies the libraries of a build into `build/`.
///
/// - Each cluster gets the name `<target>-<hash>.dll`, with a hash of the content of the file.
/// - The types library keeps its name, because its name is a hash of its sources.
/// - The `std` library of the toolchain keeps its name.
///
/// The harness never writes to a file name that exists, so it never replaces a loaded file.
pub fn install(project: &Project, output: &BuildOutput, types_lib: &str) -> io::Result<Install> {
    let mut new_files = Vec::new();
    let types_src = output.dll(types_lib)?;
    let types_name = types_src.file_name().ok_or_else(|| error("types library has no name"))?;
    let types = project.build_dir.join(types_name);
    copy_new(types_src, &types, &mut new_files)?;
    let cluster_a = install_cluster(project, output, "cluster_a", &mut new_files)?;
    let cluster_b = install_cluster(project, output, "cluster_b", &mut new_files)?;
    for path in [&types, &cluster_a, &cluster_b] {
        for lib in pe_info(path)?.imports.keys() {
            if lib.starts_with("std-") {
                let to = project.build_dir.join(lib);
                if !to.exists() {
                    copy_new(&project.find_std(lib)?, &to, &mut new_files)?;
                }
            }
        }
    }
    Ok(Install {
        types,
        cluster_a,
        cluster_b,
        new_files,
    })
}

fn install_cluster(
    project: &Project,
    output: &BuildOutput,
    target: &str,
    new_files: &mut Vec<PathBuf>,
) -> io::Result<PathBuf> {
    let data = fs::read(output.dll(target)?)?;
    let hash = KeyHasher::new().part(&data).key();
    let path = project.build_dir.join(format!("{target}-{:016x}.dll", hash.hi));
    if !path.exists() {
        write_new(&path, &data)?;
        new_files.push(path.clone());
    }
    Ok(path)
}

/// Copies `from` to `to` if `to` does not exist.
fn copy_new(from: &Path, to: &Path, new_files: &mut Vec<PathBuf>) -> io::Result<()> {
    if !to.exists() {
        write_new(to, &fs::read(from)?)?;
        new_files.push(to.to_owned());
    }
    Ok(())
}

/// Writes a new file. It writes a temporary file first and then renames it, so a stop during
/// the write never leaves a part of a library under the final name.
fn write_new(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    fs::write(&part, data)?;
    fs::rename(&part, path)
}

/// The clusters that the harness loaded. It loads each file one time and never unloads it
/// (spec 8.5). The other code refers to a cluster by its index.
#[derive(Default)]
pub struct Loaded {
    clusters: Vec<Cluster>,
    by_path: HashMap<PathBuf, usize>,
}

impl Loaded {
    /// Returns the index of the cluster in `path`. Loads the file if it is not loaded yet.
    pub fn load(&mut self, path: &Path) -> io::Result<usize> {
        if let Some(&index) = self.by_path.get(path) {
            return Ok(index);
        }
        self.clusters.push(Cluster::load(path)?);
        let index = self.clusters.len() - 1;
        self.by_path.insert(path.to_owned(), index);
        Ok(index)
    }

    pub fn get(&self, index: usize) -> &Cluster {
        &self.clusters[index]
    }

    /// The number of loaded clusters.
    pub fn count(&self) -> usize {
        self.clusters.len()
    }
}

/// Returns the total size of the files in a folder and in its subfolders.
pub fn dir_size(dir: &Path) -> io::Result<u64> {
    let mut total = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        total += if meta.is_dir() { dir_size(&entry.path())? } else { meta.len() };
    }
    Ok(total)
}

/// Returns the sorted names and sizes of the files in a folder.
pub fn listing(dir: &Path) -> io::Result<Vec<(String, u64)>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        files.push((entry.file_name().to_string_lossy().into_owned(), entry.metadata()?.len()));
    }
    files.sort();
    Ok(files)
}

/// The memory of this process.
#[derive(Debug, Clone, Copy, Default)]
pub struct Memory {
    /// The private bytes: the memory that only this process uses.
    pub private_bytes: u64,
    /// The working set: the memory of this process that is in RAM now.
    pub working_set: u64,
}

pub use sys::{memory, module_of};

#[cfg(windows)]
mod sys {
    use std::ffi::OsString;
    use std::io;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;

    use windows_sys::Win32::Foundation::HMODULE;
    use windows_sys::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        GetModuleFileNameW, GetModuleHandleExW,
    };
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    use super::Memory;

    /// Reads the memory counters of this process.
    pub fn memory() -> io::Result<Memory> {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        // SAFETY: `counters` is a writable struct of `cb` bytes. The EX struct starts with the
        // fields of `PROCESS_MEMORY_COUNTERS`, and the function reads `cb` to know the size.
        let ok = unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                (&raw mut counters).cast::<PROCESS_MEMORY_COUNTERS>(),
                counters.cb,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Memory {
            private_bytes: counters.PrivateUsage as u64,
            working_set: counters.WorkingSetSize as u64,
        })
    }

    /// Returns the path of the loaded library that holds a code address.
    pub fn module_of(addr: usize) -> io::Result<PathBuf> {
        let mut module: HMODULE = std::ptr::null_mut();
        // SAFETY: with `FROM_ADDRESS`, the second argument is an address, not a name. With
        // `UNCHANGED_REFCOUNT`, the call does not keep the library loaded.
        let ok = unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                addr as *const u16,
                &mut module,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buf = [0u16; 1024];
        // SAFETY: `module` is a loaded library, and `buf` has room for `buf.len()` characters.
        let len = unsafe { GetModuleFileNameW(module, buf.as_mut_ptr(), buf.len() as u32) };
        if len == 0 || len as usize >= buf.len() {
            return Err(io::Error::last_os_error());
        }
        Ok(PathBuf::from(OsString::from_wide(&buf[..len as usize])))
    }
}

#[cfg(not(windows))]
mod sys {
    use std::io;
    use std::path::PathBuf;

    use super::Memory;

    /// The prototype reads the memory only on Windows.
    pub fn memory() -> io::Result<Memory> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    /// The prototype finds the library of an address only on Windows.
    pub fn module_of(_addr: usize) -> io::Result<PathBuf> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}
