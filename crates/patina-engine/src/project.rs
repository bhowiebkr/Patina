//! The project files of spec 6.4. A file with a key that Patina does not know does not open.

use std::collections::BTreeMap;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::Error;

/// The newest file format that this engine reads (6.4).
pub const FORMAT: u32 = 1;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    pub format: u32,
    pub id: String,
    pub rust: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub imports: BTreeMap<String, Import>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Import {
    pub project: String,
    pub folder: String,
    pub hash: String,
    /// The hash of the copy in this project after the import or the last update (6.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct GraphFile {
    pub format: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node: Vec<NodeEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wire: Vec<Wire>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<Order>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collar: Vec<Collar>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cable: Vec<Cable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input: Vec<PublicInput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output: Vec<PublicOutput>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct NodeEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subgraph: Option<String>,
    /// A node that Patina gives, for example `delay` (9.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builtin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<[f64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub from: String,
    pub to: String,
    /// The bus name that replaces the default name (15.9). Only the editor reads it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corners: Option<Corners>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<[f64; 2]>,
}

/// The path of a wire (15.9). A wire with no style uses the graph default.
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    Curved,
    Orthogonal,
    Polyline,
}

/// The corners of an orthogonal or polyline path (15.9). The default is rounded.
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Corners {
    Rounded,
    Sharp,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Order {
    pub before: String,
    pub after: String,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Collar {
    pub pos: [f64; 2],
    pub wires: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Cable {
    pub name: String,
    pub wires: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct PublicInput {
    pub key: String,
    pub name: String,
    pub pos: [f64; 2],
    pub to: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct PublicOutput {
    pub key: String,
    pub name: String,
    pub pos: [f64; 2],
    /// Empty for an output that is not connected yet.
    #[serde(default, skip_serializing_if = "String::is_empty")]
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

/// Writes a project file with its fields in a fixed order (6.4). The text goes to a temporary
/// file first, so a crash during the write keeps the old file.
pub fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), Error> {
    let text = toml::to_string(value).map_err(|e| Error::File(path.to_owned(), e.to_string()))?;
    let temp = path.with_extension("toml.tmp");
    let io = |e| Error::Io(path.to_owned(), e);
    std::fs::write(&temp, text).map_err(io)?;
    std::fs::rename(&temp, path).map_err(io)
}

/// True for the key of a public pin: 8 lowercase hexadecimal characters (6.4).
pub fn is_key(text: &str) -> bool {
    text.len() == 8 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
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

    #[test]
    fn a_graph_file_writes_back_to_the_same_text() {
        // A Windows checkout can have CRLF line ends.
        let text = include_str!("../../../examples/hello/graphs/main.toml").replace("\r\n", "\n");
        let graph: GraphFile = toml::from_str(&text).unwrap();
        assert_eq!(toml::to_string(&graph).unwrap(), text);
    }
}
