//! The `#[param]` attribute on an input of a node (spec 4.8).

use std::fs;
use std::path::Path;

use patina_engine::{Cluster, Control, Mode, Project, ReportKind, Toolchain};

use crate::common::copy;
use crate::nodes::SCALE;

#[test]
fn a_param_attribute_gives_the_default_and_refuses_a_value_out_of_its_range() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workbenches/hello"), "param");
    let folder = project.join("nodes/scale");
    fs::create_dir_all(folder.join("src")).unwrap();
    let manifest = fs::read_to_string(project.join("nodes/add/Cargo.toml")).unwrap();
    fs::write(folder.join("Cargo.toml"), manifest.replace("\"add\"", "\"scale\"")).unwrap();
    fs::write(folder.join("src/lib.rs"), SCALE).unwrap();
    // The number 2.5 goes to `x`. The input `k` has no wire and no text.
    let graph = project.join("graphs/main.toml");
    // A run gives the value of a wired output only, so `show` takes the result.
    let more = "
[[node]]
id = \"00000000000000c1\"
folder = \"nodes/scale\"

[[node]]
id = \"00000000000000c2\"
folder = \"nodes/show\"

[[wire]]
from = \"9bc1790966687030.out\"
to = \"00000000000000c1.x\"

[[wire]]
from = \"00000000000000c1.out\"
to = \"00000000000000c2.value\"
";
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + more).unwrap();

    let opened = Project::open(&project).unwrap();
    let nodes = opened.nodes().unwrap();
    let k = &nodes.iter().find(|n| n.name == "scale").unwrap().inputs[1];
    assert_eq!((k.param.default.as_deref(), k.param.min, k.param.max), (Some("2.5"), Some(0.0), Some(10.0)));
    let mut cluster = opened.check().unwrap().build(&Toolchain::detect().unwrap(), Mode::Edit).unwrap();
    let control = Control::default();
    let scaled = |cluster: &mut Cluster| {
        let out = cluster.run(&control);
        assert!(out.ok, "{:?}", out.reports);
        out.values.into_iter().find(|v| v.from == "00000000000000c1.out").map(|v| v.text)
    };
    let mut graph = opened.graph().clone();
    assert!(cluster.set_params(&graph).ok);
    assert_eq!(scaled(&mut cluster).as_deref(), Some("6.25"), "the default of #[param]");
    let mut set = |cluster: &mut Cluster, text: Option<&str>| {
        let params = &mut graph.node.iter_mut().find(|n| n.id == "00000000000000c1").unwrap().params;
        match text {
            Some(text) => params.insert("k".to_owned(), text.to_owned()),
            None => params.remove("k"),
        };
        cluster.set_params(&graph)
    };
    assert!(set(&mut cluster, Some("4")).ok);
    assert_eq!(scaled(&mut cluster).as_deref(), Some("10.0"));
    let out = set(&mut cluster, Some("20"));
    assert!(!out.ok && out.reports.iter().any(|r| r.kind == ReportKind::BadParam && r.text.contains("out of the range 0 to 10")), "{:?}", out.reports);
    assert!(set(&mut cluster, None).ok);
    assert_eq!(scaled(&mut cluster).as_deref(), Some("6.25"), "a lost text gives the default again");
}
