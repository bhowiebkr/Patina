//! The milestone 0 prototype "Rust types across clusters" (spec 19).
//!
//! Each test builds the prototype project two times with one profile, loads the clusters and
//! passes values between them. Run with `--nocapture` to see the report.

#![cfg(windows)]

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use types_across_clusters::abi::{
    InputArg, ProbeStats, SEEN_DISTANCE, SEEN_GREETING, SEEN_POINTS, SEEN_PROBE, SinkSummary,
};
use types_across_clusters::{
    BuildOutput, Cluster, ImportKind, Installed, Mismatch, Project, accept, connect, pe_info,
};

/// The tests run one after the other, so the build times do not include the other test.
static SERIAL: Mutex<()> = Mutex::new(());

const PINS: [&str; 4] = ["greeting", "points", "probe", "distance"];

#[test]
fn dev_profile() {
    scenario("dev");
}

#[test]
fn release_profile() {
    scenario("release");
}

fn stats(live: u64, dropped: u64) -> ProbeStats {
    ProbeStats { live, dropped }
}

#[allow(clippy::too_many_lines)]
fn scenario(profile: &str) {
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("types-across-clusters").join(profile);
    let mut project = Project::from_template(&work, profile).expect("copy the project");
    println!("\n==== profile {profile} ====");
    println!("toolchain: rustc {} {}", project.toolchain.release, project.toolchain.host);

    // Build 1.
    let types1 = project.generate_shared().expect("generate build 1");
    let out1 = project.build().expect("build 1");
    let inst1 = project.install(&out1, &types1).expect("install build 1");
    report_build(&project, "build 1", &out1, &inst1);
    check_imports(&project, &inst1, &types1);

    let a1 = Cluster::load(&inst1.cluster_a).expect("load A1");
    let b1 = Cluster::load(&inst1.cluster_b).expect("load B1");
    assert_eq!(a1.pins().len(), 4);
    assert_eq!(b1.pins().len(), 4);
    report_pins("A1", &a1);
    report_pins("B1", &b1);

    // Connect by pin name. `distance` has the same Rust type name on both sides, but it comes
    // from two versions of the `units` crate.
    for name in &PINS[..3] {
        assert_eq!(connect(a1.pin(name), b1.pin(name)), Ok(()), "A1 to B1 {name}");
    }
    assert_eq!(a1.pin("distance").type_name(), b1.pin("distance").type_name());
    assert!(matches!(
        connect(a1.pin("distance"), b1.pin("distance")),
        Err(Mismatch::Key { .. })
    ));

    let [greeting, points, probe, distance] = take4(a1.source_run());
    assert_eq!(a1.probe_stats(), stats(1, 0));
    let args = [
        greeting.read_arg(),
        points.into_arg(),
        probe.into_arg(),
        InputArg::NONE,
    ];
    // SAFETY: the keys match, and the engine gave away `points` and `probe`.
    let summary = unsafe { b1.sink_run(&args) };
    println!("A1 to B1: {summary:?}");
    assert_eq!(summary.seen, SEEN_GREETING | SEEN_POINTS | SEEN_PROBE);
    check_full_summary(&summary);
    // One copy of `geometry`: B dropped the probe that A made, and both see the same counts.
    assert_eq!(summary.live_after_probe, 0);
    assert_eq!(a1.probe_stats(), stats(0, 1));
    assert_eq!(b1.probe_stats(), stats(0, 1));
    // The engine drops the values that it holds with the drop functions of A1.
    drop(greeting);
    drop(distance);
    assert_eq!(a1.point_type_id(), b1.point_type_id());

    // Build 2: the fields of `Point` change from `u32` to `f32`. The crate name, the crate
    // version, the type name, the size and the alignment stay the same.
    let geometry = "types/geometry/src/lib.rs";
    project.replace_in_file(geometry, "pub x: u32,", "pub x: f32,").unwrap();
    project.replace_in_file(geometry, "pub y: u32,", "pub y: f32,").unwrap();
    let types2 = project.generate_shared().expect("generate build 2");
    assert_ne!(types1, types2, "a change to a type gives a new library name");
    let out2 = project.build().expect("build 2");
    let inst2 = project.install(&out2, &types2).expect("install build 2");
    report_build(&project, "build 2", &out2, &inst2);
    check_imports(&project, &inst2, &types2);

    let a2 = Cluster::load(&inst2.cluster_a).expect("load A2");
    let b2 = Cluster::load(&inst2.cluster_b).expect("load B2");
    report_pins("B2", &b2);

    // The old outputs of A1 against the new inputs of B2.
    assert_eq!(connect(a1.pin("greeting"), b2.pin("greeting")), Ok(()));
    for name in ["points", "probe"] {
        let (old, new) = (a1.pin(name), b2.pin(name));
        assert_eq!(old.type_name(), new.type_name());
        assert_eq!((old.size, old.align), (new.size, new.align));
        assert!(matches!(connect(old, new), Err(Mismatch::Key { .. })), "A1 to B2 {name}");
    }
    assert!(connect(a1.pin("distance"), b2.pin("distance")).is_err());

    // The engine refuses the old values.
    let old = take4(a1.source_run());
    assert_eq!(a1.probe_stats(), stats(1, 1));
    assert_eq!(accept(b2.pin("greeting"), old[0].key()), Ok(()));
    for (i, name) in PINS.iter().enumerate().skip(1) {
        assert!(accept(b2.pin(name), old[i].key()).is_err(), "old {name} value refused");
    }
    // A `String` from build 1 still goes to build 2, because both builds use one `std`.
    let args = [old[0].read_arg(), InputArg::NONE, InputArg::NONE, InputArg::NONE];
    // SAFETY: the key of the greeting matches, and the other inputs have no connection.
    let summary = unsafe { b2.sink_run(&args) };
    assert_eq!(summary.seen, SEEN_GREETING);
    assert_eq!(summary.greeting_ok, 1);
    // The engine drops the old values with the code of A1. The counts of build 2 stay the same.
    drop(old);
    assert_eq!(a1.probe_stats(), stats(0, 2));
    assert_eq!(b2.probe_stats(), stats(0, 0));
    let type_id_same = a1.point_type_id() == b2.point_type_id();
    println!(
        "TypeId hash of Point: A1 {:016x}, B2 {:016x}, equal: {type_id_same}",
        a1.point_type_id(),
        b2.point_type_id()
    );

    // The new build works from A2 to B2.
    for name in &PINS[..3] {
        assert_eq!(connect(a2.pin(name), b2.pin(name)), Ok(()), "A2 to B2 {name}");
    }
    let [greeting, points, probe, distance] = take4(a2.source_run());
    assert_eq!(accept(b2.pin("points"), points.key()), Ok(()));
    let args = [
        greeting.read_arg(),
        points.into_arg(),
        probe.into_arg(),
        InputArg::NONE,
    ];
    // SAFETY: the keys match, and the engine gave away `points` and `probe`.
    let summary = unsafe { b2.sink_run(&args) };
    println!("A2 to B2: {summary:?}");
    assert_eq!(summary.seen & SEEN_DISTANCE, 0);
    check_full_summary(&summary);
    assert_eq!(a2.probe_stats(), stats(0, 1));
    assert_eq!(b2.probe_stats(), stats(0, 1));
    drop(greeting);
    drop(distance);
    println!("==== profile {profile}: all checks passed ====");
}

fn take4<T>(values: Vec<T>) -> [T; 4] {
    values.try_into().unwrap_or_else(|v: Vec<T>| panic!("{} values, not 4", v.len()))
}

fn check_full_summary(summary: &SinkSummary) {
    assert_eq!(summary.greeting_ok, 1);
    assert_eq!(summary.greeting_len, 20);
    assert_eq!(summary.points_len, 4);
    assert_eq!(summary.sum_x, 10.0);
    assert_eq!(summary.sum_y, 100.0);
    assert_eq!(summary.probe_label_len, 17);
}

fn report_build(project: &Project, label: &str, output: &BuildOutput, installed: &Installed) {
    println!("\n-- {label}: cargo build took {:.1} s", output.seconds);
    println!("files that cargo made:");
    for artifact in &output.artifacts {
        let names: Vec<_> = artifact
            .files
            .iter()
            .filter_map(|f| f.file_name()?.to_str())
            .collect();
        println!(
            "  {} [{}] {:?}: {}",
            artifact.target,
            artifact.package,
            artifact.kinds,
            names.join(", ")
        );
    }
    println!("libraries in build/ and what each one imports:");
    let mut entries: Vec<_> = std::fs::read_dir(&project.build_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let info = pe_info(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy();
        println!("  {name}: {} exports", info.export_count);
        for (lib, symbols) in &info.imports {
            let alloc: Vec<_> = symbols.iter().filter(|s| s.contains("rust_alloc")).collect();
            println!(
                "    {lib} ({:?}): {} symbols{}",
                project.classify(lib),
                symbols.len(),
                if alloc.is_empty() { String::new() } else { format!(", allocator {alloc:?}") }
            );
        }
    }
    let _ = installed;
}

fn report_pins(label: &str, cluster: &Cluster) {
    for pin in cluster.pins() {
        println!(
            "  {label} {:<9} {:<34} size {} align {} key {}",
            pin.name(),
            pin.type_name(),
            pin.size,
            pin.align,
            pin.key
        );
    }
}

fn check_imports(project: &Project, installed: &Installed, types_lib: &str) {
    let types_dll = format!("{types_lib}.dll");
    for cluster in [&installed.cluster_a, &installed.cluster_b] {
        let libs: Vec<String> = pe_info(cluster)
            .unwrap()
            .imports
            .keys()
            .map(|l| l.to_ascii_lowercase())
            .collect();
        assert!(libs.contains(&types_dll), "{cluster:?} imports {types_dll}: {libs:?}");
        assert!(libs.iter().any(|l| l.starts_with("std-")), "{cluster:?} imports std");
        assert!(!libs.iter().any(|l| l.starts_with("cluster_")), "clusters are independent");
    }
    let types_libs = pe_info(&installed.types).unwrap();
    assert!(types_libs.imports.keys().any(|l| l.starts_with("std-")), "types imports std");
    for entry in std::fs::read_dir(&project.build_dir).unwrap() {
        let path = entry.unwrap().path();
        for lib in pe_info(&path).unwrap().imports.keys() {
            assert_ne!(project.classify(lib), ImportKind::Missing, "{path:?} needs {lib}");
        }
    }
}
