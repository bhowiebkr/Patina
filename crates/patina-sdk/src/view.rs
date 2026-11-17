//! The text, the thumbnail, the widgets and the lanes of a value in the editor (spec 5.3, 6.3).

use std::fmt;

use crate::Ui;

/// A type crate implements `Viewer` to show its values in the editor (5.3). Without it, the
/// editor shows the `Debug` text of the value, or the type name and the size.
///
/// The **Inspector** shows the full text of a selected wire. The badge on the wire shows its
/// start (15.9).
///
/// ```
/// use std::fmt::{self, Write};
///
/// pub struct Byte(pub u8);
///
/// impl patina_sdk::Viewer for Byte {
///     fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
///         write!(out, "0x{:02x}", self.0)
///     }
/// }
/// ```
pub trait Viewer {
    /// Writes the text of the value. A long text stops at the limit of `Text`.
    fn text(&self, out: &mut Text<'_>) -> fmt::Result;

    /// Draws a small picture of the value, for example an image preview. The node body shows it
    /// under the pins, and the **Inspector** shows it for a selected wire. The default draws none.
    fn thumbnail(&self, out: &mut Thumbnail<'_>) {
        let _ = out;
    }

    /// Shows the value with the widgets of a node GUI (4.3), for example a plot, a waveform or a
    /// table. The **Inspector** shows them under the text of a selected wire. The widgets take no
    /// input, and `ui.mode()` is `Mode::Large`. The default shows none.
    ///
    /// ```
    /// use std::fmt::{self, Write};
    ///
    /// pub struct Samples(pub Vec<f32>);
    ///
    /// impl patina_sdk::Viewer for Samples {
    ///     fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
    ///         write!(out, "{} samples", self.0.len())
    ///     }
    ///
    ///     fn show(&self, ui: &mut patina_sdk::Ui) {
    ///         ui.waveform("Samples", &self.0, None);
    ///     }
    /// }
    /// ```
    fn show(&self, ui: &mut Ui) {
        let _ = ui;
    }
}

/// The small picture of a value (5.3): RGBA pixels, row by row from the top. It allocates only
/// for the first picture of a thread.
///
/// ```
/// use std::fmt::{self, Write};
///
/// pub struct Gray(pub u8);
///
/// impl patina_sdk::Viewer for Gray {
///     fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
///         write!(out, "gray {}", self.0)
///     }
///
///     fn thumbnail(&self, out: &mut patina_sdk::Thumbnail<'_>) {
///         for pixel in out.pixels(8, 8).chunks_mut(4) {
///             pixel.copy_from_slice(&[self.0, self.0, self.0, 255]);
///         }
///     }
/// }
/// ```
pub struct Thumbnail<'a> {
    buf: &'a mut Vec<u8>,
    size: [u32; 2],
}

impl<'a> Thumbnail<'a> {
    /// The longest side of a thumbnail, in pixels.
    pub const MAX: u32 = 64;

    pub(crate) fn new(buf: &'a mut Vec<u8>) -> Thumbnail<'a> {
        Thumbnail { buf, size: [0, 0] }
    }

    /// Sets the size and gives the pixels, 4 bytes for each pixel, all 0 at first. A side longer
    /// than `MAX` becomes `MAX`. A side of 0 draws no picture.
    pub fn pixels(&mut self, width: u32, height: u32) -> &mut [u8] {
        self.size = [width.min(Self::MAX), height.min(Self::MAX)];
        self.buf.clear();
        self.buf.resize((self.size[0] * self.size[1] * 4) as usize, 0);
        self.buf
    }

    pub(crate) fn size(&self) -> [u32; 2] {
        self.size
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.buf
    }
}

/// Adds `bytes` to `out` in base64, with '=' at the end.
pub(crate) fn push_base64(out: &mut String, bytes: &[u8]) {
    const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0_u32, |n, (i, &b)| n | (u32::from(b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() { char::from(DIGITS[((n >> (18 - 6 * i)) & 63) as usize]) } else { '=' });
        }
    }
}

/// The lanes of a value on a wire (6.3, 15.9). A connection kind returns it. The first lane is
/// the most significant bit, at the top of a wire that goes from left to right.
///
/// ```
/// use patina_sdk::{connection, Lanes};
///
/// /// Shows a byte as 8 lanes.
/// #[connection]
/// pub fn byte_bus(value: &u8) -> Lanes {
///     Lanes::bits(8, u64::from(*value)).label("data")
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lanes {
    pub(crate) count: u32,
    pub(crate) bits: u64,
    pub(crate) label: &'static str,
}

impl Lanes {
    /// One lane for each of the low `count` bits of `bits`. The count is 1 to 64.
    pub fn bits(count: u32, bits: u64) -> Lanes {
        Lanes { count: count.clamp(1, 64), bits, label: "" }
    }

    /// The name of the bus in the wire label, for example "data" in "data[7:0] / 8".
    pub fn label(self, label: &'static str) -> Lanes {
        Lanes { label, ..self }
    }
}

/// The text of a value. It does not allocate. A full text stops the formatting with an error,
/// and ends with "...".
pub struct Text<'a> {
    buf: &'a mut [u8],
    len: usize,
    cut: bool,
}

impl<'a> Text<'a> {
    /// The last 3 bytes of `buf` are for the "..." of a long text.
    pub(crate) fn new(buf: &'a mut [u8]) -> Text<'a> {
        Text { buf, len: 0, cut: false }
    }

    pub(crate) fn clear(&mut self) {
        self.len = 0;
        self.cut = false;
    }

    pub(crate) fn finish(&mut self) -> &str {
        if self.cut {
            self.buf[self.len..self.len + 3].copy_from_slice(b"...");
            self.len += 3;
            self.cut = false;
        }
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or_default()
    }
}

impl fmt::Write for Text<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.cut {
            return Err(fmt::Error);
        }
        let mut end = s.len().min(self.buf.len() - 3 - self.len);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        self.buf[self.len..self.len + end].copy_from_slice(&s.as_bytes()[..end]);
        self.len += end;
        self.cut = end < s.len();
        if self.cut { Err(fmt::Error) } else { Ok(()) }
    }
}
