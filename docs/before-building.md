# Before building

This list holds what is still open before and during the first milestones. It groups each item
by the milestone that it blocks. The [spec](spec/README.md) stays the source of truth.

- When the owner answers a question of section 18, write the answer into the spec, and check
  the item here.
- When a milestone 0 prototype ends, record its result in `STATUS.md`, and check the item here.
- Remove this file when all items are checked.

## 1. Setup, before any code

- [x] **GitHub remote.** The public repository is
  [bhowiebkr/Patina](https://github.com/bhowiebkr/Patina), made on 2026-11-15.
- [x] **CI.** [ci.yml](../.github/workflows/ci.yml) runs the clippy and test commands of
  AGENTS.md on Windows, Linux and macOS. Only a Windows failure fails the run (13.5).
- [x] **License.** MIT OR Apache-2.0, as the owner decided on 2026-11-15. The files are
  `LICENSE-MIT` and `LICENSE-APACHE`. The README gives the terms for contributions.
- [ ] **A clean Windows computer for the toolchain prototype.** A standard account, with no
  Visual Studio. A Hyper-V virtual machine on Windows 11 Pro is one way. Run the static build
  of the [toolchain prototype](../prototypes/toolchain/README.md) on it.
- [ ] **Linux and macOS runs (optional).** Milestone 0 runs there when a machine or a CI runner
  can (13.5). GitHub Actions runners can run the type and hot reload prototypes.

## 2. Milestone 0: risk prototypes

These are tests, not decisions. Each result can change the design. Report each result before
the next prototype (section 19).

- [x] **Rust types across clusters** (5.1, 5.2). `String`, `Vec` and a type with `Drop` between
  clusters, and the type key check.
- [x] **Hot reload** (8.5). A value that outlives a reload, the memory and disk use of each
  reload, and the time of a cold build and of a rebuild.
- [x] **Toolchain** (13.2). Zig as the C compiler, and a Patina folder at a path with spaces and
  non-ASCII characters. Result: the `x86_64-pc-windows-gnu` target, Zig for C and `rust-lld`
  as the linker (13.2). It passed on the computer of the owner. The run on a clean computer is
  open (item 1 and STATUS.md).
- [x] **Viewport** (15.8). A `wgpu` viewport in a Tauri panel. Result: the native path, a `wgpu`
  surface under a transparent web view, with the DX12 backend on Windows (15.8). The owner check
  of a visible window and a live display scale change is open (STATUS.md).
- [x] **Connections** (15.9). 1,000 wires, 200 of them buses of 8 to 32 lanes, with lane
  states that change at 60 FPS. Result: a WebGL layer under the nodes, and React (15.1). The
  owner check of a live display scale change is open (STATUS.md).

## 3. Owner decisions from section 18

The numbers are the question numbers of section 18. Each group lists the milestone that the
questions block.

Milestone 1, engine and CLI:

- [x] **Q1, project file format.** Agent decision: TOML files that Patina writes in a fixed
  order, with stable keys for the public pins of a subgraph (6.4).
- [x] **Q9, first workflow.** Agent decision: graph edits apply at once, code applies at Ctrl+S,
  and Ctrl+S saves the full project (15.10).

Milestone 2, editor:

- [x] **Q2.** React or Svelte. Agent decision: React, from the connections prototype (15.1).
- [x] **Q8, dock layout.** Agent decision: each user keeps their own layout for each project,
  in the data folder, not in the project (15.2).
- [x] **Q10, Stop.** Agent decision: a **Stop** button next to **Pause**. It asks the nodes to
  stop with a flag, and never kills a thread (15.2).

Milestone 4, automatic parallelism and the frame domain:

- [x] **Q11, side effects while editing.** Agent decision: a change runs all the batch nodes
  again, and the changes during a run go into one next run. A field or a slider applies its
  value at the end of the edit, so a drag runs the graph one time (9.1).
- [x] **Q12, live values in a fused chain.** Agent decision: the cluster sends the text of each
  value that a wire reads, also inside a chain. A live value cost 71 to 76 ns (15.9).

Later milestones:

- [ ] **Q7, fuse time.** Milestone 6 measures it.
- [x] **Q3, audio block size and number of audio threads.** Agent decision of 2026-11-15:
  256 samples at 48 kHz, and one audio thread for all audio nodes (9.3). Milestone 11 ended on
  2026-11-16 with this design.
- [ ] **Q4, process isolation on or off by default.** An optional target (16).
- [ ] **Q5, Zig and Xcode on macOS.** A test on macOS can answer it.
- [ ] **Q6, app mode.** Milestones 8 and 10, packaging and views.
- [x] **Q13, circuit graphs.** Agent decision of 2026-11-15: a synchronous model. One frame is
  one clock step, a delay is a D flip-flop, a cycle needs a delay, and a bus uses a multiplexer.
  `examples/computer` counts and adds with NAND gates. See the
  [circuit graph design](circuit-graph-design.md).

## 4. Connections

These block milestone 2, with the connections prototype of section 2.

- [x] The SDK form of a connection kind (6.3): how it names its types and gives the lanes, the
  socket, the label and the badge. Start small. The owner expects it to change. Agent decision
  in 6.3, 2026-11-15: a `#[connection]` function gives the lanes and the label.
- [x] Where the controls of a wire go: **Route**, **Corners**, **Bus**, **Lock bends** and
  **Reset route** (15.9). The mockups show them in a toolbar and in a panel. Agent decision in
  15.9, 2026-11-15: the **Look** section of the **Inspector**. **Bus** and **Lock bends** come
  with the automatic router.

## 5. Editor screens that the spec does not define

These block milestone 2. The [mockup plan](mockup-plan.md) lists them too.

- [x] How the user adds a node other than with a drag from the **Project** panel, for example
  with a search palette. Agent decision in 15.2, 2026-11-15: the **Add node** palette opens on
  a double-click on an empty area or on Shift+A.
- [x] How the user makes a new node, view or widget folder, and names it. Agent decision in
  15.2, 2026-11-15: the **+** button of the **Nodes** group opens a name field. **Views** and
  **Widgets** get the same button when they come.
- [x] How the user adds a crate to the project. Agent decision in 15.4, 2026-11-15: a crate is
  a dependency of one node folder. **Add crate** in the **Code** tab runs `cargo add`, with undo.
- [x] The start screen: new project, open project and recent projects. Spec 15.10 defines it,
  and the editor has it.
- [x] The settings screen. Agent decision in 15.10, 2026-11-15: the **Settings** button of the
  start screen shows the toolchain, the data folders and the trusted projects. **Remove** takes a
  folder off the trusted list.
- [x] Where the large node GUI goes in the inspector. Agent decision in 15.4, 2026-11-15: at
  the top, under the header and above the tabs, as the mockup plan proposed.

## 6. GUI guide colors

These affect only the look of milestone 2. See the [GUI visual guide](gui-visual-guide.md).

- [x] The selection color of the Trigger family, #E89C91, is close to the red of the error
  border, #ED7B78. A selected trigger node can look like an error. Agent decision in the
  [theme](gui-visual-guide/theme.md), 2026-11-15: the Trigger selection is now #F2C4B8.
- [x] The accent #DFA64B, the selection color of the Input family #E3B869 and the warning color
  #E5B85A look almost the same. Agent decision, 2026-11-15: the warning is now yellow, #E6D35C.
  The accent and the Input selection both mean selection, so they stay.
- [x] The copper of the Material family and the amber of the Input family are hard to tell
  apart. Agent decision, 2026-11-15: the Input header is now gold, #655219.

## 7. Not blocking

- [x] Drop or mark the planned images that mockups 4 to 9 already cover. Mockup 4 covers 0002,
  mockup 5 covers 0008, mockup 6 covers 0017, mockups 7 and 8 cover 0009, and mockup 9 partly
  covers 0006. The [mockup plan](mockup-plan.md) marks these rows **Covered**, 2026-11-15.
- [x] Link [laser-measurement-design.md](laser-measurement-design.md) and
  [circuit-graph-design.md](circuit-graph-design.md) from the README.
- [x] Create `STATUS.md` with the result of the first prototype (AGENTS.md).
- [ ] Make the Python module of a package (spec 14, milestone 8). It needs the `pyo3` crate,
  a download, and a Python install for the test. Ask the owner to approve the download.
- [ ] Add a check for updates: Patina looks for new releases on GitHub. The owner asked for it
  on 2026-11-15, for later.
- [ ] Compare the mechanics and the look of all mockups with the editor, not only mockup 9.
  List each control and behavior that the editor does not have yet. The owner asked for it on
  2026-11-15, for later. The first pass on 2026-11-15 added only visual polish: shadows, card
  panels, header gradients, pins, sliders and the selected rows.
- [x] Add the node header colors of spec 15.3 (`#[node(color = ...)]`), with the tinted node
  border of the Material Layers node of mockup 9 and the swatches of the Project panel. Done
  on 2026-11-16.
