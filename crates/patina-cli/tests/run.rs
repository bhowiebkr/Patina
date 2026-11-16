//! Builds and runs projects with the CLI, as a person does (spec 3, milestone 1).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Instant;

use patina_engine::{Control, Lanes, Mode, Project, Toolchain};

mod nodes;
use nodes::{HOLD, KIND, METER, MIX, SPEAKER, TICK, TONE};

mod common;
use common::{copy, run};

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

/// Counts the allocations of a thread while its `COUNTING` is true, to check the frame loop.
struct Counter;

// SAFETY: the system allocator does the work. The counter only reads and writes two cells.
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.get() {
            ALLOCATIONS.set(ALLOCATIONS.get() + 1);
        }
        // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `alloc` with the same layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static COUNTER: Counter = Counter;

#[test]
fn hello_adds_two_numbers_and_a_kind_shows_lanes() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), "hello");
    let kind = project.join("connections/whole");
    fs::create_dir_all(kind.join("src")).unwrap();
    let manifest = "[package]\nname = \"whole\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\npatina-sdk = { workspace = true }\n";
    fs::write(kind.join("Cargo.toml"), manifest).unwrap();
    fs::write(kind.join("src/lib.rs"), KIND).unwrap();
    let (output, stdout, stderr) = run(&project, &[]);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(stdout.trim(), "6.5");

    // Add moves into the subgraph `sum` (6.4). Use d1 adds the numbers, and use d2 adds its
    // parameter 1 to that sum.
    let main = project.join("graphs/main.toml");
    let original = fs::read_to_string(&main).unwrap();
    fs::write(project.join("graphs/sum.toml"), SUM).unwrap();
    let start = original.find("[[node]]\nid = \"f89c").or_else(|| original.find("[[node]]\r\nid = \"f89c")).unwrap();
    let uses = "[[node]]\nid = \"00000000000000d1\"\nsubgraph = \"sum\"\n\n[[node]]\nid = \"00000000000000d2\"\nsubgraph = \"sum\"\n\n[node.params]\n0000000b = \"1\"\n\n[[node]]\nid = \"3b7cec82f6a4ed1a\"\nfolder = \"nodes/show\"\n";
    let wires = "[[wire]]\nfrom = \"9bc1790966687030.out\"\nto = \"00000000000000d1.0000000a\"\n[[wire]]\nfrom = \"69779baf026654c5.out\"\nto = \"00000000000000d1.0000000b\"\n[[wire]]\nfrom = \"00000000000000d1.0000000c\"\nto = \"00000000000000d2.0000000a\"\n[[wire]]\nfrom = \"00000000000000d2.0000000c\"\nto = \"3b7cec82f6a4ed1a.value\"\n";
    fs::write(&main, format!("{}{uses}{wires}", &original[..start])).unwrap();
    let (output, stdout, stderr) = run(&project, &[]);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(stdout.trim(), "7.5");
    fs::write(&main, original).unwrap();

    // The cluster sends the lanes of each held value, the most significant lane first.
    let toolchain = Toolchain::detect().unwrap();
    let opened = Project::open(&project).unwrap();
    let mut cluster = opened.check().unwrap().build(&toolchain, Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    let values = cluster.run(&Control::default()).values;
    let sum = values.iter().find(|v| v.from == "f89c6e329e8ea336.out").unwrap();
    assert_eq!(sum.lanes, Some(Lanes { bits: "00000110".to_owned(), label: "whole".to_owned() }));

    // A frame node reads batch values at each frame, after the batch run (9.1): number f0 (1) and
    // delay f5 to mix f4, then tick f1, then mix f3 (sum 6.5 + tick) and show f2. Tick is a
    // stateful frame node (4.2) that adds its input to its count at each frame. Delay f5 gives
    // the tick of the previous frame (9.2), first 0, so the counts are 1, 3 and 7. The wires
    // give the type parameters of tick, mix and delay (4.5). The counts stay under 10, so the
    // texts stop growing after a few frames.
    for (name, code) in [("tick", TICK), ("mix", MIX)] {
        let folder = project.join("nodes").join(name);
        fs::create_dir_all(folder.join("src")).unwrap();
        fs::write(folder.join("Cargo.toml"), manifest.replace("whole", name)).unwrap();
        fs::write(folder.join("src/lib.rs"), code).unwrap();
    }
    let graph = project.join("graphs/main.toml");
    let node = |id: &str, folder: &str| format!("\n[[node]]\nid = \"00000000000000{id}\"\nfolder = \"nodes/{folder}\"\n");
    let wire = |from: &str, to: &str| format!("\n[[wire]]\nfrom = \"{from}\"\nto = \"{to}\"\n");
    let more = [
        node("f0", "number") + "[node.params]\nvalue = \"1\"\n",
        node("f1", "tick"),
        node("f3", "mix"),
        node("f2", "show"),
        node("f4", "mix"),
        "
[[node]]
id = \"00000000000000f5\"
builtin = \"delay\"
".to_owned(),
        wire("00000000000000f0.out", "00000000000000f4.a"),
        wire("00000000000000f5.out", "00000000000000f4.b"),
        wire("00000000000000f4.out", "00000000000000f1.base"),
        wire("00000000000000f1.out", "00000000000000f5.value"),
        wire("f89c6e329e8ea336.out", "00000000000000f3.a"),
        wire("00000000000000f1.out", "00000000000000f3.b"),
        wire("00000000000000f3.out", "00000000000000f2.value"),
    ]
    .concat();
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + &more).unwrap();
    let (output, stdout, stderr) = run(&project, &["--frames", "3"]);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(stdout.lines().collect::<Vec<_>>(), ["6.5", "7.5", "9.5", "13.5"]);

    // After the first frames, a frame does not allocate (9.1). The two frame tasks wait for each
    // other, so this thread runs both. The values, the lanes and the times are in each frame.
    let opened = Project::open(&project).unwrap();
    let (mut cluster, control) = (opened.check().unwrap().build(&toolchain, Mode::Edit).unwrap(), Control::default());
    cluster.set_params(opened.graph());
    assert!(cluster.run(&control).ok);
    let warm = (0..10).all(|_| cluster.frame(&control).ok);
    COUNTING.set(true);
    drop(std::hint::black_box(Box::new(1)));
    let boxed = ALLOCATIONS.replace(0);
    let ok = (0..100).all(|_| cluster.frame(&control).ok);
    COUNTING.set(false);
    let frame = cluster.frame(&control);
    assert!(warm && ok && frame.values.iter().any(|v| v.lanes.is_some()) && !frame.times.is_empty());
    assert_eq!((boxed, ALLOCATIONS.get()), (1, 0), "one box, then the allocations of 100 frames");

    // The GUI of tick (4.3): its widget lines, then a click on Reset. Mix has no GUI. The lines
    // after an event show the new state.
    let tree = cluster.ui("00000000000000f1", 0, "").0.unwrap();
    assert!(tree.starts_with("label\tTick\nbutton\tReset\nslider\tCount\t"), "{tree}");
    let (tree, out) = cluster.ui("00000000000000f1", 0, "1 1");
    assert!(out.ok && tree.unwrap().ends_with("slider\tCount\t0\t0\t10"));
    let (tree, out) = cluster.ui("00000000000000f1", 0, "2 7");
    assert!(out.ok && tree.unwrap().ends_with("slider\tCount\t7\t0\t10"));
    assert!(cluster.ui("00000000000000f3", 0, "").0.is_none());
    assert_eq!(cluster.guis(), [("00000000000000f1".to_owned(), true)]);

    // After an edit of mix, the new cluster gets the state of tick (7.2). An edit of tick changes
    // its state key, so its count starts again.
    for (name, code, moved, count) in [("mix", MIX, 1, "7"), ("tick", TICK, 0, "0")] {
        fs::write(project.join("nodes").join(name).join("src/lib.rs"), format!("{code}// An edit.\n")).unwrap();
        let mut new = Project::open(&project).unwrap().check().unwrap().build(&toolchain, Mode::Edit).unwrap();
        assert_eq!(new.adopt(&mut cluster), moved, "{name}");
        cluster = new;
        let tree = cluster.ui("00000000000000f1", 0, "").0.unwrap();
        assert!(tree.ends_with(&format!("slider\tCount\t{count}\t0\t10")), "{name}: {tree}");
    }

    // Audio (9.3): tone a1 reads mix f3 of the frames, and meter a2 reads tone for show a3. Each
    // reads the latest value of the other domain. The blocks run on their own thread, so the
    // tone that a frame gets depends on the time. Frame 0 starts before mix has a value, so
    // meter has no value. Frames 1 and 2 can get the tones of frames 0 and 1 (15 and 19).
    for (name, code) in [("tone", TONE), ("meter", METER)] {
        let folder = project.join("nodes").join(name);
        fs::create_dir_all(folder.join("src")).unwrap();
        fs::write(folder.join("Cargo.toml"), manifest.replace("whole", name)).unwrap();
        fs::write(folder.join("src/lib.rs"), code).unwrap();
    }
    let more = [
        node("a1", "tone"),
        node("a2", "meter"),
        node("a3", "show"),
        wire("00000000000000f3.out", "00000000000000a1.level"),
        wire("00000000000000a1.out", "00000000000000a2.sound"),
        wire("00000000000000a2.out", "00000000000000a3.value"),
    ]
    .concat();
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + &more).unwrap();
    let (output, stdout, stderr) = run(&project, &["--frames", "3", "--blocks", "9"]);
    assert!(output.status.success(), "{stderr}");
    let (tones, mut lines): (Vec<&str>, Vec<&str>) = stdout.lines().partition(|l| ["15", "19"].contains(l));
    lines.sort_unstable();
    assert_eq!(lines, ["13.5", "6.5", "7.5", "9.5"], "{stderr}");
    assert!(tones.len() <= 2 && stderr.contains("Node 00000000000000a2: skipped"), "{stderr}");
    // The frames have no count, so they stop when the blocks are done.
    let (output, _, stderr) = run(&project, &["--blocks", "9"]);
    assert!(output.status.success(), "{stderr}");

    // Hold a5 keeps a value that is not `Sync`, so the blocks run on the thread of the frames. A
    // frame runs before the first block that starts at or after its time: frames 1 and 2 run
    // before blocks 4 and 7, and get the tones of frames 0 and 1.
    let threaded = fs::read_to_string(&graph).unwrap();
    let folder = project.join("nodes/hold");
    fs::create_dir_all(folder.join("src")).unwrap();
    fs::write(folder.join("Cargo.toml"), manifest.replace("whole", "hold")).unwrap();
    fs::write(folder.join("src/lib.rs"), HOLD).unwrap();
    fs::write(&graph, threaded.clone() + &node("a5", "hold") + &wire("00000000000000a1.out", "00000000000000a5.sound")).unwrap();
    let (output, stdout, stderr) = run(&project, &["--frames", "3", "--blocks", "9"]);
    assert!(output.status.success(), "{stderr}");
    let mut lines: Vec<&str> = stdout.lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, ["13.5", "15", "19", "6.5", "7.5", "9.5"], "{stderr}");
    let mut held = Project::open(&project).unwrap().check().unwrap().build(&toolchain, Mode::Edit).unwrap();
    assert!(held.blocks() && held.split().is_none());
    fs::write(&graph, threaded).unwrap();

    // Speaker a4 plays tone into the mix of the project, which the engine reads after a block.
    let folder = project.join("nodes/speaker");
    fs::create_dir_all(folder.join("src")).unwrap();
    fs::write(folder.join("Cargo.toml"), manifest.replace("whole", "speaker")).unwrap();
    fs::write(folder.join("src/lib.rs"), SPEAKER).unwrap();
    let more = node("a4", "speaker") + &wire("00000000000000a1.out", "00000000000000a4.sound");
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + &more).unwrap();
    let opened = Project::open(&project).unwrap();
    let mut cluster = opened.check().unwrap().build(&toolchain, Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    assert!(cluster.run(&control).ok && cluster.frame(&control).ok && cluster.sound().is_none());
    assert!(cluster.block(&control).ok);
    assert_eq!(cluster.sound().map(|s| (s[0], s[1])), Some((15.0, 0.0)));

    // A slow frame or node GUI does not make the blocks late: the audio thread runs blocks during
    // the 20 ms of meter in a frame and in a `ui` call (4.3).
    let (mut frames, mut audio) = cluster.split().unwrap();
    let (stop, blocks) = (AtomicBool::new(false), AtomicU64::new(0));
    // The checks come after the scope: a failed check in it does not stop the audio thread, so
    // the scope never ends.
    let (frame, ui) = std::thread::scope(|s| {
        s.spawn(|| {
            let control = Control::default();
            while !stop.load(Relaxed) {
                assert!(audio.block(&control).ok);
                blocks.fetch_add(1, Relaxed);
            }
        });
        let (before, start) = (blocks.load(Relaxed), Instant::now());
        let out = frames.frame(&control);
        let frame = (out.ok && out.skipped.is_empty(), start.elapsed(), blocks.load(Relaxed) - before);
        let (before, start) = (blocks.load(Relaxed), Instant::now());
        let (tree, out) = frames.ui("00000000000000a2", 0, "");
        let ui = (out.ok && tree.is_some_and(|t| t.contains("Meter")), start.elapsed(), blocks.load(Relaxed) - before);
        stop.store(true, Relaxed);
        (frame, ui)
    });
    for (what, (ok, time, ran)) in [("frame", frame), ("ui", ui)] {
        // The meter sleeps 20 ms. On Windows, a sleep can end a little early on the clock of `Instant`.
        assert!(ok && time.as_millis() >= 15, "{what}: ok {ok}, {time:?}");
        assert!(ran > 0, "no block ran during the {what}");
    }
}

#[test]
fn failed_nodes_are_reported_and_stop_the_nodes_after_them() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/projects/fail"), "fail");
    let (output, stdout, stderr) = run(&project, &[]);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(stdout, "", "a node after a failed node must not run");
    assert!(stderr.contains("Node fea12c51067a34d8 (fail): error: \"cannot use 1.5\""), "{stderr}");
    assert!(stderr.contains("Node dc1095edd35b9589 (boom): panic: the boom node panicked"), "{stderr}");
    assert!(stderr.contains("Node df93472b07eaca16: skipped: an input has no value"), "{stderr}");
    assert!(stderr.contains("Node 7363866a8cd1c455: skipped"), "{stderr}");

    // A parameter change builds nothing again, and a bad parameter text is reported.
    let graph = project.join("graphs/main.toml");
    let text = fs::read_to_string(&graph).unwrap().replace("value = \"1.5\"", "value = \"abc\"");
    fs::write(graph, text).unwrap();
    let (output, stdout, stderr) = run(&project, &[]);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(stdout, "");
    let expected = "Node fea12c51067a34d8 (fail): bad parameter: input `value`: \"abc\" is not a number";
    assert!(stderr.contains(expected), "{stderr}");
}

/// A subgraph with one add node and its public pins (6.4).
const SUM: &str = "format = 1

[[node]]
id = \"00000000000000a1\"
folder = \"nodes/add\"

[[input]]
key = \"0000000a\"
name = \"a\"
pos = [0.0, 0.0]
to = [\"00000000000000a1.a\"]

[[input]]
key = \"0000000b\"
name = \"b\"
pos = [0.0, 40.0]
to = [\"00000000000000a1.b\"]

[[output]]
key = \"0000000c\"
name = \"sum\"
pos = [200.0, 0.0]
from = \"00000000000000a1.out\"
";

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
        self.total += *value * scale;
        *extra += note.len() as f64;
        self.total
    }
}
";

#[test]
fn a_node_in_the_slot_builds_alone_and_keeps_its_uses() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), "slot");
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

    // An edit of the body builds the slot alone. The node gets a new state.
    let edited = ACC.replace("*value * scale", "*value * scale * 10.0");
    fs::write(folder.join("src/lib.rs"), &edited).unwrap();
    let start = Instant::now();
    assert!(slotted().build_slot(&toolchain, &mut cluster).unwrap());
    eprintln!("slot build {:.1} s", start.elapsed().as_secs_f64());
    assert_eq!(value(&mut cluster, "out"), "130.0");

    // Fuse back in bake mode: the fused cluster gets the state of the slot library (7.2).
    let start = Instant::now();
    let mut fused = checked().build(&toolchain, Mode::Bake).unwrap();
    eprintln!("fuse {:.1} s", start.elapsed().as_secs_f64());
    fused.set_params(opened.graph());
    assert!(cluster.slotted() && !fused.slotted() && fused.mode() == Mode::Bake);
    assert_eq!(fused.adopt(&mut cluster), 1);
    assert_eq!(value(&mut fused, "out"), "260.0");
    assert!(!slotted().build_slot(&toolchain, &mut fused).unwrap(), "a fused cluster has no slot");

    // A change of the signature changes the base, so the slot alone does not build.
    let mut cluster = slotted().build(&toolchain, Mode::Edit).unwrap();
    fs::write(folder.join("src/lib.rs"), edited.replace("scale: f64", "scale: f32").replace("* scale", "* f64::from(scale)")).unwrap();
    assert!(!slotted().build_slot(&toolchain, &mut cluster).unwrap());
    assert!(checked().set_slot(Some("number")).is_ok());
    assert!(checked().set_slot(Some("nothing")).is_err());
}

/// The check of milestone 6 (19): one node folder with a use in the batch cluster and a use in
/// the frame cluster. Its output type changes, and changes again before the first build ends.
#[test]
fn a_type_change_during_a_build_reaches_all_uses() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), "retype");
    let manifest = fs::read_to_string(project.join("nodes/add/Cargo.toml")).unwrap();
    for (name, code) in [("tick", TICK), ("mix", MIX)] {
        let folder = project.join("nodes").join(name);
        fs::create_dir_all(folder.join("src")).unwrap();
        fs::write(folder.join("Cargo.toml"), manifest.replace("\"add\"", &format!("\"{name}\""))).unwrap();
        fs::write(folder.join("src/lib.rs"), code).unwrap();
    }
    // Use b1 of number feeds mix in the batch cluster. Use b2 is in the frame domain and feeds tick.
    let more = "
[[node]]
id = \"00000000000000b1\"
folder = \"nodes/number\"

[node.params]
value = \"2.5\"

[[node]]
id = \"00000000000000b2\"
folder = \"nodes/number\"
domain = \"frame\"

[node.params]
value = \"2.5\"

[[node]]
id = \"00000000000000b3\"
folder = \"nodes/mix\"

[[node]]
id = \"00000000000000b4\"
folder = \"nodes/tick\"

[[wire]]
from = \"00000000000000b1.out\"
to = \"00000000000000b3.a\"

[[wire]]
from = \"00000000000000b1.out\"
to = \"00000000000000b3.b\"

[[wire]]
from = \"00000000000000b2.out\"
to = \"00000000000000b4.base\"
";
    let graph = project.join("graphs/main.toml");
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + more).unwrap();
    // The numbers feed add, which takes f64, so this test removes add and show.
    let text = fs::read_to_string(&graph).unwrap();
    let kept: Vec<&str> = text.split("\n[[").filter(|part| !part.contains("f89c6e329e8ea336") && !part.contains("nodes/show")).collect();
    fs::write(&graph, kept.join("\n[[")).unwrap();
    let toolchain = Toolchain::detect().unwrap();
    let code = project.join("nodes/number/src/lib.rs");
    // A checkout with CRLF line ends must still match the patterns below.
    let number = fs::read_to_string(&code).unwrap().replace("\r\n", "\n");
    let first = number.replace("-> f64 {\n    value", "-> f32 {\n    value as f32");
    let second = number.replace("-> f64 {\n    value", "-> i32 {\n    (value * 3.0) as i32");
    assert!(first != number && second != number);

    // The first change gives f32. The second change, to i32, comes while the first build runs.
    // The first build can fail, because its plan has the old type.
    fs::write(&code, &first).unwrap();
    let checked = Project::open(&project).unwrap().check().unwrap();
    std::thread::scope(|s| {
        let toolchain = &toolchain;
        let build = s.spawn(move || checked.build(toolchain, Mode::Edit).is_ok());
        std::thread::sleep(std::time::Duration::from_millis(150));
        fs::write(&code, &second).unwrap();
        assert!(!build.is_finished(), "the first build ended before the second change");
        build.join().unwrap();
    });

    // The newest build has no errors, and both uses run the newest code.
    let opened = Project::open(&project).unwrap();
    let mut cluster = opened.check().unwrap().build(&toolchain, Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    let control = Control::default();
    let value = |values: Vec<patina_engine::Value>, from: &str| values.into_iter().find(|v| v.from == from).map(|v| v.text);
    let out = cluster.run(&control);
    assert!(out.ok, "{:?}", out.reports);
    assert_eq!(value(out.values, "00000000000000b1.out").as_deref(), Some("7"));
    let frame = cluster.frame(&control);
    assert_eq!(value(frame.values.clone(), "00000000000000b2.out").as_deref(), Some("7"));
}

#[test]
fn a_package_runs_alone_and_takes_parameters() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), "package");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hello-pkg");
    if out.exists() {
        fs::remove_dir_all(&out).unwrap();
    }
    let made = Command::new(env!("CARGO_BIN_EXE_patina-cli")).arg("package").arg(&project).arg(&out).arg("--edit").output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    assert!(out.join("licenses/LICENSE-MIT").is_file() && out.join("project/graphs/main.toml").is_file() && out.join("patina_capi.h").is_file());
    assert!(!out.join("project/build").exists(), "the build folder stays out of the package");

    let program = out.join(format!("hello-pkg{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(&program).arg("9bc1790966687030.value=1").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "5");
    let bad = Command::new(&program).arg("nothing.value=1").output().unwrap();
    assert!(!bad.status.success());

    // The C API opens the same package (14.2).
    let dir = std::ffi::CString::new(out.to_str().unwrap()).unwrap();
    // SAFETY: each pointer is a valid C string, or the package from `patina_open`.
    unsafe {
        use patina_capi::*;
        let error = || std::ffi::CStr::from_ptr(patina_error()).to_string_lossy().into_owned();
        let package = patina_open(dir.as_ptr());
        assert!(!package.is_null(), "{}", error());
        assert_eq!(patina_set(package, c"9bc1790966687030.value".as_ptr(), c"1".as_ptr()), 0);
        assert_eq!(patina_set(package, c"nothing.value".as_ptr(), c"1".as_ptr()), -1);
        assert_eq!(patina_run(package), 0, "{}", error());
        let value = patina_value(package, c"f89c6e329e8ea336.out".as_ptr());
        assert_eq!(std::ffi::CStr::from_ptr(value).to_str().unwrap(), "5.0");
        assert!(patina_value(package, c"3b7cec82f6a4ed1a.out".as_ptr()).is_null());
        assert_eq!(patina_frame(package), -1, "hello has no frame nodes");
        patina_close(package);
    }
}

#[test]
fn the_gpu_example_draws_a_gradient_with_a_wesl_shader() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/gpu"), "gpu");
    let (output, stdout, stderr) = run(&project, &[]);
    if stderr.contains("no GPU adapter") {
        return; // shortcut: CI on Linux has no adapter, so only the build is checked there.
    }
    assert!(output.status.success(), "{stderr}");
    let mean: f64 = stdout.trim().parse().unwrap();
    assert!((mean - 0.336).abs() < 0.01, "{mean}");

    // The parameter sets a WESL feature, so the shader of the other branch runs.
    let graph = project.join("graphs/main.toml");
    fs::write(&graph, fs::read_to_string(&graph).unwrap().replace("invert = \"false\"", "invert = \"true\"")).unwrap();
    let (output, stdout, stderr) = run(&project, &[]);
    assert!(output.status.success(), "{stderr}");
    assert!((stdout.trim().parse::<f64>().unwrap() - 0.664).abs() < 0.01, "{stdout}");

    // The preview node shows a small copy of the image in its GUI (10.2).
    let opened = Project::open(&project).unwrap();
    let mut cluster = opened.check().unwrap().build(&Toolchain::detect().unwrap(), Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    let outcome = cluster.run(&Control::default());
    assert!(outcome.ok, "{:?}", outcome.reports);
    let tree = cluster.ui("4d5e6f708192a3b4", 0, "").0.unwrap();
    assert!(tree.starts_with("image	Preview	64	16	"), "{tree}");

    // The scene view shows a viewport. Without a window, it reads the wheel and draws nothing (15.8).
    let library = opened.build_view(&Toolchain::detect().unwrap(), "scene").unwrap();
    let mut view = patina_engine::view::View::load(&library, &project).unwrap();
    let tree = view.ui(0, "0 wheel 100\n0 key down ArrowDown", &patina_engine::view::snapshot(opened.graph(), &[], &[], &[]), None).unwrap().tree;
    assert!(tree.starts_with("viewport	Scene	320
label	") && tree.contains("30 %"), "{tree}");
}

const VIEW: &str = "use patina_sdk::{Graph, Ui, view};

#[view]
pub struct Panel {
    clicks: u32,
}

impl Panel {
    pub fn init() -> Self {
        Panel { clicks: 0 }
    }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        let value = graph.param(\"9bc1790966687030\", \"value\");
        let sum = graph.value::<f64>(\"f89c6e329e8ea336\", \"out\");
        ui.label(&format!(\"{} nodes, {value:?}, {} clicks, {sum:?}\", graph.nodes().count(), self.clicks));
        if ui.button(\"Set\").clicked() {
            self.clicks += 1;
            graph.set_param(\"9bc1790966687030\", \"value\", 5.0);
            graph.select([\"9bc1790966687030\"]);
        }
    }
}
";

#[test]
fn a_view_reads_the_graph_and_sets_a_parameter() {
    use patina_engine::view::{View, snapshot};
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), "view");
    let opened = Project::open(&project).unwrap();
    opened.new_view("panel").unwrap();
    fs::write(project.join("views/panel/src/lib.rs"), VIEW).unwrap();
    let library = opened.build_view(&Toolchain::detect().unwrap(), "panel").unwrap();
    let graph = snapshot(opened.graph(), &[], &[], &[]);
    let mut view = View::load(&library, &project).unwrap();
    let frame = view.ui(0, "", &graph, None).unwrap();
    assert_eq!(frame.tree, "label\t4 nodes, Some(\"2.5\"), 0 clicks, None\nbutton\tSet");
    assert!(frame.edits.is_empty() && frame.select.is_none());

    // A click writes the parameter as an edit, and the view draws again with its new state.
    let frame = view.ui(0, "1 1", &graph, None).unwrap();
    assert!(frame.tree.contains("1 clicks"), "{}", frame.tree);
    let [patina_engine::Edit::SetParam { id, key, text: Some(text) }] = &frame.edits[..] else { panic!("{:?}", frame.edits) };
    assert_eq!((id.as_str(), key.as_str(), text.as_str()), ("9bc1790966687030", "value", "5"));
    assert_eq!(frame.select, Some(vec!["9bc1790966687030".to_owned()]));

    // Each instance has its own state.
    assert!(View::load(&library, &project).unwrap().ui(0, "", &graph, None).unwrap().tree.contains("0 clicks"));

    // The view reads a typed output (15.8 rule 3). The first read asks the cluster to keep a copy.
    let mut cluster = opened.check().unwrap().build(&Toolchain::detect().unwrap(), Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    let mut sum = || {
        assert!(cluster.run(&Control::default()).ok);
        view.ui(0, "", &graph, Some(&mut cluster.frame_part())).unwrap().tree
    };
    assert!(sum().contains("clicks, None"));
    assert!(sum().contains("clicks, Some(6.5)"));
}

#[test]
fn the_computer_example_of_nand_gates_counts_adds_and_runs_a_program() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/computer"), "computer");
    // The clock of the CPU ticks each 4 frames, so 560 frames run 140 instructions.
    const EVERY: u32 = 4;
    let (output, stdout, stderr) = run(&project, &["--frames", "560"]);
    assert!(output.status.success(), "{stderr}");
    // Each frame is one step: readout A shows the count, and readout B the count plus 3.
    // With b = 0011, the four full adders get all 8 input vectors. The register of readout C
    // loads its value plus 5 at each step. The memory of readout D writes C to address n % 4, so D
    // shows C of 4 steps before. Readouts E, F and G show the PC, A and OUT of the 8-bit CPU. At
    // each tick, it runs one instruction. Its program writes the Fibonacci numbers to OUT, and at
    // the carry it shows 0 and starts again. The displays show the count, and OUT as two hex
    // digits, segments a to g. One step after OUT loads, the LCD writer writes the two hex digits
    // of OUT and a space, one character each step.
    const SEGMENTS: [&str; 16] = [
        "1111110", "0110000", "1101101", "1111001", "0110011", "1011011", "1011111", "1110000", "1111111", "1111011", "1110111", "0011111",
        "1001110", "0111101", "1001111", "1000111",
    ];
    const PROGRAM: [(u8, u8); 16] =
        [(0, 0), (5, 2), (0, 1), (5, 1), (2, 0), (1, 2), (4, 13), (5, 3), (6, 1), (5, 2), (6, 3), (5, 1), (3, 4), (0, 0), (2, 0), (7, 0)];
    let (mut pc, mut acc, mut out, mut carry, mut zero, mut ram) = (0u8, 0u8, 0u8, false, false, [0u8; 4]);
    let (mut expected, mut lcd, mut shown, mut strobes) = (Vec::new(), String::new(), Vec::new(), 0u8);
    let hex = |d: u8| char::from_digit(d.into(), 16).unwrap().to_ascii_uppercase();
    for n in 0..140 * EVERY {
        // Bit i of `strobes` is on when OUT loaded i + 1 steps before.
        if let Some(i) = (0..3).find(|i| strobes >> i & 1 != 0) {
            lcd.push([hex(out >> 4), hex(out % 16), ' '][i]);
        }
        let readouts = [n % 16, (n + 3) % 16, n * 5 % 16, n.saturating_sub(4) * 5 % 16, pc.into(), acc.into(), out.into()];
        expected.extend(readouts.map(|n| format!("{n:08b} {n}")));
        expected.extend([n as usize % 16, usize::from(out >> 4), usize::from(out % 16)].map(|d| SEGMENTS[d].to_string()));
        if n % EVERY != 0 {
            strobes = strobes << 1 & 7;
            continue;
        }
        let (op, k) = PROGRAM[usize::from(pc)];
        let word = usize::from(k % 4);
        pc = (pc + 1) % 16;
        match op {
            0 => acc = k,
            1 => (acc, carry) = acc.overflowing_add(ram[word]),
            2 => {
                out = acc;
                shown.push(acc);
            }
            3 => pc = k,
            4 if carry => pc = k,
            5 => ram[word] = acc,
            6 => acc = ram[word],
            7 if zero => pc = k,
            _ => {}
        }
        if matches!(op, 0 | 1 | 6) {
            zero = acc == 0;
        }
        strobes = (strobes << 1 | u8::from(op == 2)) & 7;
    }
    assert_eq!(shown, [1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 0, 1, 1]);
    let (lcd_lines, lines): (Vec<_>, Vec<_>) = stdout.lines().partition(|l| l.starts_with("lcd "));
    assert_eq!(lines, expected);
    // After 32 characters, each new line moves the lines up.
    let (old, line) = lcd.split_at(lcd.len() - (lcd.len() - 17) % 16 - 1);
    assert_eq!(lcd_lines.last(), Some(&format!("lcd |{}|{line:<16}|", &old[old.len() - 16..]).as_str()));
}
