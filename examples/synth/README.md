# Ember sound workbench

Ember is a modular synthesizer for learning and sound design. Its graph shows how a score becomes sound.
The instrument panels have different sizes. Each panel has room for its controls and displays.

![Ember in the Patina editor](images/editor.png)

## Play

1. Open `examples/synth` in Patina.
2. Click **Run**.
3. Start with a low speaker volume.
4. Click **NIGHT DRIVE**, **GLASS STEPS**, or **ACID LINE** to change the phrase.
5. Drag a knob up or down to change its value.
6. Click the keyboard, then play A, S, D, F, G, H, J, and K.
7. Click **PLAYING** to hold the sequencer. The keyboard stays live.
8. Click **Save** to keep the graph parameters.

The knobs also accept arrow keys when they have focus. The **Inspector** shows exact parameter values and short explanations.
The large panels also support **Full size**. Use the node menu to open one.

## Learn by listening

| Try this | Listen for this |
| --- | --- |
| Select **SINE**, then **SAW**. | A saw has more upper harmonics than a sine. |
| Increase **DETUNE**. | Two close pitches beat against each other. |
| Increase **ATTACK**. | Each note fades in more slowly. |
| Reduce **CUTOFF**. | The filter removes high frequencies. |
| Increase **RESONANCE**. | The filter adds emphasis near its cutoff. |
| Increase **MOTION**. | The LFO moves the filter through a wider range. |
| Increase **TIME** and **FEEDBACK**. | The echoes spread out and last longer. |
| Click a pitch cell or drum pad. | The next pass uses your edited score. |

The score has 16 steps in C minor. **BD**, **SD**, and **HH** mean kick, snare, and hi-hat.
**GATE** controls note length within a step. **SWING** alternates longer and shorter steps without changing the bar length.

## Follow the signal

The clock produces sample-accurate pitch, gate, and drum triggers.
The oscillator makes the raw sound. The amplitude envelope shapes each note.
The filter removes harmonics. The echo adds repeats. The output mixes the voice and drums.

The scope measures the actual output. The upper trace shows one 256-sample block, or 5.33 ms.
The lower display shows a windowed spectrum from about 188 Hz to 24 kHz.
Its short window does not resolve deep bass. **PEAK** and **RMS** show levels relative to digital full scale.

Audio cables show samples from the running graph. Their motion comes from the sound.
The score and settings use separate typed connections.

## Code and checks

- `nodes/` contains the graph modules and their instrument panels.
- `types/synth-dsp/` contains the DSP and shared control types.
- `widgets/panel/` contains the reusable knobs and drawing parts.
- `graphs/main.toml` contains the saved patch and its parameter values.

The controls run on the frame thread. The sound processors run on the audio thread.
This uses the current Patina audio model. Direct GUIs on audio nodes remain deferred.

Run the DSP and panel checks from the repository root:

```
cargo test --manifest-path examples/synth/Cargo.toml --workspace --release -- --nocapture
```

The CLI integration test builds the graph in **Edit** and **Bake**. It compares their samples and checks live parameter writes.
The editor screenshot check operates the controls on a hidden Windows desktop.

On 2026-11-18, the core DSP averaged 0.016 ms per block over 2,000 warm blocks.
The p95 was 0.016 ms. The test counted zero allocations after warmup.
These measurements cover DSP processing, without the editor, device, or graph scheduler.

## Design references

[Bespoke Synth](https://www.bespokesynth.com/docs/) is the open source synth with animated patch cables.
[VCV Rack](https://vcvrack.com/Free) shows a useful modular vocabulary and separate control inputs.
[Vital](https://vital.audio/) shows waveforms, envelopes, and filter responses beside their controls.
Ember uses its own code and panel drawings.

The [workbench notes](../../docs/sound-workbench.md) record the wider node and GUI needs.

Current limits: mono output, one melodic voice, and a fixed C minor keyboard.
MIDI, sample import, audio export, and a song arrangement are not implemented in this first patch.
