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

The new **Rave lead** instrument filters a root-and-fifth source before bit reduction and track distortion.
Its **Fifth level**, **Source body / Hz**, and **Source bits** controls save with the score.
The separate **rave-oscillator** node exposes the same source for modular sound design.
Older saved scores receive default source controls when they load.
The existing **song** graph remains an original learning arrangement.

Windows example tests and Clippy passed for the source and its controls.
Workspace Clippy, workspace tests, and frontend checks passed on Windows.
One compiler process crashed during a state-migration test. Its retry and the full test run passed with two test threads.
The **sandstorm** graph passed **Edit** and **Bake** rendering, with sample differences below 0.00001.
The actual dock view passed a lead-control edit, save, reopen, and undo test.
A ten-second stereo WAV render contains 480000 frames at 48 kHz.
Its peak is 0.8830. Its left and right RMS levels are about 0.1673 and 0.1675.
The last-second RMS is 0.0000424. The source test counted zero allocations.
The debug source test averaged 0.017 ms per 256 samples across three source settings.
This measurement excludes track effects, mixing, the graph scheduler, and device output.
No listening comparison established fidelity to the original recording. Other operating systems remain unchecked.

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
