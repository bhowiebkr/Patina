//! Runs graphs with event nodes in the CLI (spec 9.4, milestone 12).

use std::fs;
use std::path::Path;

mod common;
use common::{copy, run};

/// An async node. Run 1 waits 1 s, the other runs do not wait. It gives 10 x the run ID plus the
/// number of active runs at its start.
const WAIT: &str = "use std::sync::atomic::{AtomicU64, Ordering::SeqCst};

static ACTIVE: AtomicU64 = AtomicU64::new(0);

#[patina_sdk::node]
pub async fn wait(run: &u64) -> u64 {
    let seen = ACTIVE.fetch_add(1, SeqCst) + 1;
    let ms = if *run == 1 { 1000 } else { 0 };
    patina_event::tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    ACTIVE.fetch_sub(1, SeqCst);
    *run * 10 + seen
}
";

/// Shows each value. It has no domain, so it runs in the domain of its input (9.1).
const SAY: &str = "#[patina_sdk::node]
pub fn say(value: &u64) {
    println!(\"{value}\");
}
";

/// Shows the text of an event, then fails.
const HEAR: &str = "#[patina_sdk::node]
pub fn hear(text: &String) -> Result<(), String> {
    println!(\"[{text}]\");
    Err(\"heard\".to_owned())
}
";

fn node(id: &str, kind: &str) -> String {
    let kind = if ["schedule", "trigger"].contains(&kind) { format!("builtin = \"{kind}\"") } else { format!("folder = \"nodes/{kind}\"") };
    format!("[[node]]\nid = \"00000000000000{id}\"\n{kind}\n\n")
}

fn wire(from: &str, to: &str) -> String {
    format!("[[wire]]\nfrom = \"00000000000000{from}\"\nto = \"00000000000000{to}\"\n\n")
}

#[test]
fn a_schedule_starts_runs_and_its_overlap_rule_holds() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), "events");
    for (name, code, event) in [("wait", WAIT, true), ("say", SAY, false), ("hear", HEAR, false)] {
        let folder = project.join("nodes").join(name);
        fs::create_dir_all(folder.join("src")).unwrap();
        let more = if event { "patina-event = { workspace = true }\n" } else { "" };
        let manifest = format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\npatina-sdk = {{ workspace = true }}\n{more}");
        fs::write(folder.join("Cargo.toml"), manifest).unwrap();
        fs::write(folder.join("src/lib.rs"), code).unwrap();
    }
    let main = project.join("graphs/main.toml");
    let graph = |params: &str| {
        let schedule = format!("{}[node.params]\n{params}\n\n", node("e0", "schedule"));
        format!("format = 1\n\n{schedule}{}{}{}{}", node("e1", "wait"), node("e2", "say"), wire("e0.out", "e1.run"), wire("e1.out", "e2.value"))
    };
    let lines = |stdout: &str| {
        let mut lines: Vec<u64> = stdout.lines().map(|l| l.trim().parse().unwrap()).collect();
        lines.sort_unstable();
        lines
    };

    // With `both`, runs 2 and 3 start while run 1 waits. The schedule gives the run ID. Run 3
    // sees run 2 too when the two fires come close together.
    fs::write(&main, graph("every_ms = \"20\"\noverlap = \"both\"")).unwrap();
    let (output, stdout, stderr) = run(&project, &["--events", "3"]);
    assert!(output.status.success(), "{stderr}");
    let both = lines(&stdout);
    assert!(both[..2] == [11, 22] && [32, 33].contains(&both[2]), "{both:?}");

    // With `skip`, the fires during run 1 do not start a run. Each run is alone.
    fs::write(&main, graph("every_ms = \"20\"\noverlap = \"skip\"")).unwrap();
    let (output, stdout, stderr) = run(&project, &["--events", "3"]);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(lines(&stdout), [11, 21, 31]);

    // With `queue`, the fires wait. At 16 runs, the trigger drops a fire and reports it.
    fs::write(&main, graph("every_ms = \"5\"")).unwrap();
    let (output, stdout, stderr) = run(&project, &["--events", "17"]);
    assert!(!output.status.success() && stderr.contains("so it dropped a fire"), "{stderr}");
    assert_eq!(lines(&stdout), (1..=17).map(|id| id * 10 + 1).collect::<Vec<_>>());

    // The engine reads the settings, so a bad setting stops the graph before a run.
    fs::write(&main, graph("every_ms = \"0\"")).unwrap();
    let (output, stdout, stderr) = run(&project, &["--events", "1"]);
    assert!(!output.status.success() && stdout.is_empty() && stderr.contains("bad parameter: input `every_ms`"), "{stderr}");

    // A trigger fires one time with an empty text, then the run ends. A report shows the run ID.
    let trigger = format!("format = 1\n\n{}{}{}", node("e0", "trigger"), node("e1", "hear"), wire("e0.out", "e1.text"));
    fs::write(&main, trigger).unwrap();
    let (output, stdout, stderr) = run(&project, &[]);
    assert!(!output.status.success() && stderr.contains("Run 1: Node 00000000000000e1") && stderr.contains("heard"), "{stderr}");
    assert_eq!(stdout.trim(), "[]");

    // An event node needs a trigger.
    fs::write(&main, format!("format = 1\n\n{}", node("e1", "wait"))).unwrap();
    let (output, _, stderr) = run(&project, &[]);
    assert!(!output.status.success() && stderr.contains("connect a wire or an order wire from a schedule"), "{stderr}");
}
