use panel::{Ctl, Live, Look, Strip};
use patina_sdk::{AudioBlock, audio::StereoBlock};

/// Extracts left (0), right (1), or their mean (2) for mono DSP nodes.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Channel {
    live: Live,
}
impl Channel {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
        }
    }
    pub fn process(&mut self, sound: &StereoBlock, channel: u8) -> AudioBlock {
        let out = pick(sound, channel);
        self.live
            .keep(&out, &out, [f32::from(channel.min(2)), 0.0, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "CHANNEL",
            subtitle: "STEREO TO MONO",
            color: panel::TEAL,
        };
        gui.show(
            ui,
            shown,
            look,
            &[Ctl::pick("channel", "CHANNEL", &["LEFT", "RIGHT", "MIX"])],
        );
    }
}

fn pick(sound: &StereoBlock, channel: u8) -> AudioBlock {
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
        assert_eq!(super::pick(&sound, 0)[0], 0.4);
        assert_eq!(super::pick(&sound, 1)[0], -0.2);
        assert_eq!(super::pick(&sound, 2)[0], 0.1);
    }
}
