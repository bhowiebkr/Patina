//! Builds and runs projects with the CLI, as a person does (spec 3, milestone 1).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patina_engine::{Control, Lanes, Project, Toolchain};

/// Copies a project into the temporary folder of the tests and sets the Rust version of the
/// toolchain. The `build/` folder stays, so a second test run builds only the changes.
fn copy(from: &Path, name: &str) -> PathBuf {
    let to = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    for entry in fs::read_dir(&to).into_iter().flatten().map(Result::unwrap) {
        if entry.file_name() != "build" {
            let path = entry.path();
            if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }.unwrap();
        }
    }
    copy_dir(from, &to);
    let release = Toolchain::detect().unwrap().release;
    let file = to.join("patina.toml");
    let text = fs::read_to_string(&file).unwrap().replace("rust = \"1.94.0\"", &format!("rust = \"{release}\""));
    fs::write(file, text).unwrap();
    to
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().map(Result::unwrap) {
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        if entry.file_name() == "build" {
            continue;
        } else if source.is_dir() {
            copy_dir(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

fn run(project: &Path, more: &[&str]) -> (Output, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_patina-cli")).arg("run").arg(project).args(more).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output, stdout, stderr)
}

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

/// A connection kind of the test, for the f64 wires of the hello example (6.3).
const KIND: &str = "#[patina_sdk::connection]
pub fn whole(value: &f64) -> patina_sdk::Lanes {
    patina_sdk::Lanes::bits(8, *value as u64).label(\"whole\")
}
";

/// A generic stateful frame node of the test (4.2, 4.5).
const TICK: &str = "#[patina_sdk::node(domain = frame)]
pub struct Tick<T> {
    count: f64,
    last: Option<T>,
}

impl<T: Copy + Into<f64>> Tick<T> {
    pub fn init() -> Self {
        Tick { count: 0.0, last: None }
    }

    pub fn process(&mut self, base: T) -> f64 {
        self.last = Some(base);
        self.count = (self.count + base.into()) % 10.0;
        self.count
    }

    pub fn ui(&mut self, ui: &mut patina_sdk::Ui) {
        ui.label(\"Tick\");
        if ui.button(\"Reset\").clicked() {
            self.count = 0.0;
        }
        ui.slider(\"Count\", &mut self.count, 0.0..=10.0);
    }
}
";

/// A generic node of the test (4.5).
const MIX: &str = "#[patina_sdk::node]
pub fn mix<T: std::ops::Add<Output = T> + Copy>(a: &T, b: T) -> T {
    *a + b
}
";

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

    // The cluster sends the lanes of each held value, the most significant lane first.
    let toolchain = Toolchain::detect().unwrap();
    let opened = Project::open(&project).unwrap();
    let mut cluster = opened.check().unwrap().build(&toolchain).unwrap();
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
    let (mut cluster, control) = (opened.check().unwrap().build(&toolchain).unwrap(), Control::default());
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

    // The GUI of tick (4.3): its widget lines, then a click on Reset. Mix has no GUI.
    let tree = cluster.ui("00000000000000f1", 0, "").0.unwrap();
    assert!(tree.starts_with("label\tTick\nbutton\tReset\nslider\tCount\t"), "{tree}");
    let (tree, out) = cluster.ui("00000000000000f1", 0, "1 1");
    assert!(out.ok && tree.unwrap().ends_with("slider\tCount\t0\t0\t10"));
    assert!(cluster.ui("00000000000000f3", 0, "").0.is_none());
    assert_eq!(cluster.guis(), [("00000000000000f1".to_owned(), true)]);
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
