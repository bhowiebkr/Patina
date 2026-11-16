//! The GUI of a node (spec 4.3). The `ui` function describes the full GUI each time it runs. The
//! editor draws the widgets and sends the user input back as events for the next call.

use std::fmt::Write;
use std::ops::RangeInclusive;

/// The most values of a plot line. A longer line sends the lowest and the highest value of parts.
const MAX_PLOT: usize = 1024;
/// The columns of a waveform.
const WAVE_COLUMNS: usize = 256;
/// The most points of a curve.
const MAX_CURVE: usize = 256;
/// The most pixels of an image, so that the widget lines stay fast to send.
const MAX_PIXELS: u32 = 256 * 256;

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
    /// The response of a custom widget (`Widget`).
    pub fn new(clicked: bool, changed: bool) -> Response {
        Response { clicked, changed }
    }

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

/// The place and the input of a viewport (15.8). A view draws into it with
/// `patina_gpu::ViewportSurface`. The sizes are in physical pixels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Viewport {
    /// The native handle of the editor window, a `HWND` on Windows. 0 until the editor shows the
    /// viewport.
    pub window: u64,
    /// The width and the height of the window.
    pub window_size: [u32; 2],
    /// The area of the viewport in the window: x, y, width and height.
    pub rect: [u32; 4],
    /// The pointer inputs since the last call of `ui`, from the top left corner of the area.
    pub pointer: Vec<Pointer>,
    /// The wheel movement since the last call of `ui`, in pixels of the page. Down is positive.
    pub wheel: f32,
    /// The key inputs since the last call of `ui`, while the viewport has the focus. A click gives
    /// the focus. When the viewport loses the focus, each held key comes up.
    pub keys: Vec<Key>,
}

/// A key input of a viewport. The name is the physical key of the web `KeyboardEvent.code`, for
/// example `KeyW`, `Space` or `ArrowLeft`, so it does not change with the keyboard layout.
#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Down(String),
    Up(String),
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
        self.tree.push(' ');
        push_color(self.tree, [r, g, b]);
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

    /// Shows a viewport of `height` pixels and the full width (15.8). In version 1, the editor
    /// shows it only in a view.
    pub fn viewport(&mut self, text: &str, height: f32) -> Viewport {
        let index = self.next;
        self.push("viewport", text);
        let _ = write!(self.tree, "\t{height}");
        let mut viewport = Viewport::default();
        for (_, event) in self.events.iter().filter(|(i, _)| *i == index) {
            if let Some((window, [ww, wh, x, y, w, h])) = event.strip_prefix("place ").and_then(place) {
                (viewport.window, viewport.window_size, viewport.rect) = (window, [ww, wh], [x, y, w, h]);
            } else if let Some(d) = event.strip_prefix("wheel ").and_then(|d| d.parse().ok()).filter(|d: &f32| d.is_finite()) {
                viewport.wheel += d;
            } else if let Some(p) = pointer(event) {
                viewport.pointer.push(p);
            } else if let Some(name) = event.strip_prefix("key down ").filter(|n| !n.is_empty()) {
                viewport.keys.push(Key::Down(name.to_owned()));
            } else if let Some(name) = event.strip_prefix("key up ").filter(|n| !n.is_empty()) {
                viewport.keys.push(Key::Up(name.to_owned()));
            }
        }
        viewport
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

    /// Shows a field for the number `value` with its `unit`, and two buttons that change it by `step`.
    pub fn number_unit<T: Number>(&mut self, text: &str, value: &mut T, step: T, unit: &str) -> Response {
        let event = number(self.push("number", text));
        let _ = write!(self.tree, "\t{}\t{}\t", value.to_f64(), step.to_f64().abs());
        push_clean(&mut self.tree, unit);
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
        self.choice("dropdown", text, selected, options)
    }

    /// Shows a row of `tabs` with the name `text`. `selected` is the index of the selected tab. The
    /// node shows the widgets of the selected tab after the row.
    pub fn tabs(&mut self, text: &str, selected: &mut usize, tabs: &[&str]) -> Response {
        self.choice("tabs", text, selected, tabs)
    }

    /// Shows a section that the user opens and closes. The widgets of `add` show in the open section.
    /// The node keeps `open`, so the state saves with the node. `add` also runs when the section is
    /// closed, so the widgets after the section keep their order.
    pub fn section(&mut self, text: &str, open: &mut bool, add: impl FnOnce(&mut Ui)) -> Response {
        let event = number(self.push("section", text)).map(|v| *open = v != 0.0);
        let _ = write!(self.tree, "\t{}", u8::from(*open));
        let (start, first) = (self.tree.len(), self.next);
        add(self);
        if !*open {
            // An empty line keeps the index of each hidden widget.
            self.tree.truncate(start);
            self.tree.extend((first..self.next).map(|_| '\n'));
        }
        self.push("end", "");
        changed(event)
    }

    fn choice(&mut self, kind: &str, text: &str, selected: &mut usize, options: &[&str]) -> Response {
        let event = number(self.push(kind, text)).filter(|&v| v >= 0.0 && (v as usize) < options.len());
        let _ = write!(self.tree, "\t{selected}");
        for option in options {
            self.tree.push('\t');
            push_clean(&mut self.tree, option);
        }
        changed(event.map(|v| *selected = v as usize))
    }

    /// Shows a color picker for `color`: red, green and blue.
    pub fn color(&mut self, text: &str, color: &mut [u8; 3]) -> Response {
        let event = self.push("color", text).and_then(hex);
        self.tree.push('\t');
        push_color(&mut self.tree, *color);
        changed(event.map(|v| *color = v))
    }

    /// Shows `values` as a line from left to right. `range` has the values at the bottom and the top.
    pub fn plot(&mut self, text: &str, values: &[f32], range: RangeInclusive<f32>) {
        self.push("plot", text);
        let _ = write!(self.tree, "\t{}\t{}\t", range.start(), range.end());
        if values.len() <= MAX_PLOT {
            push_numbers(&mut self.tree, values.iter().copied());
            return;
        }
        // The lowest and the highest value of each part, in their order, keep the peaks of the line.
        push_numbers(
            &mut self.tree,
            parts(values, MAX_PLOT / 2).flat_map(|part| {
                let low = part.iter().enumerate().min_by(|a, b| a.1.total_cmp(b.1));
                let high = part.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1));
                let (Some(low), Some(high)) = (low, high) else { return [f32::NAN; 2] };
                if low.0 <= high.0 { [*low.1, *high.1] } else { [*high.1, *low.1] }
            }),
        );
    }

    /// Shows audio `samples` from -1 to 1. `playhead` is a position from 0 (the start) to 1 (the
    /// end).
    pub fn waveform(&mut self, text: &str, samples: &[f32], playhead: Option<f32>) {
        self.push("waveform", text);
        self.wave(samples, playhead);
    }

    /// Shows a waveform with two handles that set `trim`: the start and the end, from 0 to 1.
    pub fn waveform_trim(&mut self, text: &str, samples: &[f32], playhead: Option<f32>, trim: &mut [f32; 2]) -> Response {
        let event = self.push("waveform", text).and_then(span).map(|v| *trim = v);
        self.wave(samples, playhead);
        let [start, end] = if trim.iter().all(|v| v.is_finite()) { sorted(*trim) } else { [0.0, 1.0] };
        let _ = write!(self.tree, "\t{start} {end}");
        changed(event)
    }

    fn wave(&mut self, samples: &[f32], playhead: Option<f32>) {
        self.tree.push('\t');
        if let Some(at) = playhead.filter(|v| v.is_finite()) {
            let _ = write!(self.tree, "{}", at.clamp(0.0, 1.0));
        }
        self.tree.push('\t');
        let columns = parts(samples, samples.len().min(WAVE_COLUMNS));
        push_numbers(
            &mut self.tree,
            columns.flat_map(|part| [part.iter().copied().fold(f32::INFINITY, f32::min), part.iter().copied().fold(f32::NEG_INFINITY, f32::max)]),
        );
    }

    /// Shows a table with the headings `columns` and the `cells`, row by row. `selected` is the
    /// selected row. A click on a row selects it.
    pub fn table<S: AsRef<str>>(&mut self, text: &str, columns: &[&str], cells: &[S], selected: &mut Option<usize>) -> Response {
        let rows = if columns.is_empty() { 0 } else { cells.len().div_ceil(columns.len()) };
        let event = number(self.push("table", text)).filter(|&v| v >= 0.0 && (v as usize) < rows);
        let _ = write!(self.tree, "\t{}\t", columns.len());
        if let Some(row) = selected {
            let _ = write!(self.tree, "{row}");
        }
        for cell in columns.iter().copied().chain(cells.iter().map(AsRef::as_ref)) {
            self.tree.push('\t');
            push_clean(&mut self.tree, cell);
        }
        changed(event.map(|v| *selected = Some(v as usize)))
    }

    /// Shows a curve editor for `points`, with x and y from 0 to 1. The user can move, add and
    /// remove points. After a change, the points are in the order of x.
    pub fn curve(&mut self, text: &str, points: &mut Vec<[f32; 2]>) -> Response {
        let edit = self.push("curve", text).and_then(curve_edit);
        for [x, y] in points.iter() {
            let _ = write!(self.tree, "\t{x} {y}");
        }
        let done = match edit {
            Some(CurveEdit::Move(i, at)) if i < points.len() => {
                points[i] = at;
                true
            }
            Some(CurveEdit::Add(at)) if points.len() < MAX_CURVE => {
                points.push(at);
                true
            }
            Some(CurveEdit::Remove(i)) if i < points.len() => {
                points.remove(i);
                true
            }
            _ => false,
        };
        if done {
            points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        }
        Response { clicked: false, changed: done }
    }

    /// Shows an image of `size` pixels. `rgba` has the red, green, blue and alpha of each pixel,
    /// row by row from the top. An image of more than 256 x 256 pixels or of a wrong length is empty.
    pub fn image(&mut self, text: &str, size: [u32; 2], rgba: &[u8]) {
        self.push("image", text);
        let pixels = size[0].checked_mul(size[1]).filter(|&p| p <= MAX_PIXELS && p as usize * 4 == rgba.len());
        if pixels.is_some() {
            let _ = write!(self.tree, "\t{}\t{}\t", size[0], size[1]);
            push_base64(&mut self.tree, rgba);
        } else {
            self.tree.push_str("\t0\t0\t");
        }
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

fn push_color(tree: &mut String, [r, g, b]: [u8; 3]) {
    let _ = write!(tree, "#{r:02x}{g:02x}{b:02x}");
}

/// Reads a color event, for example `#ff8000`.
fn hex(event: &str) -> Option<[u8; 3]> {
    let digits = event.strip_prefix('#').filter(|d| d.len() == 6 && d.bytes().all(|b| b.is_ascii_hexdigit()))?;
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

fn push_numbers(tree: &mut String, values: impl Iterator<Item = f32>) {
    for (i, v) in values.enumerate() {
        if i > 0 {
            tree.push(' ');
        }
        let _ = write!(tree, "{v}");
    }
}

/// Divides `values` into `n` parts of almost the same length. No part is empty when `n` is at
/// most the length.
fn parts(values: &[f32], n: usize) -> impl Iterator<Item = &[f32]> {
    (0..n).map(move |i| &values[i * values.len() / n..(i + 1) * values.len() / n])
}

fn push_base64(tree: &mut String, bytes: &[u8]) {
    const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0_u32, |n, (i, &b)| n | (u32::from(b) << (16 - 8 * i)));
        for i in 0..4 {
            tree.push(if i <= chunk.len() { char::from(DIGITS[((n >> (18 - 6 * i)) & 63) as usize]) } else { '=' });
        }
    }
}

/// A user edit of a curve: the index of a point and the new point.
enum CurveEdit {
    Move(usize, [f32; 2]),
    Add([f32; 2]),
    Remove(usize),
}

/// Reads a curve event: `move 1 0.5 0.25`, `add 0.5 0.25` or `remove 1`.
fn curve_edit(event: &str) -> Option<CurveEdit> {
    let mut words = event.split(' ');
    let kind = words.next()?;
    let index = if kind == "add" { 0 } else { words.next()?.parse().ok()? };
    if kind == "remove" {
        return Some(CurveEdit::Remove(index));
    }
    let mut at = [0.0; 2];
    for n in &mut at {
        *n = words.next()?.parse().ok().filter(|v: &f32| v.is_finite())?.clamp(0.0, 1.0);
    }
    match kind {
        "move" => Some(CurveEdit::Move(index, at)),
        "add" => Some(CurveEdit::Add(at)),
        _ => None,
    }
}

/// Reads a place event of a viewport: the window, its size and the area, for example
/// `place 7 800 600 10 20 300 200` without the first word.
fn place(event: &str) -> Option<(u64, [u32; 6])> {
    let mut words = event.split(' ');
    let window = words.next()?.parse().ok()?;
    let mut numbers = [0; 6];
    for n in &mut numbers {
        *n = words.next()?.parse().ok()?;
    }
    words.next().is_none().then_some((window, numbers))
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

/// Reads a trim event, for example `0.1 0.9`.
fn span(event: &str) -> Option<[f32; 2]> {
    let (a, b) = event.split_once(' ')?;
    let v = [a.parse::<f32>().ok()?, b.parse::<f32>().ok()?];
    v.iter().all(|v| v.is_finite()).then(|| sorted(v))
}

fn sorted(v: [f32; 2]) -> [f32; 2] {
    let [a, b] = v.map(|v| v.clamp(0.0, 1.0));
    if a <= b { [a, b] } else { [b, a] }
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
    fn units_tabs_and_sections_write_their_lines() {
        let (mut gain, mut tab, mut open, mut shut) = (1.5_f32, 0, false, true);
        // Index 2 opens the first section, so its label is index 3 and its end is 4. Index 5 closes the next one.
        let mut ui = Ui::new(0, "0 -3\n1 1\n2 1\n5 0\n8 1");
        assert!(ui.number_unit("Gain", &mut gain, -0.5, "d\tB").changed());
        assert!(ui.tabs("Pages", &mut tab, &["Main", "More"]).changed());
        let mut inner = false;
        assert!(ui.section("More", &mut open, |ui| ui.label("in")).changed());
        ui.section("Shut", &mut shut, |ui| {
            ui.toggle("hidden", &mut inner);
        });
        assert!(ui.button("After").clicked());
        assert_eq!((gain, tab, open, shut), (-3.0, 1, true, false));
        let lines: Vec<&str> = ui.tree().lines().collect();
        assert_eq!(lines, ["number\tGain\t1.5\t0.5\td B", "tabs\tPages\t0\tMain\tMore", "section\tMore\t1", "label\tin", "end\t", "section\tShut\t0", "", "end\t", "button\tAfter"]);
    }

    #[test]
    fn a_trim_event_sets_a_sorted_range_in_the_limits() {
        let (mut trim, mut bad) = ([0.2_f32, 0.8], [f32::NAN, 0.5]);
        let mut ui = Ui::new(0, "0 0.9 -1\n1 x 0.5");
        assert!(ui.waveform_trim("Wave", &[0.5], None, &mut trim).changed());
        assert!(!ui.waveform_trim("Bad", &[], Some(0.5), &mut bad).changed());
        assert_eq!(trim, [0.0, 0.9]);
        assert_eq!(ui.tree().lines().collect::<Vec<_>>(), ["waveform\tWave\t\t0.5 0.5\t0 0.9", "waveform\tBad\t0.5\t\t0 1"]);
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

    #[test]
    fn a_viewport_reads_its_place_and_input() {
        let mut ui = Ui::new(0, "0 place 7 800 600 10 20 300 200\n0 down 1 2\n0 wheel 40\n0 key down KeyW\n0 key up \n0 wheel -15\n0 key up KeyW\n0 place 7 900 600 0 0 1\n0 wheel x\n1 place 9 1 1 0 0 1 1");
        let viewport = ui.viewport("Scene", 240.0);
        assert_eq!((viewport.window, viewport.window_size, viewport.rect), (7, [800, 600], [10, 20, 300, 200]));
        assert_eq!((viewport.pointer, viewport.wheel), (vec![Pointer::Down([1.0, 2.0])], 25.0));
        assert_eq!(viewport.keys, [Key::Down("KeyW".into()), Key::Up("KeyW".into())]);
        assert_eq!(ui.tree(), "viewport\tScene\t240");
        assert_eq!(Ui::new(0, "").viewport("Empty", 1.0), Viewport::default());
    }

    #[test]
    fn value_widgets_write_their_data_and_take_their_edits() {
        let (mut color, mut row, mut points) = ([255, 0, 16], None, vec![[0.5, 0.5], [0.0, 0.0]]);
        let mut ui = Ui::new(0, "0 #00ff80\n2 1\n3 move 0 2 -1\n4 #zz0000\n5 9\n6 add 0.5");
        assert!(ui.color("Tint", &mut color).changed());
        ui.plot("Level", &[0.0, 1.5, f32::NAN], 0.0..=2.0);
        assert!(ui.table("Notes", &["Note", "Hz"], &["A", "440", "C"], &mut row).changed());
        assert!(ui.curve("Shape", &mut points).changed());
        assert!(!ui.color("Bad", &mut color).changed());
        assert!(!ui.table("Empty", &[], &["x"], &mut row).changed());
        assert!(!ui.curve("Bad", &mut points).changed());
        ui.waveform("Wave", &[0.0, -0.5, 1.0, f32::NAN], Some(2.0));
        ui.image("Dot", [1, 1], &[1, 2, 3, 255]);
        ui.image("Bad", [2, 1], &[0; 4]);
        assert_eq!((color, row, points), ([0, 255, 128], Some(1), vec![[0.0, 0.0], [1.0, 0.0]]));
        let lines: Vec<_> = ui.tree().lines().collect();
        assert_eq!(lines[..6], ["color\tTint\t#ff0010", "plot\tLevel\t0\t2\t0 1.5 NaN", "table\tNotes\t2\t\tNote\tHz\tA\t440\tC", "curve\tShape\t0.5 0.5\t0 0", "color\tBad\t#00ff80", "table\tEmpty\t0\t1\tx"]);
        assert_eq!(lines[7..], ["waveform\tWave\t1\t0 0 -0.5 -0.5 1 1 inf -inf", "image\tDot\t1\t1\tAQID/w==", "image\tBad\t0\t0\t"]);

        // A long plot line keeps its peak, and a long waveform has a fixed number of columns.
        let long: Vec<f32> = (0..3000).map(|i| if i == 1500 { 9.0 } else { 0.0 }).collect();
        let mut ui = Ui::new(0, "");
        ui.plot("Long", &long, 0.0..=9.0);
        ui.waveform("Long", &long, None);
        let lines: Vec<Vec<_>> = ui.tree().lines().map(|l| l.rsplit('\t').next().unwrap().split(' ').collect()).collect();
        assert_eq!((lines[0].len(), lines[0].contains(&"9")), (MAX_PLOT, true));
        assert_eq!((lines[1].len(), lines[1].contains(&"9")), (2 * WAVE_COLUMNS, true));
        assert!(ui.tree().contains("waveform\tLong\t\t0 0"));
    }
}
