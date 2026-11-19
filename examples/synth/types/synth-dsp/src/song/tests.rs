use super::*;
use std::{path::Path, sync::Arc};

#[test]
fn score_roundtrip_rejects_bad_controls_and_duplicate_ids() {
    let score = demo();
    assert_eq!(Score::parse(&score.text()).unwrap(), score);
    let mut bad = score.clone();
    bad.tracks[0].pan = 2.0;
    assert!(bad.validate().is_err());
    bad = score.clone();
    bad.tracks[1].id = bad.tracks[0].id;
    assert!(bad.validate().is_err());
    bad = score;
    bad.tracks[0].automation[0].points[1][0] = 0.0;
    assert!(bad.validate().is_err());
    assert!(Score::parse("{\"version\":99}").is_err());
}

#[test]
fn tempo_keeps_recordings_at_their_natural_duration_and_notes_on_beats() {
    let mut score = demo();
    let old = score.tracks[0].clips[0].length;
    score.tracks[0].clips[0].content = Content::Audio {
        path: "test.wav".into(),
        offset_seconds: 0.0,
        rate: 1.0,
    };
    let seconds = old * 60.0 / score.bpm;
    let note_length = score.tracks[1].clips[0].length;
    score.set_tempo(240.0).unwrap();
    assert!((score.tracks[0].clips[0].length * 60.0 / score.bpm - seconds).abs() < 1e-9);
    assert_eq!(score.tracks[1].clips[0].length, note_length);
    assert!(score.set_tempo(f64::NAN).is_err());
}

#[test]
fn mixer_controls_and_names_do_not_restart_held_notes() {
    let mut compiler = Compiler::default();
    let mut score = demo();
    let before = compiler.process(&score.text(), Path::new(""));
    score.tracks[0].gain = 0.2;
    score.tracks[0].name = "Renamed".into();
    score.tracks[0].clips[0].name = "Renamed clip".into();
    let after = compiler.process(&score.text(), Path::new(""));
    assert_eq!(before.revision, after.revision);
    score.tracks[0].clips[0].start += 1.0;
    score.tracks[0].clips[0].length -= 1.0;
    assert!(compiler.process(&score.text(), Path::new("")).revision > after.revision);
}

#[test]
fn clock_keeps_tracks_on_the_same_sample_and_wraps_inside_a_block() {
    let mut compiler = Compiler::default();
    let mut score = demo();
    score.bpm = 120.0;
    score.looping = true;
    score.loop_range = [0.0, 1.0];
    let song = compiler.process(&score.text(), Path::new(""));
    assert!(compiler.error.is_empty());
    let mut clock = BeatClock::default();
    let mut resets = Vec::new();
    for block in 0..200 {
        let time = clock.process(&song);
        for i in 0..256 {
            assert_eq!(time.position[i], (block * 256 + i) as u64 % 24000);
            if time.reset[i] {
                resets.push(block * 256 + i);
            }
        }
    }
    assert_eq!(resets, [0, 24000, 48000]);
    song.transport.play(false);
    let paused = clock.process(&song);
    assert!(paused.running.iter().all(|v| !v));
    assert!(paused.position.iter().all(|v| *v == paused.position[0]));
    song.transport.seek(0.5);
    song.transport.play(true);
    assert_eq!(clock.process(&song).position[0], 12000);
}

fn float_wav(frames: &[[f32; 2]]) -> Vec<u8> {
    let length = (frames.len() * 8) as u32;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((length + 36).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(3_u16.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(48000_u32.to_le_bytes());
    bytes.extend(384000_u32.to_le_bytes());
    bytes.extend(8_u16.to_le_bytes());
    bytes.extend(32_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(length.to_le_bytes());
    for frame in frames {
        for value in frame {
            bytes.extend(value.to_le_bytes());
        }
    }
    bytes
}

#[test]
fn wav_keeps_channels_and_rejects_corrupt_chunks_and_nonfinite_samples() {
    let bytes = float_wav(&[[0.25, -0.5], [0.75, -0.125]]);
    let sample = wav::Sample::decode(&bytes).unwrap();
    assert_eq!(sample.at(0.0, 1.0), [0.25, -0.5]);
    assert_eq!(sample.at(1.0, 1.0), [0.75, -0.125]);
    assert_eq!(sample.at(2.0, 1.0), [0.0, 0.0]);
    assert!(wav::Sample::decode(&bytes[..bytes.len() - 1]).is_err());
    assert!(wav::Sample::decode(&float_wav(&[[f32::NAN, 0.0]])).is_err());
    let mut bad = bytes;
    bad[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(wav::Sample::decode(&bad).is_err());
}

#[test]
fn audio_and_notes_share_a_track_and_solo_and_pause_release_sound() {
    let mut compiler = Compiler::default();
    let mut song = compiler.process(&demo().text(), Path::new(""));
    let mut score = (*song.score).clone();
    score.bpm = 120.0;
    score.tracks[0].cutoff = 18000.0;
    score.tracks[0].delay_mix = 0.0;
    score.tracks[0].drive = 1.0;
    score.tracks[0].release = 0.005;
    song = compiler.process(&score.text(), Path::new(""));
    let sample = Arc::new(wav::Sample::decode(&float_wav(&vec![[0.5, -0.25]; 4800])).unwrap());
    Arc::make_mut(&mut song.tracks)[0]
        .audio
        .push(super::compile::AudioEvent {
            clip: 99,
            start: 0,
            end: 4800,
            offset: 0.0,
            step: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            sample,
        });
    let (mut clock, mut player) = (BeatClock::default(), TrackPlayer::default());
    let block = player.process(&song, &clock.process(&song), 1);
    assert!(block.left[255] > 0.1 && block.right[255] < -0.05);
    song.transport.seek(32.0);
    let block = player.process(&song, &clock.process(&song), 1);
    assert!(block.peak() > 0.01);
    song.transport.play(false);
    for _ in 0..40 {
        player.process(&song, &clock.process(&song), 1);
    }
    assert!(player.process(&song, &clock.process(&song), 1).peak() < 0.0001);
    let mut score = (*song.score).clone();
    score.tracks[1].solo = true;
    let song = compiler.process(&score.text(), Path::new(""));
    song.transport.seek(32.0);
    song.transport.play(true);
    for _ in 0..40 {
        player.process(&song, &clock.process(&song), 1);
    }
    assert!(player.process(&song, &clock.process(&song), 1).peak() < 0.0001);
}

#[test]
fn song_audio_has_no_allocations_after_warmup_and_reports_processing_time() {
    let song = Compiler::default().arrangement;
    song.transport.seek(32.0);
    let mut clock = BeatClock::default();
    let mut players: Vec<_> = song
        .score
        .tracks
        .iter()
        .map(|_| TrackPlayer::default())
        .collect();
    for _ in 0..5 {
        let time = clock.process(&song);
        for (p, t) in players.iter_mut().zip(&song.score.tracks) {
            p.process(&song, &time, t.id);
        }
    }
    let mut times = Vec::with_capacity(2000);
    let mut peak = 0.0_f32;
    for _ in 0..2000 {
        crate::tests::allocations::COUNT.set(0);
        crate::tests::allocations::ENABLED.set(true);
        let start = std::time::Instant::now();
        let time = clock.process(&song);
        for (p, t) in players.iter_mut().zip(&song.score.tracks) {
            let block = p.process(&song, &time, t.id);
            assert!(
                block
                    .left
                    .iter()
                    .chain(block.right.iter())
                    .all(|v| v.is_finite())
            );
            peak = peak.max(block.peak());
        }
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        crate::tests::allocations::ENABLED.set(false);
        assert_eq!(crate::tests::allocations::COUNT.get(), 0);
        times.push(elapsed);
    }
    times.sort_by(f64::total_cmp);
    assert!(peak > 0.1);
    println!(
        "Six tracks, 256 frames: mean {:.3} ms, p95 {:.3} ms; zero audio allocations",
        times.iter().sum::<f64>() / 2000.0,
        times[1900]
    );
}
