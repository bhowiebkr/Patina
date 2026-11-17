//! The values and the node GUIs that the engine thread sends to the page after a frame (4.3, 15.9).

use std::collections::HashSet;
use std::time::{Duration, Instant};

use patina_engine::clock;
use patina_engine::{Control, Frames, Report, Value};

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
    // shortcut: the editor copies each frame and its blocks into a JSON event, so its frame loop allocates. Send a binary frame when the web view needs more FPS.
    let b = blocks.part();
    let values = changed(shared, aliases, outcome.values.iter().chain(b.values));
    let reports: Vec<Report> = outcome.reports.iter().chain(b.reports).cloned().collect();
    lock(&shared.reports).clone_from(&reports);
    let ok = outcome.ok && b.ok;
    let both = |a: &[String], b: &[String]| a.iter().chain(b).cloned().collect();
    let (skipped, stopped) = (both(&outcome.skipped, b.skipped), both(&outcome.stopped, b.stopped));
    let times = match show % TIMES_EVERY {
        1 => outcome.times.iter().chain(b.times).cloned().collect(),
        _ => Vec::new(),
    };
    quiet_logs.extend(outcome.logs.iter().chain(b.logs).cloned());
    blocks.clear();
    out.emit(&Event::Frame { serial, frames: ran, run_ms, ok, skipped, stopped, times, logs: quiet_logs, reports, values });
    draw(out, control, part, serial, shown, false);
    (ok, ran)
}

/// Keeps the values of a run and their copies for the pins of the subgraph uses. Returns the
/// badges of the values that are new or changed, because the page keeps the other values (15.9).
pub(super) fn changed<'a>(shared: &Shared, aliases: &[(String, String)], values: impl IntoIterator<Item = &'a Value>) -> Vec<Value> {
    let mut kept = lock(&shared.values);
    let mut changed = keep(&mut kept, values);
    let extra = alias(changed.iter().map(|&i| &kept[i]), aliases);
    changed.extend(keep(&mut kept, &extra));
    badges(changed.iter().map(|&i| &kept[i]))
}

/// Puts the values of a run in place of the old values of the same outputs, and returns the places
/// in `kept` of the values that are new or changed. The values of the batch domain stay. Each frame
/// gives its values in the same order, so the search starts after the last match.
fn keep<'a>(kept: &mut Vec<Value>, values: impl IntoIterator<Item = &'a Value>) -> Vec<usize> {
    let (mut at, mut changed) = (0, Vec::new());
    for value in values {
        let same = |v: &Value| v.from == value.from;
        match kept[at..].iter().position(same).map(|i| i + at).or_else(|| kept[..at].iter().position(same)) {
            Some(i) => {
                at = i + 1;
                if kept[i].text != value.text || kept[i].lanes != value.lanes || kept[i].thumb != value.thumb || kept[i].view != value.view {
                    kept[i].clone_from(value);
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

/// A copy of the value of each inner output for the output pin of its subgraph use (6.4).
pub(super) fn alias<'a>(values: impl IntoIterator<Item = &'a Value>, aliases: &[(String, String)]) -> Vec<Value> {
    let by: std::collections::HashMap<&str, &Value> = values.into_iter().map(|v| (v.from.as_str(), v)).collect();
    let value = |(outer, inner): &(String, String)| Some(Value { from: outer.clone(), ..(*by.get(inner.as_str())?).clone() });
    aliases.iter().filter_map(value).collect()
}

/// The values with the badge texts (15.9), and without the widgets. The **Inspector** asks for the
/// full value (15.7).
pub(super) fn badges<'a>(values: impl IntoIterator<Item = &'a Value>) -> Vec<Value> {
    values.into_iter().map(|v| Value { from: v.from.clone(), text: badge(&v.text), lanes: v.lanes.clone(), thumb: v.thumb.clone(), view: String::new() }).collect()
}

/// Cuts a text for its badge at a character boundary.
fn badge(text: &str) -> String {
    if text.len() <= BADGE_LEN {
        return text.to_owned();
    }
    let end = (0..=BADGE_LEN).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
    format!("{}...", &text[..end])
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
        let extra = alias(&kept, &[("u.0000000a".to_owned(), "u/b.out".to_owned()), ("v.0000000a".to_owned(), "b.out".to_owned())]);
        assert_eq!((extra.len(), extra[0].from.as_str(), extra[0].text.as_str()), (1, "v.0000000a", "3"));
    }
}
