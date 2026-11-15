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

/// A custom widget, for example from a widget folder (4.3, 6.2). It draws itself with the other
/// widgets and the canvas. `Ui::add` shows it.
pub trait Widget {
    fn show(&mut self, ui: &mut Ui) -> Response;
}

/// A pointer input on a canvas, in pixels from the top left corner of the canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    Down([f32; 2]),
    Move([f32; 2]),
    Up([f32; 2]),
}

/// A drawing area of a node GUI (4.3). The draw calls add shapes in pixels from the top left
/// corner. A color is red, green and blue.
pub struct Canvas<'a> {
    tree: &'a mut String,
    pointer: Vec<Pointer>,
}

impl Canvas<'_> {
    /// The pointer inputs since the last call of `ui`, in order.
    pub fn pointer(&self) -> &[Pointer] {
        &self.pointer
    }

    pub fn line(&mut self, from: [f32; 2], to: [f32; 2], color: [u8; 3]) {
        self.shape("line", &[from[0], from[1], to[0], to[1]], color, None);
    }

    pub fn rect(&mut self, at: [f32; 2], size: [f32; 2], color: [u8; 3], fill: bool) {
        self.shape("rect", &[at[0], at[1], size[0], size[1]], color, Some(fill));
    }

    pub fn circle(&mut self, center: [f32; 2], radius: f32, color: [u8; 3], fill: bool) {
        self.shape("circle", &[center[0], center[1], radius], color, Some(fill));
    }

    /// Writes `text` on one line. `at` is the left end of the base line.
    pub fn text(&mut self, at: [f32; 2], color: [u8; 3], text: &str) {
        self.shape("text", &at, color, None);
        self.tree.push(' ');
        push_clean(self.tree, text);
    }

    fn shape(&mut self, kind: &str, numbers: &[f32], [r, g, b]: [u8; 3], fill: Option<bool>) {
        let _ = write!(self.tree, "\t{kind}");
        for n in numbers {
            let _ = write!(self.tree, " {n}");
        }
        let _ = write!(self.tree, " #{r:02x}{g:02x}{b:02x}");
        if let Some(fill) = fill {
            let _ = write!(self.tree, " {}", u8::from(fill));
        }
    }
}

/// A number that a slider or a number field can change.
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
    events: Vec<(u32, String)>,
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
                Some((index.parse().ok()?, value.to_owned()))
            })
            .collect();
        Ui { mode, events, tree: String::new(), next: 0 }
    }

    /// The size to draw (4.3).
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Shows a custom widget.
    pub fn add(&mut self, widget: &mut impl Widget) -> Response {
        widget.show(self)
    }

    /// Shows a canvas of `size` pixels. Draw on the canvas that this returns.
    pub fn canvas(&mut self, text: &str, size: [f32; 2]) -> Canvas<'_> {
        let index = self.next;
        self.push("canvas", text);
        let _ = write!(self.tree, "\t{}\t{}", size[0], size[1]);
        let pointer = self.events.iter().filter(|(i, _)| *i == index).filter_map(|(_, v)| pointer(v)).collect();
        Canvas { tree: &mut self.tree, pointer }
    }

    /// Shows a text.
    pub fn label(&mut self, text: &str) {
        self.push("label", text);
    }

    /// Shows a button.
    pub fn button(&mut self, text: &str) -> Response {
        let clicked = self.push("button", text).is_some();
        Response { clicked, changed: false }
    }

    /// Shows a toggle for `on`.
    pub fn toggle(&mut self, text: &str, on: &mut bool) -> Response {
        let event = number(self.push("toggle", text));
        let _ = write!(self.tree, "\t{}", u8::from(*on));
        changed(event.map(|v| *on = v != 0.0))
    }

    /// Shows a slider for `value` in `range`.
    pub fn slider<T: Number>(&mut self, text: &str, value: &mut T, range: RangeInclusive<T>) -> Response {
        let (low, high) = (range.start().to_f64(), range.end().to_f64());
        let event = number(self.push("slider", text));
        let _ = write!(self.tree, "\t{}\t{low}\t{high}", value.to_f64());
        changed(event.map(|v| *value = T::from_f64(v.clamp(low, high))))
    }

    /// Shows a field for the number `value`.
    pub fn number<T: Number>(&mut self, text: &str, value: &mut T) -> Response {
        let event = number(self.push("number", text));
        let _ = write!(self.tree, "\t{}", value.to_f64());
        changed(event.map(|v| *value = T::from_f64(v)))
    }

    /// Shows a field for the text `value` on one line.
    pub fn text_field(&mut self, text: &str, value: &mut String) -> Response {
        let event = self.push("text", text).map(str::to_owned);
        self.tree.push('\t');
        push_clean(&mut self.tree, value);
        changed(event.map(|v| *value = v))
    }

    /// Shows a list of `options`. `selected` is the index of the selected option.
    pub fn dropdown(&mut self, text: &str, selected: &mut usize, options: &[&str]) -> Response {
        let event = number(self.push("dropdown", text)).filter(|&v| v >= 0.0 && (v as usize) < options.len());
        let _ = write!(self.tree, "\t{selected}");
        for option in options {
            self.tree.push('\t');
            push_clean(&mut self.tree, option);
        }
        changed(event.map(|v| *selected = v as usize))
    }

    /// The widget lines, for the editor.
    pub fn tree(&self) -> &str {
        &self.tree
    }

    /// Adds the line of the next widget, and returns its event.
    fn push(&mut self, kind: &str, text: &str) -> Option<&str> {
        let index = self.next;
        self.next += 1;
        if !self.tree.is_empty() {
            self.tree.push('\n');
        }
        self.tree.push_str(kind);
        self.tree.push('\t');
        push_clean(&mut self.tree, text);
        self.events.iter().rev().find(|(i, _)| *i == index).map(|(_, v)| v.as_str())
    }
}

/// A tab or a line break in a text would break the widget lines, so it becomes a space.
fn push_clean(tree: &mut String, text: &str) {
    tree.extend(text.chars().map(|c| if c.is_control() { ' ' } else { c }));
}

/// Reads a pointer event, for example `down 10 20`.
fn pointer(event: &str) -> Option<Pointer> {
    let mut words = event.split(' ');
    let kind = words.next()?;
    let mut at = [0.0; 2];
    for n in &mut at {
        *n = words.next()?.parse().ok().filter(|v: &f32| v.is_finite())?;
    }
    match kind {
        "down" => Some(Pointer::Down(at)),
        "move" => Some(Pointer::Move(at)),
        "up" => Some(Pointer::Up(at)),
        _ => None,
    }
}

fn number(event: Option<&str>) -> Option<f64> {
    event?.parse().ok().filter(|v: &f64| v.is_finite())
}

fn changed(event: Option<()>) -> Response {
    Response { clicked: false, changed: event.is_some() }
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

    #[test]
    fn fields_and_dropdowns_take_their_values() {
        let (mut count, mut name, mut pick) = (1_u32, String::from("a"), 0);
        let mut ui = Ui::new(0, "0 7\n1 new name\n2 5\n3 x");
        assert!(ui.number("Count", &mut count).changed());
        assert!(ui.text_field("Name", &mut name).changed());
        assert!(!ui.dropdown("Wave", &mut pick, &["Sine", "Saw"]).changed());
        assert!(!ui.number("Bad", &mut count).changed());
        assert_eq!((count, name.as_str(), pick), (7, "new name", 0));
        assert_eq!(ui.tree(), "number\tCount\t1\ntext\tName\ta\ndropdown\tWave\t0\tSine\tSaw\nnumber\tBad\t7");
    }

    #[test]
    fn a_canvas_reads_the_pointer_and_writes_its_shapes() {
        struct Dot;
        impl Widget for Dot {
            fn show(&mut self, ui: &mut Ui) -> Response {
                let mut canvas = ui.canvas("Dot", [40.0, 20.0]);
                let down = canvas.pointer().iter().any(|p| matches!(p, Pointer::Down(_)));
                canvas.circle([5.0, 5.0], 2.5, [255, 0, 16], true);
                canvas.text([1.0, 15.0], [0, 0, 0], "a\tb");
                Response { clicked: down, changed: false }
            }
        }
        let mut ui = Ui::new(0, "1 down 1 2\n1 move x 2\n1 up 3 4\n0 1");
        ui.label("Top");
        assert!(ui.add(&mut Dot).clicked());
        let mut canvas = ui.canvas("Grid", [8.0, 8.0]);
        assert!(canvas.pointer().is_empty());
        canvas.line([0.0, 0.0], [8.0, 8.0], [1, 2, 3]);
        assert_eq!(ui.canvas("Again", [1.0, 1.0]).pointer(), &[] as &[Pointer]);
        let lines: Vec<_> = ui.tree().lines().collect();
        assert_eq!(lines[1], "canvas\tDot\t40\t20\tcircle 5 5 2.5 #ff0010 1\ttext 1 15 #000000 a b");
        assert_eq!(lines[2], "canvas\tGrid\t8\t8\tline 0 0 8 8 #010203");
        let mut ui = Ui::new(0, "0 down 1 2\n0 move 3 4\n0 up 5 6");
        assert_eq!(ui.canvas("Drag", [9.0, 9.0]).pointer(), [Pointer::Down([1.0, 2.0]), Pointer::Move([3.0, 4.0]), Pointer::Up([5.0, 6.0])]);
    }
}
