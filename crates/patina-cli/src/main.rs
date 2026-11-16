//! Runs a Patina project or a package from the command line (spec 3 and 14).

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use patina_engine::clock::{self, Clock};
use patina_engine::project::GraphFile;
use patina_engine::{Cluster, Control, Mode, Outcome, Project, Toolchain, package};

const USAGE: &str = "usage: patina-cli run <project folder> [--frames <count>] [--blocks <count>] [--edit]
       patina-cli package <project folder> <package folder> [--edit]";

/// The number of frames and of audio blocks to run. `None` runs until the user stops the process.
#[derive(Debug, PartialEq, Default, Clone, Copy)]
struct Counts {
    frames: Option<u64>,
    blocks: Option<u64>,
}

#[derive(Debug, PartialEq)]
enum Command<'a> {
    Run { folder: &'a str, counts: Counts, mode: Mode },
    Package { folder: &'a str, out: &'a str, mode: Mode },
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // A copy of the CLI in a package runs that package (14).
    let dir = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.to_owned()));
    let result = match dir.filter(|dir| dir.join(package::HEADER).is_file()) {
        Some(dir) => run_package(&dir, &args),
        None => match parse(&args) {
            Some(Command::Run { folder, counts, mode }) => run(Path::new(folder), counts, mode),
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
/// (8.3). Only `run` takes `--frames` and `--blocks`.
fn parse(args: &[String]) -> Option<Command<'_>> {
    let (command, rest) = args.split_first()?;
    let (paths, flags) = rest.split_at(rest.iter().position(|a| a.starts_with("--")).unwrap_or(rest.len()));
    let (mut counts, mut mode) = (Counts::default(), Mode::Bake);
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--frames" if counts.frames.is_none() && command == "run" => counts.frames = Some(flags.next()?.parse().ok()?),
            "--blocks" if counts.blocks.is_none() && command == "run" => counts.blocks = Some(flags.next()?.parse().ok()?),
            "--edit" if mode == Mode::Bake => mode = Mode::Edit,
            _ => return None,
        }
    }
    match (command.as_str(), paths) {
        ("run", [folder]) => Some(Command::Run { folder, counts, mode }),
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
fn run(folder: &Path, counts: Counts, mode: Mode) -> Result<bool, String> {
    let checked = Project::open(folder).and_then(|p| p.check()).map_err(|e| e.to_string())?;
    let toolchain = toolchain()?;
    eprintln!("Building with Rust {}...", toolchain.release);
    let start = Instant::now();
    let mut cluster = checked.build(&toolchain, mode).map_err(|e| e.to_string())?;
    eprintln!("Built in {:.2} s", start.elapsed().as_secs_f64());
    Ok(drive(&mut cluster, checked.graph(), counts))
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

/// Runs the package in `dir`. The arguments are `--frames <count>`, `--blocks <count>` and parameters
/// `<node ID>.<input key>=<value>` (14).
fn run_package(dir: &Path, args: &[String]) -> Result<bool, String> {
    let (mut checked, mut cluster) = package::open(dir).map_err(|e| e.to_string())?;
    let (mut counts, mut params) = (Counts::default(), Vec::new());
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let count = match arg.as_str() {
            "--frames" if counts.frames.is_none() => &mut counts.frames,
            "--blocks" if counts.blocks.is_none() => &mut counts.blocks,
            _ => {
                params.push(arg.clone());
                continue;
            }
        };
        *count = Some(args.next().and_then(|n| n.parse().ok()).ok_or_else(|| format!("{arg} needs a count"))?);
    }
    package::set_args(&mut checked, &params)?;
    Ok(drive(&mut cluster, checked.graph(), counts))
}

/// Runs the batch domain one time. Then it runs the frames and the audio blocks of `counts`.
/// With audio, the frames run between the blocks on the block clock. Returns false when a node
/// failed.
fn drive(cluster: &mut Cluster, graph: &GraphFile, counts: Counts) -> bool {
    let control = Control::default();
    // A node with a bad parameter does not run, so the graph does not run.
    if !show(&cluster.set_params(graph)) || !show(&cluster.run(&control)) {
        return false;
    }
    let frames = if cluster.frames() { counts.frames.unwrap_or(u64::MAX) } else { 0 };
    if !cluster.blocks() {
        let mut clock = Clock::new(clock::FRAME);
        return (0..frames).all(|frame| {
            if frame > 0 {
                clock.wait();
            }
            show(cluster.frame(&control))
        });
    }
    // shortcut: a timer clock runs the audio blocks, with no audio device. Use the device in step 2 of milestone 11.
    let mut clock = Clock::new(clock::BLOCK);
    let mut frame = 0;
    for block in 0..counts.blocks.unwrap_or(u64::MAX) {
        if block > 0 {
            clock.wait();
        }
        while frame < frames && frame_due(frame, block) {
            if !show(cluster.frame(&control)) {
                return false;
            }
            frame += 1;
        }
        if !show(cluster.block(&control)) {
            return false;
        }
    }
    true
}

/// True when frame `frame` starts at or before the start of audio block `block`.
fn frame_due(frame: u64, block: u64) -> bool {
    u128::from(frame) * clock::FRAME.as_nanos() <= u128::from(block) * clock::BLOCK.as_nanos()
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
            Some(Command::Run { folder, counts, mode }) => Some((folder.to_owned(), counts.frames, counts.blocks, mode)),
            _ => None,
        };
        assert_eq!(run("run p"), Some(("p".to_owned(), None, None, Mode::Bake)));
        assert_eq!(run("run p --edit --frames 3"), Some(("p".to_owned(), Some(3), None, Mode::Edit)));
        assert_eq!(run("run p --blocks 9 --frames 3 --edit"), Some(("p".to_owned(), Some(3), Some(9), Mode::Edit)));
        let bad = ["run", "go p", "run p --frames", "run p --frames x", "run p --edit --edit", "run p --fast", "run p q"];
        for bad in bad.into_iter().chain(["run p --blocks 1 --blocks 2"]) {
            assert_eq!(run(bad), None, "{bad}");
        }
        let package = args("package p out --edit");
        assert_eq!(parse(&package), Some(Command::Package { folder: "p", out: "out", mode: Mode::Edit }));
        for bad in ["package p", "package p out --frames 3", "package p out --blocks 3", "package p --edit out"] {
            assert_eq!(parse(&args(bad)), None, "{bad}");
        }
    }

    #[test]
    fn a_frame_runs_before_the_first_block_after_its_time() {
        assert!(frame_due(0, 0) && frame_due(1, 4) && !frame_due(1, 3) && frame_due(2, 7) && !frame_due(2, 6));
    }
}
