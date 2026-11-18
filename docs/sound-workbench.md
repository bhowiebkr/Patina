# Sound workbench

## Purpose

Owner direction, 2026-11-18: each example is a workbench for a specific domain.
An example needs useful modules, controls, inspection tools, and complete workflows.
The sound workbench must help a beginner learn synthesis and let a sound designer make useful sounds.

The first instrument is [Ember](../examples/synth/README.md).
Its first patch establishes the visual style, audio path, and control behavior.
The next work expands its reusable library and adds export.

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
| Mixer | Shared numeric fields are available | Add a focused level panel when the expanded patches need it |
| Automation | Patina has curve and timeline controls | Reuse these controls when a sound workflow needs automation |
| Sample display | Patina has waveform and asset controls | Connect them to a tested import and playback path before claiming sample editing |

Keep domain controls in the example widget crate first.
Promote a control to the shared SDK when multiple workbenches need the same behavior.
Do not duplicate existing SDK controls to change their appearance.

## Node library work

The first patch includes a score, clock, twin oscillator, amplitude envelope, resonant filter, echo, drums, output, and scope.
The sound library also needs independent building blocks that users can connect in other ways.

The next library group covers these tasks:

- Generate tones and noise.
- Generate LFOs and independent envelopes.
- Scale, invert, hold, and smooth modulation signals.
- Mix signals and control their amplitude.
- Filter audio with external modulation.
- Add saturation, reduced bit depth, chorus, and reverb.
- Export rendered audio without opening a sound device.

Provide example patches that use the modules together. A list of unused node names does not prove a useful workflow.

## Sound checks

Use sample-accurate clocks and smooth continuous parameters.
Keep delay storage and analysis coefficients outside the audio processing loop.
Test finite output, silence, note release, filter rejection, repeatability, and effect tails.
Measure processing time and allocations after the pools warm.
Distinguish numerical checks from listening checks.

The first DSP check measured 0.016 ms mean and p95 per block on Windows.
It counted zero warm allocations. Edit and Bake produced matching output samples.
Speaker playback and other operating systems were not checked by this task.

## References

- [Bespoke module reference](https://www.bespokesynth.com/docs/): patchable modules, controls, and modulation.
- [VCV Free modules](https://vcvrack.com/Free): oscillators, LFOs, filters, envelopes, amplifiers, and mixers.
- [Vital](https://vital.audio/): direct visual feedback from synthesis controls.
- [DAFx 2016 paper](https://www.dafx.de/paper-archive/2016/dafxpapers/20-DAFx-16_paper_41-PN.pdf): antiderivative antialiasing for nonlinear processing.
