# Patina Specification: Roadmap

Part of the [Patina Specification](README.md).

## 16. Optional targets

- **Process isolation:** run native nodes in a separate process. A node crash does not crash Patina.
- **WASM:** sandboxed, portable nodes with wasmtime. Limits: no direct native libraries, 128-bit SIMD only, limited threads, 4 GB memory unless `memory64`.

## 17. Out of scope for version 1

- State migration after hot reload.
- Breakpoints.
- Retry rules and run history for event runs.
- WASM target.

## 18. Open questions

Ask the owner. Do not decide these alone.

1. Project file format. Requirements: text-based and Git-friendly. The project folders are set (6.2). Still open: the format of the graph files in `graphs/` and of `patina.toml`, and where the parameter values and the node positions go. Also where the route points, the branch collars, and the cable groups of the connections go (15.9). Also how a connection names a pin. A subgraph must keep its connections when its public pins get a new name or a new order ([circuit graph design](../circuit-graph-design.md)).
2. React or Svelte.
3. Default audio block size and number of audio threads.
4. Process isolation: default on or off.
5. Zig: does it remove the need for Xcode on macOS?
6. App mode: a layout that shows the views and the node GUIs without the graph, so a project becomes a tool for other users (15.8). Can a package include it, with its views?
7. Fuse time: how long a node stays in the slot with no change before the engine fuses it back (7.2).
8. Dock layout: does the project save the layout, the open views, and the state of each view, for example the camera of a 3D viewport? Or does each user keep their own layout (15.2)?
9. First workflow: the fewest steps from a new project to two connected nodes, a run, a save, and a reopen. When does code save: at each change, or with Ctrl+S? How does the editor show unsaved code, saved code, and the build that runs (15.4)?
10. Stop: does the top bar need a **Stop** or **Cancel** button next to **Pause** (15.2)? What can it promise for native code, which cannot stop safely in the middle of a call?
11. Side effects while editing: which batch nodes run again after a parameter change (9.1)? How does Patina prevent repeated writes to files or to the network while the user drags a slider?
12. Live values in a fused chain: a chain of small nodes inlines into one call (9.2). Thus a value between two of them can exist only in registers. How does the engine show it on the wire (15.9)? It can keep a copy point on each wire that the editor shows. It can rebuild the chain with copy points when the user looks at a wire. Or it can show live values only between tasks. Milestones 4 and 6 can measure the cost.
13. Circuit graphs: digital logic needs feedback loops without a frame delay, for example a latch. It also needs a bus with more than one driver, and defined clock edges and steps. The engine refuses a cycle without a delay node (9.2). Does Patina support these with a simulation node that runs the circuit inside it, with a new clock domain, or not in version 1? See the [circuit graph design](../circuit-graph-design.md).

## 19. Milestones

0. **Risk prototypes.** Prove these first on Windows. Also run them on Linux and macOS when a machine or a CI runner can, and record the result (13.5). A prototype passes only when all its checks pass:
   - **Rust types across clusters** (5.1, 5.2): two clusters and the shared types library. Pass a `String`, a `Vec`, and a custom type with `Drop` between them. Record the files that the build makes and the libraries that each file needs. Change a local struct from two `u32` fields to two `f32` fields, with the same name and version: the engine rejects the old values. Two versions of one crate with types of the same name do not connect.
   - **Hot reload** (8.5): reload a cluster and the shared types library many times. Keep a value from an old build alive across a reload, and check that it drops with the code of its own build. Measure the memory and the disk use that each reload adds. The engine does not need to unload old builds to pass. Measure a cold build and a rebuild after a change to one node, each on its own.
   - **Toolchain** (13.2): use a clean Windows computer with no Visual Studio, and a standard account. Install the toolchain, and build a node and a crate with C code (Zig as C compiler and linker). Load the libraries, and run the graph. Do it again after a move of the Patina folder to a path with spaces and non-ASCII characters. Record each program that was on the computer.
   - **Viewport** (15.8): a `wgpu` viewport in a panel of a minimal Tauri window, at 60 FPS at 1920 x 1080. Dock, float, resize, hide, restore, close, and reopen the panel. Change the Windows display scale. Check the keyboard focus, and a menu that opens over the viewport. Show two viewports that use the same GPU data. Measure the frame time, the input latency, the CPU and GPU use, and the bytes copied for each frame. Record what happens when the GPU device is lost. The result decides how the frames reach the window: a copy into the web view, or a native surface over the panel.
   - **Connections** (15.9): a graph view in a minimal Tauri window, with 500 nodes and 1,000 connections. 200 of the connections are buses of 8 to 32 lanes. A Rust process changes the lane states at 60 FPS and sends them by the path of 15.6. Pan, zoom, select a wire, and drag a route point while the states change. Change the Windows display scale. Measure the frame time, the CPU and GPU use, and the bytes sent for each frame. Compare SVG wires in React Flow or Svelte Flow with a canvas or WebGL layer under the nodes. The result decides how the graph view draws connections (15.1), and it informs question 2 (18).
1. **Engine:** compile one node folder (6.2), load it, run a simple graph from the CLI.
2. **Editor:** Tauri shell, dock layout, Project panel, node editor, Monaco, node states, errors, value viewers (15.2), connections with route points, live values and connection kinds (15.9, 6.3).
3. **Toolchain button and release:** the portable zip and the installers (13.1).
4. **Automatic parallelism and the frame domain.**
5. **Stateful nodes, node GUIs, and generic nodes:** with the full size and widget folders (4.3).
6. **Layers:** clusters, the base, the slot, fuse back, and **Bake** (7). Check: one node folder with uses in two clusters. Change its output type, and change it again before the first build ends. All uses switch to the newest build, and the errors refer to the newest code. Measure the fuse time.
7. **Subgraphs and reuse:** subgraphs, and **Import** and **Update** of folders from other projects (6.2).
8. **Packaging.**
9. **GPU:** shared context, GPU types, shader nodes.
10. **Views:** the `Graph` handle and the viewport (15.8).
11. **Audio domain.** Check: a slow `ui` function of an audio node while audio runs. The audio domain misses no deadline, and the GUI holds no lock that the audio thread waits for (4.3).
12. **Event domain:** async nodes and trigger nodes.

Each milestone gives a tool that works.
