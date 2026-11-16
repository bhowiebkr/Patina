//! The color ramp widget of a node GUI (spec 4.3).

use super::{Response, Ui, hex, push_color};
use std::fmt::Write;

/// The most stops of a color ramp.
const MAX_STOPS: usize = 32;

/// How a color ramp blends the colors between two stops.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blend {
    /// A straight line from one color to the next.
    #[default]
    Linear,
    /// A smooth start and end at each stop.
    Ease,
    /// The color of the stop before, up to the next stop.
    Constant,
}

const BLENDS: [(Blend, &str); 3] = [(Blend::Linear, "linear"), (Blend::Ease, "ease"), (Blend::Constant, "constant")];

/// Colors at positions from 0 to 1, for example for a gradient map. `Ui::color_ramp` edits it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorRamp {
    /// The position and the color of each stop, in the order of position.
    pub stops: Vec<(f32, [u8; 3])>,
    pub blend: Blend,
}

impl ColorRamp {
    /// The color at `at`. Before the first stop and after the last stop, it is the color of that stop.
    /// A ramp without stops is black.
    pub fn color(&self, at: f32) -> [u8; 3] {
        let next = self.stops.iter().position(|s| s.0 > at);
        let (a, b) = match next {
            Some(0) => return self.stops[0].1,
            Some(i) => (self.stops[i - 1], self.stops[i]),
            None => return self.stops.last().map_or([0; 3], |s| s.1),
        };
        let t = (at - a.0) / (b.0 - a.0);
        let t = match self.blend {
            Blend::Linear => t,
            Blend::Ease => t * t * (3.0 - 2.0 * t),
            Blend::Constant => 0.0,
        };
        std::array::from_fn(|c| (f32::from(a.1[c]) + (f32::from(b.1[c]) - f32::from(a.1[c])) * t).round() as u8)
    }

    /// Puts a stop in the order of position, and returns its index.
    fn insert(&mut self, at: f32, color: [u8; 3]) -> usize {
        let i = self.stops.partition_point(|s| s.0 <= at);
        self.stops.insert(i, (at, color));
        i
    }
}

enum RampEdit {
    Select(usize),
    Move(usize, f32),
    Color(usize, [u8; 3]),
    Add(f32),
    Remove(usize),
    Blend(Blend),
}

/// Reads a ramp event, for example `move 2 0.5`, `color 1 #ff8000` or `blend ease`.
fn ramp_edit(event: &str) -> Option<RampEdit> {
    let mut words = event.split(' ');
    let kind = words.next()?;
    let mut index = || words.next()?.parse::<usize>().ok();
    let i = if matches!(kind, "add" | "blend") { 0 } else { index()? };
    let word = words.next();
    let at = || word?.parse::<f32>().ok().filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0));
    match kind {
        "select" => Some(RampEdit::Select(i)),
        "move" => Some(RampEdit::Move(i, at()?)),
        "color" => Some(RampEdit::Color(i, hex(word?)?)),
        "add" => Some(RampEdit::Add(at()?)),
        "remove" => Some(RampEdit::Remove(i)),
        "blend" => BLENDS.iter().find(|b| Some(b.1) == word).map(|b| RampEdit::Blend(b.0)),
        _ => None,
    }
}

impl Ui {
    /// Shows a color ramp editor for `ramp`. `selected` is the stop that the user edits. The user
    /// can move, add and remove stops, and change the color of a stop and the blend. A remove
    /// keeps one stop, and an add stops at 32 stops.
    pub fn color_ramp(&mut self, text: &str, ramp: &mut ColorRamp, selected: &mut usize) -> Response {
        if !ramp.stops.is_sorted_by(|a, b| a.0.total_cmp(&b.0).is_le()) {
            ramp.stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        let edit = self.push("ramp", text).and_then(ramp_edit);
        let blend = BLENDS.iter().find(|b| b.0 == ramp.blend).map_or("linear", |b| b.1);
        let _ = write!(self.tree, "\t{blend}\t{}", (*selected).min(ramp.stops.len().saturating_sub(1)));
        for &(at, color) in &ramp.stops {
            let _ = write!(self.tree, "\t{at} ");
            push_color(&mut self.tree, color);
        }
        let n = ramp.stops.len();
        let done = match edit {
            Some(RampEdit::Select(i)) if i < n => {
                *selected = i;
                true
            }
            Some(RampEdit::Move(i, at)) if i < n => {
                let (_, color) = ramp.stops.remove(i);
                *selected = ramp.insert(at, color);
                true
            }
            Some(RampEdit::Color(i, color)) if i < n => {
                ramp.stops[i].1 = color;
                *selected = i;
                true
            }
            Some(RampEdit::Add(at)) if n < MAX_STOPS => {
                *selected = ramp.insert(at, ramp.color(at));
                true
            }
            Some(RampEdit::Remove(i)) if i < n && n > 1 => {
                ramp.stops.remove(i);
                *selected = i.min(n - 2);
                true
            }
            Some(RampEdit::Blend(blend)) => {
                ramp.blend = blend;
                true
            }
            _ => false,
        };
        Response { clicked: false, changed: done }
    }
}
