use super::*;
use patina_sdk::AudioBlock;

#[test]
fn swing_keeps_the_bar_length_and_places_notes_on_sample_boundaries() {
    let settings = Sequence {
        bpm: 120.0,
        swing: 0.25,
        ..Sequence::default()
    };
    let mut clock = Clock::default();
    let mut hits = Vec::new();
    for block in 0..376 {
        let notes = clock.process(&settings);
        for (i, &drums) in notes.drums.iter().enumerate() {
            if drums & 1 != 0 {
                hits.push(block * 256 + i);
            }
        }
    }
    assert_eq!(&hits[..5], &[0, 24000, 48000, 72000, 96000]);
}

#[test]
fn the_envelope_releases_to_silence_and_the_keyboard_retriggers() {
    let mut settings = Sequence {
        playing: false,
        keyboard: 0,
        strike: 1,
        ..Sequence::default()
    };
    let (mut clock, mut env) = (Clock::default(), Envelope::default());
    let ones = AudioBlock::from_fn(|_| 1.0);
    let contour = Contour {
        attack: 0.001,
        release: 0.01,
        ..Contour::default()
    };
    let notes = clock.process(&settings);
    assert!(notes.trigger[0]);
    assert!(env.process(&ones, &notes, &contour).peak() > 0.5);
    assert!(!clock.process(&settings).trigger[0]);
    settings.keyboard = -1;
    for _ in 0..3 {
        env.process(&ones, &clock.process(&settings), &contour);
    }
    assert_eq!(
        env.process(&ones, &clock.process(&settings), &contour)
            .peak(),
        0.0
    );
    settings.keyboard = 3;
    settings.strike += 1;
    assert!(clock.process(&settings).trigger[0]);
}

#[test]
fn the_complete_voice_and_drums_are_finite_audible_and_repeatable() {
    fn render() -> (f64, f32) {
        let (mut clock, mut osc, mut env) =
            (Clock::default(), Oscillator::default(), Envelope::default());
        let (mut filter, mut echo, mut drums) =
            (Filter::default(), Echo::default(), Drums::default());
        let settings = Sequence::default();
        let (mut energy, mut peak) = (0.0, 0.0_f32);
        for _ in 0..750 {
            let notes = clock.process(&settings);
            let raw = osc.process(&notes, &Tone::default());
            let amp = env.process(&raw, &notes, &Contour::default());
            let tone = filter.process(&amp, &Color::default());
            let fx = echo.process(&tone, &Space::default());
            let rhythm = drums.process(&notes);
            for (a, b) in fx.iter().zip(rhythm.iter()) {
                let value = ((a + b * 0.65) * 0.65).tanh() * 0.7;
                assert!(value.is_finite() && value.abs() <= 0.7);
                energy += f64::from(value * value);
                peak = peak.max(value.abs());
            }
        }
        (energy, peak)
    }
    let a = render();
    assert!(a.0 > 100.0 && a.1 > 0.2, "{a:?}");
    assert_eq!(a, render());
}

#[test]
fn invalid_controls_stay_finite_and_a_closed_filter_removes_high_tones() {
    let notes = Notes {
        hz: [6000.0; 256],
        ..Notes::default()
    };
    let mut osc = Oscillator::default();
    let mut filter = Filter::default();
    let color = Color {
        cutoff: 60.0,
        motion: 0.0,
        resonance: 0.0,
        rate: f32::NAN,
    };
    let mut ratio = 0.0;
    for _ in 0..100 {
        let raw = osc.process(
            &notes,
            &Tone {
                shape: 0,
                sub: 0.0,
                detune: f32::NAN,
                octave: 0,
            },
        );
        let out = filter.process(&raw, &color);
        assert!(out.iter().all(|s| s.is_finite()));
        ratio = out.peak() / raw.peak();
    }
    assert!(ratio < 0.01, "{ratio}");
    assert_eq!(
        parse_pattern("7 999 nope -8"),
        [
            7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1
        ]
    );
}

#[test]
fn measure_the_warm_audio_path() {
    use std::hint::black_box;
    use std::time::Instant;
    let (mut clock, mut osc, mut env) =
        (Clock::default(), Oscillator::default(), Envelope::default());
    let (mut filter, mut echo, mut drums) = (Filter::default(), Echo::default(), Drums::default());
    let settings = Sequence::default();
    let mut block = || {
        let notes = clock.process(&settings);
        let raw = osc.process(&notes, &Tone::default());
        let amp = env.process(&raw, &notes, &Contour::default());
        let tone = filter.process(&amp, &Color::default());
        let fx = echo.process(&tone, &Space::default());
        let rhythm = drums.process(&notes);
        black_box(AudioBlock::from_fn(|i| {
            ((fx[i] + rhythm[i] * 0.65) * 0.65).tanh() * 0.7
        }));
    };
    for _ in 0..100 {
        block();
    }
    let mut times = [0.0_f64; 2000];
    allocations::COUNT.set(0);
    allocations::ENABLED.set(true);
    for time in &mut times {
        let begin = Instant::now();
        block();
        *time = begin.elapsed().as_secs_f64() * 1000.0;
    }
    allocations::ENABLED.set(false);
    let count = allocations::COUNT.get();
    assert_eq!(count, 0, "allocations after the pool warms");
    times.sort_by(f64::total_cmp);
    println!(
        "DSP: 256 samples at 48 kHz, 2000 blocks; mean {:.3} ms, p95 {:.3} ms, allocations {count}",
        times.iter().sum::<f64>() / times.len() as f64,
        times[1900]
    );
}

// The test allocator measures this thread only. It is not part of the instrument library.
mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! {
        pub static ENABLED: Cell<bool> = const { Cell::new(false) };
        pub static COUNT: Cell<usize> = const { Cell::new(0) };
    }
    struct Counter;
    // SAFETY: System owns the allocations. The counter only updates thread-local cells.
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if ENABLED.get() {
                COUNT.set(COUNT.get() + 1);
            }
            // SAFETY: The caller supplies the layout required by GlobalAlloc.
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: The pointer and layout came from the matching System allocation.
            unsafe { System.dealloc(ptr, layout) }
        }
    }
    #[global_allocator]
    static ALLOCATOR: Counter = Counter;
}
