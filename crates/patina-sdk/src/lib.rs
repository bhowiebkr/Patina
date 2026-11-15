//! The crate that node code uses (spec 3). The engine copies this source into the `build/`
//! folder of each project, so a node always builds with the SDK of its engine.

#[doc(hidden)]
pub mod glue;
mod ui;
mod view;

pub use patina_macros::{connection, node};
pub use ui::{Mode, Number, Response, Ui};
pub use view::{Lanes, Text, Viewer};

/// True when the user clicked **Stop** during the run (15.2). A node that runs for a long time
/// reads it in its loop and returns early. It is false outside a run and on other threads.
pub fn stop_requested() -> bool {
    glue::stop_requested()
}

/// A type that can be a parameter: an input pin with no wire (spec 4.8).
///
/// The text form is for save and undo (6.4). Parse errors give a message for the person.
pub trait Param: Clone + Sized {
    /// The value of a new parameter.
    fn default_value() -> Self;
    /// Reads the text form.
    fn from_text(text: &str) -> Result<Self, String>;
    /// Gives the text form. `from_text` reads it back as the same value.
    fn to_text(&self) -> String;
}

macro_rules! param_from_str {
    ($($ty:ty: $what:literal),*) => {$(
        impl Param for $ty {
            fn default_value() -> Self {
                Self::default()
            }
            fn from_text(text: &str) -> Result<Self, String> {
                text.trim().parse().map_err(|_| format!("{text:?} is not {}", $what))
            }
            fn to_text(&self) -> String {
                self.to_string()
            }
        }
    )*};
}

param_from_str!(
    i8: "an i8", i16: "an i16", i32: "an i32", i64: "an i64", i128: "an i128",
    isize: "an isize", u8: "a u8", u16: "a u16", u32: "a u32", u64: "a u64", u128: "a u128",
    usize: "a usize", bool: "true or false"
);

macro_rules! param_float {
    ($($ty:ty),*) => {$(
        impl Param for $ty {
            fn default_value() -> Self {
                0.0
            }
            fn from_text(text: &str) -> Result<Self, String> {
                text.trim().parse().map_err(|_| format!("{text:?} is not a number"))
            }
            // Debug gives the shortest text that reads back as the same value, with an
            // exponent for very large and very small values (6.4).
            fn to_text(&self) -> String {
                format!("{self:?}")
            }
        }
    )*};
}

param_float!(f32, f64);

impl Param for char {
    fn default_value() -> Self {
        ' '
    }
    fn from_text(text: &str) -> Result<Self, String> {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Ok(c),
            _ => Err(format!("{text:?} is not one character")),
        }
    }
    fn to_text(&self) -> String {
        self.to_string()
    }
}

impl Param for String {
    fn default_value() -> Self {
        String::new()
    }
    fn from_text(text: &str) -> Result<Self, String> {
        Ok(text.to_owned())
    }
    fn to_text(&self) -> String {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::Param;

    fn round_trip<T: Param + PartialEq + std::fmt::Debug>(value: T) {
        assert_eq!(T::from_text(&value.to_text()), Ok(value));
    }

    #[test]
    fn text_forms_read_back() {
        round_trip(2.5f64);
        round_trip(1e-300f64);
        round_trip(f32::MAX);
        round_trip(-7i64);
        round_trip(u128::MAX);
        round_trip(true);
        round_trip('x');
        round_trip(" a b ".to_owned());
        assert_eq!(1e300f64.to_text(), "1e300");
        assert_eq!(f64::from_text(" 4 "), Ok(4.0));
    }

    #[test]
    fn bad_text_gives_a_message() {
        assert_eq!(f64::from_text("abc"), Err("\"abc\" is not a number".to_owned()));
        assert!(u8::from_text("256").is_err());
        assert!(bool::from_text("yes").is_err());
        assert!(char::from_text("ab").is_err());
    }
}
