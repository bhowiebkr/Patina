# GPU

A project that uses the GPU with WESL shaders through `wgpu`. A node draws a gray gradient on
the GPU. A second node finds its mean brightness, and a third node shows a small copy.

This example is in progress. The nodes, the text and the images will change.

![The GPU graph in the editor](images/editor.png)

## Run the example

The example needs Rust 1.100.0, as all examples do. Run
this command from the root of the repository:

```
cargo +1.100.0 run -p patina-cli -- run examples/gpu
```

To open it in the editor, start the editor with Rust 1.100.0 or later. Then open the
`examples/gpu` folder and click **Run**.

## The parts

| Path | What it is |
| --- | --- |
| `nodes/gradient/` | Draws a gray gradient on the GPU, with the shaders in `assets/`. Set **invert** to go from white to black. |
| `nodes/brightness/` | Gives the mean of the red channel. It copies the result to the CPU. |
| `nodes/preview/` | Shows a small copy of the image in the node GUI. |
| `nodes/show/` | Writes its value to the output. |
| `views/scene/` | A view with a GPU viewport. A drag sets the color. The wheel and the arrow keys set the light. |

The image stays on the GPU between the GPU nodes. A shader change needs no Rust compile. The
[GPU](../../README.md#gpu) section of the main README tells how a node uses the GPU.
