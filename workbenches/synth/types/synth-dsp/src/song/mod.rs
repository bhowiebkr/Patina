//! Song data and playback owned by the Ember workbench.
mod clock;
mod compile;
mod demo;
mod percussion;
mod player;
mod score;
mod sound;
mod voice;
pub mod wav;
pub use clock::{BeatClock, ClockBlock, Transport};
pub use compile::{Arrangement, CompiledTrack, Compiler, NoteEvent};
pub use demo::demo;
pub use player::TrackPlayer;
pub use score::{
    Automation, Clip, Content, Instrument, MAX_CLIPS, MAX_NOTES, MAX_TRACKS, Note, Score, Target,
    Track,
};
pub use sound::Sound;
#[cfg(test)]
mod sound_tests;
#[cfg(test)]
mod tests;
