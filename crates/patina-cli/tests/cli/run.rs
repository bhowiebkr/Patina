//! Builds and runs projects with the CLI, as a person does (spec 3, milestone 1).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Instant;

use patina_engine::{Cluster, Control, Lanes, Mode, Project, Toolchain};

use crate::common::{copy, run};
use crate::nodes::{HOLD, KIND, METER, MIX, SPEAKER, TICK, TONE, ZERO};

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
#[allow(clippy::too_many_lines)]
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
    // give the type parameters of tick, mix and delay (4.5). Zero f6 is in the batch domain, and
    // number f7 of the frame domain gives its type. The counts stay under 10, so the texts stop
    // growing after a few frames.
    for (name, code) in [("tick", TICK), ("mix", MIX), ("zero", ZERO)] {
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
        node("f6", "zero"),
        node("f7", "number") + "domain = \"frame\"\n",
        wire("00000000000000f6.out", "00000000000000f7.value"),
    ]
    .concat();
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + &more).unwrap();
    let (output, stdout, stderr) = run(&project, &["--frames", "3"]);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(stdout.lines().collect::<Vec<_>>(), ["6.5", "7.5", "9.5", "13.5"]);

    // After the first frames, a frame does not allocate (9.1). The two frame tasks wait for each
    // other, so this thread runs both. The values, the lanes and the times are in each frame. As
    // in the editor, three quiet frames come before each frame with values, and the GUI of tick
    // draws after it.
    let opened = Project::open(&project).unwrap();
    let (mut cluster, control) = (opened.check().unwrap().build(&toolchain, Mode::Edit).unwrap(), Control::default());
    cluster.set_params(opened.graph());
    assert!(cluster.run(&control).ok);
    let frames = |cluster: &mut Cluster, count: u32| {
        (0..count).all(|i| {
            control.set_quiet(i % 4 != 3);
            COUNTING.set(true);
            let ok = cluster.frame(&control).ok;
            COUNTING.set(false);
            ok && (i % 4 != 3 || cluster.ui(&control, "00000000000000f1", 0, "").0.is_some())
        })
    };
    let warm = frames(&mut cluster, 12);
    ALLOCATIONS.set(0);
    COUNTING.set(true);
    drop(std::hint::black_box(Box::new(1)));
    COUNTING.set(false);
    let boxed = ALLOCATIONS.replace(0);
    let ok = frames(&mut cluster, 100);
    control.set_quiet(false);
    let frame = cluster.frame(&control);
    assert!(warm && ok && frame.values.iter().any(|v| v.lanes.is_some()) && !frame.times.is_empty());
    assert_eq!((boxed, ALLOCATIONS.get()), (1, 0), "one box, then the allocations of 100 frames");
    // This thread ran the calls one after the other, in the frame (15.7).
    assert!(frame.times.iter().all(|t| t.thread == 0 && (0.0..1000.0).contains(&t.start)), "{:?}", frame.times);
    assert!(frame.times.windows(2).all(|w| w[1].start >= w[0].start), "{:?}", frame.times);

    // The GUI of tick (4.3): its widget lines, then a click on Reset. Mix has no GUI. The lines
    // after an event show the new state.
    let tree = cluster.ui(&Control::default(), "00000000000000f1", 0, "").0.unwrap();
    assert!(tree.starts_with("label\tTick\nbutton\tReset\nslider\tCount\t"), "{tree}");
    let (tree, out) = cluster.ui(&Control::default(), "00000000000000f1", 0, "1 1");
    assert!(out.ok && tree.unwrap().ends_with("slider\tCount\t0\t0\t10"));
    let (tree, out) = cluster.ui(&Control::default(), "00000000000000f1", 0, "2 7");
    assert!(out.ok && tree.unwrap().ends_with("slider\tCount\t7\t0\t10"));
    assert!(cluster.ui(&Control::default(), "00000000000000f3", 0, "").0.is_none());
    // Tick has a GUI, and so have the 4 number nodes and the 2 show nodes of hello.
    assert!(cluster.guis().contains(&("00000000000000f1".to_owned(), true)) && cluster.guis().len() == 7, "{:?}", cluster.guis());

    // After an edit of mix, the new cluster gets the state of tick (7.2). An edit of tick changes
    // its state key, so its count starts again. The number and show nodes keep their 6 states.
    for (name, code, moved, count) in [("mix", MIX, 7, "7"), ("tick", TICK, 6, "0")] {
        fs::write(project.join("nodes").join(name).join("src/lib.rs"), format!("{code}// An edit.\n")).unwrap();
        let mut new = Project::open(&project).unwrap().check().unwrap().build(&toolchain, Mode::Edit).unwrap();
        assert_eq!(new.adopt(&mut cluster), moved, "{name}");
        cluster = new;
        let tree = cluster.ui(&Control::default(), "00000000000000f1", 0, "").0.unwrap();
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
    // The frame part runs the blocks when the audio domain cannot have its own thread.
    assert!(held.run(&control).ok && held.frame_part().block(&control).is_some_and(|out| out.ok));
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

    // Delay ad between tone and speaker runs in the audio domain (9.2): a block plays the tone of
    // the block before, also with no frame between them. The first block is silent.
    let delay = "
[[node]]
id = \"00000000000000ad\"
builtin = \"delay\"
".to_owned()
        + &wire("00000000000000a1.out", "00000000000000ad.value")
        + &wire("00000000000000ad.out", "00000000000000a4.sound");
    let text = fs::read_to_string(&graph).unwrap();
    fs::write(&graph, text.replace(&wire("00000000000000a1.out", "00000000000000a4.sound"), &delay)).unwrap();
    let opened = Project::open(&project).unwrap();
    let mut delayed = opened.check().unwrap().build(&toolchain, Mode::Edit).unwrap();
    delayed.set_params(opened.graph());
    assert!(delayed.run(&control).ok && delayed.frame(&control).ok && delayed.block(&control).ok);
    let first = delayed.sound().map(|s| (s[0], s[1]));
    assert!(delayed.block(&control).ok);
    assert!(first.is_none_or(|s| s == (0.0, 0.0)), "{first:?}");
    assert_eq!(delayed.sound().map(|s| (s[0], s[1])), Some((15.0, 0.0)));
    drop(delayed);
    fs::write(&graph, text).unwrap();

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
        // A slow start of the audio thread is not a late block, so the measure starts after its first block.
        // A failed block ends the thread, so the wait has a time limit.
        let wait = Instant::now();
        while blocks.load(Relaxed) == 0 && wait.elapsed().as_secs() < 10 {
            std::thread::yield_now();
        }
        let (before, start) = (blocks.load(Relaxed), Instant::now());
        let out = frames.frame(&control);
        let frame = (out.ok && out.skipped.is_empty(), start.elapsed(), blocks.load(Relaxed) - before);
        let (before, start) = (blocks.load(Relaxed), Instant::now());
        let (tree, out) = frames.ui(&Control::default(), "00000000000000a2", 0, "");
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
    assert_eq!(stdout, "-2\n2\n", "a node after a failed node must not run");
    assert!(stderr.contains("Node fea12c51067a34d8 (fail): error: \"cannot use 1.5\""), "{stderr}");
    assert!(stderr.contains("Node dc1095edd35b9589 (boom): panic: the boom node panicked ("), "{stderr}");
    assert!(stderr.contains("lib.rs:3:5)") && !stderr.contains("panicked at"), "one message for each panic: {stderr}");
    assert!(stderr.contains("Node df93472b07eaca16: skipped: an input has no value"), "{stderr}");
    assert!(stderr.contains("Node 7363866a8cd1c455: skipped"), "{stderr}");
    // In a net of gates (9.2), a panic in the fast copy stops only the nodes after it.
    let panic = "Node a000000000000004 (div): panic: attempt to divide by zero";
    assert!(stderr.contains(panic), "{stderr}");
    for id in ["a000000000000006", "a000000000000009"] {
        assert!(stderr.contains(&format!("Node {id}: skipped")), "{stderr}");
    }
    assert!(!stderr.contains("Node a000000000000008"), "{stderr}");

    // With a wire from the fail node, the net has an input with no value and runs its slow copy.
    let graph = project.join("graphs/main.toml");
    let old = fs::read_to_string(&graph).unwrap().replace("\r\n", "\n");
    let wire = "from = \"a000000000000007.out\"\nto = \"a000000000000008.x\"";
    let text = old.replace(wire, "from = \"fea12c51067a34d8.out\"\nto = \"a000000000000008.x\"");
    assert_ne!(text, old);
    fs::write(&graph, text).unwrap();
    let (output, stdout, stderr) = run(&project, &[]);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(stdout, "-2\n", "{stderr}");
    assert!(stderr.contains(panic), "{stderr}");
    for id in ["a000000000000006", "a000000000000008", "a000000000000009", "a00000000000000b"] {
        assert!(stderr.contains(&format!("Node {id}: skipped")), "{stderr}");
    }

    // A parameter change builds nothing again, and a bad parameter text is reported.
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
    assert!(number.contains("-> f64 {") && number.contains("\n        value\n"), "{number}");
    let first = number.replace("-> f64 {", "-> f32 {").replace("\n        value\n", "\n        value as f32\n");
    let second = number.replace("-> f64 {", "-> i32 {").replace("\n        value\n", "\n        (value * 3.0) as i32\n");

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
        assert_eq!(patina_block(package), -1, "hello has no audio nodes");
        assert!(patina_sound(package).is_null());
        assert_eq!(patina_events(package, 1), -1, "hello has no event nodes");
        assert_eq!(error(), "the graph has no event nodes");
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
    // The image on the wire of the gradient has a thumbnail of 64 x 16 pixels (5.3).
    let image = outcome.values.iter().find(|v| v.from == "1a2b3c4d5e6f7081.out").unwrap();
    assert!(image.thumb.starts_with("64 16 ") && image.thumb.len() == 6 + (64 * 16 * 4_usize).div_ceil(3) * 4, "{}", image.thumb.len());
    let tree = cluster.ui(&Control::default(), "4d5e6f708192a3b4", 0, "").0.unwrap();
    assert!(tree.starts_with("canvas\tPreview 64 x 16\t200\t") && tree.contains("\nplace\t\t18 28\nimage\t\t64\t16\t"), "{tree}");

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
    let mut opened = Project::open(&project).unwrap();
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
fn a_node_with_the_log_crate_sends_its_log_lines() {
    // The cluster turns on its logger for a `log` crate with another name and for a `log` crate
    // in a target table (15.7). The logger shows Info and higher, so the debug line does not show.
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), "log");
    let (add, show) = (project.join("nodes/add"), project.join("nodes/show"));
    let manifest = fs::read_to_string(add.join("Cargo.toml")).unwrap();
    fs::write(add.join("Cargo.toml"), format!("{manifest}logger = {{ package = \"log\", version = \"0.4\" }}\n")).unwrap();
    let code = fs::read_to_string(add.join("src/lib.rs")).unwrap();
    let code = code.replace("    a + b", "    logger::info!(\"adds {a} and {b}\");\n    logger::debug!(\"a hidden line\");\n    a + b");
    fs::write(add.join("src/lib.rs"), code).unwrap();
    let manifest = fs::read_to_string(show.join("Cargo.toml")).unwrap();
    fs::write(show.join("Cargo.toml"), format!("{manifest}\n[target.'cfg(all())'.dependencies]\nlog = \"0.4\"\n")).unwrap();
    let code = fs::read_to_string(show.join("src/lib.rs")).unwrap();
    fs::write(show.join("src/lib.rs"), code.replace("    println!", "    log::warn!(\"shows {value}\");\n    println!")).unwrap();

    let (output, stdout, stderr) = run(&project, &[]);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(stdout.trim(), "6.5");
    assert!(stderr.contains("Node f89c6e329e8ea336 (add): INFO adds 2.5 and 4\n"), "{stderr}");
    assert!(stderr.contains("Node 3b7cec82f6a4ed1a (show): WARN shows 6.5\n"), "{stderr}");
    assert!(!stderr.contains("hidden"), "{stderr}");
}
