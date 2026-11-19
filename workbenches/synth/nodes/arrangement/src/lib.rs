use panel::{AMBER, INK, MUTED, TEAL, panel};
use patina_sdk::{Ui, node};
use synth_dsp::song::{Arrangement, Compiler};

/// A saved song with hybrid note and audio tracks. Open the Arrange dock to edit it.
#[node(domain = frame, color = "#655219", free_layout, full_gui)]
pub struct Song {
    compiler: Compiler,
    reload: u32,
    root: std::path::PathBuf,
}
impl Song {
    pub fn init() -> Self {
        Self {
            compiler: Compiler::default(),
            reload: 0,
            root: patina_sdk::project_path(""),
        }
    }
    pub fn process(&mut self, score: &str, reload: u32) -> Arrangement {
        if self.reload != reload {
            self.compiler.reload();
            self.reload = reload;
        }
        self.compiler.process(score, &self.root)
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        let song = &self.compiler.arrangement;
        let beat = song.transport.beat(song.samples_per_beat());
        let mut c = ui.canvas("Ember arrangement overview", [480.0, 222.0]);
        panel(
            &mut c,
            [480.0, 222.0],
            "EMBER / ARRANGEMENT",
            "COMPOSE. LAYER. SHAPE. MIX.",
            AMBER,
        );
        c.text(
            [22.0, 88.0],
            INK,
            &format!(
                "{:.0} BPM   /   {} TRACKS   /   BAR {}",
                song.score.bpm,
                song.score.tracks.len(),
                (beat / 4.0).floor() as u32 + 1
            ),
        );
        for (i, track) in song.score.tracks.iter().take(6).enumerate() {
            let y = 104.0 + i as f32 * 12.0;
            c.text(
                [22.0, y + 9.0],
                MUTED,
                &track.name.chars().take(18).collect::<String>(),
            );
            for clip in &track.clips {
                c.rect(
                    [160.0 + (clip.start / song.score.length) as f32 * 294.0, y],
                    [
                        ((clip.length / song.score.length) as f32 * 294.0 - 1.0).max(1.0),
                        8.0,
                    ],
                    if track.mute { [60, 60, 60] } else { TEAL },
                    true,
                );
            }
        }
        let x = 160.0 + (beat / song.score.length) as f32 * 294.0;
        c.line([x, 100.0], [x, 179.0], AMBER);
        c.text(
            [22.0, 201.0],
            MUTED,
            "Arrange: timeline, piano roll, mixer, and automation.",
        );
        drop(c);
        if ui
            .button(if song.transport.playing() {
                "Pause song"
            } else {
                "Play song"
            })
            .clicked()
        {
            song.transport.play(!song.transport.playing());
        }
        if ui.button("Rewind song").clicked() {
            song.transport.seek(0.0);
        }
        if !self.compiler.error.is_empty() {
            ui.label(&format!(
                "Score error: {}. The last valid score stays active.",
                self.compiler.error
            ));
        }
    }
}
