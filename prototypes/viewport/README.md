# Prototype: viewport

This is the viewport prototype of milestone 0 (spec section 19). It tests spec 15.8: a `wgpu`
viewport in a panel of a Tauri window. The result decides how the frames reach the window. The
result is in [STATUS.md](../../STATUS.md).

## What it does

- A render thread owns one `wgpu` device. A compute pass moves a grid of 96 x 96 cubes in
  each frame. All viewports draw the same cell buffer with their own camera (spec 10.2).
- The page is a small editor shell: a top bar with a **Views** menu, a left dock with
  viewport 1, a splitter, a right dock with viewport 2 and an **Inspector**, and a status bar.
- Viewport 1 can float into its own window, dock again, hide, restore, close and reopen.
- A left drag in a viewport turns the camera. The mouse wheel zooms. A right click opens a
  menu over the scene.
- The frames reach the window in one of three ways, the frame paths:
  - **native**: the `wgpu` surface is on the window, under a transparent WebView2. The
    viewport panels of the page are transparent, and the other panels are opaque. Nothing is
    copied.
  - **shared**: the GPU renders into a texture, and the render thread reads it back. It writes
    the pixels into WebView2 shared memory with three slots. The page polls the slots in each
    animation frame and draws the newest frame with WebGL2.
  - **channel**: the same readback, but the pixels go to the page through a Tauri channel.
    The page acknowledges each frame. The render thread skips frames until the acknowledgement
    comes.

## Run it

Build the page one time, and after each change to `frontend/`:

```bash
cd prototypes/viewport/frontend && npm ci && npm run build
```

Open the editor shell in a window:

```bash
cargo run --release -p viewport -- --mode native
```

- `--mode` is `native`, `shared` or `channel`.
- `--low-power-gpu` uses the integrated GPU of a computer with two GPUs.
- On Windows, the `wgpu` backend is DX12. Set `WGPU_BACKEND` to `vulkan` to use Vulkan.

## Run the bench

```bash
cargo run --release -p viewport -- --bench
```

The bench is a measurement for one design decision. It is not a test, and `cargo test` does
not run it. Run it again only when the frame paths change.

The bench opens an off-screen window that cannot take the focus and has no taskbar button. It
drives the page with DevTools input, so it does not use the mouse or the keyboard of the
desktop. For each frame path, it measures three scenarios and runs the checks of spec 15.8.
Then it writes `artifacts/viewport/bench-<unix time>.json` and prints a table. The default
bench takes about 45 s.

| Option | Effect |
| --- | --- |
| `--run native,shared` | Runs these frame paths. The default is all three. |
| `--seconds 3` | The measure time of each scenario. The default is 2 s. |
| `--shots` | Saves each probe capture to `artifacts/viewport/shot-<path>-<check>.bmp`. |
| `--low-power-gpu` | Uses the integrated GPU, for `wgpu` and for the web view. |

The scenarios are `one` (viewport 1 docked), `two` (both viewports) and `full` (viewport 1
fills the window at 1920 x 1080). In each scenario, the bench drags the camera of viewport 1
in a circle, with one move in each animation frame.

The bench measures:

- The frames of each viewport, the frame interval and the dropped frames.
- The render work of each frame on the render thread.
- The input latency. On the native path, it is the time from the input event to the present
  call. On the copy paths, it is the time from the input event to the end of the page draw.
- The bytes copied for each frame, and the upload time of the page.
- The CPU use of the process tree and the GPU 3D use, with Windows performance counters.

The checks capture the composed window with `PrintWindow` and count the scene pixels in each
probe rectangle. A check captures again, 10 times at most, until the result is correct. The
report gives the number of captures of each check:

| Check | What it shows |
| --- | --- |
| `composite` | The scene shows in the viewport, and the **Inspector** panel covers it. |
| `menu` | A menu that opens over the viewport covers the scene, and the scene comes back. |
| `splitter` | A drag of the splitter resizes the viewport. |
| `resize` | A window resize configures the surface again. |
| `hide_restore`, `close_reopen` | The viewport goes away and comes back. |
| `float_dock` | The viewport shows in a float window and docks again. |
| `focus` | Keys go to a text field or to the focused viewport, never to both. |
| `display_scale` | The viewport follows a display scale of 200 % and 100 %. |
| `two_views` | Two viewports show the same GPU data. |
| `device_lost` | The render thread makes a new device after a device loss, and the scene comes back. |

## Files

| Path | Content |
| --- | --- |
| `src/main.rs` | The Tauri app, the commands, the page server and the arguments. |
| `src/render.rs` | The render thread, the viewports, the frame sinks and the device-loss recovery. |
| `src/gpu.rs` | The device, the pipelines and the test scene. |
| `src/scene.wesl` | The compute and draw shaders. |
| `src/shared.rs` | The WebView2 shared memory of the shared path. |
| `src/stats.rs` | The render counters. |
| `src/bench.rs` | The run table, the CPU and GPU sampler and the report. |
| `src/capture.rs` | The `PrintWindow` capture and the DevTools calls. |
| `frontend/src/main.ts` | The editor shell and the float window. |
| `frontend/src/viewport.ts` | One viewport panel, its input and the copy-path drawer. |
| `frontend/src/bench.ts` | The bench scenarios and the checks. |

## Limits

- The prototype ran on Windows only. It builds on Linux and macOS, but the shared path, the
  capture and the sampler use Windows APIs, so the bench needs Windows.
- The page loads from `frontend/dist` in the source tree, so the prototype runs only from a
  checkout.
- The bench window is off-screen. The display scale checks emulate the scale with the zoom of
  the web view. They do not change the Windows display scale.
