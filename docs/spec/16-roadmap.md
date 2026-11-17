# Patina Specification: Roadmap

Part of the [Patina Specification](README.md).

## 16. Optional targets

### 16.1 Process isolation

Patina is a development tool, so users often run nodes that fail. A node that fails must never crash the editor UI. Owner decision of 2026-11-16 (18, question 4). The editor has this target since 2026-11-16.

- The editor runs the engine in a second process, the node process. It is the same `patina` executable with a hidden argument. The node process does the builds, loads the clusters, and runs the nodes, the views, the node GUIs and the triggers.
- The editor process keeps the window, the page, the project, the undo history and the trust. It sends jobs to the node process and gets events back. The messages are lines of JSON on the standard input and the standard output of the node process.
- The node process sends its own standard output to the editor as messages, so the output of a node does not mix with the messages. Its standard error goes to the editor directly, so the last text of a crash is not lost. Both go to the **Output** tab (15.10).
- Process isolation is always on in the editor. It has no setting. `patina-cli` and packages run the nodes in their own process, because a crash there ends only that run.
- When the node process ends without a request, the editor shows the exit code and the last error text in the **Output** tab and on the run. The node states, the live values and the states of the open views are lost. The **Layers** panel and the **Code** tab keep the state of the last build, because the build did not fail. The next run starts a new node process. The editor does not start it again in a loop.
- On Windows, the node process records the node that runs before each call (15.2). A crash handler writes that node to the error text, so the editor can name the node. The handler runs on the thread that crashed, and names the node of the run on that thread. When that thread is not in a run, it names the node of the engine thread, or else of an active event run (6). Then it ends the process at once, without the error report dialog of Windows. A stack overflow also goes through the handler. A Rust abort skips the handler, so only its own message shows. On Linux and macOS, only the exit status shows.
- The page checks each widget line of a node GUI or a view, and does not draw a bad line. When the drawing of a GUI fails, that GUI shows the error, and the rest of the page stays. The next widget lines draw again. (Agent decision, 2026-11-16. The owner can change it.)
- **Stop** asks the nodes to stop (15.2). When "Node X does not stop" shows, **Stop** ends the node process. The node states are lost.
- On Windows, the node process runs in a job object. When the editor ends, also after a crash, the job ends the node process and each process that it started. When the node process ends, the processes that it started end too. A node can start a process with `CREATE_BREAKAWAY_FROM_JOB` to keep it. (Agent decision, 2026-11-16. The owner can change it.)
- The job limits the committed memory of all its processes to three quarters of the physical memory. The builds are in the job too. A larger allocation fails and ends the node process. So a node that leaks memory cannot fill the memory of the computer and stop the editor. When the last error text shows a failed allocation, the failure text also names the limit in MB. (Agent decision, 2026-11-16. The owner can change it.)
- On Linux and macOS, the node process ends when the editor closes its standard input. The processes that a node starts do not end with it, and the memory has no limit.
- Views and node GUIs send text, so they cross the process boundary with no change. A native viewport (15.8) makes its surface on the editor window from the node process.
- The node process sends the same events as before, so the cost is one copy of each event into JSON and through a pipe.
- The node process also gives the new standard input and output to the C runtime. So `printf` of C code in a node shows in **Output**, and C code cannot read or write the message pipes. The editor shows each line that is not a message in **Output**.
- The editor sends the output to the **Output** tab at most each 100 ms. When more than 256 KB of output waits, the editor skips the oldest lines and shows how many lines it skipped. So a node that prints without a pause does not stop the window, and **Stop** works. (Agent decision, 2026-11-16. The owner can change it.)

### 16.2 WASM sandbox

The WASM sandbox runs a node with wasmtime, with no access to the computer outside the limits below. It adds security to trust (11) for code that is new to the computer, and a sandboxed node is portable. Section 17 keeps it out of version 1. The rules below are agent decisions of 2026-11-16, from the direction of the owner. The owner can change them.

**Which nodes run in the sandbox.** The unit is a node folder (6.2).

- A node folder that is new to the computer runs in the sandbox by default. This includes each node folder of a project that gets trust for the first time (11), each folder that **Import** adds and each folder that **Update** changes (6.2).
- A node folder that the user makes in Patina runs native. So do the node folders of the example projects in the zip.
- The **Inspector** of a node has a **Run native** check box for its node folder. The user selects it to run the folder native.
- Patina keeps the choice in the data folder of the user, by project ID and folder, as the trusted list (11). A project cannot give itself native rights.
- An **Update** from outside clears **Run native**, because the code is new again.
- A sandboxed node runs in the node process (16.1), so a trap or a crash of wasmtime does not crash the editor.

**Feedback to the user.**

- A sandboxed node shows a sandbox mark on its header. The **Inspector** shows why the node is in the sandbox.
- Code that was not written for WASM often fails in the sandbox. Then the node shows the error, and a notice shows: "Node X does not run in the sandbox: <reason>. Select **Run native** in the **Inspector** to run it native."
- The reasons are: the wasm32 build failed, the module needs a missing import (for example a native library or a thread), a trap, a refused access to a file, the network or the environment, or a pin type with no byte form.
- The notice shows one time for each folder. It shows again only after a change to the code of the folder.

**Tracking in the project.** The `Cargo.toml` of a node folder records whether the node runs in the sandbox:

```toml
[package.metadata.patina]
sandbox = false
sandbox-reason = "uses the cpal native audio library"
```

- `sandbox = true` means that the node builds and runs in the sandbox. `sandbox = false` means that it does not, and `sandbox-reason` gives the reason.
- The author can write the key. Patina writes it after the first result in the sandbox, when the project is not read-only. A key that Patina wrote is a normal code change, so it goes into Git with the folder.
- A folder with `sandbox = false` does not try the sandbox. The node shows the notice at once, with the reason.
- The key is information, not a right. Only the user gives native rights, with **Run native**.
- **Import** shows the key of each folder before the import, so the user knows which nodes need native rights.

**Limits of the sandbox.**

- The node reads only its project folder. It writes only to the folders that the project gives to it. It has no network, no environment variables and no other processes.
- It has no GPU context (10), no CUDA and no native libraries. A GPU node runs native.
- The memory is 4 GB at most, unless the module uses `memory64`. SIMD is 128-bit only. Threads are limited.
- Epoch interruption lets **Stop** end a call at once. A trap is a node error, not a crash.
- Values cross the boundary as copies. A pin type needs a byte form: the scalars, `bool`, `String`, and `Vec` and arrays of these, or a type with `serde`. Large values are slower than in a native node.
- Each sandboxed node folder builds as its own wasm32-wasip1 module, as the slot is its own library (7). The engine does not fuse it into the base. The toolchain needs the `wasm32-wasip1` target.

**What the sandbox does not cover.** The build runs native. Build scripts and procedural macros run code on the computer (11). So trust still controls the build, and the sandbox protects only the run.

**Downloads.** The sandbox is optional. The build, the zip and the installers of Patina do not contain wasmtime. This is an owner decision of 2026-11-16.

- The user downloads the sandbox only when they want it. The **Sandbox** part of the settings screen (15.10) has **Download sandbox**. It shows each step, as **Install toolchain** does.
- The download adds the `wasm32-wasip1` target to the toolchain and a sandbox node process to the data folder. The sandbox node process contains wasmtime. The editor starts it in place of the native node process for sandboxed nodes. This part is an agent decision of 2026-11-16. The owner can change it.
- **Remove sandbox** deletes the sandbox node process. Then all node folders run native.
- Without the sandbox, each trusted node folder runs native, as before. The default of "Which nodes run in the sandbox" applies only when the sandbox is downloaded.

## 17. Out of scope for version 1

- State migration after hot reload.
- Breakpoints.
- Retry rules and run history for event runs.
- WASM target: the WASM sandbox (16.2).

## 18. Open questions

Ask the owner about an open question. Do not decide it alone. A question keeps its number when it gets a decision, because other sections refer to it.

**Open**

No question waits for a decision of the owner. This question waits for a test:

- **5.** Zig and Xcode on macOS: does Zig remove the need for the Xcode Command Line Tools (13.4)? A build on a macOS computer without the Xcode tools answers it. The macOS runners of CI have Xcode, so they cannot answer it. The owner works on Windows only (13.5), so the question waits for a contributor with a Mac.

**Decided**

The agent decided these questions while it built the milestones, except question 4, which the owner decided. The code follows each decision. The owner can change each decision. The agent does not ask about them again.

- **1.** Project file format. Decided: TOML files that Patina writes in a fixed order, with stable keys for the public pins of a subgraph (6.4). Agent decision of 2026-11-15. The owner can change it.
- **2.** React or Svelte. Decided: React, with React Flow (15.1). Agent decision from the connections prototype, 2026-11-15. The owner can change it.
- **3.** Default audio block size and number of audio threads. Decided: 256 samples at 48 kHz, and one audio thread for all audio nodes (9.3). Agent decision of 2026-11-15. The owner can change it.
- **4.** Process isolation: default on or off. Decided: on in the editor, with no setting. Patina is a development tool, so users often run nodes that fail, and a node that fails must never crash the editor UI. The editor runs the engine in a node process (16.1). `patina-cli` and packages run in one process. A native viewport needs a shared GPU texture across the two processes (15.8). Owner decision of 2026-11-16. It replaces the agent decision "off" of the same day.
- **6.** App mode: a layout that shows the views and the node GUIs without the graph, so a project becomes a tool for other users (15.8). Can a package include it, with its views? Decided: not in version 1. The views and the large node GUI in the **Inspector** already show a project without the code. An executable package (14.1) is a command-line runner with no window, so it cannot show views. App mode can come with a package that has a window. Agent decision of 2026-11-16. The owner can change it.
- **7.** Fuse time: how long a node stays in the slot with no change before the engine fuses it back (7.2). Decided: 30 s with no change to the graph or the code. A bake build of a small cluster takes a few seconds, so a shorter time can build again while the user still edits. Agent decision of 2026-11-15. The owner can change it.
- **8.** Dock layout. Decided: each user keeps their own layout for each project, in the data folder of the user, not in the project (15.2). Agent decision of 2026-11-15. The owner can change it.
- **9.** First workflow. Decided: graph edits apply at once, code applies at Ctrl+S, and Ctrl+S saves the full project (15.10). Agent decision of 2026-11-15. The owner can change it.
- **10.** Stop. Decided: the top bar has a **Stop** button next to **Pause**. Stop asks the nodes to stop, and never kills a thread (15.2). Agent decision of 2026-11-15. The owner can change it.
- **11.** Side effects while editing: which batch nodes run again after a parameter change (9.1)? How does Patina prevent repeated writes to files or to the network while the user drags a slider? Decided: a change runs all the batch nodes again, and the engine merges the changes that come during a run into one next run. A parameter field and a slider apply their value when the user ends the edit, so a drag runs the graph one time (9.1). Agent decision of 2026-11-15. The owner can change it.
- **12.** Live values in a fused chain: a chain of small nodes inlines into one call (9.2). Thus a value between two of them can exist only in registers. How does the engine show it on the wire (15.9)? It can keep a copy point on each wire that the editor shows. It can rebuild the chain with copy points when the user looks at a wire. Or it can show live values only between tasks. Milestones 4 and 6 can measure the cost. Decided: a copy point on each wire. The cluster sends the text of each value that a wire reads after its node runs, also inside a chain (15.9). Agent decision of 2026-11-15. The owner can change it.
- **13.** Circuit graphs: digital logic needs feedback loops without a frame delay, for example a latch. It also needs a bus with more than one driver, and defined clock edges and steps. The engine refuses a cycle without a delay node (9.2). Does Patina support these with a simulation node that runs the circuit inside it, with a new clock domain, or not in version 1? See the [circuit graph design](../circuit-graph-design.md). Decided for version 1: a synchronous model without an engine change. One frame is one clock step. A `delay` node is a D flip-flop, so a register is a delay and a counter is a loop through delays. A cycle without a delay, for example a latch of two NAND gates, stays refused (9.2). A wire has one driver, so a shared bus uses an explicit multiplexer. A simulation node with its own propagation can come later. The `examples/computer` project shows the model. A slower clock is a `clock` node: its tick goes into the load and write inputs of the registers, so the state changes only at a tick. Agent decision of 2026-11-15. The owner can change it.
- **14.** Shared node libraries: how do users share nodes, widgets, connections, types and subgraphs? Decided: a library is a normal Patina project in a Git repository, so it needs no new format. Patina keeps a small core set in the zip, as the example projects. **Settings** gets a **Libraries** list. The user adds the URL of a repository and selects a release tag, for example `v1.2.0`. Patina pins the library to that tag. It records the commit of the tag and the SHA-256 hash of the archive. It downloads the archive of the tag over HTTPS into the data folder, so it needs no `git` program. A tag that moves to another commit gives a warning, and Patina keeps the old copy. A newer tag shows as an update, and the user selects it. The user trusts the library before Patina builds it (11), because build scripts and procedural macros run code. A drag from a library uses **Import**, and a change to a newer tag uses **Update** (6.2). An index repository holds one TOML file with the name, URL, description and pinned tag of each library. Patina shows it as a list to browse, and a pull request to the index adds a library. Agent decision of 2026-11-15. The owner can change it.

## 19. Milestones

0. **Risk prototypes.** Prove these first on Windows. Also run them on Linux and macOS when a machine or a CI runner can, and record the result (13.5). A prototype passes only when all its checks pass:
   - **Rust types across clusters** (5.1, 5.2): two clusters and the shared types library. Pass a `String`, a `Vec`, and a custom type with `Drop` between them. Record the files that the build makes and the libraries that each file needs. Change a local struct from two `u32` fields to two `f32` fields, with the same name and version: the engine rejects the old values. Two versions of one crate with types of the same name do not connect.
   - **Hot reload** (8.5): reload a cluster and the shared types library many times. Keep a value from an old build alive across a reload, and check that it drops with the code of its own build. Measure the memory and the disk use that each reload adds. The engine does not need to unload old builds to pass. Measure a cold build and a rebuild after a change to one node, each on its own.
   - **Toolchain** (13.2): use a clean Windows computer with no Visual Studio, and a standard account. Install the toolchain, and build a node and a crate with C code (Zig as the C compiler, and `rust-lld` as the linker on Windows). Load the libraries, and run the graph. Do it again after a move of the Patina folder to a path with spaces and non-ASCII characters. Record each program that was on the computer.
   - **Viewport** (15.8): a `wgpu` viewport in a panel of a minimal Tauri window, at 60 FPS at 1920 x 1080. Dock, float, resize, hide, restore, close, and reopen the panel. Change the Windows display scale. Check the keyboard focus, and a menu that opens over the viewport. Show two viewports that use the same GPU data. Measure the frame time, the input latency, the CPU and GPU use, and the bytes copied for each frame. Record what happens when the GPU device is lost. The result decides how the frames reach the window: a copy into the web view, or a native surface over the panel.
   - **Connections** (15.9): a graph view in a minimal Tauri window, with 500 nodes and 1,000 connections. 200 of the connections are buses of 8 to 32 lanes. A Rust process changes the lane states at 60 FPS and sends them by the path of 15.6. Pan, zoom, select a wire, and drag a route point while the states change. Change the Windows display scale. Measure the frame time, the CPU and GPU use, and the bytes sent for each frame. Compare SVG wires in React Flow or Svelte Flow with a canvas or WebGL layer under the nodes. The result decides how the graph view draws connections (15.1), and it informs question 2 (18).
1. **Engine:** compile one node folder (6.2), load it, run a simple graph from the CLI.
2. **Editor:** Tauri shell, dock layout, Project panel, node editor, Monaco, node states, errors, value viewers (15.2), connections with route points, live values and connection kinds (15.9, 6.3).
3. **Toolchain button and release:** the portable zip and the installers (13.1).
4. **Automatic parallelism and the frame domain.**
5. **Stateful nodes, node GUIs, and generic nodes:** with the full size and widget folders (4.3), and the delay node (9.2).
6. **Layers:** clusters, the base, the slot, fuse back, and **Bake** (7). Check: one node folder with uses in two clusters. Change its output type, and change it again before the first build ends. All uses switch to the newest build, and the errors refer to the newest code. Measure the fuse time.
7. **Subgraphs and reuse:** subgraphs, and **Import** and **Update** of folders from other projects (6.2).
8. **Packaging.**
9. **GPU:** shared context, GPU types, shader nodes with WESL (10.1).
10. **Views:** the `Graph` handle and the viewport (15.8).
11. **Audio domain.** Ended on 2026-11-16. The owner deferred the GUI of audio nodes (4.3), so the check used a slow `ui` function of a frame node. Check: a slow `ui` function of an audio node while audio runs. The audio domain misses no deadline, and the GUI holds no lock that the audio thread waits for (4.3).
12. **Event domain:** async nodes and trigger nodes. Ended on 2026-11-16. Check: the triggers `schedule`, `trigger`, `file_change` and `webhook` start runs in `patina-cli`, and the three overlap modes keep their rules (9.4). In the editor, the **Fire** button and a view fire a `trigger` node. The editor makes and removes order wires (9.2).

Each milestone gives a tool that works.
