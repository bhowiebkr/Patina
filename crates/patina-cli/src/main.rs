//! Runs a Patina project or a package from the command line (spec 3 and 14).


use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::time::{Duration, Instant};

use patina_engine::clock::{self, Clock, frame_due};
use patina_engine::device::{Notice, Pace, raise_priority};
use patina_engine::project::GraphFile;
use patina_engine::{Audio, Cluster, Control, EventRun, Events, Frames, Mode, Outcome, Project, Toolchain, package};

mod interrupt;

const USAGE: &str = "usage: patina-cli run <project folder> [--frames <count>] [--blocks <count>] [--events <count>] [--edit]
       patina-cli package <project folder> <package folder> [--edit]";

/// The number of frames, of audio blocks and of event runs to run. `None` runs until the user stops the process.
#[derive(Debug, PartialEq, Default, Clone, Copy)]
struct Counts {
    frames: Option<u64>,
    blocks: Option<u64>,
    events: Option<u64>,
}

#[derive(Debug, PartialEq)]
enum Command<'a> {
    Run { folder: &'a str, counts: Counts, mode: Mode },
    Package { folder: &'a str, out: &'a str, mode: Mode },
}

fn main() -> ExitCode {
    if let Some(code) = patina_engine::install::run_wrapper() {
        std::process::exit(code);
    }
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
        Ok(_) if interrupt::stopped() => {
            eprintln!("Stopped by Ctrl+C");
            ExitCode::from(130)
        }
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Reads the arguments. `--edit` makes a fast build in edit mode, else the build is in bake mode
/// (8.3). Only `run` takes `--frames`, `--blocks` and `--events`.
fn parse(args: &[String]) -> Option<Command<'_>> {
    let (command, rest) = args.split_first()?;
    let (paths, flags) = rest.split_at(rest.iter().position(|a| a.starts_with("--")).unwrap_or(rest.len()));
    let (mut counts, mut mode) = (Counts::default(), Mode::Bake);
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--frames" if counts.frames.is_none() && command == "run" => counts.frames = Some(flags.next()?.parse().ok()?),
            "--blocks" if counts.blocks.is_none() && command == "run" => counts.blocks = Some(flags.next()?.parse().ok()?),
            "--events" if counts.events.is_none() && command == "run" => counts.events = Some(flags.next()?.parse().ok()?),
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

/// The toolchain of the toolchain folder, or the Rust of the system (13.3).
fn toolchain() -> Result<Toolchain, String> {
    match std::env::current_exe().ok().and_then(|exe| Some((toolchain_folder(exe.parent()?, dirs::data_local_dir()), exe))) {
        Some((folder, exe)) => {
            if let Err(e) = patina_engine::install::add_wrappers(&folder, &exe) {
                eprintln!("warning: Patina cannot add the Zig wrappers to the toolchain: {e}");
            }
            Toolchain::find(&folder)
        }
        None => Toolchain::detect(),
    }
    .map_err(|e| e.to_string())
}

/// The toolchain folder of the editor (13.1): the folder next to the CLI in the portable edition,
/// else the folder of the user. A toolchain next to the CLI is also used without the marker.
fn toolchain_folder(exe_dir: &Path, local_data: Option<PathBuf>) -> PathBuf {
    let next = exe_dir.join("toolchain");
    if exe_dir.join("portable").is_file() || Toolchain::installed(&next) {
        return next;
    }
    let name = if cfg!(target_os = "linux") { "patina" } else { "Patina" };
    local_data.map_or(next, |base| base.join(name).join("toolchain"))
}

/// Builds the main graph and runs it. Returns false when a node failed.
fn run(folder: &Path, counts: Counts, mode: Mode) -> Result<bool, String> {
    let checked = Project::open(folder).and_then(|p| p.check()).map_err(|e| e.to_string())?;
    let toolchain = toolchain()?;
    eprintln!("Building with Rust {}...", toolchain.release);
    let start = Instant::now();
    let mut cluster = checked.build(&toolchain, mode).map_err(|e| e.to_string())?;
    eprintln!("Built in {:.2} s", start.elapsed().as_secs_f64());
    Ok(drive(&mut cluster, checked.graph(), counts, checked.frame_rate()))
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

/// Runs the package in `dir`. The arguments are `--frames <count>`, `--blocks <count>`,
/// `--events <count>` and parameters
/// `<node ID>.<input key>=<value>` (14).
fn run_package(dir: &Path, args: &[String]) -> Result<bool, String> {
    let (mut checked, mut cluster) = package::open(dir).map_err(|e| e.to_string())?;
    let (mut counts, mut params) = (Counts::default(), Vec::new());
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let count = match arg.as_str() {
            "--frames" if counts.frames.is_none() => &mut counts.frames,
            "--blocks" if counts.blocks.is_none() => &mut counts.blocks,
            "--events" if counts.events.is_none() => &mut counts.events,
            _ => {
                params.push(arg.clone());
                continue;
            }
        };
        *count = Some(args.next().and_then(|n| n.parse().ok()).ok_or_else(|| format!("{arg} needs a count"))?);
    }
    package::set_args(&mut checked, &params)?;
    Ok(drive(&mut cluster, checked.graph(), counts, checked.frame_rate()))
}

/// Runs the batch domain one time. Then it runs the event runs, the frames and the audio blocks of
/// `counts`, with `rate` frames each second. The blocks and the event runs run on their own
/// threads, at the same time as the frames (9.3, 9.4). Returns false when a node failed.
fn drive(cluster: &mut Cluster, graph: &GraphFile, counts: Counts, rate: u32) -> bool {
    interrupt::catch();
    let control: &Control = &interrupt::CONTROL;
    // The CLI shows no values and no run times, so the runs do not send them (9.1).
    control.set_quiet(true);
    // A node with a bad parameter does not run, so the graph does not run.
    if !show(&cluster.set_params(graph)) || !show(&cluster.run(control)) {
        return false;
    }
    let (has_frames, has_blocks) = (cluster.frames(), cluster.blocks());
    if cluster.events() {
        if let Some((mut frame, mut audio, events)) = cluster.live() {
            return together(&mut frame, audio.as_mut(), Some(events), counts, has_frames, has_blocks, rate);
        }
        // The slot library keeps a value that is not `Sync`, and it holds an event node and a node
        // of another domain, so the event runs come first (9.4).
        if !cluster.run_events(counts.events.unwrap_or(u64::MAX), control, |run| _ = show_run(run)) {
            return false;
        }
    }
    if !has_blocks {
        let count = if has_frames { counts.frames } else { Some(0) };
        return frames(|| show(cluster.frame(control)), count, rate, &AtomicBool::new(false));
    }
    let Some((mut frame, mut audio)) = cluster.split() else {
        return between(&mut cluster.frame_part(), counts, has_frames, rate, &AtomicBool::new(false));
    };
    together(&mut frame, Some(&mut audio), None, counts, has_frames, has_blocks, rate)
}

/// Runs the frames on this thread, and the audio blocks and the event runs on their own threads.
/// When the audio domain cannot have its own thread, this thread runs the blocks too. Returns
/// false when a node failed.
fn together(frame: &mut Frames, audio: Option<&mut Audio>, events: Option<Events>, counts: Counts, has_frames: bool, has_blocks: bool, rate: u32) -> bool {
    let control: &Control = &interrupt::CONTROL;
    let here = has_blocks && audio.is_none();
    // A domain without a count runs until each domain with a count is done, or a node fails.
    let counted = [
        (has_frames && counts.frames.is_some()) || (here && counts.blocks.is_some()),
        audio.is_some() && counts.blocks.is_some(),
        events.is_some() && counts.events.is_some(),
    ];
    let left = AtomicUsize::new(counted.iter().filter(|&&c| c).count());
    // The event runs have their own control, so the end of the other domains is not a Ctrl+C.
    let (stop, run_control) = (AtomicBool::new(false), Control::default());
    run_control.set_quiet(true);
    let done = |ok: bool, counted: bool| {
        if !ok || (counted && left.fetch_sub(1, Relaxed) == 1) {
            stop.store(true, Relaxed);
            run_control.set_stop(true);
        }
        ok
    };
    let (done, stop, run_control) = (&done, &stop, &run_control);
    std::thread::scope(|scope| {
        let blocks = audio.map(|audio| scope.spawn(move || done(blocks(audio, counts.blocks, stop), counted[1])));
        let runs = events.map(|events| {
            scope.spawn(move || done(events.run(counts.events.unwrap_or(u64::MAX), run_control, None, |run| _ = show_run(run)), counted[2]))
        });
        let framed = if here {
            done(between(frame, counts, has_frames, rate, stop), counted[0])
        } else {
            !has_frames || done(frames(|| show(frame.frame(control)), counts.frames, rate, stop), counted[0])
        };
        // A Ctrl+C reaches the event runs here.
        while runs.as_ref().is_some_and(|runs| !runs.is_finished()) {
            if interrupt::stopped() {
                run_control.set_stop(true);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // A panic of a thread is a failure of the run.
        let blocked = blocks.is_none_or(|thread| thread.join().unwrap_or(false));
        let ran = runs.is_none_or(|thread| thread.join().unwrap_or(false));
        blocked && ran && framed
    })
}

/// Runs `count` frames on the frame clock, or frames until `stop`. Returns false when a node
/// failed.
fn frames(mut frame: impl FnMut() -> bool, count: Option<u64>, rate: u32, stop: &AtomicBool) -> bool {
    let (mut clock, count, mut done) = (Clock::new(clock::FRAME), count.unwrap_or(u64::MAX), 0_u64);
    for tick in 1.. {
        let due = done.saturating_add(clock::frames_at(tick, rate)).min(count);
        while done < due {
            if stop.load(Relaxed) || interrupt::stopped() {
                return true;
            }
            if !frame() {
                return false;
            }
            done += 1;
        }
        if done == count || stop.load(Relaxed) || interrupt::stopped() {
            return true;
        }
        clock.wait();
    }
    true
}

/// Runs `count` audio blocks, or blocks until `stop`, on the audio thread. Returns false when a
/// node failed.
fn blocks(audio: &mut Audio, count: Option<u64>, stop: &AtomicBool) -> bool {
    raise_priority();
    let (control, mut pace) = (Control::default(), Pace::default());
    control.set_quiet(true);
    for block in 0..count.unwrap_or(u64::MAX) {
        if block > 0 {
            pace.wait();
        }
        if stop.load(Relaxed) || interrupt::stopped() {
            break;
        }
        if !show(audio.block(&control)) {
            return false;
        }
        notice(pace.play(audio.sound().map(|s| s.as_slice())));
    }
    pace.drain();
    true
}

/// Runs the blocks and the frames on one thread, when the audio domain cannot have its own
/// thread (9.3). A frame runs before the first block that starts at or after its time. Returns
/// false when a node failed. It stops at `stop`.
fn between(part: &mut Frames, counts: Counts, has_frames: bool, rate: u32, stop: &AtomicBool) -> bool {
    let (control, mut pace): (&Control, _) = (&interrupt::CONTROL, Pace::default());
    let limit = if has_frames { counts.frames.unwrap_or(u64::MAX) } else { 0 };
    let mut frame = 0;
    for block in 0..counts.blocks.unwrap_or(u64::MAX) {
        // A domain without a count runs until each domain with a count is done.
        if interrupt::stopped() || stop.load(Relaxed) || (counts.blocks.is_none() && has_frames && counts.frames == Some(frame)) {
            break;
        }
        if block > 0 {
            pace.wait();
        }
        while frame < limit && frame_due(frame, block, rate) {
            if !show(part.frame(control)) {
                return false;
            }
            frame += 1;
        }
        if !part.block(control).is_some_and(show) {
            return false;
        }
        notice(pace.play(part.sound().map(|s| s.as_slice())));
    }
    pace.drain();
    let rest = counts.frames.filter(|_| has_frames).map_or(0, |count| count - frame);
    frames(|| show(part.frame(control)), Some(rest), rate, stop)
}

/// Shows a change of the audio device.
fn notice(notice: Option<Notice>) {
    match notice {
        Some(n) if n.warning() => eprintln!("warning: {n}"),
        Some(n) => eprintln!("{n}"),
        None => {}
    }
}

fn show(outcome: &Outcome) -> bool {
    show_with(outcome, "")
}

/// Shows the outcome of an event run. Each line starts with its run ID (9.4).
fn show_run(run: &EventRun) -> bool {
    show_with(&run.out, &if run.id > 0 { format!("Run {}: ", run.id) } else { String::new() })
}

fn show_with(outcome: &Outcome, prefix: &str) -> bool {
    for line in &outcome.logs {
        eprintln!("{prefix}{line}");
    }
    for report in &outcome.reports {
        eprintln!("{prefix}{report}");
    }
    for id in &outcome.skipped {
        eprintln!("{prefix}Node {id}: skipped: an input has no value");
    }
    if !outcome.ok && outcome.reports.is_empty() {
        eprintln!("{prefix}error: the cluster failed with no report");
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
            Some(Command::Run { folder, counts, mode }) => Some((folder.to_owned(), counts.frames, counts.blocks, counts.events, mode)),
            _ => None,
        };
        assert_eq!(run("run p"), Some(("p".to_owned(), None, None, None, Mode::Bake)));
        assert_eq!(run("run p --edit --frames 3"), Some(("p".to_owned(), Some(3), None, None, Mode::Edit)));
        assert_eq!(run("run p --blocks 9 --frames 3 --edit"), Some(("p".to_owned(), Some(3), Some(9), None, Mode::Edit)));
        assert_eq!(run("run p --events 2"), Some(("p".to_owned(), None, None, Some(2), Mode::Bake)));
        let bad = ["run", "go p", "run p --frames", "run p --frames x", "run p --edit --edit", "run p --fast", "run p q"];
        for bad in bad.into_iter().chain(["run p --blocks 1 --blocks 2", "run p --events 1 --events 2"]) {
            assert_eq!(run(bad), None, "{bad}");
        }
        let package = args("package p out --edit");
        assert_eq!(parse(&package), Some(Command::Package { folder: "p", out: "out", mode: Mode::Edit }));
        for bad in ["package p", "package p out --frames 3", "package p out --events 3", "package p --edit out"] {
            assert_eq!(parse(&args(bad)), None, "{bad}");
        }
    }

    #[test]
    fn the_toolchain_folder_is_next_to_a_portable_cli_else_in_the_user_folder() {
        let dir = std::env::temp_dir().join(format!("patina-cli-toolchain-{}", std::process::id()));
        let (exe, user) = (dir.join("bin"), dir.join("local"));
        std::fs::create_dir_all(&exe).unwrap();
        let name = if cfg!(target_os = "linux") { "patina" } else { "Patina" };
        assert_eq!(toolchain_folder(&exe, Some(user.clone())), user.join(name).join("toolchain"));
        assert_eq!(toolchain_folder(&exe, None), exe.join("toolchain"));
        std::fs::write(exe.join("portable"), "").unwrap();
        assert_eq!(toolchain_folder(&exe, Some(user.clone())), exe.join("toolchain"));
        std::fs::remove_file(exe.join("portable")).unwrap();
        let bin = exe.join("toolchain").join("cargo").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(format!("cargo{}", std::env::consts::EXE_SUFFIX)), "").unwrap();
        assert_eq!(toolchain_folder(&exe, Some(user)), exe.join("toolchain"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
