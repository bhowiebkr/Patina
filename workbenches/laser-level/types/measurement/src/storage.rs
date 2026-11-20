use crate::{acquisition::identity, core::survey::Session};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

pub fn load(path: &Path) -> Result<Session, String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 32 * 1024 * 1024 {
        return Err("Session exceeds 32 MB".into());
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let session = Session::from_json(&text).map_err(|e| e.to_string())?;
    if session.runs.len() > 256 || session.runs.iter().any(|r| r.stations.len() > 4096 || r.visits.len() > 65536) {
        return Err("Session has too many runs, stations, or visits".into());
    }
    Ok(session)
}

pub fn save(path: &Path, text: &str) -> Result<(), String> {
    if text.len() > 32 * 1024 * 1024 {
        return Err("Session exceeds 32 MB. The sample stays pending.".into());
    }
    let parent = path.parent().ok_or("Choose a session file")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(".laser-{:x}.tmp", identity()));
    let result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|e| e.to_string())
}

pub fn export(path: &Path, text: &str) -> Result<(), String> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| e.to_string())?;
    if let Err(e) = file.write_all(text.as_bytes()).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(e.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replace_session_but_never_replace_export() {
        let path = std::env::temp_dir().join(format!("laser-{:x}.json", identity()));
        let s = Session::new(crate::core::scale::Scale::from_pixel_pitch(3.0));
        save(&path, &s.to_json()).unwrap();
        save(&path, &s.to_json()).unwrap();
        assert_eq!(load(&path).unwrap(), s);
        assert!(export(&path, "changed").is_err());
        assert!(save(&path, &" ".repeat(32 * 1024 * 1024 + 1)).is_err());
        assert_eq!(load(&path).unwrap(), s);
        fs::remove_file(path).unwrap();
    }
}
