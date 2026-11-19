use patina_sdk::{AudioBlock, audio::StereoBlock};
/// Extracts left (0), right (1), or their mean (2) for mono DSP nodes.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub fn channel(sound: &StereoBlock, channel: u8) -> AudioBlock {
    AudioBlock::from_fn(|i| match channel {
        0 => sound.left[i],
        1 => sound.right[i],
        _ => (sound.left[i] + sound.right[i]) * 0.5,
    })
}
#[cfg(test)]
mod tests {
    #[test]
    fn channels_and_downmix_keep_their_levels() {
        let sound = patina_sdk::audio::StereoBlock {
            left: patina_sdk::AudioBlock::from_fn(|_| 0.4),
            right: patina_sdk::AudioBlock::from_fn(|_| -0.2),
        };
        assert_eq!(super::channel(&sound, 0)[0], 0.4);
        assert_eq!(super::channel(&sound, 1)[0], -0.2);
        assert_eq!(super::channel(&sound, 2)[0], 0.1);
    }
}
