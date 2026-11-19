use crate::{parameters::*, title};
use patina_sdk::{Graph, Ui};

const LESSONS: [(&str, &str, &str, &str); 6] = [
    (
        "Start with a phrase",
        SEQUENCE,
        "A sequencer repeats notes and drum hits in time.",
        "Open Patterns. Remove a note, then add a rest. Listen for the space between notes.",
    ),
    (
        "Hear the oscillator",
        TIMBRE,
        "An oscillator repeats a waveform. Its shape determines the harmonics.",
        "Compare sine and saw. Use Analysis to see how the saw adds high frequencies.",
    ),
    (
        "Shape each note",
        CONTOUR,
        "Attack, decay, sustain, and release control the level over time.",
        "Raise attack to 0.3 seconds. Hold a note. Compare the soft start with a short attack.",
    ),
    (
        "Remove harmonics",
        COLOR,
        "A low-pass filter removes frequencies above its cutoff. Resonance emphasizes the cutoff.",
        "Lower cutoff from 4000 Hz to 400 Hz. Compare the envelope and filter signals in Analysis.",
    ),
    (
        "Add motion",
        COLOR,
        "A low-frequency oscillator changes another parameter. Ember sweeps the filter cutoff in octaves.",
        "Set motion to 2 octaves. Set LFO rate to 0.5 Hz. Listen for one sweep every two seconds.",
    ),
    (
        "Give sound space",
        SPACE,
        "Delay repeats earlier sound. Feedback sends each repeat into the delay again.",
        "Raise wet to 0.4. Shorten the phrase. Listen to the repeats fill its rests.",
    ),
];

#[derive(Default)]
pub struct Learn {
    lesson: usize,
    follow: bool,
    glossary: bool,
}

impl Learn {
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        title(ui, "EMBER / LEARN", "One experiment at a time");
        ui.toggle("Follow selected node", &mut self.follow);
        if self.follow
            && let Some(index) = LESSONS
                .iter()
                .position(|(_, id, _, _)| graph.selected().any(|s| s == *id))
        {
            self.lesson = index;
        }
        ui.dropdown("Lesson", &mut self.lesson, &LESSONS.map(|l| l.0));
        let (name, id, explanation, task) = LESSONS[self.lesson];
        ui.label(name);
        ui.label(explanation);
        ui.label("TRY THIS");
        ui.label(task);
        if graph.folder(id).is_some() {
            if ui.button("Locate node").clicked() {
                graph.select([id]);
            }
        } else {
            ui.label("Open Ember's main graph to locate this lesson's node.");
        }
        ui.section("Signal glossary", &mut self.glossary, |ui| {
            for text in [
                "Hz: cycles per second. Pitch increases with frequency.",
                "dBFS: level relative to the largest digital sample. 0 dBFS is full scale.",
                "Gate: a signal that holds a note open.",
                "CV: a changing control signal, such as an LFO or envelope.",
                "Octave: a doubling or halving of frequency.",
                "Mono: one sound channel. Ember's current audio path is mono.",
            ] {
                ui.label(text);
            }
        });
        ui.label(
            "Experiments change the patch only when you edit a control. Undo restores your edits.",
        );
    }
}
