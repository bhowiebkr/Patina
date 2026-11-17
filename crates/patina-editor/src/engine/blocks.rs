//! The results of the audio blocks since the last frame (9.3). The lists keep their old items
//! with their texts, so the audio thread does not allocate after the first blocks.

use std::fmt::Write;

use patina_engine::device::Notice;
use patina_engine::{NodeLog, NodeTime, Outcome, Report, Value};

/// A list whose items after `used` are spare: old items that keep their texts for the next fill.
struct List<T> {
    items: Vec<T>,
    used: usize,
}

impl<T: Clone + Refill> List<T> {
    fn get(&self) -> &[T] {
        &self.items[..self.used]
    }

    fn push(&mut self, item: &T) {
        match self.items.get_mut(self.used) {
            Some(old) => old.refill(item),
            None => self.items.push(item.clone()),
        }
        self.used += 1;
    }

    fn set(&mut self, items: &[T]) {
        self.used = 0;
        items.iter().for_each(|item| self.push(item));
    }
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self { items: Vec::new(), used: 0 }
    }
}

/// The results of one block, or of the blocks since the last frame.
pub(super) struct Part<'a> {
    pub ok: bool,
    pub values: &'a [Value],
    pub reports: &'a [Report],
    pub logs: &'a [NodeLog],
    pub skipped: &'a [String],
    pub stopped: &'a [String],
    pub times: &'a [NodeTime],
}

impl<'a> From<&'a Outcome> for Part<'a> {
    fn from(o: &'a Outcome) -> Self {
        Part { ok: o.ok, values: &o.values, reports: &o.reports, logs: &o.logs, skipped: &o.skipped, stopped: &o.stopped, times: &o.times }
    }
}

/// The results of the blocks since the last frame.
pub(super) struct Blocks {
    ok: bool,
    values: List<Value>,
    reports: List<Report>,
    logs: List<NodeLog>,
    skipped: List<String>,
    stopped: List<String>,
    times: List<NodeTime>,
}

impl Default for Blocks {
    fn default() -> Self {
        let (values, reports, logs) = (List::default(), List::default(), List::default());
        Blocks { ok: true, values, reports, logs, skipped: List::default(), stopped: List::default(), times: List::default() }
    }
}

impl Blocks {
    pub(super) fn part(&self) -> Part<'_> {
        let (ok, values, reports, logs) = (self.ok, self.values.get(), self.reports.get(), self.logs.get());
        Part { ok, values, reports, logs, skipped: self.skipped.get(), stopped: self.stopped.get(), times: self.times.get() }
    }

    /// Adds the results of one block or of other blocks. The values replace the old values of the
    /// same outputs. The run times and the nodes that did not run are those of the last block.
    pub(super) fn gather(&mut self, part: &Part) {
        self.ok &= part.ok;
        for value in part.values {
            let used = self.values.used;
            match self.values.items[..used].iter_mut().find(|v| v.from == value.from) {
                Some(old) => old.refill(value),
                None => self.values.push(value),
            }
        }
        part.reports.iter().for_each(|r| self.reports.push(r));
        part.logs.iter().for_each(|l| self.logs.push(l));
        self.skipped.set(part.skipped);
        self.stopped.set(part.stopped);
        // A quiet block sends no times, so the times of the last block that sent them stay.
        if !part.times.is_empty() {
            self.times.set(part.times);
        }
    }

    /// Adds a log line for a change of the audio device.
    pub(super) fn device(&mut self, notice: Notice) {
        let level = if notice.warning() { "WARN" } else { "INFO" };
        let line = NodeLog { id: "audio device".into(), folder: "Patina".into(), level: level.into(), text: String::new() };
        // A notice comes only when the device opens or stops, so this line may allocate.
        self.logs.push(&line);
        let text = &mut self.logs.items[self.logs.used - 1].text;
        let _ = write!(text, "{notice}");
    }

    /// Empties the lists, and keeps their items for the next blocks.
    pub(super) fn clear(&mut self) {
        self.ok = true;
        self.values.used = 0;
        self.reports.used = 0;
        self.logs.used = 0;
        self.skipped.used = 0;
        self.stopped.used = 0;
        self.times.used = 0;
    }
}

/// Copies an item into an old item, and keeps the texts of the old item.
trait Refill {
    fn refill(&mut self, from: &Self);
}

impl Refill for String {
    fn refill(&mut self, from: &Self) {
        self.clone_from(from);
    }
}

impl Refill for Value {
    fn refill(&mut self, from: &Self) {
        self.from.clone_from(&from.from);
        self.text.clone_from(&from.text);
        self.thumb.clone_from(&from.thumb);
        self.view.clone_from(&from.view);
        match (&mut self.lanes, &from.lanes) {
            (Some(old), Some(new)) => {
                old.bits.clone_from(&new.bits);
                old.label.clone_from(&new.label);
            }
            (old, new) => old.clone_from(new),
        }
    }
}

impl Refill for Report {
    fn refill(&mut self, from: &Self) {
        self.id.clone_from(&from.id);
        self.folder.clone_from(&from.folder);
        self.text.clone_from(&from.text);
        self.kind = from.kind;
    }
}

impl Refill for NodeLog {
    fn refill(&mut self, from: &Self) {
        self.id.clone_from(&from.id);
        self.folder.clone_from(&from.folder);
        self.level.clone_from(&from.level);
        self.text.clone_from(&from.text);
    }
}

impl Refill for NodeTime {
    fn refill(&mut self, from: &Self) {
        self.id.clone_from(&from.id);
        self.ms = from.ms;
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    use super::*;

    thread_local! {
        static COUNTING: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    /// Counts the allocations of a thread while its `COUNTING` is true.
    struct Counter;

    // SAFETY: the counter only counts, and `System` does the allocations.
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if COUNTING.get() {
                ALLOCATIONS.set(ALLOCATIONS.get() + 1);
            }
            // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: the caller keeps the contract of `GlobalAlloc::dealloc`.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static COUNTER: Counter = Counter;

    fn block(ok: bool, text: &str, ms: f64) -> Outcome {
        Outcome {
            ok,
            values: vec![Value { from: "t.out".to_owned(), text: text.to_owned(), ..Value::default() }],
            reports: (!ok).then(Report::default).into_iter().collect(),
            logs: vec![NodeLog { id: "t".to_owned(), text: format!("line {text}"), ..NodeLog::default() }],
            times: vec![NodeTime { id: "t".to_owned(), ms }],
            ..Outcome::default()
        }
    }

    #[test]
    fn the_blocks_since_a_frame_keep_all_problems_and_logs_and_the_last_values() {
        let mut blocks = Blocks::default();
        blocks.gather(&Part::from(&block(false, "1", 1.0)));
        blocks.gather(&Part::from(&block(true, "2", 2.0)));
        // A quiet block sends no values and no times, so the last ones stay.
        blocks.gather(&Part::from(&Outcome { ok: true, ..Outcome::default() }));
        let part = blocks.part();
        assert!(!part.ok && part.reports.len() == 1 && part.values.len() == 1 && part.logs.len() == 2);
        assert_eq!((part.values[0].text.as_str(), part.times[0].ms), ("2", 2.0));
        blocks.clear();
        let part = blocks.part();
        assert!(part.ok && part.values.is_empty() && part.logs.is_empty() && part.times.is_empty());
    }

    #[test]
    fn the_audio_thread_does_not_allocate_after_the_first_blocks() {
        let (one, two) = (block(true, "1", 1.0), block(true, "2", 2.0));
        let (mut mine, mut out) = (Blocks::default(), Blocks::default());
        let mut round = || {
            mine.gather(&Part::from(&one));
            mine.gather(&Part::from(&two));
            out.gather(&mine.part());
            mine.clear();
            // The frames swap `out` with their empty blocks.
            std::mem::swap(&mut out, &mut mine);
            mine.clear();
        };
        COUNTING.set(true);
        (0..2).for_each(|_| round());
        let first = ALLOCATIONS.replace(0);
        (0..100).for_each(|_| round());
        COUNTING.set(false);
        assert!(first > 0, "the first rounds fill the lists");
        assert_eq!(ALLOCATIONS.get(), 0, "the allocations of 100 rounds of two blocks");
    }
}
