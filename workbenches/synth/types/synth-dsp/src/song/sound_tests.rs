use super::*;
use std::path::Path;

fn render(track: &Track, key: u8, count: usize) -> Vec<[f32; 2]> {
    let mut voice = voice::Voice::default();
    voice.start(
        NoteEvent {
            start: 0,
            end: 48000,
            key,
            velocity: 0.9,
            offset: 0,
        },
        0,
    );
    let prepared = sound::Prepared::new(track);
    (0..count)
        .map(|_| voice.next(track, &prepared, None))
        .collect()
}

fn energy(samples: &[[f32; 2]]) -> f32 {
    samples
        .iter()
        .map(|p| (p[0] + p[1]).powi(2) * 0.25)
        .sum::<f32>()
        / samples.len() as f32
}

#[test]
fn sound_defaults_load_old_scores_and_controls_do_not_reset_notes() {
    let mut value: serde_json::Value = serde_json::from_str(&demo().text()).unwrap();
    for track in value["tracks"].as_array_mut().unwrap() {
        track.as_object_mut().unwrap().remove("sound");
    }
    let mut score = Score::parse(&value.to_string()).unwrap();
    assert_eq!(score.tracks[0].sound, Sound::default());
    let mut compiler = Compiler::default();
    let before = compiler.process(&score.text(), Path::new(""));
    score.tracks[0].sound.sweep = 3.5;
    score.tracks[0].sound.room = 0.25;
    let after = compiler.process(&score.text(), Path::new(""));
    assert_eq!(before.revision, after.revision);
    assert_eq!(Score::parse(&score.text()).unwrap(), score);
    score.tracks[0].release = 0.005;
    score.tracks[0].delay_mix = 0.8;
    assert!(score.validate().is_ok());
    assert_eq!(Score::parse(&score.text()).unwrap(), score);
    score.tracks[0].sound.resonance = 1.1;
    assert!(score.validate().is_err());
    score.tracks[0].sound = Sound {
        edge: f32::NAN,
        ..Sound::default()
    };
    assert!(score.validate().is_err());
    score.tracks[0].sound = Sound {
        highpass: -1.0,
        ..Sound::default()
    };
    assert!(score.validate().is_err());
}

#[test]
fn stereo_unison_survives_mono_and_filter_envelopes_change_the_tone() {
    let mut track = Track::new(1, "Pad", Instrument::Pad);
    track.sound.width = 0.9;
    let wide = render(&track, 60, 12000);
    let side = wide
        .iter()
        .map(|p| (p[0] - p[1]).powi(2) * 0.25)
        .sum::<f32>()
        / wide.len() as f32;
    assert!(side > 0.00001 && energy(&wide) > side);
    track.sound.width = 0.0;
    let mono = render(&track, 60, 12000);
    assert!(mono.iter().all(|p| p[0] == p[1]));
    track.instrument = Instrument::Bass;
    track.sound.brightness = 100.0;
    track.sound.sweep = 0.0;
    let closed = render(&track, 40, 12000);
    track.sound.sweep = 5.0;
    track.sound.sweep_decay = 0.16;
    let swept = render(&track, 40, 12000);
    assert!(energy(&swept[..2400]) > energy(&closed[..2400]) * 1.3);
}

#[test]
fn open_hats_decay_longer_and_all_voices_remain_finite_at_extreme_settings() {
    let hat = Track::new(1, "Hat", Instrument::Hat);
    let closed = render(&hat, 42, 12000);
    let open = render(&hat, 46, 12000);
    assert!(energy(&open[6000..]) > energy(&closed[6000..]) * 100.0);
    for instrument in Instrument::ALL
        .into_iter()
        .filter(|i| *i != Instrument::Sampler)
    {
        let mut track = Track::new(1, "Extreme", instrument);
        track.sound.resonance = 0.9;
        track.sound.brightness = 18000.0;
        track.sound.sweep = 6.0;
        track.sound.edge = 16.0;
        for key in [0, 60, 127] {
            assert!(
                render(&track, key, 4096)
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && v.abs() < 8.0)
            );
        }
    }
}

#[test]
fn revised_six_track_patch_has_no_audio_allocations_and_reports_time() {
    let text = include_str!("../../../../graphs/sandstorm.toml")
        .split("'''")
        .nth(1)
        .unwrap();
    let mut score = Score::parse(text).unwrap();
    score.looping = true;
    let song = Compiler::default().process(&score.text(), Path::new(""));
    let mut clock = BeatClock::default();
    let mut players: Vec<_> = score
        .tracks
        .iter()
        .map(|_| TrackPlayer::default())
        .collect();
    let mut times = Vec::with_capacity(2000);
    for block in 0..2020 {
        let time = clock.process(&song);
        crate::tests::allocations::COUNT.set(0);
        crate::tests::allocations::ENABLED.set(block >= 20);
        let start = std::time::Instant::now();
        for (player, track) in players.iter_mut().zip(&score.tracks) {
            let sound = player.process(&song, &time, track.id);
            assert!(
                sound
                    .left
                    .iter()
                    .chain(sound.right.iter())
                    .all(|v| v.is_finite())
            );
        }
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        crate::tests::allocations::ENABLED.set(false);
        assert_eq!(crate::tests::allocations::COUNT.get(), 0);
        if block >= 20 {
            times.push(elapsed);
        }
    }
    times.sort_by(f64::total_cmp);
    println!(
        "Revised six-track patch / 256 frames: mean {:.3} ms, p95 {:.3} ms; zero allocations",
        times.iter().sum::<f64>() / 2000.0,
        times[1900]
    );
}

#[test]
fn a_closed_hat_chokes_the_open_hat_and_short_notes_play_one_shot() {
    let run = |choke: bool| {
        let mut score = demo();
        score.bpm = 120.0;
        let mut track = Track::new(1, "Hats", Instrument::Hat);
        track.release = 0.005;
        track.cutoff = 18000.0;
        let mut notes = vec![Note {
            start: 0.0,
            length: 0.01,
            key: 46,
            velocity: 0.9,
        }];
        if choke {
            notes.push(Note {
                start: 0.5,
                length: 0.01,
                key: 42,
                velocity: 0.001,
            });
        }
        track.clips.push(Clip {
            id: 1,
            name: "Hats".into(),
            start: 0.0,
            length: 4.0,
            fade_in: 0.0,
            fade_out: 0.0,
            content: Content::Notes {
                period: 4.0,
                offset: 0.0,
                notes,
            },
        });
        score.tracks = vec![track];
        let song = Compiler::default().process(&score.text(), Path::new(""));
        assert!(song.error.is_empty(), "{}", song.error);
        let (mut player, mut clock) = (TrackPlayer::default(), BeatClock::default());
        let mut frames = Vec::new();
        for _ in 0..80 {
            let block = player.process(&song, &clock.process(&song), 1);
            frames.extend(
                block
                    .left
                    .iter()
                    .zip(block.right.iter())
                    .map(|(l, r)| [*l, *r]),
            );
        }
        frames
    };
    let open = run(false);
    let choked = run(true);
    assert!(
        energy(&open[4800..9600]) > 1e-6,
        "open hat energy {}",
        energy(&open[4800..9600])
    );
    assert!(energy(&choked[14400..]) < energy(&open[14400..]) * 0.01);
}
