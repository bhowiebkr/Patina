use patina_sdk::audio::{BLOCK, StereoBlock};
/// Adds two stereo channels without changing their pan or gain.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub fn mix(a: &StereoBlock, b: &StereoBlock) -> StereoBlock {
    let mut out = StereoBlock::default();
    for i in 0..BLOCK {
        out.left[i] = a.left[i] + b.left[i];
        out.right[i] = a.right[i] + b.right[i];
    }
    out
}
