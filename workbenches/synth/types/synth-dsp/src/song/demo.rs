use super::{Automation, Clip, Content, Instrument, Note, Score, Target, Track};

/// An original learning arrangement. It uses no recording or sample from Sandstorm.
pub fn demo() -> Score {
    let mut tracks = Vec::new();
    for (id, name, instrument) in [
        (1, "Pulse lead", Instrument::Lead),
        (2, "Sub bass", Instrument::Bass),
        (3, "Kick", Instrument::Kick),
        (4, "Snare", Instrument::Snare),
        (5, "Closed hat", Instrument::Hat),
        (6, "Wide pad", Instrument::Pad),
    ] {
        let mut track = Track::new(id, name, instrument);
        track.gain = match id {
            1 => 0.42,
            2 => 0.52,
            3 => 0.72,
            4 => 0.58,
            5 => 0.45,
            _ => 0.22,
        };
        track.pan = match id {
            4 => -0.12,
            5 => 0.25,
            _ => 0.0,
        };
        if id == 1 {
            track.drive = 3.0;
            track.delay_mix = 0.16;
            track.release = 0.025;
        }
        if id == 6 {
            track.attack = 0.6;
            track.release = 1.4;
            track.delay_mix = 0.22;
            track.cutoff = 3500.0;
        }
        let notes: Vec<Note> = match id {
            1 => [
                62, 62, 69, 62, 65, 62, 72, 69, 62, 65, 62, 69, 74, 72, 69, 65,
            ]
            .iter()
            .enumerate()
            .map(|(i, key)| Note {
                start: i as f64 * 0.25,
                length: 0.16,
                key: *key,
                velocity: if i % 4 == 0 { 0.95 } else { 0.72 },
            })
            .collect(),
            2 => (0..8)
                .map(|i| Note {
                    start: i as f64 * 0.5 + 0.25,
                    length: 0.22,
                    key: 38,
                    velocity: 0.9,
                })
                .collect(),
            3 => (0..4)
                .map(|i| Note {
                    start: f64::from(i),
                    length: 0.7,
                    key: 36,
                    velocity: 1.0,
                })
                .collect(),
            4 => [1.0, 3.0]
                .map(|start| Note {
                    start,
                    length: 0.5,
                    key: 38,
                    velocity: 0.85,
                })
                .to_vec(),
            5 => (0..8)
                .map(|i| Note {
                    start: f64::from(i) * 0.5 + 0.25,
                    length: 0.15,
                    key: 42,
                    velocity: if i % 2 == 0 { 0.8 } else { 0.5 },
                })
                .collect(),
            _ => [50, 57, 62, 65]
                .map(|key| Note {
                    start: 0.0,
                    length: 3.8,
                    key,
                    velocity: 0.65,
                })
                .to_vec(),
        };
        for (part, start, length) in [
            (0, 0.0, 32.0),
            (1, 32.0, 32.0),
            (2, 64.0, 32.0),
            (3, 96.0, 32.0),
        ] {
            if (id == 1 && part == 0) || (id == 3 && part == 2) {
                continue;
            }
            track.clips.push(Clip {
                id: id * 10 + part,
                name: ["INTRO", "PULSE", "BREAK", "RETURN"][part as usize].into(),
                start,
                length,
                fade_in: 0.005,
                fade_out: 0.005,
                content: Content::Notes {
                    period: 4.0,
                    offset: 0.0,
                    notes: notes.clone(),
                },
            });
        }
        if id == 1 {
            track.automation.push(Automation {
                target: Target::Cutoff,
                points: vec![
                    [0.0, 700.0],
                    [31.0, 1800.0],
                    [32.0, 9000.0],
                    [64.0, 900.0],
                    [95.0, 12000.0],
                    [96.0, 9000.0],
                    [128.0, 6000.0],
                ],
            });
        }
        tracks.push(track);
    }
    Score {
        version: 1,
        bpm: 136.0,
        length: 128.0,
        looping: false,
        loop_range: [32.0, 64.0],
        tracks,
    }
}
