//! The GUI of a node (spec 4.3). The `ui` function describes the full GUI each time it runs. The
//! editor draws the widgets and sends the user input back as events for the next call.

use std::fmt::Write;
use std::ops::RangeInclusive;

/// The size that the node draws its GUI in (4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// In the node body.
    Compact,
    /// In the inspector.
    Large,
    /// In the whole graph area, for a node with `full_gui`.
    Full,
}

/// What the user did to a widget since the last call of `ui`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Response {
    clicked: bool,
    changed: bool,
}

impl Response {
    /// True when the user clicked the widget.
    pub fn clicked(&self) -> bool {
        self.clicked
    }

    /// True when the user changed the value of the widget.
    pub fn changed(&self) -> bool {
        self.changed
    }
}

/// A number that a slider can change.
pub trait Number: Copy {
    fn to_f64(self) -> f64;
    fn from_f64(value: f64) -> Self;
}

macro_rules! number {
    ($($ty:ty),*) => {$(
        impl Number for $ty {
            fn to_f64(self) -> f64 {
                self as f64
            }
            fn from_f64(value: f64) -> Self {
                value as $ty
            }
        }
    )*};
}

number!(f32, f64, i32, i64, u32);

/// The widgets of one call of `ui`.
///
/// ```
/// use patina_sdk::Ui;
///
/// let (mut on, mut gain) = (false, 0.5);
/// let mut ui = Ui::new(0, "1 1\n2 0.25");
/// ui.label("Mixer");
/// assert!(ui.toggle("On", &mut on).changed());
/// ui.slider("Gain", &mut gain, 0.0..=1.0);
/// assert_eq!((on, gain), (true, 0.25));
/// ```
pub struct Ui {
    mode: Mode,
    /// The events from the editor: the index of the widget and the new value.
    events: Vec<(u32, f64)>,
    /// One line for each widget: the kind, the text and the value, with tabs between them.
    tree: String,
    next: u32,
}

impl Ui {
    /// A GUI in mode `mode` (0 compact, 1 large, 2 full), with the events of `events`. Each line of
    /// `events` is the index of a widget and its new value. The editor makes this text.
    pub fn new(mode: u32, events: &str) -> Ui {
        let mode = match mode {
            1 => Mode::Large,
            2 => Mode::Full,
            _ => Mode::Compact,
        };
        let events = events
            .lines()
            .filter_map(|line| {
                let (index, value) = line.split_once(' ')?;
                Some((index.parse().ok()?, value.parse().ok()?))
            })
            .collect();
        Ui { mode, events, tree: String::new(), next: 0 }
    }

    /// The size to draw (4.3).
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Shows a text.
    pub fn label(&mut self, text: &str) {
        self.add("label", text, None);
    }

    /// Shows a button.
    pub fn button(&mut self, text: &str) -> Response {
        let clicked = self.add("button", text, None).is_some();
        Response { clicked, changed: false }
    }

    /// Shows a toggle for `on`.
    pub fn toggle(&mut self, text: &str, on: &mut bool) -> Response {
        let event = self.add("toggle", text, Some(f64::from(u8::from(*on))));
        self.apply(event, |v| *on = v != 0.0)
    }

    /// Shows a slider for `value` in `range`.
    pub fn slider<T: Number>(&mut self, text: &str, value: &mut T, range: RangeInclusive<T>) -> Response {
        let (low, high) = (range.start().to_f64(), range.end().to_f64());
        let event = self.add("slider", text, Some(value.to_f64()));
        let _ = write!(self.tree, "\t{low}\t{high}");
        self.apply(event, |v| *value = T::from_f64(v.clamp(low, high)))
    }

    /// The widget lines, for the editor.
    pub fn tree(&self) -> &str {
        &self.tree
    }

    /// Adds the line of the next widget, and returns its event.
    fn add(&mut self, kind: &str, text: &str, value: Option<f64>) -> Option<f64> {
        let index = self.next;
        self.next += 1;
        if !self.tree.is_empty() {
            self.tree.push('\n');
        }
        self.tree.push_str(kind);
        self.tree.push('\t');
        self.tree.extend(text.chars().map(|c| if c.is_control() { ' ' } else { c }));
        if let Some(value) = value {
            let _ = write!(self.tree, "\t{value}");
        }
        self.events.iter().rev().find(|(i, _)| *i == index).map(|(_, v)| *v)
    }

    fn apply(&self, event: Option<f64>, set: impl FnOnce(f64)) -> Response {
        let Some(value) = event.filter(|v| v.is_finite()) else { return Response::default() };
        set(value);
        Response { clicked: false, changed: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widgets_read_their_events_and_write_their_lines() {
        let (mut on, mut level) = (true, 3_i32);
        let mut ui = Ui::new(1, "0 1\n2 99\nbad\n3 0");
        assert_eq!(ui.mode(), Mode::Large);
        assert!(ui.button("Re\tset").clicked());
        assert!(!ui.toggle("On", &mut on).changed());
        assert!(ui.slider("Level", &mut level, 0..=10).changed());
        assert!(ui.toggle("Off", &mut on).changed());
        assert_eq!((on, level), (false, 10));
        assert_eq!(ui.tree(), "button\tRe set\ntoggle\tOn\t1\nslider\tLevel\t3\t0\t10\ntoggle\tOff\t1");
    }
}
