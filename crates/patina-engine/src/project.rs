//! The project files of spec 6.4. A file with a key that Patina does not know does not open.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::Error;

/// The newest file format that this engine reads (6.4).
pub const FORMAT: u32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    pub format: u32,
    pub id: String,
    pub rust: String,
    #[serde(default)]
    pub imports: BTreeMap<String, Import>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Import {
    pub project: String,
    pub folder: String,
    pub hash: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphFile {
    pub format: u32,
    #[serde(default)]
    pub node: Vec<NodeEntry>,
    #[serde(default)]
    pub wire: Vec<Wire>,
    #[serde(default)]
    pub order: Vec<Order>,
    #[serde(default)]
    pub collar: Vec<Collar>,
    #[serde(default)]
    pub cable: Vec<Cable>,
    #[serde(default)]
    pub input: Vec<PublicInput>,
    #[serde(default)]
    pub output: Vec<PublicOutput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeEntry {
    pub id: String,
    pub folder: Option<String>,
    pub subgraph: Option<String>,
    pub pos: Option<[f64; 2]>,
    pub domain: Option<String>,
    pub settings: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub from: String,
    pub to: String,
    pub style: Option<String>,
    #[serde(default)]
    pub points: Vec<[f64; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Order {
    pub before: String,
    pub after: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collar {
    pub pos: [f64; 2],
    pub wires: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cable {
    pub name: String,
    pub wires: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicInput {
    pub key: String,
    pub name: String,
    pub pos: [f64; 2],
    pub to: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicOutput {
    pub key: String,
    pub name: String,
    pub pos: [f64; 2],
    pub from: String,
}

/// Reads a TOML file of the project and checks its `format`.
pub fn read<T: DeserializeOwned>(path: &Path, format: impl Fn(&T) -> u32) -> Result<T, Error> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::Io(path.to_owned(), e))?;
    let value: T = toml::from_str(&text).map_err(|e| Error::File(path.to_owned(), e.to_string()))?;
    match format(&value) {
        FORMAT => Ok(value),
        found => Err(Error::File(
            path.to_owned(),
            format!("format {found} is not known. This Patina reads format {FORMAT}."),
        )),
    }
}

/// True for a stable random ID: 16 lowercase hexadecimal characters (6.4).
pub fn is_id(text: &str) -> bool {
    text.len() == 16 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_keys_and_newer_formats_do_not_open() {
        assert!(toml::from_str::<GraphFile>("format = 1\nzoom = 2").is_err());
        let dir = std::env::temp_dir().join(format!("patina-format-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("main.toml");
        std::fs::write(&path, "format = 2\n").unwrap();
        let result = read::<GraphFile>(&path, |g| g.format);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(result, Err(Error::File(_, text)) if text.contains("format 2")));
        assert!(is_id("5c0e9b2a7f13d846"));
        assert!(!is_id("5C0E9B2A7F13D846") && !is_id("5c0e"));
    }
}
