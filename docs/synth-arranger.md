# Ember arrangement workbench

The next synth workbench adds a song timeline above the existing modular sound tools.
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

The left track list identifies each track and its sound source.
The right inspector edits the selected track or clip, including trims and fades.
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
The arrangement, track player, sampler, and dock editors are not implemented yet.
The current learning patch still uses mono signal nodes and its original 16-step sequencer.

Acceptance checks cover channel separation, sample timing, loop boundaries, silence after stop, and finite audio.
They also cover clip edits, save and undo, imported files, and matching **Edit** and **Bake** renders.
Measure audio processing time and allocations after warmup.
Speaker listening and other operating systems require separate checks.
