//! Atomic native document writes and bounded backups. Recovery writes use the same atomic primitive.
use crate::{Error, Result};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

pub fn atomic_write(path: &Path, bytes: &[u8], backup: bool) -> Result<()> {
    let parent = parent_directory(path);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Invalid(e.to_string()))?
        .as_nanos();
    let temp = parent.join(format!(".forge-{}-{stamp}.tmp", std::process::id()));
    let result = (|| {
        {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        if backup && path.is_file() {
            backup_previous(path, stamp)?;
        }
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
pub fn backup_directory(path: &Path) -> PathBuf {
    let name: String = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
        .collect();
    parent_directory(path)
        .join(".backups")
        .join(if name.is_empty() { "material" } else { &name })
}
fn backup_previous(path: &Path, stamp: u128) -> Result<()> {
    let directory = backup_directory(path);
    std::fs::create_dir_all(&directory)?;
    let canonical = directory.canonicalize()?;
    if !canonical.starts_with(parent_directory(path).canonicalize()?) {
        return Err(Error::Invalid(
            "Backup directory resolves outside the document folder".into(),
        ));
    }
    let backup = directory.join(format!("backup-{stamp:020}.png"));
    std::fs::copy(path, backup)?;
    let mut versions: Vec<_> = std::fs::read_dir(&directory)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with("backup-")
                && e.path().extension().is_some_and(|e| e == "png")
        })
        .map(|e| e.path())
        .collect();
    versions.sort();
    let remove = versions.len().saturating_sub(20);
    for version in versions.into_iter().take(remove) {
        std::fs::remove_file(version)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacing_document_keeps_previous_version_and_bounds_backups() -> Result<()> {
        let folder = std::env::temp_dir().join(format!("forge-backups-{}", std::process::id()));
        std::fs::create_dir_all(&folder)?;
        let file = folder.join("Rusted steel.png");
        atomic_write(&file, b"original", false)?;
        atomic_write(&file, b"next", true)?;
        assert_eq!(std::fs::read(&file)?, b"next");
        let first = std::fs::read_dir(backup_directory(&file))?
            .next()
            .ok_or_else(|| Error::Invalid("No backup".into()))??;
        assert_eq!(std::fs::read(first.path())?, b"original");
        for i in 0..24 {
            atomic_write(&file, &[i], true)?;
        }
        assert_eq!(std::fs::read_dir(backup_directory(&file))?.count(), 20);
        // Test owns this exact temporary directory; individual files are removed without recursive shell commands.
        for entry in std::fs::read_dir(backup_directory(&file))? {
            std::fs::remove_file(entry?.path())?;
        }
        std::fs::remove_dir(backup_directory(&file))?;
        std::fs::remove_dir(folder.join(".backups"))?;
        std::fs::remove_file(file)?;
        std::fs::remove_dir(folder)?;
        Ok(())
    }
}
