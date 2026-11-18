# Patina Specification: Overview

Part of the [Patina Specification](README.md).

## 1. Overview

Patina is a visual node graph editor and engine. Each node is Rust source code. Patina compiles the whole graph to one optimized build of native machine code and runs independent nodes in parallel. When the user edits a node, Patina compiles only that node. The target time is a few seconds. Later, Patina fuses the node back into the optimized build (7). Nodes can have their own interactive GUIs (4.3). Views add custom panels to the editor, for example a timeline, a 3D viewport, or a spectrum display (15.8). With both, a project can become a tool with its own editors.

Patina is the Rust successor to PyFlowGraph.

Example uses (not limits): sound effect editors, image and shader editors, automation pipelines, simulation, and scientific data processing.

## 2. Principles

1. **Universal.** Patina does not limit what a graph does. The engine knows nodes, pins, connections, and clock domains. It does not know data types.
2. **Open.** Nodes are source code. Each node has its own folder, and other projects can reuse it (6.2). Packages include the source. Patina does not use closed or precompiled node formats.
3. **Fast execution.** Native code, optimized across nodes, and automatic parallelism. Real-time targets: 60 FPS for frame graphs, no dropouts for audio graphs.
4. **Fast iteration.** A change to one node recompiles only that node. The target is a few seconds. Milestone 0 measures it (19). The rest of the graph stays optimized (7).
5. **Portable or installed.** The user unzips a release and runs it. Or the user installs it with the installer of the platform (13.1). macOS has only the installed edition. The portable edition makes no changes to the system. Patina downloads extra components with one button. Exceptions: see 13.4.
6. **Windows first.** Windows is the main platform. Linux and macOS get basic builds, and contributors who use them improve them (13.5).

## 3. Architecture

| Part | Description |
|---|---|
| `patina-engine` | Compiles the graph in layers (7), schedules and runs it. No UI code. |
| `patina-editor` | Tauri 2 desktop app with a web frontend. Talks to the engine through messages. |
| `patina-cli` | Runs a project or a package from the command line. |
| `patina-sdk` | The crate that node, view, widget, and connection code uses: node and view macros, `Ui` API for node GUIs and views, `Graph` handle for views, viewer and connection traits, buffer pools, shared GPU context, logging. |

All code is Rust, except the web frontend of the editor.

`patina-engine` stays one crate. After an edit of one of its functions, a check of the crate
took 0.7 s, a debug build 1.2 s and a debug build of the editor 4.0 s, with a game in the
background. A split into the model, the build and the runtime would save little time. (Agent
decision, 2026-11-17. The owner can change it.)

The web frontend builds with Node.js 22 or later and npm, from the `package-lock.json` of its
folder. Only contributors need them. The portable folder and the installers contain the built
page, so a user does not need Node.js. A contributor can use the zip edition of Node.js, which
needs no admin rights. (Agent decision, 2026-11-15. The owner can change it.)

### 3.1 Threads

- The UI and the engine run on separate threads.
- Each clock domain has its own threads (see 9.1).
- The UI sends edits. The engine sends results, errors, and status.
- The engine never waits for the UI. If the UI is slow:
  - Snapshots, for example a viewer image or a node GUI: the UI gets only the latest one.
  - Node states and errors: the latest state of each node always arrives.
  - Logs: a queue with a fixed size. When it is full, the oldest lines drop, and the Logs tab shows how many dropped.
- Edits from the UI are never dropped.
