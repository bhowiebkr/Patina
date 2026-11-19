//! The project files of spec 6.4. A file with a key that Patina does not know does not open.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::node::{self, Node};

/// The newest file format that this engine reads (6.4).
pub const FORMAT: u32 = 1;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    pub format: u32,
    pub id: String,
    pub rust: String,
    /// The frames each second of the frame domain (9.1). None is 60.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_rate: Option<u32>,
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
    /// The hash of each file that the import or the last update copied from the other project (6.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct GraphFile {
    pub format: u32,
    /// True for a top graph that runs, as `main` is (6.1). A subgraph use can not name it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub top: bool,
    /// The path style of the wires with no style of their own (15.9). `None` is the automatic route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    /// The symbol of each use of this subgraph (4.3). `None` shows the box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<Symbol>,
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
    /// The colored frames on the canvas (15.12). The build does not read them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group: Vec<Group>,
    /// The Markdown texts on the canvas (15.12). The build does not read them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Note>,
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
    /// The wire name (15.9): the name of a bus, or the label of one line. Only the editor reads it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corners: Option<Corners>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<[f64; 2]>,
}

/// The path of a wire (15.9). A wire with no style uses the graph default, and a graph with no
/// style uses the automatic route.
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// The automatic route around the nodes. A wire sets it to differ from the graph default.
    Automatic,
    Curved,
    Orthogonal,
    Polyline,
}

impl Style {
    /// The style that keeps the path of a wire when it moves from a graph with the default `from`
    /// to a graph with the default `to`, for example at a paste or an expand.
    pub fn moved(style: Option<Style>, from: Option<Style>, to: Option<Style>) -> Option<Style> {
        if style.is_some() || from == to { style } else { Some(from.unwrap_or(Style::Automatic)) }
    }
}

/// A circuit symbol in place of the box of a node (4.3).
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Symbol {
    pub shape: Shape,
    /// The text on a chip or a block. `None` shows the name in upper case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The fill of the body as "#rrggbb". `None` uses the color of the theme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The corners of the body of the custom shape, in symbol pixels from the top left.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outline: Vec<[f64; 2]>,
}

/// The shape of a symbol (4.3): the logic gates, the blocks, the chip and a custom outline.
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Not,
    Buffer,
    Mux,
    Alu,
    Adder,
    Chip,
    Bus,
    Custom,
}

impl Shape {
    pub fn parse(text: &str) -> Option<Shape> {
        use serde::de::IntoDeserializer;
        Shape::deserialize(IntoDeserializer::<serde::de::value::Error>::into_deserializer(text)).ok()
    }
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

/// A colored frame with a title around nodes on the canvas (15.12).
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub title: String,
    pub color: GroupColor,
    pub pos: [f64; 2],
    pub size: [f64; 2],
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum GroupColor {
    Gray,
    Amber,
    Green,
    Teal,
    Violet,
    Pink,
    Copper,
}

/// A Markdown text on the canvas (15.12). Its height fits the text.
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub id: String,
    pub pos: [f64; 2],
    pub width: f64,
    pub text: String,
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

/// Crate names that a node folder cannot have, after `-` changes to `_`.
/// Crate names of Rust and Patina, the keywords of Rust, the folder names of Cargo and the device
/// names of Windows. Cargo or Windows refuses a crate or a folder with one of these names.
const RESERVED: &str = "std core alloc proc_macro test patina_sdk patina_macros patina_log patina_event cluster_batch cluster_frame cluster_audio cluster_event \
    as async await break const continue crate dyn else enum extern false fn for gen if impl in let \
    loop match mod move mut pub ref return self static struct super trait true type unsafe use \
    where while abstract become box do final macro override priv try typeof unsized virtual yield \
    build deps examples incremental con prn aux nul com1 com2 com3 com4 com5 com6 com7 com8 com9 \
    lpt1 lpt2 lpt3 lpt4 lpt5 lpt6 lpt7 lpt8 lpt9";

/// Checks the name of a node folder. A name with a path separator or `..` does not pass.
pub fn check_name(name: &str) -> Result<(), String> {
    let valid = name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if name.is_empty() || name.len() > 64 || !valid || name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err("a folder name has 1 to 64 letters, digits, _ and -, and starts with a letter".into());
    }
    let key = name_key(name);
    if RESERVED.split_whitespace().any(|r| r == key) {
        return Err(format!("the name {name:?} is reserved. Use another name"));
    }
    Ok(())
}

/// The crate name as Rust and Windows compare it: with no regard to case, and with `-` as `_`.
pub fn name_key(name: &str) -> String {
    name.to_ascii_lowercase().replace('-', "_")
}

/// The last parse of each node folder, with the code that it came from. A parse of Rust code is
/// slow, and each edit in the editor reads all the node folders again.
static PARSED: Mutex<BTreeMap<PathBuf, Parsed>> = Mutex::new(BTreeMap::new());

/// The code of a node folder and the result of its parse.
type Parsed = (String, Result<Node, String>);

/// Reads the pins of the node folder `nodes/<name>` (4.1).
pub fn read_node(root: &Path, name: &str) -> Result<Node, String> {
    let code = read_crate(root, "nodes", name)?;
    let dir = root.join("nodes").join(name);
    if let Some((old, node)) = PARSED.lock().unwrap_or_else(PoisonError::into_inner).get(&dir)
        && *old == code
    {
        return node.clone();
    }
    let node = node::parse(&code).map_err(|e| format!("src/lib.rs: {e}"));
    keep(&mut PARSED.lock().unwrap_or_else(PoisonError::into_inner), dir, (code, node.clone()), KEPT);
    node
}

/// The most node folders that the parse cache keeps. This is far more than a project has.
const KEPT: usize = 4096;

/// Puts a parse into the cache. A full cache first drops all entries, so removed folders and the
/// folders of other projects do not stay.
fn keep(parsed: &mut BTreeMap<PathBuf, Parsed>, dir: PathBuf, entry: Parsed, most: usize) {
    if parsed.len() >= most && !parsed.contains_key(&dir) {
        parsed.clear();
    }
    parsed.insert(dir, entry);
}

/// Reads `src/lib.rs` of the crate folder `<group>/<name>`, after the checks of its name.
pub(crate) fn read_crate(root: &Path, group: &str, name: &str) -> Result<String, String> {
    check_name(name)?;
    let dir = root.join(group).join(name);
    let read = |file: &str| {
        std::fs::read_to_string(dir.join(file)).map_err(|e| format!("cannot read {file}: {e}"))
    };
    let manifest: toml::Table = read("Cargo.toml")?.parse().map_err(|e| format!("Cargo.toml: {e}"))?;
    let package = manifest.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str());
    if package != Some(name) {
        return Err(format!("the package name in Cargo.toml must be {name:?}, the folder name"));
    }
    read("src/lib.rs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_node_folder_reads_again() {
        let root = std::env::temp_dir().join(format!("patina-read-node-{}", std::process::id()));
        let dir = root.join("nodes").join("n");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]
name = \"n\"
").unwrap();
        let inputs = |code: &str| {
            std::fs::write(dir.join("src/lib.rs"), format!("#[patina_sdk::node]
{code}")).unwrap();
            read_node(&root, "n").map(|n| n.inputs.len())
        };
        assert_eq!(inputs("pub fn n(a: f32) -> f32 { a }"), Ok(1));
        assert_eq!(inputs("pub fn n(a: f32) -> f32 { a }"), Ok(1));
        assert_eq!(inputs("pub fn n(a: f32, b: f32) -> f32 { a + b }"), Ok(2));
        assert!(inputs("pub fn n(").is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_full_parse_cache_drops_its_entries() {
        let mut parsed = BTreeMap::new();
        let entry = || (String::new(), Err(String::new()));
        for dir in ["a", "b", "b", "c"] {
            keep(&mut parsed, dir.into(), entry(), 2);
        }
        assert_eq!(parsed.keys().collect::<Vec<_>>(), [Path::new("c")]);
    }

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

    #[test]
    fn a_moved_wire_keeps_its_path() {
        let (auto, curved) = (Some(Style::Automatic), Some(Style::Curved));
        assert_eq!(Style::moved(None, None, None), None);
        assert_eq!(Style::moved(None, curved, curved), None);
        assert_eq!(Style::moved(None, curved, None), curved);
        assert_eq!(Style::moved(None, None, curved), auto);
        assert_eq!(Style::moved(Some(Style::Polyline), None, curved), Some(Style::Polyline));
        let graph: GraphFile = toml::from_str("format = 1\nstyle = \"orthogonal\"\n").unwrap();
        assert_eq!(graph.style, Some(Style::Orthogonal));
    }

    #[test]
    fn symbol_reads_and_writes() {
        let text = "format = 1\n\n[symbol]\nshape = \"chip\"\nlabel = \"CPU\"\n";
        let graph: GraphFile = toml::from_str(text).unwrap();
        assert_eq!(graph.symbol, Some(Symbol { shape: Shape::Chip, label: Some("CPU".to_owned()), color: None, outline: Vec::new() }));
        assert_eq!(toml::to_string(&graph).unwrap(), text);
        let text = "format = 1

[symbol]
shape = \"custom\"
color = \"#335577\"
outline = [[0.0, 0.0], [40.0, 10.0], [0.0, 20.0]]
";
        let graph: GraphFile = toml::from_str(text).unwrap();
        assert_eq!(graph.symbol.as_ref().map(|s| (s.shape, s.outline.len())), Some((Shape::Custom, 3)));
        assert_eq!(toml::to_string(&graph).unwrap(), text);
        assert!(toml::from_str::<GraphFile>("format = 1\n[symbol]\nshape = \"star\"\n").is_err());
        assert!(toml::from_str::<GraphFile>("format = 1\n[symbol]\nshape = \"or\"\ncolor = 1\n").is_err());
        assert_eq!((Shape::parse("xnor"), Shape::parse("Chip")), (Some(Shape::Xnor), None));
    }
}
