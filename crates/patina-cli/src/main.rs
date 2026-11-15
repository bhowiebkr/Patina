//! Runs a Patina project from the command line (spec 3).

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use patina_engine::clock::{self, Clock};
use patina_engine::{Control, Mode, Outcome, Project, Toolchain};

const USAGE: &str = "usage: patina-cli run <project folder> [--frames <count>] [--edit]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((folder, frames, mode)) = parse(&args) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match run(Path::new(folder), frames, mode) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Reads the arguments: the project folder, the number of frames and the build mode. `--edit`
/// makes a fast build in edit mode, else the build is in bake mode (8.3).
fn parse(args: &[String]) -> Option<(&String, Option<u64>, Mode)> {
    let [command, folder, flags @ ..] = args else { return None };
    let (mut frames, mut mode) = (None, Mode::Bake);
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--frames" if frames.is_none() => frames = Some(flags.next()?.parse().ok()?),
            "--edit" if mode == Mode::Bake => mode = Mode::Edit,
            _ => return None,
        }
    }
    (command == "run").then_some((folder, frames, mode))
}

/// Builds the main graph and runs its batch domain one time. Then it runs `frames` frames, or
/// frames until the user stops the process. Returns false when a node failed.
fn run(folder: &Path, frames: Option<u64>, mode: Mode) -> Result<bool, patina_engine::Error> {
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
    let mut cluster = checked.build(&toolchain, mode)?;
    eprintln!("Built in {:.2} s", start.elapsed().as_secs_f64());
    let (params, control) = (cluster.set_params(checked.graph()), Control::default());
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
        if !show(cluster.frame(&control)) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flags_come_in_any_order_one_time_each() {
        let args = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        let parsed = |text: &str| parse(&args(text)).map(|(folder, frames, mode)| (folder.clone(), frames, mode));
        assert_eq!(parsed("run p"), Some(("p".to_owned(), None, Mode::Bake)));
        assert_eq!(parsed("run p --edit --frames 3"), Some(("p".to_owned(), Some(3), Mode::Edit)));
        assert_eq!(parsed("run p --frames 3 --edit"), Some(("p".to_owned(), Some(3), Mode::Edit)));
        for bad in ["run", "go p", "run p --frames", "run p --frames x", "run p --edit --edit", "run p --fast"] {
            assert_eq!(parsed(bad), None, "{bad}");
        }
    }
}
