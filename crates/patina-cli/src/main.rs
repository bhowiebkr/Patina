//! Runs a Patina project or a package from the command line (spec 3 and 14).

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use patina_engine::clock::{self, Clock};
use patina_engine::project::GraphFile;
use patina_engine::{Cluster, Control, Mode, Outcome, Project, Toolchain, package};

const USAGE: &str = "usage: patina-cli run <project folder> [--frames <count>] [--edit]
       patina-cli package <project folder> <package folder> [--edit]";

#[derive(Debug, PartialEq)]
enum Command<'a> {
    Run { folder: &'a str, frames: Option<u64>, mode: Mode },
    Package { folder: &'a str, out: &'a str, mode: Mode },
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // A copy of the CLI in a package runs that package (14).
    let dir = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.to_owned()));
    let result = match dir.filter(|dir| dir.join(package::HEADER).is_file()) {
        Some(dir) => run_package(&dir, &args),
        None => match parse(&args) {
            Some(Command::Run { folder, frames, mode }) => run(Path::new(folder), frames, mode),
            Some(Command::Package { folder, out, mode }) => make(Path::new(folder), Path::new(out), mode),
            None => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        },
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Reads the arguments. `--edit` makes a fast build in edit mode, else the build is in bake mode
/// (8.3). Only `run` takes `--frames`.
fn parse(args: &[String]) -> Option<Command<'_>> {
    let (command, rest) = args.split_first()?;
    let (paths, flags) = rest.split_at(rest.iter().position(|a| a.starts_with("--")).unwrap_or(rest.len()));
    let (mut frames, mut mode) = (None, Mode::Bake);
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--frames" if frames.is_none() && command == "run" => frames = Some(flags.next()?.parse().ok()?),
            "--edit" if mode == Mode::Bake => mode = Mode::Edit,
            _ => return None,
        }
    }
    match (command.as_str(), paths) {
        ("run", [folder]) => Some(Command::Run { folder, frames, mode }),
        ("package", [folder, out]) => Some(Command::Package { folder, out, mode }),
        _ => None,
    }
}

/// The toolchain folder next to the CLI (portable edition), or the Rust of the system.
fn toolchain() -> Result<Toolchain, String> {
    // shortcut: only the toolchain folder next to the CLI (portable edition), change it with the installers.
    let folder = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("toolchain")));
    match folder {
        Some(folder) => Toolchain::find(&folder),
        None => Toolchain::detect(),
    }
    .map_err(|e| e.to_string())
}

/// Builds the main graph and runs it. Returns false when a node failed.
fn run(folder: &Path, frames: Option<u64>, mode: Mode) -> Result<bool, String> {
    let checked = Project::open(folder).and_then(|p| p.check()).map_err(|e| e.to_string())?;
    let toolchain = toolchain()?;
    eprintln!("Building with Rust {}...", toolchain.release);
    let start = Instant::now();
    let mut cluster = checked.build(&toolchain, mode).map_err(|e| e.to_string())?;
    eprintln!("Built in {:.2} s", start.elapsed().as_secs_f64());
    Ok(drive(&mut cluster, checked.graph(), frames))
}

/// Builds the project and writes a package to the folder `out` (14).
fn make(folder: &Path, out: &Path, mode: Mode) -> Result<bool, String> {
    let toolchain = toolchain()?;
    let runner = std::env::current_exe().map_err(|e| e.to_string())?;
    eprintln!("Building with Rust {}...", toolchain.release);
    let warnings = package::make(folder, &toolchain, mode, &runner, out).map_err(|e| e.to_string())?;
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
    eprintln!("Made the package {}", out.display());
    Ok(true)
}

/// Runs the package in `dir`. The arguments are `--frames <count>` and parameters
/// `<node ID>.<input key>=<value>` (14).
fn run_package(dir: &Path, args: &[String]) -> Result<bool, String> {
    let (mut checked, mut cluster) = package::open(dir).map_err(|e| e.to_string())?;
    let (mut frames, mut params) = (None, Vec::new());
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--frames" && frames.is_none() {
            frames = Some(args.next().and_then(|n| n.parse().ok()).ok_or("--frames needs a count")?);
        } else {
            params.push(arg.clone());
        }
    }
    package::set_args(&mut checked, &params)?;
    Ok(drive(&mut cluster, checked.graph(), frames))
}

/// Runs the batch domain one time. Then it runs `frames` frames, or frames until the user stops
/// the process. Returns false when a node failed.
fn drive(cluster: &mut Cluster, graph: &GraphFile, frames: Option<u64>) -> bool {
    let control = Control::default();
    // A node with a bad parameter does not run, so the graph does not run.
    if !show(&cluster.set_params(graph)) || !show(&cluster.run(&control)) {
        return false;
    }
    let count = if cluster.frames() { frames.unwrap_or(u64::MAX) } else { 0 };
    let mut clock = Clock::new(clock::FRAME);
    for frame in 0..count {
        if frame > 0 {
            clock.wait();
        }
        if !show(cluster.frame(&control)) {
            return false;
        }
    }
    true
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flags_come_in_any_order_one_time_each() {
        let args = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        let run = |text: &str| match parse(&args(text)) {
            Some(Command::Run { folder, frames, mode }) => Some((folder.to_owned(), frames, mode)),
            _ => None,
        };
        assert_eq!(run("run p"), Some(("p".to_owned(), None, Mode::Bake)));
        assert_eq!(run("run p --edit --frames 3"), Some(("p".to_owned(), Some(3), Mode::Edit)));
        assert_eq!(run("run p --frames 3 --edit"), Some(("p".to_owned(), Some(3), Mode::Edit)));
        for bad in ["run", "go p", "run p --frames", "run p --frames x", "run p --edit --edit", "run p --fast", "run p q"] {
            assert_eq!(run(bad), None, "{bad}");
        }
        let package = args("package p out --edit");
        assert_eq!(parse(&package), Some(Command::Package { folder: "p", out: "out", mode: Mode::Edit }));
        for bad in ["package p", "package p out --frames 3", "package p --edit out"] {
            assert_eq!(parse(&args(bad)), None, "{bad}");
        }
    }
}
