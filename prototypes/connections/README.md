# Prototype: connections

This is the connections prototype of milestone 0 (spec section 19). It tests spec 15.9 and
15.1: live wires and buses in a graph view, with lane states that change at 60 FPS. The result
is in [STATUS.md](../../STATUS.md).

## What it does

- The Rust side makes a seeded graph: 500 nodes and 1,000 connections. 200 connections are
  buses of 8 to 32 lanes, and 100 connections have route points.
- A Rust thread changes the lane states at 60 FPS. It sends each frame as packed bits through
  a Tauri channel (spec 15.6). A frame has 4,376 lanes and 559 bytes.
- The page shows the graph with React Flow. The connections draw in one of two ways:
  - **svg**: one React Flow edge for each connection, with an SVG path for each lane.
  - **webgl**: one WebGL2 canvas under the nodes for all wires, and a 2D canvas for the
    badges.
- At a far zoom, a bus shows as one line. At a near zoom, it shows each lane.
- The user can pan, zoom, select a wire and drag its route points (spec 15.2 controls).

## Run it

Build the page one time, and after each change to `frontend/`:

```bash
cd prototypes/connections/frontend && npm ci && npm run build
```

Open the graph view in a window:

```bash
cargo run --release -p connections -- --renderer webgl --feed raw
```

- `--renderer` is `svg` or `webgl`.
- `--feed` is `raw` (packed bits in a channel), `json` (a JSON event) or `off`.
- Middle drag or Space + left drag pans. The mouse wheel zooms. **F** fits the view, and
  **Esc** clears the selection.

## Run the bench

```bash
cargo run --release -p connections -- --bench
```

The bench is a measurement for one design decision: SVG wires or a WebGL layer (spec 15.1). It
is not a test, and `cargo test` does not run it. Run it again only when the wire renderer
changes.

The bench opens an off-screen window that cannot take the focus and has no taskbar button. It
runs each scenario, measures it and writes `artifacts/connections/bench-<unix time>.json`.
Then it prints a table. The default bench compares the two renderers with the live feed, and it
takes about 45 s.

| Option | Effect |
| --- | --- |
| `--run 0,2,5` | Runs these rows of the run table in `src/bench.rs`. The default is `0,1`. Rows 2 to 7 isolate the feed and the display scale. |
| `--seconds 3` | The measure time of each scenario. The default is 3 s. |
| `--shots` | Saves a screenshot before each scenario, to `artifacts/connections/shot-*.png`. |
| `--low-power-gpu` | Asks the web view for the integrated GPU of a computer with two GPUs. |

The scenarios are `idle-far`, `idle-near`, `pan`, `zoom`, `select` and `drag`. The runs with a
DPR emulate a display scale with the zoom of the web view.

The bench measures:

- The frame interval and the dropped frames, from `requestAnimationFrame`.
- The main-thread work of each frame, up to the end of the paint.
- The latency from the Rust send to the page receive, and the send time in Rust.
- The CPU use of the process tree and the GPU 3D use, with Windows performance counters.

## Files

| Path | Content |
| --- | --- |
| `src/main.rs` | The Tauri app, the commands, the page server and the arguments. |
| `src/graph.rs` | The seeded test graph. |
| `src/feed.rs` | The 60 FPS lane-state feed. |
| `src/bench.rs` | The run table, the CPU and GPU sampler and the report. |
| `src/shot.rs` | Screenshots through the DevTools protocol of WebView2. |
| `frontend/src/App.tsx` | The graph view, the frame loop and the route handles. |
| `frontend/src/wires.ts` | The wire geometry, the route points and the hit test. |
| `frontend/src/svg.tsx` | The SVG renderer. |
| `frontend/src/gl.ts` | The WebGL renderer and the badge overlay. |
| `frontend/src/lanes.ts` | The receiver of the lane-state frames. |
| `frontend/src/bench.ts` | The bench scenarios and the frame statistics. |

## Limits

- The CPU and GPU sampler and the screenshots work on Windows only.
- The page loads from `frontend/dist` in the source tree, so the prototype runs only from a
  checkout.
- The nodes do not move. Bus socket contacts for each lane are not drawn.
