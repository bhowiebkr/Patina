//! The types of the gun workbench: parts, makers, rarities and stats. The shape rules are here,
//! so the nodes stay thin. Units are meters, +X points to the muzzle, Y is the width and Z is up.

mod gun;
mod parts;

use std::fmt::{self, Write};

use geometry::{Solid, Sockets};
use patina_sdk::{Param, Text, Thumbnail, Ui, Viewer};

pub use gun::{Outline, assemble, generate, look, side_view};
pub use parts::part;

/// A value of a short list with a name, for the text of a parameter.
pub trait Choice: Copy + 'static {
    const ALL: &'static [Self];
    fn name(self) -> &'static str;
}

/// The kind of a part. The receiver has a socket with the name of each other kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Receiver,
    Barrel,
    Grip,
    Stock,
    Magazine,
    Sight,
    Accessory,
}

impl Choice for Kind {
    const ALL: &'static [Kind] = &[Kind::Receiver, Kind::Barrel, Kind::Grip, Kind::Stock, Kind::Magazine, Kind::Sight, Kind::Accessory];
    fn name(self) -> &'static str {
        ["receiver", "barrel", "grip", "stock", "magazine", "sight", "accessory"][self as usize]
    }
}

/// A maker: a set of shape rules and stat changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Maker {
    /// Blocky boxes and cut corners, high damage and a low fire rate.
    #[default]
    Blockwerk,
    /// Sleek round shapes and tapers, high accuracy.
    Vesper,
    /// Plates at random offsets, pipes and bolts, a high fire rate and large magazines.
    Scrapjaw,
    /// Rings and coils, a fast reload.
    Halo,
}

impl Choice for Maker {
    const ALL: &'static [Maker] = &[Maker::Blockwerk, Maker::Vesper, Maker::Scrapjaw, Maker::Halo];
    fn name(self) -> &'static str {
        ["blockwerk", "vesper", "scrapjaw", "halo"][self as usize]
    }
}

impl Maker {
    /// The name with a capital letter, for example "Vesper".
    pub fn title(self) -> &'static str {
        ["Blockwerk", "Vesper", "Scrapjaw", "Halo"][self as usize]
    }

    /// The body color as sRGB 0xRRGGBB: olive, off-white, rust or teal.
    pub fn color(self) -> u32 {
        [0x6b7044, 0xd8d4c8, 0x8a4b2a, 0x2f8f8a][self as usize]
    }
}

/// The rarity of a gun. It changes the stats, the chance of the extra parts and the accent color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Choice for Rarity {
    const ALL: &'static [Rarity] = &[Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic, Rarity::Legendary];
    fn name(self) -> &'static str {
        ["common", "uncommon", "rare", "epic", "legendary"][self as usize]
    }
}

impl Rarity {
    /// The chance of each rarity in percent, for a random rarity.
    pub const WEIGHTS: [f64; 5] = [50.0, 25.0, 14.0, 8.0, 3.0];

    /// 0 for common to 4 for legendary.
    pub fn level(self) -> f64 {
        f64::from(self as u8)
    }

    /// The accent color as sRGB 0xRRGGBB: gray, green, blue, purple or orange.
    pub fn color(self) -> u32 {
        [0x9a9a9a, 0x4caf50, 0x3d7bd9, 0x9b4dca, 0xe8892c][self as usize]
    }
}

/// The value of the list with the name `text`, without case. The error lists `also` and the names.
fn choose<T: Choice>(text: &str, also: &str) -> Result<T, String> {
    let word = text.trim().to_ascii_lowercase();
    T::ALL.iter().copied().find(|c| c.name() == word).ok_or_else(|| {
        let names: Vec<&str> = T::ALL.iter().map(|c| c.name()).collect();
        format!("{text:?} is not a choice: write {also}one of {}", names.join(", "))
    })
}

/// The text is the name of the rarity, for example `rare`.
impl Param for Rarity {
    fn default_value() -> Rarity {
        Rarity::Common
    }

    fn from_text(text: &str) -> Result<Rarity, String> {
        choose(text, "")
    }

    fn to_text(&self) -> String {
        self.name().into()
    }
}

impl Viewer for Rarity {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// A parameter with a value of a short list, `random` or `none`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pick<T> {
    #[default]
    Random,
    None,
    Of(T),
}

/// The text is `random`, `none` or a name of the list, for example `vesper`.
impl<T: Choice> Param for Pick<T> {
    fn default_value() -> Pick<T> {
        Pick::Random
    }

    fn from_text(text: &str) -> Result<Pick<T>, String> {
        let word = text.trim().to_ascii_lowercase();
        match word.as_str() {
            "random" => Ok(Pick::Random),
            "none" => Ok(Pick::None),
            _ => choose(text, "random, none or ").map(Pick::Of),
        }
    }

    fn to_text(&self) -> String {
        match self {
            Pick::Random => "random".into(),
            Pick::None => "none".into(),
            Pick::Of(c) => c.name().into(),
        }
    }
}

impl<T: Choice + fmt::Debug + Send> Viewer for Pick<T> {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        out.write_str(&self.to_text())
    }
}

/// The stats of a gun, or the changes that one part gives to them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub damage: f64,
    /// Shots per second.
    pub rate: f64,
    /// 0 to 100.
    pub accuracy: f64,
    pub magazine: f64,
    /// Seconds.
    pub reload: f64,
}

impl std::ops::Add for Stats {
    type Output = Stats;

    fn add(self, other: Stats) -> Stats {
        Stats {
            damage: self.damage + other.damage,
            rate: self.rate + other.rate,
            accuracy: self.accuracy + other.accuracy,
            magazine: self.magazine + other.magazine,
            reload: self.reload + other.reload,
        }
    }
}

/// Shows "damage 41, rate 5.2 per s, accuracy 78, magazine 18, reload 1.9 s".
impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Stats { damage, rate, accuracy, magazine, reload } = *self;
        write!(f, "damage {damage:.0}, rate {rate:.1} per s, accuracy {accuracy:.0}, magazine {magazine:.0}, reload {reload:.1} s")
    }
}

/// One part with its mount at the origin: the body in the color of its maker, the accent in the
/// color of the rarity, the sockets and the stat changes. A part with no maker is no part.
#[derive(Clone, Default)]
pub struct Part {
    pub kind: Kind,
    pub maker: Option<Maker>,
    pub body: Solid,
    pub accent: Solid,
    pub sockets: Sockets,
    pub stats: Stats,
}

impl Part {
    /// The body and the accent as one solid.
    pub fn solid(&self) -> Solid {
        self.body.union(&self.accent)
    }
}

/// Shows "barrel by Vesper, 412 triangles" or "no stock".
impl fmt::Debug for Part {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.maker {
            Some(maker) => write!(f, "{} by {}, {} triangles", self.kind.name(), maker.title(), self.body.triangles() + self.accent.triangles()),
            None => write!(f, "no {}", self.kind.name()),
        }
    }
}

impl Viewer for Part {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }

    fn thumbnail(&self, out: &mut Thumbnail<'_>) {
        self.solid().thumbnail(out);
    }

    fn show(&self, ui: &mut Ui) {
        self.solid().show(ui);
    }
}

/// A gun: the parts at their places on the receiver, the maker of the receiver, the rarity, the
/// stats, and the `muzzle` and `grip` sockets.
#[derive(Clone, Default)]
pub struct Gun {
    pub maker: Maker,
    pub rarity: Rarity,
    pub parts: Vec<Part>,
    pub stats: Stats,
    pub sockets: Sockets,
}

impl Gun {
    /// All the parts as one solid.
    pub fn solid(&self) -> Solid {
        let solids: Vec<Solid> = self.parts.iter().flat_map(|p| [p.body.clone(), p.accent.clone()]).collect();
        Solid::union_all(&solids)
    }

    /// The name, the stats and the parts, with one item on each line.
    pub fn report(&self) -> String {
        let parts: Vec<String> = self.parts.iter().filter_map(|p| Some(format!("{} {}", p.kind.name(), p.maker?.name()))).collect();
        let Stats { damage, rate, accuracy, magazine, reload } = self.stats;
        format!(
            "{} {}\ndamage {damage:.0}\nrate {rate:.1} per s\naccuracy {accuracy:.0}\nmagazine {magazine:.0}\nreload {reload:.1} s\nparts: {}\n",
            self.maker.title(),
            self.rarity.name(),
            parts.join(", ")
        )
    }
}

/// Shows "Vesper rare, damage 41, rate 5.2 per s, accuracy 78, magazine 18, reload 1.9 s".
impl fmt::Debug for Gun {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}, {}", self.maker.title(), self.rarity.name(), self.stats)
    }
}

impl Viewer for Gun {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }

    fn thumbnail(&self, out: &mut Thumbnail<'_>) {
        self.solid().thumbnail(out);
    }

    fn show(&self, ui: &mut Ui) {
        for line in self.report().lines() {
            ui.label(line);
        }
        self.solid().show(ui);
    }
}

#[cfg(test)]
mod tests;
