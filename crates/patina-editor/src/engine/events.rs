//! The live event runs (spec 9.4, 15.2): the event domain runs on its own threads while the engine
//! thread runs the frames and the audio thread runs the blocks.

use std::sync::atomic::{AtomicBool, Ordering};

use patina_engine::clock::{self, Clock};
use patina_engine::{Audio, EventRun, Events, Frames, NodeLog, Outcome, Report, Value};
use tauri::Emitter;

use super::{Event, Job, Live, alias, badges, frames_only, keep, lock, split};

/// Runs the event domain until a job comes or the user stops the run, with the frames when the
/// graph has frame nodes and with the blocks when `audio` is there. At the end, no new event run
/// starts, and the engine waits for the active runs (15.2). Returns the job that ended the run.
pub(super) fn run(live: &mut Live, part: &mut Frames, audio: Option<Audio>, events: Events, frames: bool) -> Option<Job> {
    let (remote, inbox) = patina_engine::remote();
    let (app, shared, serial, aliases) = (live.app, live.shared, live.serial, live.aliases);
    *lock(&shared.remote) = Some(remote.clone());
    let ended = AtomicBool::new(false);
    std::thread::scope(|s| {
        let dispatch = std::thread::Builder::new().name("patina-events".to_owned()).spawn_scoped(s, || {
            events.run(u64::MAX, &shared.control, Some(inbox), |run| done(app, shared, serial, aliases, run));
            ended.store(true, Ordering::Relaxed);
        });
        if let Err(e) = dispatch {
            warn(live, format!("The event runs did not start: {e}"));
            ended.store(true, Ordering::Relaxed);
        }
        let job = match (audio, frames) {
            (Some(audio), _) => split(live, part, audio),
            (None, true) => frames_only(live, part),
            (None, false) => jobs_only(live, part, &ended),
        };
        *lock(&shared.remote) = None;
        remote.end();
        job
    })
}

/// Sends a warning line of Patina to the output, as a report of no event run.
pub(super) fn warn(live: &Live, text: String) {
    let log = NodeLog { id: "events".to_owned(), folder: "Patina".to_owned(), level: "WARN".to_owned(), text };
    let out = Outcome { ok: true, logs: vec![log], ..Outcome::default() };
    send(live.app, live.serial, &EventRun { id: 0, trigger: String::new(), out }, Vec::new());
}

/// Takes the jobs of a graph with only event nodes after batch, until a job ends the run or the
/// event runs end.
fn jobs_only(live: &mut Live, part: &mut Frames, ended: &AtomicBool) -> Option<Job> {
    let mut clock = Clock::new(clock::FRAME);
    loop {
        clock.wait();
        if let Some(end) = live.job(part) {
            return end;
        }
        if ended.load(Ordering::Relaxed) {
            return None;
        }
    }
}

/// Keeps the values and the node errors of an event run for the **Inspector** and the views, and
/// sends the run to the page.
fn done(app: &tauri::AppHandle, shared: &super::Shared, serial: u64, aliases: &[(String, String)], run: &EventRun) {
    let mut values = run.out.values.clone();
    let extra = alias(&values, aliases);
    values.extend(extra);
    keep(&mut lock(&shared.values), &values);
    merge(&mut lock(&shared.event_reports), &run.out);
    send(app, serial, run, badges(&values));
}

fn send(app: &tauri::AppHandle, serial: u64, run: &EventRun, values: Vec<Value>) {
    let out = &run.out;
    let (id, trigger, ok) = (run.id, run.trigger.clone(), out.ok);
    let (skipped, stopped, times, logs, reports) = (out.skipped.clone(), out.stopped.clone(), out.times.clone(), out.logs.clone(), out.reports.clone());
    let _ = app.emit("engine", Event::Fired { serial, id, trigger, ok, skipped, stopped, times, logs, reports, values });
}

/// Puts the reports of an event run in place of the old reports of the nodes that the run reached.
fn merge(kept: &mut Vec<Report>, out: &Outcome) {
    let reached = |id: &str| {
        out.times.iter().any(|t| t.id == id) || out.skipped.iter().chain(&out.stopped).any(|s| s == id) || out.reports.iter().any(|r| r.id == id)
    };
    kept.retain(|r| !reached(&r.id));
    kept.extend_from_slice(&out.reports);
}

#[cfg(test)]
mod tests {
    use patina_engine::NodeTime;

    use super::*;

    #[test]
    fn a_run_replaces_only_the_reports_of_the_nodes_it_reached() {
        let report = |id: &str, text: &str| Report { id: id.to_owned(), text: text.to_owned(), ..Report::default() };
        let mut kept = vec![report("a", "old"), report("b", "old"), report("c", "old")];
        let time = NodeTime { id: "a".to_owned(), ms: 1.0 };
        let out = Outcome { times: vec![time], skipped: vec!["b".to_owned()], reports: vec![report("d", "new")], ..Outcome::default() };
        merge(&mut kept, &out);
        let texts: Vec<(&str, &str)> = kept.iter().map(|r| (r.id.as_str(), r.text.as_str())).collect();
        assert_eq!(texts, [("c", "old"), ("d", "new")]);
    }
}
