use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {}", .path.display(), .source)]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{}: {}", .path.display(), .source)]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("{}: {}", .path.display(), .message)]
    Format { path: PathBuf, message: String },
}

impl Error {
    pub fn format(path: &Path, message: impl Into<String>) -> Self {
        Self::Format {
            path: path.to_owned(),
            message: message.into(),
        }
    }
}

pub fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

pub fn write_bytes(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| Error::Io {
            path: dir.to_owned(),
            source,
        })?;
    }
    std::fs::write(path, data).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = read_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(|source| Error::Json {
        path: path.to_owned(),
        source,
    })
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(|source| Error::Json {
        path: path.to_owned(),
        source,
    })?;
    text.push('\n');
    write_bytes(path, text.as_bytes())
}
