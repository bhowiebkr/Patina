//! Runs a Patina project from the command line (spec 3).

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use patina_engine::{Control, Outcome, Project, Toolchain};

const USAGE: &str = "usage: patina-cli run <project folder>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [command, folder] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if command != "run" {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match run(Path::new(folder)) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Builds the main graph and runs it one time. Returns false when a node failed.
fn run(folder: &Path) -> Result<bool, patina_engine::Error> {
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
    let params = cluster.set_params(project.graph());
    let ok = show(&params);
    // A node with a bad parameter does not run, so the graph does not run.
    Ok(ok && show(&cluster.run(&Control::default())))
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
