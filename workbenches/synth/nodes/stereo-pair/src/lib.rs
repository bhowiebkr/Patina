use patina_sdk::{AudioBlock, audio::StereoBlock};
/// Joins two mono channels. Connect the same mono output twice for a centered signal.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub fn pair(left: &AudioBlock, right: &AudioBlock) -> StereoBlock {
    StereoBlock {
        left: left.clone(),
        right: right.clone(),
    }
}
