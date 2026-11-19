/// Transposes pitch while preserving the gates, retriggers and drum events.
#[patina_sdk::node(domain = audio, color = "#655219")]
pub fn transpose(notes: &synth_dsp::Notes, semitones: f32) -> synth_dsp::Notes {
    let ratio = 2.0_f32.powf(synth_dsp::finite(semitones, 0.0, -48.0, 48.0) / 12.0);
    let mut out = notes.clone();
    for hz in &mut out.hz {
        *hz = synth_dsp::finite(*hz * ratio, 110.0, 10.0, 16_000.0);
    }
    out
}
