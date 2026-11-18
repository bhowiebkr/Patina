/// A constant audio or control block. Zero supplies silence to unused audio inputs. One supplies a full gate.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub fn constant(value: f32) -> patina_sdk::AudioBlock {
    let value = synth_dsp::finite(value, 0.0, -32.0, 32.0);
    patina_sdk::AudioBlock::from_fn(|_| value)
}
