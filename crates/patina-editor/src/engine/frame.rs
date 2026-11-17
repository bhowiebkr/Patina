//! The values and the node GUIs that the engine thread sends to the page after a frame (4.3, 15.9).

use std::collections::HashSet;
use std::time::Instant;

use patina_engine::{Control, Frames, Report, Value};

use super::blocks::Blocks;
use super::{Event, GuiInput, Shared, Tree, lock};
use crate::node_process::Out;

/// The longest text of a badge, in bytes. A longer text ends with "...".
const BADGE_LEN: usize = 64;

/// Gives the GUI input to node `input.id`, and sends its new widget lines. Returns `Some` when
/// the change must run the graph again: `Some(true)` when the node is in the frame domain.
pub(super) fn gui(out: &Out, control: &Control, part: &mut Frames, serial: u64, input: &GuiInput, shown: &mut Shown) -> Option<bool> {
    if let Some(node) = (input.mode as usize).checked_sub(1).and_then(|i| shown.sizes.get_mut(i)) {
        *node = (!input.id.is_empty()).then(|| input.id.clone());
    }
    let (tree, done) = part.ui(control, &input.id, input.mode, &input.events);
    let trees = tree.map(|tree| Tree { id: input.id.clone(), mode: input.mode, tree }).into_iter().collect();
    out.emit(&Event::Gui { serial, trees, reports: done.reports });
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
        out.emit(&Event::Gui { serial, trees, reports });
    }
}

/// Runs one frame, and sends its values, the results of `blocks` and the node GUIs. Then
/// empties `blocks`. Returns false when the frame or a block failed.
pub(super) fn frame(out: &Out, (part, shown, aliases): (&mut Frames, &Shown, &[(String, String)]), shared: &Shared, serial: u64, blocks: &mut Blocks) -> bool {
    let start = Instant::now();
    let outcome = part.frame(&shared.control);
    let run_ms = start.elapsed().as_secs_f64() * 1000.0;
    // shortcut: the editor copies each frame and its blocks into a JSON event, so its frame loop allocates. Send a binary frame when the web view needs more FPS.
    let b = blocks.part();
    let mut values: Vec<Value> = outcome.values.iter().chain(b.values).cloned().collect();
    let extra = alias(&values, aliases);
    values.extend(extra);
    keep(&mut lock(&shared.values), &values);
    let reports: Vec<Report> = outcome.reports.iter().chain(b.reports).cloned().collect();
    lock(&shared.reports).clone_from(&reports);
    let ok = outcome.ok && b.ok;
    let both = |a: &[String], b: &[String]| a.iter().chain(b).cloned().collect();
    let (skipped, stopped) = (both(&outcome.skipped, b.skipped), both(&outcome.stopped, b.stopped));
    let times = outcome.times.iter().chain(b.times).cloned().collect();
    let logs = outcome.logs.iter().chain(b.logs).cloned().collect();
    blocks.clear();
    let values = badges(&values);
    out.emit(&Event::Frame { serial, run_ms, ok, skipped, stopped, times, logs, reports, values });
    draw(out, &shared.control, part, serial, shown, false);
    ok
}

/// Puts the values of a frame in place of the old values of the same outputs. The values of the
/// batch domain stay.
pub(super) fn keep(kept: &mut Vec<Value>, values: &[Value]) {
    for value in values {
        match kept.iter_mut().find(|v| v.from == value.from) {
            Some(old) => old.clone_from(value),
            None => kept.push(value.clone()),
        }
    }
}

/// A copy of the value of each inner output for the output pin of its subgraph use (6.4).
pub(super) fn alias(values: &[Value], aliases: &[(String, String)]) -> Vec<Value> {
    let by: std::collections::HashMap<&str, &Value> = values.iter().map(|v| (v.from.as_str(), v)).collect();
    let value = |(outer, inner): &(String, String)| Some(Value { from: outer.clone(), ..(*by.get(inner.as_str())?).clone() });
    aliases.iter().filter_map(value).collect()
}

/// The values with the badge texts (15.9).
pub(super) fn badges(values: &[Value]) -> Vec<Value> {
    values.iter().map(|v| Value { from: v.from.clone(), text: badge(&v.text), lanes: v.lanes.clone() }).collect()
}

/// Cuts a text for its badge at a character boundary.
fn badge(text: &str) -> String {
    if text.len() <= BADGE_LEN {
        return text.to_owned();
    }
    let end = (0..=BADGE_LEN).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
    format!("{}...", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let value = |from: &str, text: &str| Value { from: from.to_owned(), text: text.to_owned(), lanes: None };
        let mut kept = vec![value("a.out", "1"), value("b.out", "2")];
        keep(&mut kept, &[value("b.out", "3"), value("c.out", "4")]);
        let texts: Vec<_> = kept.iter().map(|v| (v.from.as_str(), v.text.as_str())).collect();
        assert_eq!(texts, [("a.out", "1"), ("b.out", "3"), ("c.out", "4")]);
        let extra = alias(&kept, &[("u.0000000a".to_owned(), "u/b.out".to_owned()), ("v.0000000a".to_owned(), "b.out".to_owned())]);
        assert_eq!((extra.len(), extra[0].from.as_str(), extra[0].text.as_str()), (1, "v.0000000a", "3"));
    }
}
