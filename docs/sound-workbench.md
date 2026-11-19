# Sound workbench

## Purpose

Owner direction, 2026-11-18: each example is a workbench for a specific domain.
An example needs useful modules, controls, inspection tools, and complete workflows.
The sound workbench must help a beginner learn synthesis and let a sound designer make useful sounds.

The first instrument is [Ember](../examples/synth/README.md).
Its three patches share 34 node types, instrument panels, and a reusable DSP library.
The CLI renders saved patches to WAV files for further work in a sound editor.

Owner direction, 2026-11-18: Patina supplies the architecture. Each workbench supplies a specific tool built on that architecture.
The sound workbench needs its own views and widgets around the graph.
Keep the graph, docks, undo, project files, build tools, and shared controls in Patina.
Keep sound presets, performance controls, musical editing, and audio analysis in Ember.

## GUI research, 2026-11-18

This review uses official product pages and manuals. It also checks the current Ember source and Patina view API.
The items below are recommendations. A working drawing does not prove the audio or file workflow behind it.

### What other instruments provide

| Reference | Relevant UI features | Lesson for Ember |
| --- | --- | --- |
| [Surge XT manual](https://surge-synthesizer.github.io/manual-xt/) | Patch search, categories, favorites, macros, modulation list, numeric edits, and standalone MIDI/audio setup | Give sound design its own control surface and sound browser. |
| [Bespoke reference](https://www.bespokesynth.com/docs/) | Pinned controls, transport, snapshots, sample browser, song scenes, and capture of recent audio | Keep useful performance controls available while the user explores the patch. |
| [VCV Rack getting started](https://vcvrack.com/manual/GettingStarted) | Audio device setup, MIDI input, module browsing, fine knob adjustment, and parameter reset | Make connection setup and precise control part of the tool. |
| [VCV Rack menu](https://vcvrack.com/manual/MenuBar) | Undo, save, tooltips, module position lock, and optional performance meters | Reuse Patina's editor functions. Add sound-specific feedback where needed. |
| [Vital](https://vital.audio/) | Animated modulation, filter responses, waveforms, spectrum displays, and modulation previews | Show what a control changes. Distinguish a response preview from measured audio. |
| [Hive 2](https://u-he.com/products/hive/) | Assignable XY pads and a modulation matrix | Provide a small set of expressive controls for several sound parameters. |

A synthesizer needs precise parameter controls, sound selection, modulation editing, and output feedback.
A standalone synthesizer also needs device setup and a way to play notes.
Arrangement, sample editing, and multitrack recording belong to a larger sound workbench. They can follow the core instrument workflow.

### Missing work surfaces

Priority 1 completes the basic sound-design workflow. Priority 2 adds deeper editing. Priority 3 adds production workflows.

| Priority | View or component | Current evidence | Proposed work |
| --- | --- | --- | --- |
| 1 | Performance view | This task adds and checks `views/perform` on Windows. | Connect scene selection, phrase transport, useful sound controls, and precise values to the existing graph. |
| 1 | Sound preset browser | Three hard-coded scenes and three phrase patterns exist. Project and Library browse code and graphs. | Add named sound presets, search, tags, favorites, save, and a modified-state marker. Protect unsaved edits when loading. |
| 1 | Output and audio setup | Master and drum levels exist. The scope shows peak and RMS. The host chooses the default output device. | Show output level, mute, clipping, device availability, and block deadline failures. Device selection needs host support. |
| 1 | Export panel | The tested CLI writes saved patches to float WAV. No editor export action exists. | Add duration, destination, render progress, cancellation, and result feedback. Reuse the renderer. |
| 1 | Complete parameter widget | Ember knobs support dragging and arrow keys. Exact values live in the Inspector. | Put precise entry, units, fine adjustment, reset, and help beside the control. Use the existing graph undo path. |
| 1 | Shared view layout and gestures | `Ui` supports a column and fixed widget positions. View edits group by call. | Add reusable rows and columns with size constraints. Group a complete view drag into one undo step. |
| 2 | Modulation view | Users connect modulation nodes and edit their parameters. No routing overview exists. | Show source, destination, depth, polarity, range, and bypass. A row must edit the actual graph connection or parameter. |
| 2 | Performance macros and XY pad | General Control supplies one scalar. Some patches expose three useful knobs. | Add named controls with visible assignments. Start with fixed, useful assignments before a general mapping editor. |
| 2 | Analysis view | A scope node shows one 256-sample block and a short spectrum. | Add source selection, a triggered scope, FFT size, peak hold, and frequency/dB axes. Support low-frequency inspection. |
| 2 | Musical transport and pattern editor | The phrase has tempo, swing, gate, a 16-step grid, and a small keyboard. | Add clear position, reset, pattern length, velocity, note length, and scale controls. Keep transport separate from engine Run. |
| 2 | MIDI and performance input | The node keyboard accepts eight computer keys. No MIDI input exists. | Add device/channel selection, activity, learn/clear assignment, pitch bend, sustain, and all-notes-off. Build the input path first. |
| 2 | Guided learning | The README explains several listening experiments. | Add a view with short experiments, control explanations, and selection of the related nodes. |
| 3 | Automation and arrangement | Patina has timeline and curve widgets. Ember has no automation playback or recording model. | Connect lanes to real parameter targets and the audio clock. Add loop, snap, and record modes after that works. |
| 3 | Sample browser and editor | Patina has asset and waveform widgets. Ember has no sample loader or player. | Build import and playback first. Then add audition, trim, loop boundaries, pitch, and sample metadata. |
| 3 | Mixer and recording | Ember mixes mono signals. There are no stereo buses, live recording, or stems. | Add channel strips, mute/solo, sends, pan, meters, and recording when the audio model supports them. |

### Visualizer and utility nodes

The existing **SIGNAL / OBSERVATORY** node already computes a waveform, spectrum, peak level, and RMS level from its input block.
It displays 256 samples, or 5.33 ms at 48 kHz. Its spectrum starts near 188 Hz.
That window cannot show a full cycle of a low bass note or a slow modulation signal.
Its tests check sample measurements. This review did not prove continuous capture from the live audio thread.

[VCV Scope](https://vcvrack.com/Free) provides two inputs, time and gain controls, triggering, and XY display.
[Bespoke](https://www.bespokesynth.com/docs/) provides separate waveform, spectrum, audio meter, modulation visualizer, note display, and Lissajous modules.
These references support analysis as patchable nodes, with an optional larger view for inspection.

| Proposed node | What it explains | Ember gap |
| --- | --- | --- |
| Triggered oscilloscope | Wave shape, clipping, phase, and envelope changes | Add continuous history, time range, trigger level, freeze, and two-signal comparison to the existing scope. |
| Spectrum analyzer | Fundamental frequency, harmonics, filter action, and unwanted noise | Add a longer analysis window, frequency and dB axes, a cursor, and peak hold. |
| Spectrogram | How frequency content changes over time | Add a scrolling frequency image for sweeps, drum attacks, and effect tails. |
| Level meter | Peak level, average level, and clipping | Add a compact meter, clip hold/reset, and before/after comparison. The scope already provides basic peak and RMS numbers. |
| Modulation monitor | What an LFO, envelope, gate, or control signal does | Add a seconds-long trace with values and trigger markers. Existing envelope and filter drawings preview settings. |
| Tuner | Detuning, note frequency, and pitch error | Add pitch estimation with a confidence indicator. Silence and noisy inputs must not produce a false note. |
| Stereo vectorscope and correlation meter | Stereo width and possible cancellation when channels mix to mono | Build stereo signal support first. The current sound path is mono. |
| Note and event monitor | Notes, velocity, gates, and timing | Show the actual note stream. MIDI monitoring needs a MIDI input path first. |

Start with the scope, spectrum, and modulation monitor. These directly support the learning exercises.
For example, compare an oscillator with its filtered output while moving cutoff. The display should explain the audible change.
Keep the current console node. Give analysis nodes enough space for readable axes and controls.
An analysis view can enlarge a selected node's data. Reuse the same captured data instead of running a second analyzer.

Continuous analysis needs bounded buffers between audio processing and display work.
Keep allocation, drawing, and expensive analysis outside the audio callback. Limit display refresh independently from the sample rate.
Record the sample rate and capture position. Show an unavailable or stopped state when current samples cannot be read.

Other missing node families include a sample player, looper, arpeggiator, scale quantizer, wavetable oscillator, and MIDI input/output.
These are proposed work. Sample import, polyphonic note events, stereo signals, and device access require supporting data paths.
Add each family with a usable example patch and checks, rather than an unused node name.

### Architecture findings

The recommended workspace keeps the graph available and adds task-specific surfaces:

| Location | Work surface | Main use |
| --- | --- | --- |
| Center tabs | **Graph** and **Perform** | Build the instrument or play it. |
| Left dock | Sound browser | Find and save sounds. Keep code libraries in the existing **Library** tab. |
| Right dock | Modulation and analysis | Inspect assignments, levels, and frequency content. |
| Lower dock | Pattern and automation editor | Edit notes and changes over musical time. |
| Output panel | Audio setup, monitoring, and export | Check the output path and make an audio file. |

These are proposed placements. Users can use Patina's existing docking system to arrange views for their task.

Patina already has the needed panel host: `#[view]`, dock tabs, floating panels, and `Graph` access.
A view reads graph parameters and writes through `Graph::set_param`. The editor groups each call's edits for undo.
The view must read current parameters again after undo or a change from another panel.
The graph remains the source of saved sound settings.

Existing SDK controls include tabs, sections, fields with units, sliders, curves, waveforms, tables, and timelines.
Reuse them. The existence of a timeline widget does not provide sample-accurate automation playback.
Patina's **Debug** panel already shows the audio block deadline. A sound status panel can summarize that information.
Ember's rotary widget now shares its interaction and drawing between nodes and views.
Node controls write with `Ui::set_param`. View controls write with `Graph::set_param`.

The current view handle describes the open graph. It cannot navigate arbitrary nested patches by itself.
Typed reads require a connected output. Audio-thread output reads remain a known limit in `STATUS.md`.
A new analyzer needs a tested audio-to-view data path before it can show measurements.
It must never invent a moving waveform or label old samples as live.

Use separate view state for tab choice, zoom, meter hold, and pointer drags.
Use graph state for sound parameters, patterns, and stored presets.
Use transient input for held notes. Do not put every key press into the undo history.

The first view also exposes two shared GUI gaps. Its fixed canvas needs vertical scrolling in a short dock.
Flexible widget groups would let controls rearrange when a dock becomes narrow or short.
Node GUI drags already have gesture IDs. View drags need the same undo grouping across calls.
These improvements belong to Patina's shared GUI API, because other workbenches need them too.

### First implementation

The first implementation adds a **perform** view for the existing main patch.
It brings scene selection, phrase transport, master level, and phrase sound controls into one surface.
Its XY pad controls cutoff and resonance. The exact-value field edits the selected control.
The pad labels its assignments and accepts arrow keys. The **Learn** page provides five short experiments.
The shared dial code serves both nodes and the view. The view adds no audio processing and no external dependency.

The view binds the main patch's stable node IDs and checks their folders.
It cannot control a node that was removed, replaced, or given a connected parameter input.
The phrase controls remain phrase controls when another scene plays. The view states that limit.
The envelope display previews settings with a fixed 0.4-second sustain hold. It is not a live measurement.
An XY input groups both parameter edits. A continuous drag can still create several undo steps, one for each view call.

Windows validation passed: workspace clippy and tests, three frontend builds, frontend tests, and the standalone example checks.
The integration check builds the view in Edit and Bake. It changes both XY parameters, saves, reopens, and undoes the edit.
The hidden GUI check operates scenes, phrase transport, XY input, exact values, reset, and all five pages.
It also checks a 1280 x 720 window and access to the controls through vertical scrolling.
The GUI check uses synthetic page events on a hidden desktop. It does not test physical controllers or listening quality.
Linux and macOS were not run.

The implementation does not yet provide presets, live analysis, device setup, MIDI, recording, or a GUI export command.

Then add the preset browser, output monitoring, and export panel.
These complete the loop: choose a sound, change it, compare it, save it, and export it.

## Panel design

Size each node for its job. Do not force all nodes into one size.
The sequencer needs horizontal space for 16 steps. The envelope needs a readable curve and four controls.
The scope needs enough display area to compare the signal and its spectrum.
Small processing nodes can remain compact between these panels.

Use the shared charcoal palette. Use amber for sources, pink for envelopes, teal for filtering, and violet for effects.
Keep the numeric value below each knob. Show units, including milliseconds, hertz, and decibels.
Keep the signal path visible. Name cables by their purpose.

The narrow Inspector uses parameter fields and explanations. Full instrument panels stay available on the graph and in full size.
This avoids shrinking a complex panel until its controls become difficult to use.

## GUI components

| Component | Current support | Workbench requirement |
| --- | --- | --- |
| Rotary knob | Ember widget with drag and arrow keys | Precise values, logarithmic ranges, units, and parameter undo |
| Step sequencer | Ember pitch grid and drum pads | Clear active step, rests, presets, and editable rhythm |
| Piano keyboard | Ember canvas with pointer and physical keys | Visible held keys and release on focus loss |
| Envelope display | Ember ADSR preview | Explain time and level while controls change |
| Filter response | Ember response preview | Show cutoff and resonance beside the controls |
| Oscilloscope | Ember frame node | Display actual samples, with an honest time scale |
| Spectrum and meters | Ember frame node | Actual spectrum, peak, and RMS with clear units |
| Mixer | Workbench master and drum knobs; four-input audio mixer | Use the Inspector for individual mixer levels |
| General parameter knob | Ember Control node | Configurable range, name, and logarithmic response |
| Patch selection | Ember Console node | Smooth scene changes and a clear active patch |
| Automation | Patina has curve and timeline controls | Reuse these controls when a sound workflow needs automation |
| Sample display | Patina has waveform and asset controls | Connect them to a tested import and playback path before claiming sample editing |

Keep domain controls in the example widget crate first.
Promote a control to the shared SDK when multiple workbenches need the same behavior.
Do not duplicate existing SDK controls to change their appearance.

## Node library work

The first patch includes a score, clock, twin oscillator, amplitude envelope, resonant filter, echo, drums, output, and scope.
The sound library also needs independent building blocks that users can connect in other ways.

The implemented library covers these tasks:

- Generate tones and noise.
- Generate LFOs and independent envelopes.
- Scale, invert, hold, and smooth modulation signals.
- Mix signals and control their amplitude.
- Filter audio with external modulation.
- Add saturation, reduced bit depth, chorus, and reverb.

The three patches exercise these modules together. The CLI export command completes the saved-patch rendering workflow.
Live recording, MIDI, sample import, stereo processing, and song arrangement remain open.

Provide example patches that use the modules together. A list of unused node names does not prove a useful workflow.

## Sound checks

Use sample-accurate clocks and smooth continuous parameters.
Keep delay storage and analysis coefficients outside the audio processing loop.
Test finite output, silence, note release, filter rejection, repeatability, and effect tails.
Measure processing time and allocations after the pools warm.
Distinguish numerical checks from listening checks.

The core DSP check measured 0.015 ms mean and 0.016 ms p95 per block on Windows.
The expanded 15-processor check measured 0.030 ms mean and 0.032 ms p95. Both counted zero warm allocations.
Tests cover filter rejection, compressor gain, DC removal, reverb decay, oscillator transitions, and reduced saturation aliasing.
Edit and Bake produced matching phrase samples. Both modes produced finite audio from all three scenes.
Speaker playback and other operating systems were not checked by this task.

## References

- [Bespoke module reference](https://www.bespokesynth.com/docs/): patchable modules, controls, and modulation.
- [VCV Free modules](https://vcvrack.com/Free): oscillators, LFOs, filters, envelopes, amplifiers, and mixers.
- [Vital](https://vital.audio/): direct visual feedback from synthesis controls.
- [DAFx 2016 paper](https://www.dafx.de/paper-archive/2016/dafxpapers/20-DAFx-16_paper_41-PN.pdf): antiderivative antialiasing for nonlinear processing.
