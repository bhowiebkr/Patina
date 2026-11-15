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
- [ ] **CI.** A GitHub Actions workflow that runs the clippy and test commands of AGENTS.md
  on Windows. Until it exists, run them before each commit.
- [x] **License.** MIT OR Apache-2.0, as the owner decided on 2026-11-15. The files are
  `LICENSE-MIT` and `LICENSE-APACHE`. The README gives the terms for contributions.
- [ ] **A clean Windows computer for the toolchain prototype.** A standard account, with no
  Visual Studio. A Hyper-V virtual machine on Windows 11 Pro is one way.
- [ ] **Linux and macOS runs (optional).** Milestone 0 runs there when a machine or a CI runner
  can (13.5). GitHub Actions runners can run the type and hot reload prototypes.

## 2. Milestone 0: risk prototypes

These are tests, not decisions. Each result can change the design. Report each result before
the next prototype (section 19).

- [x] **Rust types across clusters** (5.1, 5.2). `String`, `Vec` and a type with `Drop` between
  clusters, and the type key check.
- [ ] **Hot reload** (8.5). A value that outlives a reload, the memory and disk use of each
  reload, and the time of a cold build and of a rebuild.
- [ ] **Toolchain** (13.2). Zig as the C compiler, and a Patina folder at a path with spaces and
  non-ASCII characters. The result decides the Windows target, for example
  `x86_64-pc-windows-gnu` (13).
- [ ] **Viewport** (15.8). A `wgpu` viewport in a Tauri panel. The result decides how the frames
  reach the window: a copy into the web view, or a native surface over the panel.
- [ ] **Connections** (15.9). 1,000 wires, 200 of them buses of 8 to 32 lanes, with lane
  states that change at 60 FPS. The result decides SVG wires or a canvas or WebGL layer, and
  informs Q2.

## 3. Owner decisions from section 18

The numbers are the question numbers of section 18. Each group lists the milestone that the
questions block.

Milestone 1, engine and CLI:

- [ ] **Q1, project file format.** The graph files in `graphs/`, `patina.toml`, where the
  parameter values and the node positions go, and the import record (6.2). The CLI cannot run a
  saved graph without it. Decide this first. It also covers the route points, branch collars
  and cable groups of wires, and pin IDs that survive a rename of a subgraph pin (15.9).
- [ ] **Q9, first workflow.** Save at each change or with Ctrl+S. How the editor shows unsaved
  code, saved code and the build that runs. It affects the file format.

Milestone 2, editor:

- [ ] **Q2.** React or Svelte.
- [ ] **Q8.** The project saves the dock layout and the state of each view, or each user keeps
  their own layout.
- [ ] **Q10.** A **Stop** or **Cancel** button next to **Pause**, and what it can promise for
  native code.

Milestone 4, automatic parallelism and the frame domain:

- [ ] **Q11, side effects while editing.** Which batch nodes run again after a parameter change.
  How Patina prevents repeated writes to files or the network while the user drags a slider.
- [ ] **Q12, live values in a fused chain.** How the engine shows the value of a wire between
  two inlined nodes (9.2, 15.9). Milestones 4 and 6 can measure the cost.

Later milestones:

- [ ] **Q7, fuse time.** Milestone 6 measures it.
- [ ] **Q3, audio block size and number of audio threads.** Milestone 11.
- [ ] **Q4, process isolation on or off by default.** An optional target (16).
- [ ] **Q5, Zig and Xcode on macOS.** A test on macOS can answer it.
- [ ] **Q6, app mode.** Milestones 8 and 10, packaging and views.
- [ ] **Q13, circuit graphs.** Feedback without a frame delay, buses with more than one driver,
  and clock edges. A simulation node, a new clock domain, or not in version 1. See the
  [circuit graph design](circuit-graph-design.md).

## 4. Connections

These block milestone 2, with the connections prototype of section 2.

- [ ] The SDK form of a connection kind (6.3): how it names its types and gives the lanes, the
  socket, the label and the badge. Start small. The owner expects it to change.
- [ ] Where the controls of a wire go: **Route**, **Corners**, **Bus**, **Lock bends** and
  **Reset route** (15.9). The mockups show them in a toolbar and in a panel. The spec does not
  place them.

## 5. Editor screens that the spec does not define

These block milestone 2. The [mockup plan](mockup-plan.md) lists them too.

- [ ] How the user adds a node other than with a drag from the **Project** panel, for example
  with a search palette.
- [ ] How the user makes a new node, view or widget folder, and names it.
- [ ] How the user adds a crate to the project.
- [ ] The start screen: new project, open project and recent projects.
- [ ] The settings screen.
- [ ] Where the large node GUI goes in the inspector. The mockup plan proposes the top, under
  the header. The spec does not say.

## 6. GUI guide colors

These affect only the look of milestone 2. See the [GUI visual guide](gui-visual-guide.md).

- [ ] The selection color of the Trigger family, #E89C91, is close to the red of the error
  border, #ED7B78. A selected trigger node can look like an error.
- [ ] The accent #DFA64B, the selection color of the Input family #E3B869 and the warning color
  #E5B85A look almost the same.
- [ ] The copper of the Material family and the amber of the Input family are hard to tell
  apart.

## 7. Not blocking

- [ ] Drop or mark the planned images that mockups 4 to 9 already cover. Mockup 4 covers 0002,
  mockup 5 covers 0008, mockup 6 covers 0017, mockups 7 and 8 cover 0009, and mockup 9 partly
  covers 0006.
- [x] Link [laser-measurement-design.md](laser-measurement-design.md) and
  [circuit-graph-design.md](circuit-graph-design.md) from the README.
- [x] Create `STATUS.md` with the result of the first prototype (AGENTS.md).
