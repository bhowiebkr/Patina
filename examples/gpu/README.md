# GPU

A project that uses the GPU with WESL shaders through `wgpu`. The nodes look like the front
panels of lab instruments. The first row draws a gray gradient on the GPU, finds its mean
brightness and shows a small copy. The second row makes color bars, mixes them with the gradient
and shows the result on a monitor and a waveform monitor.

![The GPU graph in the editor](images/editor.png)

## Run the example

The example needs Rust 1.100.0, as all examples do. Run
this command from the root of the repository:

```
cargo +1.100.0 run -p patina-cli -- run examples/gpu
```

The command writes the mean brightness of the gradient, about `0.336`.

To open it in the editor, start the editor with Rust 1.100.0 or later. Then open the
`examples/gpu` folder and click **Run**.

## The parts

| Path | What it is |
| --- | --- |
| `nodes/gradient/` | Draws a gray gradient on the GPU, with the shaders in `assets/`. The **INV** switch sets **invert**, so the gradient goes from white to black. |
| `nodes/brightness/` | Gives the mean of the red channel. It copies the image to the CPU and shows a histogram with a line at the mean. |
| `nodes/preview/` | Shows a small copy of the image on the screen of a monitor. |
| `nodes/show/` | Writes its value to the output, and shows it with a bar. |
| `nodes/knob/` | A knob that gives a value from 0 to 1. |
| `nodes/bars/` | Makes the color bars of a test signal on the GPU, with a gray ramp under them. The **level** input sets the brightness through a WGSL `override` constant. |
| `nodes/mix/` | Mixes two images on the GPU, as the crossfader of a video mixer. The result has the size of the image on **a**. |
| `nodes/waveform/` | Shows the brightness of each column of the image, as a waveform monitor does. |
| `widgets/scope/` | The drawing parts of the node GUIs: the panel, the screws, the screens and the readouts. |
| `views/scene/` | A view with a GPU viewport. A drag sets the color. The wheel and the arrow keys set the light. |

The images stay on the GPU between the GPU nodes. A shader change needs no Rust compile. The
[GPU](../../README.md#gpu) section of the main README tells how a node uses the GPU.

## Try this

- Drag the knob up or to the right. The color bars, the monitor and the waveform get brighter.
- Drag the fader of the mix node from **A** to **B**. The gradient comes in over the color bars.
- Click the **INV** switch of the gradient. The histogram and the mean change.
- Open `nodes/bars/assets/bars.wesl` and change the colors. Click **Run** to see the change.
