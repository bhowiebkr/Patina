//! The values and the node GUIs that the engine thread sends to the page after a frame (4.3, 15.9).

use std::collections::HashSet;
use std::time::{Duration, Instant};

use patina_engine::clock;
use patina_engine::{Control, Frames, Lanes, NodeLog, NodeTime, Report, Value};
use serde::{Serialize, Serializer};

use super::blocks::Blocks;
use super::{Event, GuiInput, ParamWrite, Shared, Tree, lock};
use crate::node_process::Out;

/// The longest text of a badge, in bytes. A longer text ends with "...".
const BADGE_LEN: usize = 64;
/// The frames send the node times at each 6th frame: 10 Hz at 60 FPS (15.9).
const TIMES_EVERY: u64 = 6;

/// Gives the GUI input to node `input.id`, and sends its new widget lines. Returns `Some` when
/// the change must run the graph again: `Some(true)` when the node is in the frame domain.
pub(super) fn gui(out: &Out, control: &Control, part: &mut Frames, serial: u64, input: &GuiInput, shown: &mut Shown) -> Option<bool> {
    if let Some(node) = (input.mode as usize).checked_sub(1).and_then(|i| shown.sizes.get_mut(i)) {
        *node = (!input.id.is_empty()).then(|| input.id.clone());
    }
    let (tree, done) = part.ui(control, &input.id, input.mode, &input.events);
    let trees = tree.map(|tree| Tree { id: input.id.clone(), mode: input.mode, tree }).into_iter().collect();
    let params = done.params.into_iter().map(|(key, text)| ParamWrite { id: input.id.clone(), key, text }).collect();
    out.emit(&Event::Gui { serial, trees, reports: done.reports, params });
    let frame = part.guis().iter().find(|(id, _)| *id == input.id).map(|&(_, frame)| frame)?;
    (input.rerun && !input.events.is_empty()).then_some(frame)
}

/// The nodes whose GUIs the page shows (4.3).
#[derive(Default)]
pub(super) struct Shown {
    /// The node of the **Inspector** size and the node of the full size.
    pub(super) sizes: [Option<String>; 2],
    /// The nodes in view. None until the page sends them, so that each node draws.
    pub(super) visible: Option<HashSet<String>>,
}

impl Shown {
    /// True when node `id` draws in size `mode`. The node body size draws for the nodes in view,
    /// and for all nodes when `all`.
    fn draws(&self, id: &str, mode: u32, all: bool) -> bool {
        match mode {
            0 => all || self.visible.as_ref().is_none_or(|v| v.contains(id)),
            _ => self.sizes[mode as usize - 1].as_deref() == Some(id),
        }
    }
}

/// Sends the widget lines of the node GUIs (4.3): the node body size of the nodes in view, and
/// the larger sizes of the nodes in `shown`. After a run, `all` draws each node one time, so that
/// each node gets its size.
pub(super) fn draw(out: &Out, control: &Control, part: &mut Frames, serial: u64, shown: &Shown, all: bool) {
    let (mut trees, mut reports) = (Vec::new(), Vec::new());
    for n in 0..part.guis().len() {
        let modes = [0, 1, 2].map(|m| shown.draws(&part.guis()[n].0, m, all));
        if !modes.contains(&true) {
            continue;
        }
        let id = part.guis()[n].0.clone();
        for mode in (0..3).filter(|&m| modes[m as usize]) {
            let (tree, done) = part.ui(control, &id, mode, "");
            reports.extend(done.reports);
            trees.extend(tree.map(|tree| Tree { id: id.clone(), mode, tree }));
        }
    }
    if !trees.is_empty() || !reports.is_empty() {
        out.emit(&Event::Gui { serial, trees, reports, params: Vec::new() });
    }
}

/// Runs `count` frames (9.1), and sends the values that changed in the last frame, the logs of all
/// the frames, the results of `blocks` and the node GUIs. The other frames are quiet: they send no
/// values and no times. A failed frame shows at once. Display frame `show` of the run sends the
/// node times only at 10 Hz (15.9). Then empties `blocks`. Returns false when a frame or a block
/// failed, and the number of frames that ran. After one tick period, the next frame is the last and
/// the frames after it do not run, so the page shows each tick (9.1).
pub(super) fn frame(out: &Out, (part, shown, aliases): (&mut Frames, &Shown, &[(String, String)]), shared: &Shared, (serial, show, count): (u64, u64, u64), blocks: &mut Blocks) -> (bool, u64) {
    let (start, control) = (Instant::now(), &shared.control);
    let (mut quiet_logs, mut ran) = (Vec::new(), 0);
    for left in (0..count.max(1)).rev() {
        let last = last(left, start.elapsed());
        control.set_quiet(!last);
        let outcome = part.frame(control);
        ran += 1;
        if last || !outcome.ok {
            break;
        }
        quiet_logs.extend_from_slice(&outcome.logs);
    }
    control.set_quiet(false);
    let outcome = part.outcome();
    let run_ms = start.elapsed().as_secs_f64() * 1000.0;
    let b = blocks.part();
    let mut kept = lock(&shared.values);
    let changed = keep_all(&mut kept, aliases, outcome.values.iter().chain(b.values));
    let mut reports = lock(&shared.reports);
    reports.clear();
    reports.extend(outcome.reports.iter().chain(b.reports).cloned());
    drop(reports);
    let ok = outcome.ok && b.ok;
    let times = if show % TIMES_EVERY == 1 { [&outcome.times[..], b.times] } else { [&[][..]; 2] };
    out.emit(&FrameEvent {
        serial,
        frames: ran,
        run_ms,
        ok,
        skipped: Joined(&[&outcome.skipped, b.skipped]),
        stopped: Joined(&[&outcome.stopped, b.stopped]),
        times: Joined(&times),
        logs: Joined(&[&quiet_logs, &outcome.logs, b.logs]),
        reports: Joined(&[&outcome.reports, b.reports]),
        values: Badges { kept: &kept, changed: &changed },
    });
    drop(kept);
    blocks.clear();
    draw(out, control, part, serial, shown, false);
    (ok, ran)
}

/// The result of one frame (9.1), with the fields of `Event::Ran`, for the nodes of the frame domain
/// and for the blocks of the audio domain since the last frame (9.3). It is the last of `frames`
/// frames, and `run_ms` is the time of all the frames. It borrows its lists, so that the frame
/// loop does not copy them.
#[derive(Serialize)]
#[serde(tag = "state", rename = "frame", rename_all = "camelCase")]
struct FrameEvent<'a> {
    serial: u64,
    frames: u64,
    run_ms: f64,
    ok: bool,
    skipped: Joined<'a, String>,
    stopped: Joined<'a, String>,
    times: Joined<'a, NodeTime>,
    logs: Joined<'a, NodeLog>,
    reports: Joined<'a, Report>,
    values: Badges<'a>,
}

/// The items of some lists, as one list.
struct Joined<'a, T>(&'a [&'a [T]]);

impl<T: Serialize> Serialize for Joined<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().flat_map(|part| part.iter()))
    }
}

/// The badges of the values at the places `changed` in `kept`, as `badges` gives them.
struct Badges<'a> {
    kept: &'a [Value],
    changed: &'a [usize],
}

impl<'a> Serialize for Badges<'a> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let badge = |v: &'a Value| Badge { from: &v.from, text: Cut(&v.text), lanes: v.lanes.as_ref(), thumb: &v.thumb };
        s.collect_seq(self.changed.iter().map(|&i| badge(&self.kept[i])))
    }
}

/// A value with the badge text and without the widgets, as `badges` gives it.
#[derive(Serialize)]
struct Badge<'a> {
    from: &'a str,
    text: Cut<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lanes: Option<&'a Lanes>,
    #[serde(skip_serializing_if = "str::is_empty")]
    thumb: &'a str,
}

/// A text, cut for its badge as `badge` does, with no copy.
struct Cut<'a>(&'a str);

impl Serialize for Cut<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match cut(self.0) {
            Some(end) => s.collect_str(&format_args!("{}...", &self.0[..end])),
            None => s.serialize_str(self.0),
        }
    }
}

/// Keeps the values of a run and their copies for the pins of the subgraph uses. Returns the
/// badges of the values that are new or changed, because the page keeps the other values (15.9).
pub(super) fn changed<'a>(shared: &Shared, aliases: &[(String, String)], values: impl IntoIterator<Item = &'a Value>) -> Vec<Value> {
    let mut kept = lock(&shared.values);
    let changed = keep_all(&mut kept, aliases, values);
    badges(changed.iter().map(|&i| &kept[i]))
}

/// Keeps the values of a run and their copies for the pins of the subgraph uses (6.4), as `keep`
/// does. `aliases` is in the order of the inner outputs (`by_inner`).
fn keep_all<'a>(kept: &mut Vec<Value>, aliases: &[(String, String)], values: impl IntoIterator<Item = &'a Value>) -> Vec<usize> {
    let mut changed = keep(kept, values);
    let mut at = 0;
    for n in 0..changed.len() {
        let i = changed[n];
        for (outer, _) in pins(aliases, &kept[i].from) {
            match find(kept, at, outer) {
                Some(j) => {
                    at = j + 1;
                    let (value, pin) = pair(kept, i, j);
                    if update(pin, value) {
                        changed.push(j);
                    }
                }
                None => {
                    let pin = Value { from: outer.clone(), ..kept[i].clone() };
                    changed.push(kept.len());
                    kept.push(pin);
                }
            }
        }
    }
    changed
}

/// The aliases of the subgraph pins (6.4) in the order of their inner outputs, for `alias` and
/// `keep_all`.
pub(super) fn by_inner(aliases: &[(String, String)]) -> Vec<(String, String)> {
    let mut sorted = aliases.to_vec();
    sorted.sort_by(|a, b| a.1.cmp(&b.1));
    sorted
}

/// The aliases of inner output `inner`. `aliases` is in the order of the inner outputs.
fn pins<'a>(aliases: &'a [(String, String)], inner: &str) -> &'a [(String, String)] {
    let start = aliases.partition_point(|(_, i)| i.as_str() < inner);
    let end = start + aliases[start..].partition_point(|(_, i)| i == inner);
    &aliases[start..end]
}

/// Value `i` and value `j` of `kept`, with `j` to change. The two places are not equal.
fn pair(kept: &mut [Value], i: usize, j: usize) -> (&Value, &mut Value) {
    if i < j {
        let (a, b) = kept.split_at_mut(j);
        (&a[i], &mut b[0])
    } else {
        let (a, b) = kept.split_at_mut(i);
        (&b[0], &mut a[j])
    }
}

/// Puts the values of a run in place of the old values of the same outputs, and returns the places
/// in `kept` of the values that are new or changed. The values of the batch domain stay. Each frame
/// gives its values in the same order, so the search starts after the last match.
fn keep<'a>(kept: &mut Vec<Value>, values: impl IntoIterator<Item = &'a Value>) -> Vec<usize> {
    let (mut at, mut changed) = (0, Vec::new());
    for value in values {
        match find(kept, at, &value.from) {
            Some(i) => {
                at = i + 1;
                if update(&mut kept[i], value) {
                    changed.push(i);
                }
            }
            None => {
                changed.push(kept.len());
                kept.push(value.clone());
            }
        }
    }
    changed
}

/// The place of output `from` in `kept`. The search starts at `at`, because each frame gives its
/// values in the same order.
fn find(kept: &[Value], at: usize, from: &str) -> Option<usize> {
    let same = |v: &Value| v.from == from;
    kept[at..].iter().position(same).map(|i| i + at).or_else(|| kept[..at].iter().position(same))
}

/// Copies value `from` into `to` in the buffers of `to`, and keeps the output of `to`. True when
/// the value changed.
fn update(to: &mut Value, from: &Value) -> bool {
    let changed = to.text != from.text || to.lanes != from.lanes || to.thumb != from.thumb || to.view != from.view;
    if changed {
        to.text.clone_from(&from.text);
        match (&mut to.lanes, &from.lanes) {
            (Some(to), Some(from)) => {
                to.bits.clone_from(&from.bits);
                to.label.clone_from(&from.label);
            }
            (to, from) => to.clone_from(from),
        }
        to.thumb.clone_from(&from.thumb);
        to.view.clone_from(&from.view);
    }
    changed
}

/// A copy of the value of each inner output for the output pins of its subgraph uses (6.4), in the
/// order of `keep_all`. `aliases` is in the order of the inner outputs (`by_inner`).
pub(super) fn alias<'a>(values: impl IntoIterator<Item = &'a Value>, aliases: &[(String, String)]) -> Vec<Value> {
    let copies = |v: &'a Value| pins(aliases, &v.from).iter().map(move |(outer, _)| Value { from: outer.clone(), ..v.clone() });
    values.into_iter().flat_map(copies).collect()
}

/// The values with the badge texts (15.9), and without the widgets. The **Inspector** asks for the
/// full value (15.7).
pub(super) fn badges<'a>(values: impl IntoIterator<Item = &'a Value>) -> Vec<Value> {
    values.into_iter().map(|v| Value { from: v.from.clone(), text: badge(&v.text), lanes: v.lanes.clone(), thumb: v.thumb.clone(), view: String::new() }).collect()
}

/// Cuts a text for its badge at a character boundary.
fn badge(text: &str) -> String {
    cut(text).map_or_else(|| text.to_owned(), |end| format!("{}...", &text[..end]))
}

/// The end of the badge text in `text`, at a character boundary. None when all of the text fits.
fn cut(text: &str) -> Option<usize> {
    (text.len() > BADGE_LEN).then(|| (0..=BADGE_LEN).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0))
}

/// True for the last frame of a tick: no frames are left, or the frames used one tick period.
fn last(left: u64, spent: Duration) -> bool {
    left == 0 || spent >= clock::FRAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frames_of_a_tick_stop_after_one_tick_period() {
        let ms = Duration::from_millis;
        assert!(!last(5, ms(1)) && last(0, ms(1)));
        assert!(last(5, ms(17)) && last(5, clock::FRAME));
    }

    #[test]
    fn badges_cut_long_texts_at_a_character_boundary() {
        assert_eq!(badge("2.5"), "2.5");
        let full = "a".repeat(BADGE_LEN);
        assert_eq!(badge(&full), full);
        // The 33rd two-byte character ends at byte 66, so the badge keeps 32 of them.
        assert_eq!(badge(&"\u{e9}".repeat(40)), format!("{}...", "\u{e9}".repeat(32)));
    }

    #[test]
    fn the_frames_draw_only_the_node_guis_in_view() {
        let mut shown = Shown { sizes: [Some("b".to_owned()), None], visible: None };
        assert!(shown.draws("a", 0, false));
        shown.visible = Some(HashSet::from(["a".to_owned()]));
        assert!(shown.draws("a", 0, false) && !shown.draws("b", 0, false) && shown.draws("b", 0, true));
        assert!(shown.draws("b", 1, false) && !shown.draws("a", 1, false) && !shown.draws("b", 2, false));
    }

    #[test]
    fn a_frame_replaces_only_the_values_of_its_outputs() {
        let value = |from: &str, text: &str| Value { from: from.to_owned(), text: text.to_owned(), ..Value::default() };
        let mut kept = vec![value("a.out", "1"), value("b.out", "2")];
        let changed = keep(&mut kept, &[value("b.out", "3"), value("c.out", "4")]);
        let texts: Vec<_> = kept.iter().map(|v| (v.from.as_str(), v.text.as_str())).collect();
        assert_eq!(texts, [("a.out", "1"), ("b.out", "3"), ("c.out", "4")]);
        assert_eq!(changed.len(), 2);
        // Only the changed values return, also in a new order.
        let changed = keep(&mut kept, &[value("c.out", "4"), value("a.out", "5"), value("b.out", "3")]);
        assert_eq!((changed.len(), kept[changed[0]].text.as_str(), kept.len()), (1, "5", 3));
        // A new thumbnail with the same text is a change too.
        let thumb = Value { thumb: "1 1 AQIDBA==".to_owned(), ..value("c.out", "4") };
        assert_eq!(keep(&mut kept, std::slice::from_ref(&thumb)).len(), 1);
        let view = Value { view: "label\tx".to_owned(), ..thumb };
        assert_eq!(keep(&mut kept, &[view]).len(), 1);
        assert_eq!(badges(&kept)[2].view, "", "the run event has no widgets");
        let lanes = Some(Lanes { bits: "01".to_owned(), label: String::new() });
        kept.push(Value { text: "\u{e9}".repeat(40), lanes, ..value("d.out", "") });
        let all: Vec<usize> = (0..kept.len()).collect();
        let json = serde_json::to_value(Badges { kept: &kept, changed: &all }).unwrap();
        assert_eq!(json, serde_json::to_value(badges(&kept)).unwrap(), "a frame sends the badges of a run");
        let aliases = by_inner(&[("u.0000000a".to_owned(), "u/b.out".to_owned()), ("v.0000000a".to_owned(), "b.out".to_owned())]);
        let extra = alias(&kept, &aliases);
        assert_eq!((extra.len(), extra[0].from.as_str(), extra[0].text.as_str()), (1, "v.0000000a", "3"));
    }

    #[test]
    fn a_frame_copies_each_changed_value_to_the_pins_of_its_subgraph_uses() {
        let value = |from: &str, text: &str| Value { from: from.to_owned(), text: text.to_owned(), ..Value::default() };
        let pin = |outer: &str, inner: &str| (outer.to_owned(), inner.to_owned());
        let aliases = by_inner(&[pin("u.0000000a", "u/b.out"), pin("w.0000000a", "a.out"), pin("v.0000000a", "a.out")]);
        let mut kept = Vec::new();
        assert_eq!(keep_all(&mut kept, &aliases, &[value("a.out", "1"), value("c.out", "1")]).len(), 4);
        assert_eq!(keep_all(&mut kept, &aliases, &[value("a.out", "1"), value("c.out", "2")]), [1]);
        let text = kept[3].text.as_ptr();
        let changed = keep_all(&mut kept, &aliases, &[value("a.out", "3"), value("c.out", "2")]);
        assert_eq!(changed, [0, 2, 3]);
        let texts: Vec<_> = kept.iter().map(|v| (v.from.as_str(), v.text.as_str())).collect();
        assert_eq!(texts, [("a.out", "3"), ("c.out", "2"), ("w.0000000a", "3"), ("v.0000000a", "3")]);
        assert_eq!(kept[3].text.as_ptr(), text, "a changed copy keeps its buffer");
    }
}
