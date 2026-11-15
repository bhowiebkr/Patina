//! The milestone 0 prototype "hot reload" (spec 8.5 and 19).
//!
//! The test reloads cluster A after each change to a node, and reloads both clusters and the
//! types library after each change to a type crate. Values from old builds stay alive across
//! the reloads and drop with the code of their own build. The test measures the build times
//! and the memory and disk use that each reload adds. Run it with `--nocapture` for the report.
//!
//! The default run is short: the `dev` profile with 2 node reloads and 1 types reload. Set
//! `PATINA_FULL=1` for the measured run: more reloads, the `release` profile and the LTO check.
//! `PATINA_NODE_RELOADS` and `PATINA_TYPE_RELOADS` set the number of reloads in each profile.

#![cfg(windows)]

use std::fs;
use std::path::Path;

use hot_reload::{Install, Loaded, Memory, dir_size, install, listing, memory, module_of};
use types_across_clusters::abi::{
    InputArg, ProbeStats, SEEN_GREETING, SEEN_POINTS, SEEN_PROBE, SinkSummary,
};
use types_across_clusters::{Cluster, Project, Value, accept, connect, pe_info};

const SOURCE: &str = "nodes/source/src/lib.rs";
const GEOMETRY: &str = "types/geometry/src/lib.rs";
const POINTS_CALL: &str = "Point::from_ints(i, i * 10)";
const GEOMETRY_DOC: &str = "//! A local type crate (spec 5.1).";
const EXCLUDE_LINE: &str = "exclude = [\"types/units-1\", \"types/units-2\"]";
const PINS: [&str; 3] = ["greeting", "points", "probe"];
const KB: f64 = 1024.0;

/// One test runs the profiles one after the other, so the memory numbers of one profile do not
/// include a build of the other profile at the same time.
#[test]
fn hot_reload() {
    let env = |var: &str| std::env::var(var).ok().and_then(|v| v.parse::<u32>().ok());
    let nodes = env("PATINA_NODE_RELOADS");
    let types = env("PATINA_TYPE_RELOADS");
    if std::env::var_os("PATINA_FULL").is_none() {
        scenario("dev", nodes.unwrap_or(2), types.unwrap_or(1));
        return;
    }
    scenario("dev", nodes.unwrap_or(10), types.unwrap_or(4));
    scenario("release", nodes.unwrap_or(4), types.unwrap_or(2));
    lto_check();
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Cold,
    NoChange,
    Node,
    FixAfterFailure,
    Types,
}

struct Row {
    step: Step,
    seconds: f64,
    new_files: usize,
    new_bytes: u64,
    build_dir: u64,
    target_dir: u64,
    memory: Memory,
    loaded: usize,
}

/// The libraries of the build that runs now.
struct Current {
    types: std::path::PathBuf,
    a: usize,
    b: usize,
}

fn scenario(profile: &str, node_reloads: u32, type_reloads: u32) {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hot-reload").join(profile);
    let mut project = Project::from_template(&work, profile).expect("copy the project");
    // The MSVC linker writes a time stamp and a random PDB ID into each library. `/Brepro`
    // makes them a hash of the content, so the same source gives the same file.
    let msvc = project.settings.target.ends_with("-msvc");
    if msvc {
        project.settings.rustflags.push_str(" -C link-arg=/Brepro");
    }
    let source_text = read(&project, SOURCE);
    let geometry_text = read(&project, GEOMETRY);
    assert!(source_text.contains(POINTS_CALL) && geometry_text.contains(GEOMETRY_DOC));
    println!("\n==== profile {profile}: {node_reloads} node reloads, {type_reloads} types reloads ====");
    println!("toolchain: rustc {} {}", project.toolchain.release, project.toolchain.host);
    let mut loaded = Loaded::default();
    let mut rows = Vec::new();
    let start = memory().expect("memory");

    // Build 0 is a cold build.
    let mut types_lib = project.generate_shared().expect("generate build 0");
    let inst = build_install(&project, &types_lib, &mut rows, Step::Cold);
    let mut cur = Current {
        a: loaded.load(&inst.cluster_a).expect("load A0"),
        b: loaded.load(&inst.cluster_b).expect("load B0"),
        types: inst.types,
    };
    let first_a = cur.a;
    check(&run_pair(&loaded, cur.a, cur.b), 0, 0);
    sample(&mut rows, &loaded);

    // The keeper is a value of build 0. It stays alive until the end of the test.
    let keeper = take4(loaded.get(cur.a).source_run()).into_iter().nth(2).expect("probe");
    let keeper_module = drop_module(&keeper, &[&loaded.get(cur.a).path, &cur.types]);
    println!("the drop function of a Probe of build 0 is in {keeper_module}");

    // A build with no change gives the same files, so nothing reloads.
    let same = build_install(&project, &types_lib, &mut rows, Step::NoChange);
    sample(&mut rows, &loaded);
    assert!(same.new_files.is_empty(), "no new files: {:?}", same.new_files);
    assert_eq!(same.cluster_a, loaded.get(cur.a).path);

    // Node reloads: each one changes the `points` function of the source node.
    let mut held = loaded.get(cur.a).source_run();
    let mut node_text = source_text.clone();
    for k in 1..=node_reloads {
        node_text = source_text.replace(POINTS_CALL, &format!("Point::from_ints(i, i * 10 + {k})"));
        write(&project, SOURCE, &node_text);
        let inst = build_install(&project, &types_lib, &mut rows, Step::Node);
        assert_eq!(inst.types, cur.types, "the types library did not change");
        assert_eq!(inst.cluster_b, loaded.get(cur.b).path, "cluster B did not change");
        assert_ne!(inst.cluster_a, loaded.get(cur.a).path, "cluster A has a new file");
        assert_eq!(inst.new_files, std::slice::from_ref(&inst.cluster_a), "only A is new");
        let new_a = loaded.load(&inst.cluster_a).expect("load the new A");
        sample(&mut rows, &loaded);

        // The new build runs. One copy of `geometry` holds the keeper and the old probe.
        let summary = run_pair(&loaded, new_a, cur.b);
        check(&summary, k, 2);
        // The values of the old build go to cluster B after the reload.
        drop_module(&held[2], &[&loaded.get(cur.a).path, &cur.types]);
        check(&give(loaded.get(cur.b), held), k - 1, 1);
        held = loaded.get(new_a).source_run();
        cur.a = new_a;
    }

    // A failed build does not change `build/`, and the loaded build still runs.
    let before = listing(&project.build_dir).expect("listing");
    write(&project, SOURCE, &format!("{node_text}\ncompile_error!(\"a failed build\");\n"));
    let failure = project.build().err().expect("the build fails");
    assert!(failure.to_string().contains("a failed build"));
    assert_eq!(listing(&project.build_dir).expect("listing"), before, "build/ did not change");
    check(&run_pair(&loaded, cur.a, cur.b), node_reloads, 2);
    write(&project, SOURCE, &node_text);
    let fix = build_install(&project, &types_lib, &mut rows, Step::FixAfterFailure);
    let identical = fix.cluster_a == loaded.get(cur.a).path;
    println!("after the failed build, the same source gave the same cluster A file: {identical}");
    if msvc {
        assert!(identical && fix.new_files.is_empty(), "the same source gives the same files");
    }
    cur.a = loaded.load(&fix.cluster_a).expect("load A");
    sample(&mut rows, &loaded);
    // The held values must come from the cluster A that runs now.
    held = loaded.get(cur.a).source_run();

    // Types reloads: each one changes a comment in `geometry`, so its build ID changes (5.1).
    for j in 1..=type_reloads {
        let text = geometry_text.replace(GEOMETRY_DOC, &format!("{GEOMETRY_DOC} Revision {j}."));
        write(&project, GEOMETRY, &text);
        let new_types = project.generate_shared().expect("generate");
        assert_ne!(new_types, types_lib, "the types library has a new name");
        types_lib = new_types;
        let inst = build_install(&project, &types_lib, &mut rows, Step::Types);
        assert_eq!(inst.new_files.len(), 3, "new files: {:?}", inst.new_files);
        let next = Current {
            a: loaded.load(&inst.cluster_a).expect("load A"),
            b: loaded.load(&inst.cluster_b).expect("load B"),
            types: inst.types,
        };
        sample(&mut rows, &loaded);
        check(&run_pair(&loaded, next.a, next.b), node_reloads, 0);

        // The new cluster B refuses the old `points` and `probe`, and takes the old `String`.
        let (old_a, new_b) = (loaded.get(cur.a), loaded.get(next.b));
        assert_eq!(connect(old_a.pin("greeting"), new_b.pin("greeting")), Ok(()));
        for (i, name) in PINS.iter().enumerate().skip(1) {
            assert!(connect(old_a.pin(name), new_b.pin(name)).is_err(), "old {name} pin");
            assert!(accept(new_b.pin(name), held[i].key()).is_err(), "old {name} value");
        }
        let args = [held[0].read_arg(), InputArg::NONE, InputArg::NONE, InputArg::NONE];
        // SAFETY: the key of the greeting matches, and the other inputs have no connection.
        let summary = unsafe { new_b.sink_run(&args) };
        assert_eq!((summary.seen, summary.greeting_ok), (SEEN_GREETING, 1));

        // The old values drop with the code of the old build.
        let (old_stats, new_stats) = (old_a.probe_stats(), loaded.get(next.a).probe_stats());
        drop_module(&held[2], &[&old_a.path, &cur.types]);
        drop(held);
        assert_eq!(old_a.probe_stats(), stats(old_stats.live - 1, old_stats.dropped + 1));
        assert_eq!(loaded.get(next.a).probe_stats(), new_stats);
        held = loaded.get(next.a).source_run();
        cur = next;
    }

    // The keeper of build 0 drops with the code of build 0.
    drop(held);
    let first_stats = loaded.get(first_a).probe_stats();
    let last_stats = loaded.get(cur.a).probe_stats();
    drop(keeper);
    assert_eq!(loaded.get(first_a).probe_stats(), stats(0, first_stats.dropped + 1));
    if type_reloads > 0 {
        assert_eq!(loaded.get(cur.a).probe_stats(), last_stats);
    }
    lock_check(&loaded.get(first_a).path);
    report(profile, &rows, start, &project);
    println!("==== profile {profile}: all checks passed ====");
}

fn stats(live: u64, dropped: u64) -> ProbeStats {
    ProbeStats { live, dropped }
}

fn read(project: &Project, relative: &str) -> String {
    fs::read_to_string(project.root.join(relative)).expect("read a project file")
}

fn write(project: &Project, relative: &str, text: &str) {
    fs::write(project.root.join(relative), text).expect("write a project file");
}

fn take4<T>(values: Vec<T>) -> [T; 4] {
    values.try_into().unwrap_or_else(|v: Vec<T>| panic!("{} values, not 4", v.len()))
}

/// Builds, installs and adds a row with the measurements.
fn build_install(
    project: &Project,
    types_lib: &str,
    rows: &mut Vec<Row>,
    step: Step,
) -> Install {
    let output = project.build().expect("cargo build");
    let inst = install(project, &output, types_lib).expect("install");
    let new_bytes = inst.new_files.iter().map(|f| fs::metadata(f).expect("new file").len()).sum();
    rows.push(Row {
        step,
        seconds: output.seconds,
        new_files: inst.new_files.len(),
        new_bytes,
        build_dir: dir_size(&project.build_dir).expect("build/ size"),
        target_dir: dir_size(&project.target_dir).expect("target size"),
        memory: Memory::default(),
        loaded: 0,
    });
    inst
}

/// Adds the memory and the number of loaded clusters to the last row, after the loads.
fn sample(rows: &mut [Row], loaded: &Loaded) {
    let row = rows.last_mut().expect("a row");
    row.memory = memory().expect("memory");
    row.loaded = loaded.count();
}

/// Runs cluster A and gives its values to cluster B.
fn run_pair(loaded: &Loaded, a: usize, b: usize) -> SinkSummary {
    let (a, b) = (loaded.get(a), loaded.get(b));
    for name in PINS {
        assert_eq!(connect(a.pin(name), b.pin(name)), Ok(()), "{name}");
    }
    give(b, a.source_run())
}

/// Gives the four values of a source run to cluster B. The engine drops the values it keeps.
fn give(b: &Cluster, values: Vec<Value>) -> SinkSummary {
    let [greeting, points, probe, distance] = take4(values);
    for (name, value) in PINS.iter().zip([&greeting, &points, &probe]) {
        assert_eq!(accept(b.pin(name), value.key()), Ok(()), "{name}");
    }
    let args = [greeting.read_arg(), points.into_arg(), probe.into_arg(), InputArg::NONE];
    // SAFETY: the keys match, and the engine gave away `points` and `probe`.
    let summary = unsafe { b.sink_run(&args) };
    drop(greeting);
    drop(distance);
    summary
}

/// Checks a run of the source with change `k`, and the number of other live probes.
fn check(summary: &SinkSummary, k: u32, live_after: u64) {
    assert_eq!(summary.seen, SEEN_GREETING | SEEN_POINTS | SEEN_PROBE);
    assert_eq!((summary.greeting_ok, summary.points_len), (1, 4));
    assert_eq!(summary.sum_x, 10.0);
    assert_eq!(summary.sum_y, f64::from(100 + 4 * k), "the points of change {k}");
    assert_eq!(summary.live_after_probe, live_after);
}

/// Returns the file name of the library that holds the drop function of a value, and checks
/// that it is one of `allowed`.
fn drop_module(value: &Value, allowed: &[&Path]) -> String {
    let module = module_of(value.drop_fn_addr()).expect("the module of a drop function");
    let name = module.file_name().expect("name").to_string_lossy().to_ascii_lowercase();
    let names: Vec<String> = allowed
        .iter()
        .map(|p| p.file_name().expect("name").to_string_lossy().to_ascii_lowercase())
        .collect();
    assert!(names.contains(&name), "drop function in {name}, not in {names:?}");
    name
}

/// Tries to delete and to rename a loaded library, and reports the result.
fn lock_check(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => println!("delete of a loaded library: it worked"),
        Err(e) => println!("delete of a loaded library: refused, {e}"),
    }
    let mut renamed = path.as_os_str().to_owned();
    renamed.push(".renamed");
    match fs::rename(path, &renamed) {
        Ok(()) => {
            println!("rename of a loaded library: it worked");
            fs::rename(&renamed, path).expect("rename back");
        }
        Err(e) => println!("rename of a loaded library: refused, {e}"),
    }
}

fn report(profile: &str, rows: &[Row], start: Memory, project: &Project) {
    println!("\nstep              build s  new  new KB  build/ KB  target MB  private KB  working KB  loaded");
    let mut prev = start;
    let mut prev_dir = 0;
    let mut growth: Vec<(Step, f64, f64, f64, f64)> = Vec::new();
    for row in rows {
        let private = (row.memory.private_bytes as f64 - prev.private_bytes as f64) / KB;
        let working = (row.memory.working_set as f64 - prev.working_set as f64) / KB;
        let dir = (row.build_dir as f64 - prev_dir as f64) / KB;
        println!(
            "{:<17} {:>7.2}  {:>3}  {:>6.0}  {:>9.0}  {:>9.1}  {:>+10.0}  {:>+10.0}  {:>6}",
            format!("{:?}", row.step),
            row.seconds,
            row.new_files,
            row.new_bytes as f64 / KB,
            row.build_dir as f64 / KB,
            row.target_dir as f64 / KB / KB,
            private,
            working,
            row.loaded
        );
        growth.push((row.step, row.seconds, dir, private, working));
        prev = row.memory;
        prev_dir = row.build_dir;
    }
    println!("\nsummary for {profile}:");
    for step in [Step::Cold, Step::NoChange, Step::Node, Step::FixAfterFailure, Step::Types] {
        let part: Vec<_> = growth.iter().filter(|g| g.0 == step).collect();
        if part.is_empty() {
            continue;
        }
        let n = part.len() as f64;
        let mean = |f: fn(&&(Step, f64, f64, f64, f64)) -> f64| part.iter().map(f).sum::<f64>() / n;
        let min = part.iter().map(|g| g.1).fold(f64::MAX, f64::min);
        let max = part.iter().map(|g| g.1).fold(0.0, f64::max);
        println!(
            "  {step:?} x{}: build {min:.2} to {max:.2} s (mean {:.2}); each adds build/ {:.0} KB, private {:+.0} KB, working set {:+.0} KB",
            part.len(),
            mean(|g| g.1),
            mean(|g| g.2),
            mean(|g| g.3),
            mean(|g| g.4)
        );
    }
    let files = listing(&project.build_dir).expect("listing");
    println!("  build/ has {} files at the end", files.len());
}

/// Builds the project in the `release` profile with LTO and `-C prefer-dynamic`, as bake mode
/// would (spec 8.3). It reports the result and does not fail.
fn lto_check() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hot-reload").join("release-lto");
    let project = Project::from_template(&work, "release").expect("copy the project");
    let profile = format!("{EXCLUDE_LINE}\n\n[profile.release]\nlto = \"fat\"");
    project.replace_in_file("Cargo.toml", EXCLUDE_LINE, &profile).expect("add LTO");
    let types_lib = project.generate_shared().expect("generate");
    println!("\n==== release with lto = \"fat\" and -C prefer-dynamic ====");
    match project.build() {
        Ok(output) => {
            println!("the build passed in {:.2} s", output.seconds);
            for target in ["cluster_a", "cluster_b", types_lib.as_str()] {
                let info = output.dll(target).and_then(pe_info);
                match info {
                    Ok(info) => {
                        let libs: Vec<_> = info.imports.keys().collect();
                        println!("  {target} imports {libs:?}");
                    }
                    Err(e) => println!("  {target}: {e}"),
                }
            }
        }
        Err(e) => {
            let text = e.to_string();
            let lines: Vec<_> = text.lines().filter(|l| l.contains("error")).take(6).collect();
            println!("the build failed:\n{}", lines.join("\n"));
        }
    }
}
