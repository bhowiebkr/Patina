//! The outcome of one call into a cluster: the reports of the nodes, the live values, the
//! times and the logs. The loader, the scheduler and the event threads fill it.

use std::fmt;

use patina_sdk::glue;
use serde::Serialize;

use crate::library::Sink;

/// The kind of a problem of one node (4.7, 4.8).
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ReportKind {
    /// The node returned an `Err` (4.7).
    #[default]
    Error,
    /// The node panicked (4.7).
    Panic,
    /// A parameter text did not read back (4.8).
    BadParam,
}

/// A problem of one node during a call into the cluster.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Report {
    pub id: String,
    /// The node folder name.
    pub folder: String,
    pub kind: ReportKind,
    pub text: String,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            ReportKind::Error => "error: ",
            ReportKind::Panic => "",
            ReportKind::BadParam => "bad parameter: ",
        };
        write!(f, "Node {} ({}): {kind}{}", self.id, self.folder, self.text)
    }
}

/// The live value of an output after a run (15.9).
#[derive(Serialize, Clone, Debug, Default)]
pub struct Value {
    /// The output, as "<node id>.<output key>".
    pub from: String,
    /// The `Debug` text of the value, or its type name and size (5.3).
    pub text: String,
    /// The lanes from the connection kind of the type (6.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lanes: Option<Lanes>,
    /// The thumbnail of the `Viewer` (5.3): "<width> <height> <base64 RGBA>", or an empty text.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub thumb: String,
    /// The widget lines of `Viewer::show` (5.3), or an empty text.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub view: String,
    /// The samples of `Viewer::wave` (15.9): integers from -100 to 100 with a space between them,
    /// or an empty text.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub wave: String,
}

/// Reads the text of a `THUMB` report into `thumb`, and keeps its buffer. A defective cluster can
/// send a bad text, which gives no thumbnail.
fn read_thumb(text: Option<&str>, thumb: &mut String) {
    let valid = |text: &str| {
        let mut parts = text.splitn(3, ' ');
        let side = |part: Option<&str>| part.and_then(|p| p.parse::<u32>().ok()).filter(|n| (1..=patina_sdk::Thumbnail::MAX).contains(n));
        let (Some(width), Some(height), Some(data)) = (side(parts.next()), side(parts.next()), parts.next()) else { return false };
        let base64 = |b: u8| b.is_ascii_alphanumeric() || b"+/=".contains(&b);
        data.len() == (width * height * 4).div_ceil(3) as usize * 4 && data.bytes().all(base64)
    };
    put(thumb, text.filter(|t| valid(t)).unwrap_or_default());
}

/// Reads the text of a `WAVE` report into `wave`, and keeps its buffer. A defective cluster can
/// send a bad text, which gives no wave.
fn read_wave(text: Option<&str>, wave: &mut String) {
    let sample = |part: &str| part.parse::<i8>().is_ok_and(|v| (-100..=100).contains(&v));
    let valid = |text: &str| text.split(' ').count() <= glue::WAVE_LEN && text.split(' ').all(sample);
    put(wave, text.filter(|t| valid(t)).unwrap_or_default());
}

/// The lanes of a live value (6.3).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Lanes {
    /// One '0' or '1' for each lane, the most significant lane first.
    pub bits: String,
    /// The name of the bus, or an empty text.
    pub label: String,
}

impl Lanes {
    /// Reads the text of a `LANES` report into `lanes`, and keeps the texts of old lanes. A
    /// defective cluster can send a bad text.
    fn read(text: Option<&str>, lanes: &mut Option<Lanes>) {
        let valid = |bits: &str| (1..=64).contains(&bits.len()) && bits.bytes().all(|b| b == b'0' || b == b'1');
        match (text.and_then(|t| t.split_once(' ')).filter(|(bits, _)| valid(bits)), lanes.as_mut()) {
            (Some((bits, label)), Some(old)) => {
                put(&mut old.bits, bits);
                put(&mut old.label, label);
            }
            (Some((bits, label)), None) => *lanes = Some(Lanes { bits: bits.to_owned(), label: label.to_owned() }),
            (None, _) => *lanes = None,
        }
    }
}

/// The run time of one node call (15.7).
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct NodeTime {
    pub id: String,
    pub ms: f64,
    /// The start of the call in milliseconds after the start of the run of its domain.
    pub start: f64,
    /// The worker pool thread of the call. The thread that starts the run is 0.
    pub thread: u32,
}

/// A log line of a node, from the `log` crate (15.7).
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct NodeLog {
    pub id: String,
    /// The node folder name.
    pub folder: String,
    /// "ERROR", "WARN" or "INFO".
    pub level: String,
    pub text: String,
}

impl fmt::Display for NodeLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node {} ({}): {} {}", self.id, self.folder, self.level, self.text)
    }
}

/// The result of one call: false when a node failed or the cluster failed with no report.
#[derive(Clone, Default)]
pub struct Outcome {
    pub ok: bool,
    pub reports: Vec<Report>,
    pub values: Vec<Value>,
    /// The ID of each node that did not run, because a wired input had no value (15.3).
    pub skipped: Vec<String>,
    /// The ID of each node that did not start, because the user clicked **Stop** (15.2).
    pub stopped: Vec<String>,
    /// The run time of each node call that started (15.7).
    pub times: Vec<NodeTime>,
    /// The log lines of the nodes, in the order of the run (15.7).
    pub logs: Vec<NodeLog>,
    /// The parameter writes of a call of `ui`, as the key and the text, for the node of the call (4.3).
    pub params: Vec<(String, String)>,
}

/// The items that `fill` took off the ends of the lists of an outcome. The next `fill` uses them
/// again, so a quiet frame (9.1) with no values does not free the texts of the values.
#[derive(Default)]
pub(crate) struct Spare {
    reports: Vec<Report>,
    values: Vec<Value>,
    ids: Vec<String>,
    times: Vec<NodeTime>,
    logs: Vec<NodeLog>,
}

/// Splits the reports of one call into the problems of nodes and the live values. It fills the
/// lists of `out` again and keeps their texts in `out` and `spare`, so it does not allocate after
/// the first frames.
pub(crate) fn fill(out: &mut Outcome, spare: &mut Spare, ok: bool, sink: &Sink, names: &[(String, String)], probes: &[String]) {
    let (mut reports, mut values, mut skipped, mut stopped, mut times, mut logs) = (0, 0, 0, 0, 0, 0);
    let mut items = sink.iter().peekable();
    while let Some((index, kind, text)) = items.next() {
        let name = names.get(index as usize);
        match kind {
            glue::SKIPPED | glue::STOPPED => {
                let (list, used) = if kind == glue::SKIPPED { (&mut out.skipped, &mut skipped) } else { (&mut out.stopped, &mut stopped) };
                if let Some((id, _)) = name {
                    put(reuse(list, used, &mut spare.ids), id);
                }
            }
            glue::VALUE => {
                // Only a defective cluster sends an unknown index, and its value has no wire to show on.
                let Some(from) = probes.get(index as usize) else { continue };
                let value = reuse(&mut out.values, &mut values, &mut spare.values);
                put(&mut value.from, from);
                put(&mut value.text, text);
                // The thumbnail, the widgets, the wave and the lanes come after the text of the value.
                let thumb = items.next_if(|&(i, k, _)| (i, k) == (index, glue::THUMB));
                read_thumb(thumb.map(|(_, _, text)| text), &mut value.thumb);
                let view = items.next_if(|&(i, k, _)| (i, k) == (index, glue::VIEW));
                put(&mut value.view, view.map_or("", |(_, _, text)| text));
                let wave = items.next_if(|&(i, k, _)| (i, k) == (index, glue::WAVE));
                read_wave(wave.map(|(_, _, text)| text), &mut value.wave);
                let lanes = items.next_if(|&(i, k, _)| (i, k) == (index, glue::LANES));
                Lanes::read(lanes.map(|(_, _, text)| text), &mut value.lanes);
            }
            // Lanes, a thumbnail, widgets or a wave with no value before them come only from a defective
            // cluster. `Cluster::ui` reads the widget lines and the parameter writes.
            glue::LANES | glue::THUMB | glue::VIEW | glue::WAVE | glue::UI | glue::EDIT => {}
            glue::TIME => {
                // The nanoseconds of the call, then the `clock` at its end and the thread (`collect`).
                let mut parts = text.split(' ').map(|part| part.parse::<u64>().ok());
                let (ns, end, thread) = (parts.next().flatten(), parts.next().flatten(), parts.next().flatten());
                // A defective cluster can send a bad text or index. Its time has no node to show on.
                if let (Some((id, _)), Some(ns)) = (name, ns) {
                    let time = reuse(&mut out.times, &mut times, &mut spare.times);
                    put(&mut time.id, id);
                    time.ms = ns as f64 / 1e6;
                    time.start = end.map_or(0, |end| end.saturating_sub(ns).saturating_sub(sink.begin)) as f64 / 1e6;
                    time.thread = thread.and_then(|t| u32::try_from(t).ok()).unwrap_or(0);
                }
            }
            glue::LOG => {
                // A defective cluster can send a bad text or index. Its line has no node to show on.
                if let (Some((id, folder)), Some((level, text))) = (name, text.split_once(' ')) {
                    let log = reuse(&mut out.logs, &mut logs, &mut spare.logs);
                    put(&mut log.id, id);
                    put(&mut log.folder, folder);
                    put(&mut log.level, level);
                    put(&mut log.text, text);
                }
            }
            _ => {
                let report = reuse(&mut out.reports, &mut reports, &mut spare.reports);
                let (id, folder) = name.map_or(("", ""), |(id, folder)| (id.as_str(), folder.as_str()));
                put(&mut report.id, id);
                put(&mut report.folder, folder);
                put(&mut report.text, text);
                report.kind = match kind {
                    glue::PANIC => ReportKind::Panic,
                    glue::BAD_PARAM => ReportKind::BadParam,
                    _ => ReportKind::Error,
                };
            }
        }
    }
    out.ok = ok;
    // In reverse, so that `reuse` gives each item back to its old place, and a value keeps its lanes.
    spare.reports.extend(out.reports.drain(reports..).rev());
    spare.values.extend(out.values.drain(values..).rev());
    spare.ids.extend(out.skipped.drain(skipped..).chain(out.stopped.drain(stopped..)).rev());
    spare.times.extend(out.times.drain(times..).rev());
    spare.logs.extend(out.logs.drain(logs..).rev());
}

/// The next item of `list` to fill: an old item that keeps its texts, a spare item, or a new item.
fn reuse<'a, T: Default>(list: &'a mut Vec<T>, used: &mut usize, spare: &mut Vec<T>) -> &'a mut T {
    if *used == list.len() {
        list.push(spare.pop().unwrap_or_default());
    }
    *used += 1;
    &mut list[*used - 1]
}

/// Puts `text` in `to`, and keeps the capacity of `to`.
fn put(to: &mut String, text: &str) {
    to.clear();
    to.push_str(text);
}

#[cfg(test)]
mod tests {
    use patina_sdk::glue::Reporter;

    use super::*;
    use crate::library::collect;

    /// Spec 18, question 12: the cost of a live value, from its text in the cluster to the
    /// outcome. Run with `--release --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn measure_live_values() {
        use std::fmt::Write;
        let n = 1000u32;
        let names: Vec<(String, String)> = (0..n).map(|i| (format!("n{i}"), "number".to_owned())).collect();
        let probes: Vec<String> = (0..n).map(|i| format!("n{i}.out")).collect();
        let (mut sink, mut out, mut spare) = (Sink::default(), Outcome::default(), Spare::default());
        let mut frame = |sink: &mut Sink, out: &mut Outcome| {
            sink.clear();
            let rep = Reporter { ctx: (&raw mut *sink).cast(), report: collect, control: std::ptr::null() };
            for i in 0..n {
                let text: glue::TextFn<f64> = |v, t| {
                    let _ = write!(t, "{v}");
                };
                glue::send_value(&rep, i, &(f64::from(i) * 0.1), (text, None));
            }
            fill(out, &mut spare, true, sink, &names, &probes);
        };
        for _ in 0..3 {
            frame(&mut sink, &mut out);
        }
        let start = std::time::Instant::now();
        for _ in 0..100 {
            frame(&mut sink, &mut out);
        }
        assert_eq!(out.values.len(), n as usize);
        println!("{n} live values: {:.0} ns each", start.elapsed().as_secs_f64() * 1e9 / f64::from(100 * n));
    }

    #[test]
    fn values_go_apart_from_the_reports() {
        let names = [("a1".to_owned(), "number".to_owned())];
        let probes = ["a1.out".to_owned()];
        let sink = |items: &[(u32, u32, &str)]| {
            let mut sink = Sink::default();
            sink.begin = 1_000_000;
            items.iter().for_each(|&(node, kind, text)| sink.push(node, kind, text));
            sink
        };
        let run = sink(&[
            (0, glue::VALUE, "2.5"),
            (0, glue::THUMB, "1 1 AQIDBA=="),
            (0, glue::VIEW, "label\tx"),
            (0, glue::WAVE, "-100 0 55"),
            (0, glue::LANES, "0110 data bus"),
            (0, glue::THUMB, "1 1 AQIDBA=="),
            (0, glue::VIEW, "label\ty"),
            (0, glue::WAVE, "1"),
            (7, glue::VALUE, "x"),
            (0, glue::PANIC, "p"),
            (0, glue::TIME, "2500000 5000000 3"),
            (0, glue::TIME, "x"),
            (5, glue::TIME, "1"),
            (0, glue::LOG, "INFO two words"),
            (0, glue::LOG, "INFO"),
            (5, glue::LOG, "INFO x"),
        ]);
        let (mut out, mut spare) = (Outcome::default(), Spare::default());
        fill(&mut out, &mut spare, false, &run, &names, &probes);
        assert!(out.skipped.is_empty() && out.stopped.is_empty());
        assert_eq!(out.logs.iter().map(ToString::to_string).collect::<Vec<_>>(), ["Node a1 (number): INFO two words"]);
        assert_eq!(out.times, [NodeTime { id: "a1".to_owned(), ms: 2.5, start: 1.5, thread: 3 }]);
        assert_eq!(out.values.len(), 1);
        assert_eq!((out.values[0].from.as_str(), out.values[0].text.as_str()), ("a1.out", "2.5"));
        assert_eq!(out.values[0].lanes, Some(Lanes { bits: "0110".to_owned(), label: "data bus".to_owned() }));
        assert_eq!(out.values[0].thumb, "1 1 AQIDBA==");
        assert_eq!(out.values[0].view, "label\tx");
        assert_eq!(out.values[0].wave, "-100 0 55");
        let long = vec!["1"; glue::WAVE_LEN + 1].join(" ");
        for bad in [None, Some(""), Some("101"), Some("-101 0"), Some("1  2"), Some("1.5"), Some(" 1"), Some(long.as_str())] {
            let mut wave = out.values[0].wave.clone();
            read_wave(bad, &mut wave);
            assert_eq!(wave, "", "{bad:?}");
        }
        for bad in [None, Some("1 1"), Some("0 1 "), Some("65 1 AQIDBA=="), Some("1 1 AQIDBA="), Some("1 1 AQ.DBA=="), Some("x 1 AQIDBA==")] {
            let mut thumb = out.values[0].thumb.clone();
            read_thumb(bad, &mut thumb);
            assert_eq!(thumb, "", "{bad:?}");
        }
        for bad in [None, Some("0110"), Some("012 x"), Some(" x"), Some(&format!("{} x", "1".repeat(65)))] {
            let mut lanes = out.values[0].lanes.clone();
            Lanes::read(bad, &mut lanes);
            assert_eq!(lanes, None, "{bad:?}");
        }
        assert_eq!(out.reports.len(), 1);
        assert_eq!((out.reports[0].folder.as_str(), out.reports[0].kind), ("number", ReportKind::Panic));

        // A second fill keeps the texts of the first fill. A shorter list keeps the rest as spare
        // items, and a longer list uses them again.
        let text = out.values[0].text.as_ptr();
        fill(&mut out, &mut spare, false, &run, &names, &probes);
        assert_eq!(out.values[0].text.as_ptr(), text, "the text of the value keeps its buffer");
        fill(&mut out, &mut spare, true, &sink(&[(0, glue::SKIPPED, ""), (0, glue::STOPPED, ""), (9, glue::STOPPED, "")]), &names, &probes);
        assert!(out.ok && out.values.is_empty() && out.reports.is_empty() && out.times.is_empty() && out.logs.is_empty());
        assert_eq!((&out.skipped[..], &out.stopped[..]), (&["a1".to_owned()][..], &["a1".to_owned()][..]));
        fill(&mut out, &mut spare, false, &run, &names, &probes);
        assert_eq!(out.values[0].text.as_ptr(), text, "the value of a spare item keeps its buffer");
    }

    #[test]
    fn a_quiet_frame_gives_each_value_its_old_item_again() {
        let names = [("a1".to_owned(), "number".to_owned()), ("a2".to_owned(), "number".to_owned())];
        let probes = ["a1.out".to_owned(), "a2.out".to_owned()];
        let mut run = Sink::default();
        run.push(0, glue::VALUE, "6");
        run.push(0, glue::LANES, "0110 bus");
        run.push(1, glue::VALUE, "1");
        let (mut out, mut spare) = (Outcome::default(), Spare::default());
        fill(&mut out, &mut spare, true, &run, &names, &probes);
        let bits = out.values[0].lanes.as_ref().map(|l| l.bits.as_ptr());
        // A quiet frame sends no values, so all items go to the spare list.
        fill(&mut out, &mut spare, true, &Sink::default(), &names, &probes);
        fill(&mut out, &mut spare, true, &run, &names, &probes);
        assert_eq!(out.values[0].lanes.as_ref().map(|l| l.bits.as_ptr()), bits, "the lanes keep their buffer");
        assert_eq!((out.values[1].from.as_str(), &out.values[1].lanes), ("a2.out", &None));
    }
}
