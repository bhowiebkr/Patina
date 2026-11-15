//! The text and the lanes of a value in the editor (spec 5.3, 6.3).

use std::fmt;

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
