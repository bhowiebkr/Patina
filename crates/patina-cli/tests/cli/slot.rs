//! Builds a node in the slot of the base cluster, as the editor does (spec 7.1).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use patina_engine::{Control, Mode, Project, Toolchain};

use crate::common::copy;

fn hello() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello")
}

/// A stateful node with each kind of input, for the slot (7.1).
const ACC: &str = "#[patina_sdk::node]
pub struct Acc {
    total: f64,
}

impl Acc {
    pub fn init() -> Self {
        Acc { total: 0.0 }
    }

    pub fn process(&mut self, value: &f64, scale: f64, note: &str, extra: &mut f64) -> f64 {
        assert!(note != \"boom\", \"the note is boom\");
        self.total += *value * scale;
        *extra += note.len() as f64;
        self.total
    }

    pub fn ui(&mut self, ui: &mut patina_sdk::Ui) {
        ui.slider(\"Total\", &mut self.total, 0.0..=1000.0);
    }
}
";

#[test]
fn a_node_in_the_slot_builds_alone_and_keeps_its_uses() {
    let project = copy(&hello(), "slot");
    let folder = project.join("nodes/acc");
    fs::create_dir_all(folder.join("src")).unwrap();
    let manifest = fs::read_to_string(project.join("nodes/add/Cargo.toml")).unwrap().replace("\"add\"", "\"acc\"");
    fs::write(folder.join("Cargo.toml"), manifest).unwrap();
    fs::write(folder.join("src/lib.rs"), ACC).unwrap();
    let graph = project.join("graphs/main.toml");
    let more = "
[[node]]
id = \"00000000000000a1\"
folder = \"nodes/acc\"

[node.params]
scale = \"2\"
note = \"abc\"
extra = \"1\"

[[node]]
id = \"00000000000000a2\"
folder = \"nodes/show\"

[[node]]
id = \"00000000000000a3\"
folder = \"nodes/show\"

[[wire]]
from = \"f89c6e329e8ea336.out\"
to = \"00000000000000a1.value\"

[[wire]]
from = \"00000000000000a1.out\"
to = \"00000000000000a2.value\"

[[wire]]
from = \"00000000000000a1.extra\"
to = \"00000000000000a3.value\"
";
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + more).unwrap();
    let toolchain = Toolchain::detect().unwrap();
    let control = Control::default();
    let value = |cluster: &mut patina_engine::Cluster, from: &str| {
        let out = cluster.run(&control);
        assert!(out.ok, "{:?}", out.reports);
        out.values.into_iter().find(|v| v.from == format!("00000000000000a1.{from}")).unwrap().text
    };
    let checked = || Project::open(&project).unwrap().check().unwrap();
    let slotted = || {
        let mut checked = checked();
        checked.set_slot(Some("acc")).unwrap();
        checked
    };

    // The base calls each input kind through the slot library, and the state stays in it.
    let opened = Project::open(&project).unwrap();
    let mut cluster = slotted().build(&toolchain, Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    assert_eq!(value(&mut cluster, "extra"), "4.0");
    assert_eq!(value(&mut cluster, "out"), "26.0");
    // The GUI of the node draws through the slot library too (4.3).
    let gui = |cluster: &mut patina_engine::Cluster| cluster.ui(&Control::default(), "00000000000000a1", 0, "").0.unwrap();
    assert!(gui(&mut cluster).ends_with("slider\tTotal\t26\t0\t1000"), "{}", gui(&mut cluster));

    // An edit of the body builds the slot alone. The node gets a new state and the new GUI.
    let edited = ACC.replace("*value * scale", "*value * scale * 10.0").replace("\"Total\"", "\"Sum\"");
    fs::write(folder.join("src/lib.rs"), &edited).unwrap();
    let start = Instant::now();
    assert!(slotted().build_slot(&toolchain, &mut cluster).unwrap());
    eprintln!("slot build {:.1} s", start.elapsed().as_secs_f64());
    assert_eq!(value(&mut cluster, "out"), "130.0");
    assert!(gui(&mut cluster).ends_with("slider\tSum\t130\t0\t1000"), "{}", gui(&mut cluster));

    // Fuse back in bake mode: the fused cluster gets the state of the slot library (7.2).
    let start = Instant::now();
    let mut fused = checked().build(&toolchain, Mode::Bake).unwrap();
    eprintln!("fuse {:.1} s", start.elapsed().as_secs_f64());
    fused.set_params(opened.graph());
    assert!(cluster.slotted() && !fused.slotted() && fused.mode() == Mode::Bake);
    assert!(!fused.sources_changed());
    assert_eq!(fused.adopt(&mut cluster), 1);
    assert_eq!(value(&mut fused, "out"), "260.0");
    assert!(gui(&mut fused).ends_with("slider\tSum\t260\t0\t1000"), "{}", gui(&mut fused));
    // An input that loses its text gets its default value, and the state of the node stays (4.8).
    let mut lost = opened.graph().clone();
    lost.node.iter_mut().find(|n| n.id == "00000000000000a1").unwrap().params.remove("scale");
    assert!(fused.set_params(&lost).ok);
    assert_eq!(value(&mut fused, "out"), "260.0");
    // A panic in `process` drops the state. The GUI call survives, and the next run makes a new state.
    lost.node.iter_mut().find(|n| n.id == "00000000000000a1").unwrap().params.insert("note".into(), "boom".into());
    assert!(fused.set_params(&lost).ok);
    let out = fused.run(&control);
    assert!(!out.ok && out.reports.iter().any(|r| r.text.contains("the note is boom")), "{:?}", out.reports);
    let _ = fused.ui(&Control::default(), "00000000000000a1", 0, "");
    assert!(fused.set_params(opened.graph()).ok);
    assert_eq!(value(&mut fused, "out"), "130.0");
    assert!(!slotted().build_slot(&toolchain, &mut fused).unwrap(), "a fused cluster has no slot");

    // A new field of the state changes the outline of the node crate, so the slot alone does not build.
    let mut cluster = slotted().build(&toolchain, Mode::Edit).unwrap();
    fs::write(folder.join("src/lib.rs"), edited.replace("total: f64,", "total: f64,
    count: u32,").replace("total: 0.0 }", "total: 0.0, count: 0 }")).unwrap();
    assert!(!slotted().build_slot(&toolchain, &mut cluster).unwrap());
    // A change of the signature changes the base too.
    fs::write(folder.join("src/lib.rs"), edited.replace("scale: f64", "scale: f32").replace("* scale", "* f64::from(scale)")).unwrap();
    assert!(!slotted().build_slot(&toolchain, &mut cluster).unwrap());
    // The fused cluster sees the change of the node file, so the editor builds it in edit mode (8.3).
    assert!(fused.sources_changed());
    assert!(checked().set_slot(Some("number")).is_ok());
    assert!(checked().set_slot(Some("nothing")).is_err());
}

const KEEP: &str = "#[patina_sdk::node]
pub fn keep<T: Clone>(value: &T) -> T {
    value.clone()
}
";

const TEXT: &str = "#[patina_sdk::node]
pub fn text(value: &str) -> String {
    value.to_uppercase()
}
";

const ZERO: &str = "#[patina_sdk::node]
pub fn zero<T: Default>() -> T {
    T::default()
}
";

/// A generic node goes into the slot: each use gets a copy, and its witness gives the types (4.5).
#[test]
fn a_generic_node_in_the_slot_gets_a_copy_for_each_use() {
    let project = copy(&hello(), "generic-slot");
    let manifest = fs::read_to_string(project.join("nodes/add/Cargo.toml")).unwrap();
    for (name, code) in [("keep", KEEP), ("text", TEXT), ("zero", ZERO)] {
        let folder = project.join("nodes").join(name);
        fs::create_dir_all(folder.join("src")).unwrap();
        fs::write(folder.join("Cargo.toml"), manifest.replace("\"add\"", &format!("\"{name}\""))).unwrap();
        fs::write(folder.join("src/lib.rs"), code).unwrap();
    }
    // Keep b1 gets an f64 from add, keep b3 and b6 get a String, and show b5 gives the type of zero b4.
    // Output b6 has no reader, so it has no value.
    let node = |id: &str, folder: &str| format!("\n[[node]]\nid = \"00000000000000{id}\"\nfolder = \"nodes/{folder}\"\n");
    let wire = |from: &str, to: &str| format!("\n[[wire]]\nfrom = \"00000000000000{from}\"\nto = \"00000000000000{to}\"\n");
    let more = [
        node("b1", "keep"),
        node("b2", "text") + "[node.params]\nvalue = \"abc\"\n",
        node("b3", "keep"),
        node("b4", "zero"),
        node("b5", "show"),
        node("b6", "keep"),
        node("b7", "show"),
        "\n[[wire]]\nfrom = \"f89c6e329e8ea336.out\"\nto = \"00000000000000b1.value\"\n".to_owned(),
        wire("b1.out", "b7.value"),
        wire("b2.out", "b3.value"),
        wire("b3.out", "b6.value"),
        wire("b4.out", "b5.value"),
    ]
    .concat();
    let graph = project.join("graphs/main.toml");
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + &more).unwrap();
    let toolchain = Toolchain::detect().unwrap();
    let control = Control::default();
    let opened = Project::open(&project).unwrap();
    let values = |cluster: &mut patina_engine::Cluster| {
        let out = cluster.run(&control);
        assert!(out.ok, "{:?}", out.reports);
        move |id: &str| {
            let value = out.values.iter().find(|v| v.from == format!("00000000000000{id}.out"));
            value.unwrap_or_else(|| panic!("no value of {id} in {:?}", out.values)).text.clone()
        }
    };
    let slotted = |name: &str| {
        let mut checked = Project::open(&project).unwrap().check().unwrap();
        checked.set_slot(Some(name)).unwrap();
        checked
    };

    let mut cluster = slotted("keep").build(&toolchain, Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    let value = values(&mut cluster);
    assert_eq!(value("b1"), "6.5");
    assert_eq!(value("b3"), "\"ABC\"");

    // An edit of the body builds the slot alone, with the copy for each use.
    fs::write(project.join("nodes/keep/src/lib.rs"), KEEP.replace("value.clone()", "let copy = value.clone();\n    copy")).unwrap();
    assert!(slotted("keep").build_slot(&toolchain, &mut cluster).unwrap());
    let value = values(&mut cluster);
    assert_eq!(value("b1"), "6.5");
    assert_eq!(value("b3"), "\"ABC\"");

    // A generic node with no input gets its type from the reader of its output.
    let mut cluster = slotted("zero").build(&toolchain, Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    assert_eq!(values(&mut cluster)("b4"), "0.0");
}

/// An async node. It waits, then gives the length of the text plus 7.
const SHOUT: &str = "#[patina_sdk::node]
pub async fn shout(text: &String) -> f64 {
    patina_event::tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    text.len() as f64 + 7.0
}
";

/// An event node goes into the slot: the slot call waits for the future of an async node (9.4).
#[test]
fn an_event_node_in_the_slot_waits_for_its_future() {
    let project = copy(&hello(), "event-slot");
    let manifest = fs::read_to_string(project.join("nodes/add/Cargo.toml")).unwrap();
    let folder = project.join("nodes/shout");
    fs::create_dir_all(folder.join("src")).unwrap();
    fs::write(folder.join("Cargo.toml"), manifest.replace("\"add\"", "\"shout\"") + "patina-event = { workspace = true }\n").unwrap();
    fs::write(folder.join("src/lib.rs"), SHOUT).unwrap();
    fs::create_dir_all(project.join("nodes/zero/src")).unwrap();
    fs::write(project.join("nodes/zero/Cargo.toml"), manifest.replace("\"add\"", "\"zero\"")).unwrap();
    fs::write(project.join("nodes/zero/src/lib.rs"), ZERO).unwrap();
    // Async shout e4 reads zero e3, so it gives the type of zero (4.5).
    let graph = "format = 1\n\n\
        [[node]]\nid = \"00000000000000e0\"\nbuiltin = \"trigger\"\n\n\
        [[node]]\nid = \"00000000000000e1\"\nfolder = \"nodes/shout\"\n\n\
        [[node]]\nid = \"00000000000000e2\"\nfolder = \"nodes/show\"\n\n\
        [[node]]\nid = \"00000000000000e3\"\nfolder = \"nodes/zero\"\n\n\
        [[node]]\nid = \"00000000000000e4\"\nfolder = \"nodes/shout\"\n\n\
        [[node]]\nid = \"00000000000000e5\"\nfolder = \"nodes/show\"\n\n\
        [[wire]]\nfrom = \"00000000000000e0.out\"\nto = \"00000000000000e1.text\"\n\n\
        [[wire]]\nfrom = \"00000000000000e1.out\"\nto = \"00000000000000e2.value\"\n\n\
        [[wire]]\nfrom = \"00000000000000e3.out\"\nto = \"00000000000000e4.text\"\n\n\
        [[wire]]\nfrom = \"00000000000000e4.out\"\nto = \"00000000000000e5.value\"\n\n\
        [[order]]\nbefore = \"00000000000000e0\"\nafter = \"00000000000000e4\"\n";
    fs::write(project.join("graphs/main.toml"), graph).unwrap();
    let toolchain = Toolchain::detect().unwrap();
    let control = Control::default();
    let opened = Project::open(&project).unwrap();
    let slotted = |name: &str| {
        let mut checked = Project::open(&project).unwrap().check().unwrap();
        checked.set_slot(Some(name)).unwrap();
        checked
    };
    // The batch run gives the value of zero e3, as in the editor.
    let value = |cluster: &mut patina_engine::Cluster| {
        assert!(cluster.run(&control).ok);
        let mut text = None;
        assert!(cluster.run_events(1, &control, |run| {
            assert!(run.out.ok, "{:?}", run.out.reports);
            let text_of = |id: &str| run.out.values.iter().find(|v| v.from == format!("00000000000000{id}.out")).map(|v| v.text.clone());
            assert_eq!(text_of("e4"), text_of("e1"), "zero gives an empty text");
            text = text_of("e1");
        }));
        text.expect("no value of e1")
    };

    let mut cluster = slotted("shout").build(&toolchain, Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    assert_eq!(value(&mut cluster), "7.0");

    // An edit of the body builds the slot alone.
    fs::write(folder.join("src/lib.rs"), SHOUT.replace("+ 7.0", "+ 8.0")).unwrap();
    assert!(slotted("shout").build_slot(&toolchain, &mut cluster).unwrap());
    assert_eq!(value(&mut cluster), "8.0");

    // A node that is not async also goes into the slot in the event domain.
    let mut cluster = slotted("show").build(&toolchain, Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    assert_eq!(value(&mut cluster), "8.0");
}
