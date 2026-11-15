//! Runs a Patina project from the command line (spec 3).

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use patina_engine::clock::{self, Clock};
use patina_engine::{Control, Outcome, Project, Toolchain};

const USAGE: &str = "usage: patina-cli run <project folder> [--frames <count>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = match args.as_slice() {
        [command, folder] if command == "run" => Some((folder, None)),
        [command, folder, flag, count] if command == "run" && flag == "--frames" => {
            count.parse().ok().map(|count| (folder, Some(count)))
        }
        _ => None,
    };
    let Some((folder, frames)) = parsed else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match run(Path::new(folder), frames) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Builds the main graph and runs its batch domain one time. Then it runs `frames` frames, or
/// frames until the user stops the process. Returns false when a node failed.
fn run(folder: &Path, frames: Option<u64>) -> Result<bool, patina_engine::Error> {
    let project = Project::open(folder)?;
    let checked = project.check()?;
    // shortcut: only the toolchain folder next to the CLI (portable edition), change it with the installers.
    let folder = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("toolchain")));
    let toolchain = match folder {
        Some(folder) => Toolchain::find(&folder)?,
        None => Toolchain::detect()?,
    };
    eprintln!("Building with Rust {}...", toolchain.release);
    let start = Instant::now();
    let mut cluster = checked.build(&toolchain)?;
    eprintln!("Built in {:.2} s", start.elapsed().as_secs_f64());
    let (params, control) = (cluster.set_params(project.graph()), Control::default());
    // A node with a bad parameter does not run, so the graph does not run.
    if !show(&params) || !show(&cluster.run(&control)) {
        return Ok(false);
    }
    let count = if cluster.frames() { frames.unwrap_or(u64::MAX) } else { 0 };
    let mut clock = Clock::new(clock::FRAME);
    for frame in 0..count {
        if frame > 0 {
            clock.wait();
        }
        if !show(&cluster.frame(&control)) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn show(outcome: &Outcome) -> bool {
    for line in &outcome.logs {
        eprintln!("{line}");
    }
    for report in &outcome.reports {
        eprintln!("{report}");
    }
    for id in &outcome.skipped {
        eprintln!("Node {id}: skipped: an input has no value");
    }
    if !outcome.ok && outcome.reports.is_empty() {
        eprintln!("error: the cluster failed with no report");
    }
    outcome.ok
}
