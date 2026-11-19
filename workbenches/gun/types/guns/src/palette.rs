//! The materials of the parts, the paint schemes of the makers and the elements.

use std::fmt::{self, Write};

use patina_sdk::{Param, Text, Viewer};

use crate::{Choice, Maker, choose};

/// The material of one solid of a part. Each material has its own color in the scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// The main color of the maker scheme.
    Paint,
    /// The second color of the maker scheme.
    Trim,
    /// Barrels, bolts, rails and other bare metal.
    Metal,
    /// Grips, pads, tape and wood.
    Grip,
    /// Small accents in the color of the rarity.
    Rarity,
    /// Lenses, cells and strips in the glow color of the maker or of the element.
    Glow,
}

impl Material {
    pub const ALL: [Material; 6] = [Material::Paint, Material::Trim, Material::Metal, Material::Grip, Material::Rarity, Material::Glow];

    pub fn name(self) -> &'static str {
        ["paint", "trim", "metal", "grip", "rarity", "glow"][self as usize]
    }
}

impl Maker {
    /// The paint, the trim and the grip color of the scheme `scheme` (0 to 2), as sRGB 0xRRGGBB.
    pub fn scheme(self, scheme: u8) -> [u32; 3] {
        const SCHEMES: [[[u32; 3]; 3]; 4] = [
            [[0x5f6b3a, 0x2a2a26, 0xb59a5e], [0xb89a62, 0x5a4632, 0x2a2a26], [0x7a7f84, 0x33363a, 0x5a4632]],
            [[0xe6e3da, 0x2e3033, 0x3a3436], [0x26272b, 0xb8bcc2, 0x1d1d20], [0xa3283a, 0xe6e3da, 0x2e3033]],
            [[0x8a4b2a, 0xd9b23a, 0x7a5530], [0x2f7f7a, 0x8a4b2a, 0x9a9488], [0xb03a2e, 0x5c5a55, 0x7a5530]],
            [[0x2f8f8a, 0xdde4e2, 0x2b3134], [0x5a3f8f, 0xd8b45a, 0x24222c], [0xd8732a, 0x34383c, 0x2b2d30]],
        ];
        SCHEMES[self as usize][usize::from(scheme % 3)]
    }

    /// The color of bare metal.
    pub fn metal(self) -> u32 {
        [0x3a3b3d, 0x8d9196, 0x5c5a55, 0x9aa3a8][self as usize]
    }

    /// The glow color of a gun with no element.
    pub fn glow(self) -> u32 {
        [0xffb02e, 0x5fe3ff, 0xff5a3a, 0x7dffd8][self as usize]
    }
}

/// The element of the shots. A gun with an element does less direct damage, and its glow parts
/// have the color of the element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Element {
    #[default]
    None,
    Fire,
    Shock,
    Corrosive,
    Cryo,
}

impl Choice for Element {
    const ALL: &'static [Element] = &[Element::None, Element::Fire, Element::Shock, Element::Corrosive, Element::Cryo];
    fn name(self) -> &'static str {
        ["none", "fire", "shock", "corrosive", "cryo"][self as usize]
    }
}

impl Element {
    /// The glow color as sRGB 0xRRGGBB, or no color for no element.
    pub fn color(self) -> Option<u32> {
        [None, Some(0xff6a1f), Some(0x4fb8ff), Some(0x8cff3a), Some(0xbff4ff)][self as usize]
    }

    /// The first word of the name of a gun with this element.
    pub fn prefix(self) -> Option<&'static str> {
        [None, Some("Searing"), Some("Crackling"), Some("Caustic"), Some("Frosted")][self as usize]
    }
}

/// The text is the name of the element, for example `fire`, or `none`.
impl Param for Element {
    fn default_value() -> Element {
        Element::None
    }

    fn from_text(text: &str) -> Result<Element, String> {
        choose(text, "")
    }

    fn to_text(&self) -> String {
        self.name().into()
    }
}

impl Viewer for Element {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}
