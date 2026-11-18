use crate::*;
use patina_sdk::AudioBlock;

fn energy(block: &AudioBlock) -> f32 {
    block.iter().map(|v| v * v).sum::<f32>() / 256.0
}

#[test]
fn held_modulation_changes_only_at_a_rising_edge_and_slew_is_monotonic() {
    let input = AudioBlock::from_fn(|i| i as f32 / 256.0);
    let gate = AudioBlock::from_fn(|i| {
        if (32..64).contains(&i) || i >= 128 {
            1.0
        } else {
            0.0
        }
    });
    let mut hold = SampleHold::default();
    let held = hold.process(&input, &gate);
    assert_eq!(
        (held[31], held[32], held[127], held[128]),
        (0.0, 0.125, 0.125, 0.5)
    );
    let mut slew = Slew::default();
    let ramp = slew.process(&AudioBlock::from_fn(|_| 1.0), 0.01, 0.02);
    assert!(ramp.windows(2).all(|p| p[1] > p[0]));
    assert!((ramp[255] - (1.0 - (-256.0_f32 / 480.0).exp())).abs() < 0.0001);
}

#[test]
fn the_four_filter_modes_reject_the_correct_frequencies() {
    let zero = AudioBlock::silence();
    let mut energies = [[0.0; 2]; 4];
    for (mode, row) in energies.iter_mut().enumerate() {
        for (column, hz) in [100.0_f32, 10_000.0].into_iter().enumerate() {
            let mut filter = Svf::default();
            for block in 0..120 {
                let input = AudioBlock::from_fn(|i| {
                    ((block * 256 + i) as f32 * hz * std::f32::consts::TAU / RATE).sin()
                });
                row[column] = energy(&filter.process(&input, &zero, 1000.0, 0.0, 0.0, mode as u8));
            }
        }
    }
    assert!(
        energies[0][0] > 0.4 && energies[0][1] < 0.0001,
        "{energies:?}"
    );
    assert!(
        energies[1][1] > 0.4 && energies[1][0] < 0.0001,
        "{energies:?}"
    );
    assert!(energies[2].iter().all(|v| *v < 0.03));
    assert!(energies[3].iter().all(|v| *v > 0.4));
}

#[test]
fn the_compressor_reaches_its_ratio_and_dc_filter_removes_offset() {
    let ones = AudioBlock::from_fn(|_| 1.0);
    let mut compressor = Compressor::default();
    let settings = Dynamics {
        threshold: -20.0,
        ratio: 4.0,
        attack: 0.001,
        release: 0.1,
        makeup: 0.0,
    };
    let mut peak = 0.0;
    for _ in 0..150 {
        peak = compressor.process(&ones, settings).peak();
    }
    assert!((20.0 * peak.log10() + 15.0).abs() < 0.02, "peak {peak}");
    let mut dc = DcBlock::default();
    for _ in 0..100 {
        dc.process(&ones);
    }
    assert!(dc.process(&ones).peak() < 0.00001);
}

#[test]
fn the_reverb_has_a_tail_that_decays_and_chorus_moves_the_signal() {
    let zero = AudioBlock::silence();
    let mut reverb = Reverb::default();
    for _ in 0..100 {
        reverb.process(&zero, 0.6, 0.2, 1.0);
    }
    let impulse = AudioBlock::from_fn(|i| f32::from(i == 0));
    reverb.process(&impulse, 0.6, 0.2, 1.0);
    let (mut early, mut late) = (0.0, 0.0);
    for block in 0..600 {
        let wet = reverb.process(&zero, 0.6, 0.2, 1.0);
        assert!(wet.iter().all(|v| v.is_finite()));
        if block < 100 {
            early += energy(&wet);
        }
        if block > 500 {
            late += energy(&wet);
        }
    }
    assert!(early > 0.0001 && late < early * 0.00001, "{early} / {late}");
    let mut chorus = Chorus::default();
    let mut difference = 0.0;
    for block in 0..200 {
        let dry = AudioBlock::from_fn(|i| ((block * 256 + i) as f32 * 0.05).sin());
        let wet = chorus.process(&dry, 0.4, 0.006, 0.7);
        difference += dry
            .iter()
            .zip(wet.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>();
    }
    assert!(difference > 1000.0);
}

#[test]
fn antiderivative_saturation_reduces_a_folded_harmonic() {
    // At 8 kHz the fifth harmonic folds to 8 kHz; use 9 kHz so the third folds to 21 kHz.
    let mut saturator = Saturator::default();
    let zero = AudioBlock::silence();
    for _ in 0..100 {
        saturator.process(&zero, 8.0, 1.0);
    }
    let mut projections = [[0.0_f64; 2]; 2];
    for block in 0..120 {
        let input = AudioBlock::from_fn(|i| {
            (((block * 256 + i) as f32) * std::f32::consts::TAU * 9000.0 / RATE).sin() * 0.5
        });
        let filtered = saturator.process(&input, 8.0, 1.0);
        if block < 20 {
            continue;
        }
        for (i, (&raw, &wet)) in input.iter().zip(filtered.iter()).enumerate() {
            let phase =
                (block * 256 + i) as f64 * std::f64::consts::TAU * 21000.0 / f64::from(RATE);
            for (sum, value) in projections.iter_mut().zip([(raw * 8.0).tanh(), wet]) {
                sum[0] += f64::from(value) * phase.cos();
                sum[1] += f64::from(value) * phase.sin();
            }
        }
    }
    let amplitude = projections.map(|p| p[0].hypot(p[1]));
    assert!(
        amplitude[1] < amplitude[0] * 0.75,
        "alias amplitudes {amplitude:?}"
    );
}

#[test]
fn the_utility_nodes_and_noise_handle_extreme_controls() {
    let (mut lfo, mut noise, mut vco) = (Lfo::default(), Noise::default(), Vco::default());
    let (mut gain, mut scale, mut mixer, mut crush) = (
        Vca::default(),
        Attenuverter::default(),
        Mixer::default(),
        Crusher::default(),
    );
    let mut first = Noise::default();
    let a = first.process(1, 0.5, 123);
    let b = noise.process(1, 0.5, 123);
    assert_eq!(&*a, &*b);
    for shape in 0..4 {
        let cv = lfo.process(f32::NAN, shape, 0.5, 0.5);
        let raw = vco.process(&cv, 1000.0, f32::INFINITY, shape);
        let scaled = scale.process(&raw, -1.0, 0.2);
        let amp = gain.process(&scaled, &cv, 1.0, 0.0);
        let mixed = mixer.process([&raw, &scaled, &amp, &b], [0.2, -0.2, 0.2, f32::NAN]);
        let crushed = crush.process(&mixed, 0, f32::NAN, 1.0);
        assert!(crushed.iter().all(|v| v.is_finite() && v.abs() < 4.0));
    }
}

#[test]
fn waveform_selection_crossfades_instead_of_jumping() {
    let mut shape = crate::voice::WaveShape::default();
    let before = shape.next(0.25, 0.001, 0);
    let first = shape.next(0.25, 0.001, 1);
    assert!((first - before).abs() < 0.01);
    for _ in 0..240 {
        shape.next(0.25, 0.001, 1);
    }
    assert!((shape.next(0.25, 0.001, 1) + 0.5).abs() < 0.0001);
}

#[test]
fn measure_the_expanded_effect_path_without_warm_allocations() {
    use crate::tests::allocations;
    use std::hint::black_box;
    use std::time::Instant;
    let (mut noise, mut lfo, mut vco, mut filter) = (
        Noise::default(),
        Lfo::default(),
        Vco::default(),
        Svf::default(),
    );
    let (mut hold, mut slew, mut amp, mut scale) = (
        SampleHold::default(),
        Slew::default(),
        Vca::default(),
        Attenuverter::default(),
    );
    let (mut mixer, mut drive, mut crusher) =
        (Mixer::default(), Saturator::default(), Crusher::default());
    let (mut chorus, mut reverb, mut dc, mut compressor) = (
        Chorus::default(),
        Reverb::default(),
        DcBlock::default(),
        Compressor::default(),
    );
    let mut block = || {
        let air = noise.process(1, 0.3, 123);
        let cv = lfo.process(0.3, 0, 0.5, 0.5);
        let random = hold.process(&air, &cv);
        let smooth = slew.process(&random, 0.3, 0.3);
        let pitch = scale.process(&smooth, 0.5, 0.1);
        let tone = vco.process(&pitch, 110.0, 0.1, 1);
        let voice = amp.process(&tone, &cv, 0.5, 0.5);
        let mix = mixer.process([&voice, &air, &voice, &air], [0.5, 0.1, 0.1, 0.01]);
        let color = filter.process(&mix, &cv, 1000.0, 0.4, 1.0, 0);
        let driven = drive.process(&color, 4.0, 0.5);
        let crushed = crusher.process(&driven, 12, 24000.0, 0.1);
        let wide = chorus.process(&crushed, 0.3, 0.004, 0.4);
        let room = reverb.process(&wide, 2.0, 0.4, 0.3);
        let clean = dc.process(&room);
        black_box(compressor.process(
            &clean,
            Dynamics {
                threshold: -18.0,
                ratio: 3.0,
                attack: 0.01,
                release: 0.1,
                makeup: 2.0,
            },
        ));
    };
    for _ in 0..100 {
        block();
    }
    let mut times = [0.0_f64; 2000];
    allocations::COUNT.set(0);
    allocations::ENABLED.set(true);
    for time in &mut times {
        let start = Instant::now();
        block();
        *time = start.elapsed().as_secs_f64() * 1000.0;
    }
    allocations::ENABLED.set(false);
    let count = allocations::COUNT.get();
    assert_eq!(count, 0);
    times.sort_by(f64::total_cmp);
    println!(
        "Library: 256 samples, 2000 blocks; mean {:.3} ms, p95 {:.3} ms, allocations {count}",
        times.iter().sum::<f64>() / times.len() as f64,
        times[1900]
    );
}
