# Ember arrangement workbench

The synth workbench adds a song timeline beside the existing modular sound tools.
Note clips and recorded audio share tracks, transport, mixing, and automation.
The node canvas remains the main workspace.

## Research

- [Bitwig tracks](https://www.bitwig.com/userguide/latest/the_arrange_view_and_tracks/)
  use a musical ruler, clips, and track automation.
  [Hybrid tracks](https://www.bitwig.com/userguide/latest/arranger_clips_and_the_browser_panel/)
  accept both notes and audio.
- [Ableton Arrangement](https://www.ableton.com/en/manual/arrangement-view/)
  supports clip movement, trimming, loop regions, locators, fades, and snapping.
  Its timeline and detail editor serve different editing scales.
- [FL Studio workflow](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_workflow.htm)
  connects patterns, the piano roll, the Playlist, the Browser, and the Mixer.
  Ember needs explicit track routing because its canvas already shows signal routing.
- [Bespoke](https://www.bespokesynth.com/docs/)
  combines modular sound processing with sequencers, samples, and song organization.
  This is the closest architectural reference for Patina.
- [Darude's album interview](https://www.musicradar.com/news/classic-album-darude-on-before-the-storm)
  identifies the JP-8080 Sandstorm preset as a pad sound.
  [Sound On Sound's interview](https://www.soundonsound.com/techniques/classic-tracks-darude-sandstorm)
  describes a processed sample for the lead. Its indexed excerpt was available; the full page refused access.
- [Surge XT's manual](https://surge-synthesizer.github.io/manual-xt/)
  describes separate amplitude and filter envelopes, unison, waveshaping, and effect routing.
  These are useful instrument controls beyond note scheduling.
- [Roland's TR-909 history](https://www.roland.com/uk/promos/roland_tr-909/)
  explains its combination of synthesized drums and recorded cymbals.
  Ember's procedural hats are an alternative design, not a TR-909 sample recreation.

Sandstorm is a useful capability test: fast repeated notes, drums, bass, pads, effects, and changes across song sections.
A supersaw alone does not reproduce its lead process.
The supplied learning arrangement must state which sounds are original examples.
It must not claim to contain Darude's recording or original samples.

## Sandstorm hook study

The **sandstorm** graph contains a short hook excerpt at 136 BPM, repeated across four bars.
Its six tracks remain editable in the timeline and piano roll.
Its synthesized accompaniment approximates the bass, drums, and pad.
It does not reproduce the complete song arrangement or use audio from the original recording.

[Darude's Funktasy interview](https://funktasy.com/exclusive-interviews/darude/)
describes an 8-bit source, distortion, and a fifth within the lead sound.
The indexed interview text was available. The full page returned HTTP 403.
The [piano note guide](https://pianoletternotes.blogspot.com/2018/03/sandstorm-by-darude.html)
informed the short hook's pitches and repeated-note pattern.
The octave, gates, mix, accompaniment, and synthesis settings are approximations.

The **Rave lead** instrument filters a low saw and a pulse 31 semitones above it.
The note pitch sets the pulse frequency. Bit reduction and source distortion follow the source filter.
Distortion precedes the amplitude envelope so short notes retain their shape.
The source runs at 192 kHz. A 96-tap FIR reduces ultrasonic content before decimation to 48 kHz.
Its source, amplitude, filter, and effect controls save with the score.
The separate **rave-oscillator** node exposes the same source for modular sound design.
Older saved scores receive default source controls when they load.
The existing **song** graph remains an original learning arrangement.

The earlier render passed numerical checks, but the owner rejected its sound.
Those checks did not establish instrument quality or fidelity to the recording.

## Instrument revision, 2026-11-19

The timeline schedules notes. The workbench's instrument code creates their sound.
The earlier voices shared simple envelopes and offered little control over their harmonic shape.
Track distortion also changed the lead's amplitude envelope after the note envelope had run.

- The lead now shapes its source before its filter and amplitude envelope.
- The bass aligns its sine body with the saw's fundamental. Their earlier phase relationship weakened the fundamental.
- The pad and pulse lead use seven oscillators with controlled detune and stereo spread.
- Each pitched voice has a resonant filter and a velocity-sensitive filter sweep.
- The kick, snare, and hats use separate body, transient, and decay designs.
- Drums play one shot. Closed hats choke open hats; stopping playback releases all voices.
- Track processing adds low cut, a stereo room, and beat-synchronized volume reduction.
- The **Sound** page exposes these controls in responsive columns.
- Track validation now accepts the exact minimum release value represented as an `f32`.

The revised graph moves the lead up one octave and retunes its gates, source, envelopes, effects, and mix.
These choices remain a synthesis approximation. No listening comparison established fidelity to Darude's recording.

### Checks of the instrument revision

Windows workbench tests and Clippy passed. The tests cover source alias reduction, mono compatibility, filter sweeps, drum decay, hat choking, and control validation.
Workspace Clippy, workspace tests, the three frontend builds, and all 203 editor frontend tests passed on Windows.
The actual **sandstorm** graph passed **Edit** and **Bake** rendering with sample differences below 0.00001.
The **Mixer** and **Sound** controls passed edit, save, reopen, and undo checks through the generated view library.

The ten-second render has 480000 stereo frames at 48 kHz.
Its peak is 0.829138. Its RMS over the musical section is 0.199170. No samples reach the 0.95 limiter threshold.
The last-second peak is 0.000475. Six solo renders permit separate checks of the instruments.
The previous and revised mixes also have an RMS-matched comparison file. RMS matching does not establish equal perceived loudness.

The optimized six-track test measured a mean of 0.159 ms and p95 of 0.182 ms per 256 frames.
It counted zero allocations after warmup.
This measures track processing and effects. It excludes the transport, graph scheduler, device output, and editor.
The tests do not establish listening quality. Speaker playback, visual inspection of the new page, and other operating systems remain unchecked.

## Architecture

- The workbench owns scores, instruments, samples, tracks, clips, and effects.
- Patina owns generic node execution, dock views, stereo blocks, and device output.
- One audio node advances the transport in samples. Views only display its position.
- Track nodes receive the same transport block. Their output remains visible on canvas wires.
- A frame node prepares immutable score data and loads files before audio processing.
- Saved graph parameters hold the score. View edits use normal graph edits for save and undo.
- Note timing uses beats. Audio source offsets use seconds. These units stay explicit.
- Each track can contain note clips and WAV clips. A sampler can also play a WAV from notes.
- A stereo mix reaches both device playback and offline WAV export.

## Dock layout

The bottom **Arrange** view provides the clip timeline and transport.
Its detail page provides a chromatic piano roll, velocity, and exact note values.
Its mixer page provides track gain, pan, mute, solo, and effect settings.
The dock can grow vertically or move to the center for detailed editing.

Track selection and clip fields currently sit below the timeline.
Separate left track and right clip inspector views remain planned.
The existing graph panels and learning views remain available.

## Required behavior

- Create, move, trim, split, duplicate, and remove clips.
- Edit overlapping notes, pitch, duration, and velocity.
- Import mono or stereo WAV files. Validate their format and size before allocation.
- Keep imported recordings at their natural speed unless the user changes playback rate.
- Use one loop region and one sample clock across all tracks.
- Release notes on stop, seek, and loop changes.
- Apply automation in the audio domain, including between GUI frames.
- Keep filesystem access, parsing, and allocation outside the audio loop.
- Render the saved arrangement through the same graph that plays it live.

## Progress and checks

The stereo foundation passed Windows workspace Clippy, tests, and frontend checks.
Tests verified independent stereo channels in live graph blocks, WAV export, and the C API.
The existing C API mono function still returns a downmix.
The arrangement, track player, sampler, and dock editors are implemented.
The **song** graph contains six tracks and an original 32-bar study.
The current learning patch still uses mono signal nodes and its original 16-step sequencer.

Windows unit tests passed for channel separation, sample timing, loop boundaries, silence after pause, and finite audio.
View tests passed for clip edits, piano input, and track routing.
A real graph test plays imported 44.1 kHz stereo WAV audio and overlapping synth notes together.
Its **Edit** and **Bake** results match within 0.00001. Its view edits survive save and undo.
Workspace Clippy, workspace tests, and frontend checks passed.
Hidden Windows GUI checks passed clip edits, undo, track creation, piano input, WAV import, saving, and graph switching.
All six demo tracks fit in a 450 px bottom dock. The piano shows note velocity below the keys.
A 60-second **Bake** render contains 2880000 stereo frames at 48 kHz.
Its peak is 0.7252. Its final second has an RMS level of 0.0000065.
The render contains finite samples and distinct left and right channels. These checks do not replace listening.

The six-track debug test processed 256 frames in a mean of 0.73 ms, with a 0.83 ms p95.
The block deadline is 5.33 ms. The warm audio path made zero allocations.
These measurements cover the supplied six-track workload. They do not establish a maximum track count for every computer.
The earlier inactive-voice loop took a mean of 3.72 ms. Skipping inactive voices removed most of that work.

The score supports 32 tracks, 64 clips per track, 2048 notes per pattern, and 200000 expanded note events.
Each track has 32 voices. Import limits each WAV to 128 MiB and the sample cache to 256 MiB.
The timeline displays eight tracks per page. The piano roll displays 24 pitches with octave and time controls.
The musical ruler currently uses four quarter-note beats per bar.

Microphone capture, hardware MIDI, time stretching, compressed codecs, VST hosting, and stem export remain open.
Speaker listening and other operating systems remain unchecked.

## Lead correction of 2026-11-19

The owner rejected the previous melody instrument. That source used a saw and a pulse only seven semitones apart.
[Syntorial's remake recipe](https://www.syntorial.com/preset-recipe/darude-sandstorm-lead/) uses a 31-semitone separation.
Its public audio example has a dominant pulse and a much lower saw component.
The revised source follows that relationship and uses less drive. The low cut, envelope, and effect balance also changed.
The other five instruments keep their previous settings.

The reference is a published remake, not Darude's original isolated lead.
The first Apple preview contained the intro. Its spectrogram was not a valid lead comparison.
The remake spectrum guided the source balance. Regression tests check its pitch relationship and odd harmonics.
No downloaded audio enters the instrument or the repository. Listening comparison with the original lead remains necessary.

A 60 ms B4 segment gave these relative spectral levels. The pulse fundamental is the 0 dB reference.

| Component | Published remake | Previous lead | Revised lead |
| --- | --- | --- | --- |
| Low saw near 82 Hz | -14.1 dB | -64.5 dB | -14.2 dB |
| Unwanted upper fifth near 741 Hz | -36.7 dB | -5.2 dB | -33.5 dB |
| Second pulse harmonic near 988 Hz | -35.0 dB | -6.4 dB | -34.6 dB |
| Third pulse harmonic near 1482 Hz | -10.2 dB | -16.0 dB | -9.7 dB |

These measurements describe one note segment. They do not establish a listening match to the original recording.

## Spectrogram node of 2026-11-19

The owner requested a frequency-over-time viewer on the node canvas.
**audio-capture** writes each block into the existing capture buffer and returns a shared stream handle.
**spectrogram** reads that handle in the frame domain. It shares the FFT implementation with the dock analyzer.
Each FFT uses 2048 samples and a 1024-sample hop. A ring stores 512 spectrum columns without shifting them.
The GUI uses the existing image widget, axes, tabs, and buttons. No host renderer changes are necessary.

The **sandstorm** graph connects the viewer to the melody track through the existing stereo-channel node.
The stereo channel uses the mean by default. Phase cancellation can hide content in that mode; select left or right when necessary.
**Freeze**, **Clear**, frequency range, level floor, and time span operate on the local display state.
Display settings and history reset when the node is rebuilt. They do not alter the score.

A capture overrun resets the history and shows a notice. The display does not invent samples across a missed interval.
This viewer has a fixed 23.4 Hz FFT spacing. Longer windows and saved spectrogram export remain open.
