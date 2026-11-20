# Status

All milestones of the roadmap (spec section 16) ended. Milestone 12, the event domain, ended on
2026-11-16. The [quality plan](docs/quality-plan.md) of 2026-11-17 is done, except two items
that need the owner. The next work is the open items of the known limits and of
[docs/before-building.md](docs/before-building.md).
The 7 items of the owner feedback of 2026-11-19 are done since 2026-11-20:
[docs/feedback-2026-11-19.md](docs/feedback-2026-11-19.md).
Milestone 11, the audio domain, ended on 2026-11-16. Since 2026-11-20, an audio node can have a GUI
with a display value (spec 4.3). The settings and the messages of audio GUIs stay deferred.
The circuit graphs of question 13 (spec section 18) work: the computer example of NAND gates is
complete since 2026-11-16. Milestone 10, views,
ended on 2026-11-15. Milestone 9, the GPU, ended on 2026-11-15. Milestone 8, packaging, ended on 2026-11-15 without the Python module, which needs
a download that the owner did not approve yet. Milestones 2 to 10 work, with the open items in their known limits. The installers of
milestone 3 are not made yet: the WiX tools need a download that the owner did not approve yet.
Milestone 1, the engine and the CLI, works. Milestone 0 ended on 2026-11-15, with three owner
checks open.


The records of the ended milestones and of the finished work are in [docs/history.md](docs/history.md).

## What works

- Stereo audio: pooled stereo blocks reach device playback and float32 WAV export.
  Mono nodes feed both channels. The C API keeps its mono function and adds a stereo function.
  Windows workspace Clippy, workspace tests, and frontend checks passed on 2026-11-19.
  Edit and Bake exports matched. Tests checked channel separation and C API compatibility.
  The synth arrangement uses this path. Speaker listening, Linux, and macOS remain unchecked.

- Smart material prototype: the GPU example adds 14 node types and a second top graph with 28 material nodes and three workflow nodes.
  On Windows MSVC and RTX 4090 DX12, all material channels matched Surface exactly at 512 x 512.
  The 640 x 640 shaded preview matched Surface's renderer. Independent roughness and metallic tests passed.
  Hidden editor checks, workspace Clippy, workspace tests, and frontend checks passed on Windows.
  Object loading, traced mesh maps, target replacement, and eight PBR PNG exports passed GPU and hidden editor checks.
  The renderer, mesh baker, object loader, and export stay inside `workbenches/gpu`.
  General recipe import, UV unwrapping, overlapping UV detection, other object formats, and non-MSVC workflows remain open.
  The graph uses Millwright's original sphere, UV atlas, and saved 2048 maps. Loaded maps match the reference exactly.
  Explicit 2048 rebaking matched Millwright's direct baker. The corrected sphere workflow passed hidden editor and both CLI build modes.
  Existing bake files load from disk. Newly traced bake files are not saved. GPU allocation counts were not measured.

- View layout: twelve dock instances, responsive columns, dock size reads, and one undo step per canvas drag.
  Windows frontend checks, workspace Clippy, and workspace tests passed on 2026-11-18.
  View metadata selects the initial dock. A tab menu keeps crowded docks usable.
  The hidden Windows GUI check passed the nine synth workflows at narrow side widths and a shallow bottom height.

- Center split and workbenches: the center area splits into the graph and a second pane of a
  changeable size (15.2). The examples are now workbenches, and the start screen lists them (13.3).
- Geometry kernel prototype: Manifold cuts a seeded gun of 30 parts in 2.7 ms on Windows. The model workbench will use it (task D0).
- Model workbench base nodes: primitives, sketches, extrude, revolve, moves, arrays, booleans,
  attach at a socket, and seeded random values and picks (task D3).
- Model workbench run model: a model builds at **Run** and after each change, not at each frame
  (task D4).
- Model workbench 3D view: `model-view` opens next to the graph and shows the solid of the
  selected node, with orbit, pan, zoom, a grid, part colors and a wireframe (task D5).
- Model workbench export: the `export` node writes `.glb`, `.obj` and `.stl` files after a
  click on **Export** or at a trigger fire. The sockets go into the glTF extras (task D6).
- Model workbench guide: a note and an `export` node in `main`, and the guide graphs
  **solids**, **sketches**, **seeds** and **sockets** (tasks D7 and D8).
- City workbench: a seeded organic city on hills with a river and a coast, grown roads, zones
  and five kinds of detailed buildings, a planned grid city, trees and lamps as instances, and
  an export of each building to a `.glb` file (task E).
- Gun workbench: seeded guns from seven kinds of parts, six classes, four original makers with
  their own shape language, five rarities and five elements, with generated names, stats, a
  **Toon** look in `model-view`, the `gun-wall` view and an export to `.glb` and `.txt` (task F).
- Start screen: **Open project** opens the folder dialog directly, **New project** opens a dialog, and the recent list has **Name** and **Path** columns and a filter (15.10).
- Groups and notes: colored transparent groups with a title bar move the nodes in them, and notes show Markdown on the canvas (15.12).
- Error display of mockup 3: one **Errors: N** count with a red dot, a time on each **Logs** line, and one error text in each place (15.5).

- Ember sound workbench: 44 node types, three learning patches, a six-track song, instrument panels, and dock editors for sound design and arrangement.
  The **sandstorm** graph adds a short hook study with a pulse lead, bit reduction, and six editable tracks.
  Its revised voices add oversampled lead shaping, filter envelopes, aligned bass phases, stereo unison, and separate drum bodies and transients.
  The **Sound** page exposes these controls, stereo room, low cut, and beat-synchronized volume reduction.
  Drums play one shot. Closed hats choke open hats. Track validation accepts the minimum release value.
  The study approximates the sound and accompaniment. It does not contain the full song arrangement.
  Windows example tests, workspace checks, and stereo rendering passed. Its dock controls passed save and undo checks.
  The owner rejected the previous lead. Its correction uses a dominant pulse and a saw 31 semitones below it.
  The source follows a published remake. It does not claim to reproduce the original processed sample.
  The corrected mix peaks at 0.765722 without limiter hits. Its solo render and comparison are available locally.
  Optimized track processing measured 0.161 ms mean and 0.191 ms p95 per 256 frames, with zero allocations after warmup.
  The synth workspace uses its copied SDK. Its tracked manifest no longer points outside the workbench.
  The **audio-capture** and **spectrogram** nodes add continuous frequency history on the canvas.
  The **sandstorm** graph connects the viewer to its melody track. It has freeze, clear, frequency, level, and time controls.
  FFT history updates measured 0.032 ms mean and 0.042 ms p95, with zero allocations after initialization.
  The viewer passed fused Edit, slot, and Bake checks. A headless browser displayed its actual GUI and dispatched freeze and clear events.
  Listening fidelity, full-editor visual inspection of these changes, and other operating systems remain unchecked.

- Hello and gpu examples: hello is a pocket calculator with keypads, tapes and an **AVG** subgraph. gpu has color bars, a mix, a knob and a waveform monitor.
- Node symbols and styles: Gate and block nodes show circuit symbols, and a subgraph sets a **Shape** for its uses. Each example has its own style crate in `widgets/`.
- Subgraph tools: **New subgraph** in the **Graphs** group makes an empty subgraph. Two or more selected nodes show **Collapse into subgraph...** in the **Inspector**.
- Editor menus: The **Project** panel has right-click menus, and the WebView menu does not show. A build keeps the node GUIs, and only the slot node shows **Compiling**.
- Rust version of a project: The `rust` of `patina.toml` is the oldest Rust version that builds the project. **Update toolchain** raises the toolchain.
- Copy, paste and libraries: **Copy**, **Cut** and **Paste** of nodes work. **Settings** > **Libraries** and **Add from Git** give nodes from other projects. The **Library** tab shows them with a preview of what a pick brings.
- Gate speed: Light nodes share one task, nets share one check and skip the `Option` checks when their inputs have values, and the frame rate goes up to 100000 FPS.
- Touchpad pan: A two-finger touchpad scroll pans the graph. The mouse wheel and a pinch zoom.
- Arrange with node sizes and bus widths: **Arrange** uses node sizes and wire widths. The 25 graphs of `workbenches/computer` have 0 node overlaps.
- No save without Save: Only Ctrl+S or **Save** writes graph files, the frame rate and the text of **Add crate**. The recovery file keeps the unsaved graphs, code and frame rate. An undo of a code edit puts the text into the **Code** tab and writes no file.
- Snake on the computer: The `programs/snake.asm` game runs on the computer. The CLI test checks 400 frames.
- Sprite GPU of the computer: The `sprites` node draws 8 sprites, and the `monitor` view draws the screen on the GPU.
- Bigger CPU of the computer: The `cpu` has 16 operations and 794 NAND gates. The `ram` node holds 256 bytes.
- Joystick of the computer: The `joystick` node gives key input on ports 0xF6 to 0xF8. A canvas can read keys.
- Assembly editor of the computer: The `program` node has an assembly editor with line numbers and error lines. A node GUI can write parameters.
- Speed, frame rate and random numbers of the computer: The editor sends only changed values. The frame of the computer went from 8.3 ms to 1.8 ms.
- IO devices of the computer: The `cpu` has IO ports 0xF0 to 0xFF with a keyboard, a terminal, a GPU and a screen.
- Program node of the computer: The `program` node keeps the CPU code in an `.asm` file and has an assembler.
- Open with no rebuild: An open of a trusted project loads the last build, so a close, an open and **Run** build nothing.
- Canvas editing: A drag from the **Project** panel adds a use. A right-click opens a menu for each target.
- Process isolation: The editor runs the engine in a node process. A crash ends only that process.
- Circuit graphs: The computer example of NAND gates is complete: gates, adders, a register, a RAM and a CPU.
- Milestone 12: the event domain: The event domain runs async nodes and triggers in the CLI and in the editor, with **Fire**.
- Milestone 11: the audio domain: The audio domain runs blocks on an audio thread and plays them with `cpal`. An audio node GUI shows a display value (4.3).
- Milestone 10: views: `#[view]` builds a view library. The editor lists views, and a view reads and edits the graph.
- Milestone 9: the GPU: `patina-gpu` gives a `wgpu` device and a WESL linker. `patina-cuda` compiles CUDA with NVRTC.
- Milestone 8: packaging: `patina-cli package` and the **Package** button make a package folder with a runner. The Python module is open.
- Milestone 7: subgraphs and reuse: A graph uses subgraphs with public pins. Ctrl+G collapses nodes. **Import** and **Update** bring nodes.
- Milestone 6: layers: Each cluster links a shared `std`. Builds are reproducible, and the state of a node moves to a new build.
- Milestone 5: stateful nodes, node GUIs and generic nodes: Nodes keep a state, show a GUI, and take type parameters. A canvas and a full size GUI work.
- Milestone 4: automatic parallelism and the frame domain: The plan makes tasks, and a worker pool runs them in parallel. The frame domain runs without allocation.
- Milestone 2: editor: The editor opens projects, edits the graph and the code, runs and stops it, and shows values, views and logs.
- Milestone 1: engine and CLI: `patina-engine` and `patina-cli` read a project, check the graph, build the clusters and run them.
- Milestone 0: risk prototypes: The five prototypes passed on Windows 11. Two have an owner check open.

## Checks

Run these commands before a commit, as [CI](.github/workflows/ci.yml) does:

- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- In `crates/patina-editor/frontend`: `npm run check` and `npm test`

## Speed measurements

The numbers of phase 4 of the [quality plan](docs/quality-plan.md). They come from the debug
editor with the built page, on the Windows 11 computer of the owner.

- Start of the editor page, 2026-11-17. Monaco and the check code now load only when needed.
  - The start script went from 4,102,438 bytes to 609,205 bytes. The start CSS went from
    211,145 bytes to 51,495 bytes.
  - Monaco (3,406,377 bytes of script, 159,651 bytes of CSS) loads with the first open of the
    **Code** tab. The selfcheck code (72,994 bytes) loads only with `--check`, and the
    screenshot code (1,597 bytes) only with `--shot`.
  - The page start, at the end of `DOMContentLoaded` in the hidden selfcheck window, went from
    137 ms and 131 ms to 31 ms and 34 ms (two runs each).
- Canvas with 500 and 2000 nodes, 2026-11-17. `patina --check <dir> --bench` makes the graphs
  and measures the mean time between frames of a pan at the zooms 1, 0.5, 0.3 and 0.2, and of a
  node drag at the zoom 1.
  - The cause was the Layerize step of Chromium. Each React Flow pin had a `translate`
    transform, the order pin a `rotate`, and each icon an inline SVG.
  - The pins now have margins, the order pin is a diamond of gradients, and each icon is a CSS
    background.
  - 2000 nodes: the pan went from 533, 514, 503 and 448 ms to 279, 274, 283 and 251 ms. The
    drag went from 536 ms to 328 ms.
  - 500 nodes: the pan went from 41 ms to 24, 23, 23 and 17 ms. The drag went from 82 ms to
    41 ms. The old bench stayed at the zoom 1 for this graph.
  - The view of 2000 nodes has 251,961 bytes. A move edit takes 62 ms, and 101 ms to its frame.
  - Later runs had a game in the background. The same-run control stayed near 280 ms, so the
    comparisons below are valid, but their absolute numbers are high.
  - The pins that went out at the left and the right of a node cost a factor of 4. The pins
    are now in an 8 px gutter of the node wrapper, and `.patina-node` has no `position`.
  - Two runs each, with the old pin layout as the control in the same run: the 2000-node pan
    went from 280, 279, 296 and 258 ms (and 277, 274, 292 and 256 ms) to 70, 75, 92 and 67 ms
    (and 70, 76, 94 and 67 ms). The drag went from 350 and 343 ms to 155 and 159 ms.
  - 500 nodes: the pan went from 24, 22, 23 and 17 ms to 17 ms. The drag went from 41 ms to
    27 ms. The edit frame did not change.
  - The rule that sends a click in the gutter to the canvas costs nothing: the same pan with
    and without it.
- Edit on the computer example, 2026-11-17. A game ran in the background, so the absolute
  numbers are high. The old code ran as the control in the same run.
  - The view has 177,944 bytes. The domains are 150,005 bytes of it (1728 entries), the graph
    10,945 and the node kinds 19,907. The JSON costs 0.2 ms, and a parse in the page 0.2 ms.
  - The time was in the engine. A release test of the view parts went from about 148 ms to
    28 ms: the check from 75 to 21 ms, the subgraph pins from 61 to 3.4 ms, and the node folders
    from 11.6 to 1.7 ms.
  - The causes: each use of a subgraph read and parsed its file again, and each edit parsed the
    Rust code of all node folders again. A move of a node also ran the full check.
  - Now one call reads each subgraph file one time, the parse of a node folder stays until its
    code changes, and an edit of only the layout keeps the last check. A move gives the same
    domains and problems as before.
  - In the debug editor page, a move went from about 500 ms to 31 ms, and an undo to 30 ms. An
    open of the graph takes 135 ms, because it runs the check. In release, a move needs about
    6 ms in the engine.
- Frame loop of the editor on the computer example, 2026-11-17. A temporary allocation counter
  measured each shown frame of a hidden `--shot`. A game ran in the background.
  - The frame of the engine part went from 1332 to 12 allocations (65 bytes). The reports and
    the GUI calls now reuse their buffers. The CLI test graph gives 0.
  - The values and the event for the page went from 8320 allocations (803 KB) and 4.5 ms to
    9 allocations (16.8 KB) and 2.6 ms. About 787 values change in each frame.
  - The event now borrows the lists of the frame, and the copies for the subgraph pins reuse
    their buffers. The rest of the time is the JSON of the badges in the debug build.
  - The 9 allocations were the list of the changed values. The frames now use one list again.
    A unit test counts 0 allocations in 100 frames of 787 changed values, before 900.
- Tests and builds of the computer example, 2026-11-17. A game ran in the background. The old
  code ran as the control in the same run.
  - The code generation searched all steps for the readers of each output. In a debug test, the
    code of the computer example (2,177,691 bytes) took about 1060 ms.
  - Now each step keeps the readers of its outputs. The same code takes about 30 ms, and the
    text is the same, byte for byte. The editor and the CLI generate it at each build.
  - A build generates the code two times: for the hash of the base and for the files. The second
    time now costs about 25 ms, so it stays.
  - The CLI tests also copied the cargo `target` folder of a project. The copy now skips it, as
    the walk of the project files does.
  - The CLI tests were 4 test binaries, and cargo runs them one after the other. They are now
    one binary, `tests/cli`, so the 11 tests run in parallel.
  - The tests of `patina-cli` went from 30.0 s to 12.9 s and 12.4 s. The computer test alone
    went from 16.1 s (in an earlier check) to 8.9 s. The events test is now the longest: it runs
    the CLI 12 times, with a build after each graph change.
  - Each wait of the selfcheck looked at the page again only every 250 ms. An engine event
    stopped the wait, but a change of the page did not. Now it looks every 20 ms.
  - The selfcheck went from 62.4 s to 33.6 s, with the same 201 steps. The slowest steps that
    stay do real work: a build, the 2 s flood test and the stop of an endless loop.
- Node drag while the graph runs, 2026-11-17 (owner report: the drag stuttered). A game ran in
  the background.
  - The cause: each run frame made all nodes again from the view, so a dragged node jumped back
    to its saved place, and all nodes and wires rendered at each frame.
  - Now a frame keeps each node and wire whose data did not change. A node in a drag, or in a
    move edit that did not end, keeps its place.
  - The selfcheck drags a node for 50 animation frames while the frame node runs. The node does
    not jump back, and it stays at the drop point. The animation frames took 16.7 ms on average
    and 16.8 ms at most, so no frame dropped. The test graph is small: nobody measured the drag
    on the computer example.

## Known limits

The open items of each section. Items that the text marks as fixed are in [docs/history.md](docs/history.md).

### Workbenches and the center split, 2026-11-19

- The center area of the dock can show a view next to the graph or under it (spec 15.2, task A
  of [docs/workbenches-plan.md](docs/workbenches-plan.md)). The splitter changes the size with a
  drag or the arrow keys. The layout file keeps the split. A view manifest can set
  `dock = "split"`.
- The examples are now workbenches in `workbenches/` (tasks B2 to B5). The start screen lists the
  included workbenches in a **Workbenches** box, and **Open** opens one. The package copies them
  to `Patina/workbenches/`.
- A repo check fails when a text file of a workbench names a relative path outside its folder.
  The READMEs link to the repository page in place of `../../docs`.
- Checked on 2026-11-19, on Windows only: the CI clippy, `cargo test --workspace`, the frontend
  check and tests, the selfcheck with a split step and the shots of `computer`, `hello` and
  `gpu`. No person used the new GUI.
- The selfcheck opens the project again with a view in the split pane at 544 px, and the split
  comes back (2026-11-19). It makes a project in the `workbenches` folder of the check folder and
  clicks **Open** in the **Workbenches** box. Then it sends the arrow keys to the recent list and
  presses Ctrl+O. The selfcheck passed 359 steps on Windows on 2026-11-19.
- Known limits:
  - The installed edition does not list the workbenches yet. Only the portable folder and a
    checkout have them.
  - An old portable folder keeps its copies of the examples in `projects/`. The package update
    does not remove them, because they can have edits of the person.
  - No person opened a workbench from the **Workbenches** box yet.

### Geometry kernel prototype, 2026-11-19

- Task D0 of [docs/workbenches-plan.md](docs/workbenches-plan.md) measured kernels for the model
  workbench. [prototypes/geometry](prototypes/geometry/README.md) has the pure Rust baseline and
  the results.
- Manifold cut a gun of 30 parts in 2.7 ms and made a block of 500 buildings in 81 ms, with valid
  manifolds. monstertruck failed the gun union. csgrs does not resolve.
- Agent decision: Manifold behind the type crate of the model workbench.
- Known limits:
  - The trials ran only on Windows, in throwaway crates.
  - The default Manifold crate build needs CMake and git. Spec 13.4 does not allow them.
- Task D0b: the `cc` crate built Manifold with no CMake. With the Patina toolchain (gnu target,
  Zig, `rust-lld`), the cold build took 8 s, and a gun took 3.1 ms.
  - The trial set `CXX`, libc++ and a shim for 8 UCRT imports by hand. Task D0c adds this to
    the toolchain.
- Task D0c: the toolchain compiles C++ crates (spec 13.2). The `zig-cxx.exe` wrapper makes
  `toolchain/zig/cxx/libc++.a` at the first C++ compile: libc++, libc++abi and the shim.
  - Checked on Windows with a toolchain run at a Unicode path: a node crate with a C++ file
    threw and caught an exception in its cluster, and the graph showed 40. The cluster imports
    no UCRT function. The first build took 12.7 s with the runtime, and a rebuild took 0.05 s.
  - Fixed with it: build scripts did not start with the own toolchain. They link the shared
    `std`, which needs libgcc_s from the `bin` folder of the target. `PATH` now has it.
  - Unit tests check the environment, the flags and `PATH`. No automated test builds C++ with
    the own toolchain, because the CLI tests use the system toolchain.
  - Known limits: libc++ goes into each cluster DLL (480 KB in the check). The first C++
    compile waits 10 to 20 s for the runtime. `cc` does not build again after a `.cpp` edit,
    unless the build script prints `rerun-if-changed`. Only Windows: the Linux toolchain sets
    no `CXX`, and Linux and macOS did not run.

### Model workbench base nodes, 2026-11-19

- Task D3: [workbenches/model](workbenches/model/README.md) has 25 nodes and a `Sketch` type.
  The sample graph makes a tower from a seed. The agent decisions are in
  [the workbenches plan](docs/workbenches-plan.md), task D3.
- Each boolean runs at once, and the mesh sorts its triangles by their corners. Before this,
  the hash of the tower changed between runs, because the kernel orders the triangles by IDs
  from a global counter.
- Checked on 2026-11-19, on Windows only:
  - The CI clippy and `cargo test --workspace`. The CLI test runs the tower and compares the
    mesh hash.
  - 12 unit tests of the workbench crates and their clippy passed with the MSVC toolchain. CI
    does not run them.
  - The tower gave one mesh hash in 14 runs: 4 with MSVC and 10 with a toolchain run.
- Known limits:
  - Round shapes and turns that are not multiples of 90 degrees use the sine of the C++
    runtime. Their hashes were not compared across platforms.
  - The kernel has no fillet and no bevel. The loft and the top chamfer came later, see the
    loft section.
  - `pick` chooses one of two inputs. Chain picks for more choices.
  - The mesh sort adds a sort of all triangles to each `Solid::mesh` call. Task D4 measured it.

### Gun workbench, 2026-11-19

- Task F: [workbenches/gun](workbenches/gun/README.md) makes seeded guns with the model workbench
  as a library. The agent decisions are in [the workbenches plan](docs/workbenches-plan.md),
  task F.
- The second version adds six classes (pistol, SMG, rifle, shotgun, sniper rifle and
  launcher), detailed parts for each maker, six materials with three paint schemes for each
  maker, the elements fire, shock, corrosive and cryo, and names such as **Brutal Gale**.
- Changes to the model workbench: `types/geometry` has `Solid::project` and `Sketch::outlines`
  for side views. `model-view` has a **Toon** toggle: three light bands, outlines from the back
  faces, ink lines on sharp edges and a lighter background. The city workbench has the new
  imports.
- Checked on 2026-11-19, on Windows only:
  - 14 unit tests of `types/guns`, 1 test of `export-gun`, 2 tests of `gun-wall` and the 5
    tests of `model-view` with the crease test, and the clippy of the gun crates, with the MSVC
    toolchain. CI does not run them.
  - The CLI test runs `main`, checks the two stats reports, and runs the export with a trigger.
  - The gun `.glb` file has the look of the 3D view (2026-11-19): one primitive for each
    material of each part, and one glTF material for each color. The parts are not joined, so
    the file has the faces inside the overlaps. The CLI export gave 27 primitives and 6
    materials, and the JSON parsed.
  - The editor shot of `gun` opens `gun-wall` and `model-view`, turns **Toon** on and off, and
    changes the seed. The captures show the gun wall in the maker colors and the toon bands.
  - A click on a tile of `gun-wall` sets the seed and selects the `look` node, so the 3D view
    shows that gun. The shot clicks the first tile and checks the selection and the new mesh.
  - Maker marks (2026-11-19): each receiver has the logo and the name of its maker on its two
    sides, in raised letters of the trim color. Two unit tests check the letters, the logos and
    the strokes. A preview of the meshes of the 24 guns of all classes and makers showed each
    mark readable on the receiver, from left to right on the two sides.
  - Sculpted grips (2026-11-19): the Vesper and Halo grips are lofts through egg outlines at
    four heights, with a taper, a cut bottom edge and a palm swell. A unit test checks the
    `taper` helper. A preview of the pistols showed the taper, the swell and the finger grooves.
  - Sculpted stocks (2026-11-19): the Vesper and Halo stocks are lofts along the gun. The
    Vesper stock has round sides, a narrow wrist, a round butt and its glow line in a groove.
    The Halo spine is round, with a waist. A preview of the rifles and the snipers showed the
    shapes, and the CLI test has the new triangle count.
  - Faceted Blockwerk grip (2026-11-19): the Blockwerk grip is a loft through chamfered
    outlines, with a palm swell, raised rubber bands, a cap and a long pin. It keeps the blocky
    style. A preview of the six Blockwerk guns showed the facets and the bands. The CLI test
    has the new triangle count.
  - Faceted Blockwerk stock (2026-11-19): the Blockwerk stock is a loft along the gun through
    chamfered outlines, with the frame hole. The cheek rest sits on the flat sides. A preview
    of the Blockwerk shotgun and sniper showed the facets.
  - Serial numbers (2026-11-19): each receiver has a serial number of six digits on its right
    side. The seed of the gun sets the number. A preview of the rifles, the pistols and the
    launchers showed the numbers on a free area of each maker. The CLI test has the new
    triangle counts.
- Known limits:
  - The Scrapjaw grip and stocks are extrusions and turned profiles, which fit the junk style.
    The other parts are extrusions, turned profiles and booleans.
  - The maker mark and the serial number are the only text. The other parts have no decals,
    logos or text. The serial number adds about 1400 triangles to a gun.
  - Linux and macOS did not run the workbench.

### City workbench, 2026-11-19

- Task E: [workbenches/city](workbenches/city/README.md) makes a seeded city with the model
  workbench as a library. The agent decisions are in
  [the workbenches plan](docs/workbenches-plan.md), task E.
- Changes to the model workbench: `types/geometry` has a `Scene` type (items of one mesh at many
  places, with a group for each building). `model-view` draws a scene with GPU instances.
- Checked on 2026-11-19, on Windows only:
  - 6 unit tests of `types/city` and 1 test of `export-buildings`, and the clippy of the city
    crates, with the MSVC toolchain. CI does not run them.
  - The CLI test runs `main`, a district of 1 km x 1 km, and the export of each building with a
    trigger.
  - The editor shot on a hidden desktop opens `main`, shows the `merge` node in `model-view` in
    the second center pane, and checks that seed 8 and undo change the mesh.
- The second version (E9 to E12, 2026-11-19): `main` is an organic city. `terrain`, `river`
  and `ground` make the land. `density`, `highways`, `grow` and `prune` make the roads.
  `zoning` and `heights` make the zones. `buildings`, `walls`, `facades` and `roofs` make
  houses, apartments, shops, towers and warehouses. The grid city is the `planned` graph.
  - The roads, the road lines and the streets stay in the area of the terrain.
  - The river follows the valleys: the path of least height from the highest edge to the
    lowest edge, with a pull to the middle.
  - A cut across a concave block gives one lot for each part, not one lot with a bridge of no
    width over the street.
  - A thin lot becomes a park (its inset by 2 m has less than 16 m2), so it gets no thin
    building.
  - `terrain` has hydraulic erosion (2026-11-19): drops of rain cut the valleys and sharpen the
    ridges. The sea takes their soil, so the coast stays. **erosion** sets the drops for each
    sample (default 1, 0 to 8). A unit test checks the seed, 0 drops, the range and the soil.
    The hidden editor shot of `city` showed the eroded city in `model-view`, and seed 8 passed.
  - Each building `.glb` file has one glTF material for each item color (2026-11-19). The
    color goes from sRGB to linear, with no metal. A `geometry` test checks the material JSON,
    and the `export-buildings` test finds the materials in a written file.
  - Checked on Windows: 20 unit tests of `types/city` and the other city crate tests, and the
    CLI test with `main`, `planned`, the 1 km check and the export of 173 buildings.
  - `main`: 574 items, 42651 places and 1329526 triangles. `planned`: 1269 items and 4580506
    triangles. A city of 1 km x 1 km: 3128723 triangles, 0.34 s after the build in the CLI
    test.
  - `river` cuts a valley (2026-11-19). **valley** sets the width of each side in meters, and 0
    gives no valley. The floor goes down along the river and stays at the sea level or above.
    - `main` uses 80 m. A valley of 40 m gave steep sides, and `main` had 130 buildings. With
      no valley it had 173, and with 80 m it has 162.
    - `main`: 541 items, 39909 places and 1250223 triangles. A city of 1 km x 1 km: 92397
      places and 2927971 triangles, 0.32 s after the build in the CLI test.
    - Checked on Windows: the unit tests of `types/city`, with a test that the valley cuts the
      slopes beside the bank and not the far edge, and the city CLI test.
  - The erosion cuts streams (2026-11-19). The water of each sample runs down to the sea or the
    edge, also out of the pits. Where enough water collects, a stream cuts a bed with smooth
    valley sides. More drops cut deeper, and flat land stays flat.
    - `river` now follows a stream valley in the north of a 1 km area.
    - `main`: 533 items, 42244 places and 1308703 triangles. `planned` has no change. A city of
      1 km x 1 km: 87263 places and 2751225 triangles, 0.24 s after the build in the CLI test.
    - The erosion takes 19 ms for `main` and 81 ms for 1 km x 1 km in a release build.
    - Checked on Windows: 21 unit tests of `types/city`, with a test that a stream cuts down
      along a valley and out of a pit, and the city CLI test. A shaded preview of the terrain
      showed streams with branches.
  - The river follows the routes of the water (2026-11-19). It starts at a low point of the
    edge opposite the lowest edge and runs down the steepest route to the sea, as the streams
    do. Thus the streams join it.
    - A pull to a line that the seed picks and a soft wall near the side edges keep the river
      off the side edges. Without the wall, the `main` river ran along the north edge.
    - `main`: 539 items, 39364 places and 1223578 triangles. `planned` has no change. A city of
      1 km x 1 km: 92954 places and 2917617 triangles, 0.34 s after the build in the CLI test.
    - Checked on Windows: 22 unit tests of `types/city`, with a test that the river follows a
      valley that turns, and the city CLI test. A shaded preview showed the river in the north
      of `main` and of the 1 km area, with curves and off the side edges.
  - A thin part of a cut joins a lot (2026-11-19). A cut of a block can give a thin part. That
    part now joins the lot across the cut with the longest shared piece of the cut. The part
    stays a park when the joined lot is also thin.
    - `main`: 539 items, 39740 places and 1235164 triangles. `planned` has no change. A city of
      1 km x 1 km: 1731 items, 94298 places and 2960785 triangles, 0.23 s after the build in
      the CLI test.
    - Checked on Windows: 24 unit tests of `types/city`, with a U block with a thin arm and a
      block where the thin part touches two lots, and the city CLI test.
  - Each lot keeps a street edge (2026-11-19). A lot with no street edge became a park, so
    blocks had green clusters inside. Now a cut goes across the short axis when the cut across
    the long axis gives a part with less street edge than the lot width. No part of that cut
    can be more than 4 times as long as wide. Else the lot stays bigger.
    - Parks: 68 to 43 in `main`, and 199 to 58 in the 1 km area. A top view showed deeper lots
      and few parks inside the blocks.
    - `main`: 567 items, 40915 places and 1265020 triangles. `planned` has no change. A city of
      1 km x 1 km: 1803 items, 95518 places and 2956283 triangles, 0.23 s after the build in
      the CLI test.
    - Checked on Windows: 25 unit tests of `types/city`, with a test that each lot of a large
      block keeps a street edge, and the city CLI test.
  - A lot with a thin arm is not thin (2026-11-19). A lot is thin when its inset by 2 m has
    less than 16 m2. For a rectangle, this is the old rule of 2 x area / perimeter below 4 m.
    The inset also drops an arm that closes, so the house of such a lot stands on the large
    part. Before, the inset of such a lot crossed itself or gave no house.
    - `main`: 573 items, 41357 places and 1277498 triangles. `planned` has no change. A city of
      1 km x 1 km: 1791 items, 93244 places and 2886571 triangles, 0.33 s after the build in
      the CLI test.
    - Checked on Windows: 26 unit tests of `types/city`, with a lot of 14 m x 14 m with an arm
      of 2 m x 46 m, and the city CLI test.
- Measurements of the first version, on the computer of the owner, with a game open at the
  same time:
  - The `main` graph: the bake build takes 7.2 s, and the run takes about 0.3 s. The edit build
    takes 9.2 s the first time. The result has 559 items, 53244 places and 664546 triangles.
  - A district of 1 km x 1 km: 2163 items, 105482 places and 1352998 triangles. The run after the
    build took 0.14 s in the CLI test and 0.35 s in a CLI run by hand. The E8 target is 2 s.
- Known limits:
  - The CLI test checks the counts and the bounds, not the scene hash. Round shapes and turned
    roofs can give another hash on another platform.
  - D4b stays open: in the editor, a run of the `main` graph stops the frames and the views for
    about 0.3 s after each change.
  - Linux and macOS did not run the workbench.

### Model workbench loft and the fillet trial, 2026-11-19

- `Solid::loft` and the `loft` node of [workbenches/model](workbenches/model/README.md) make a
  solid from the outline of a bottom sketch at z = 0 to the outline of a top sketch.
  - Each outline gets the corners of the other outline at the same part of its length. Each
    outline starts where a line from its center to +X leaves it, so the sides do not twist.
  - The Manifold triangulator closes the two ends, and the kernel checks the closed mesh.
  - The `sketches` guide graph lofts a square to a circle.
  - Holes (2026-11-19): each sketch has one outer outline, and the two sketches have the same
    number of holes. Each hole of the bottom joins the hole of the top with the nearest center.
    A unit test lofts a frame to a round tube, and two holes where one hole moves.
  - Islands (2026-11-19): a sketch can have more than one outer outline. Each outer outline
    joins the outer outline of the other sketch with the nearest center and size. A hole goes
    to the smallest outer outline around it. Two joined outer outlines must have the same
    number of holes. A unit test lofts a frame and a square to a ring and a circle, and an
    island in a hole to an island in a hole.
- The fillet trial, measured in a release build while the computer ran a game:
  - A Minkowski opening with a ball took 1.5 to 7.3 s for a cylinder and box of 324
    triangles, and 3.9 to 23 s for a sphere and box of 2112 triangles. This is too slow for
    a live node.
  - `SmoothOut` with `Refine` took less than 8 ms, but the faces bulge out. A cube of 2 grew
    to +-1.169 at a smoothness of 0.3 and to +-1.337 at 0.6.
  - Thus the model workbench has no fillet node and no bevel node.
- Top chamfer (2026-11-19): `Solid::chamfered` and the **chamfer** of `extrude` cut the top
  edges at 45 degrees. Each edge moves into the material, and the corners stay sharp. A
  chamfer that removes an edge, or that makes a hole touch an outline, gives an error. It
  works only with no twist and no taper. A unit test measures the volume of a box, a frame
  and an L.
- Checked on 2026-11-19, on Windows only: the `geometry` unit tests of the loft and the
  chamfer, the unit tests of the city and gun copies, and the CLI test of the guide graphs.
- Known limits:
  - A loft cannot join one outline to two outlines, for example one square to two circles.
  - Very different outlines, for example a U to a circle, can give sides that cross. The
    kernel then gives an error or a solid that is not correct.

### Model workbench guide graphs and checks, 2026-11-19

- Tasks D7 and D8: the `main` graph of [workbenches/model](workbenches/model/README.md) has a
  note and an `export` node. Four guide graphs teach the nodes, each with a note.
- The D8 checks:
  - The seed and mesh tests are the unit tests of `geometry` (tasks D2 and D3).
  - The CLI test runs `main`, the export with a trigger, and each guide graph.
  - The hidden model shot opens the view in the second pane, changes the seed and sees a new
    mesh (task D5). It is the selfcheck step of D8.
  - The shot opens each guide graph and checks its layout (2026-11-19). The rows of the guide
    graphs are now 420 units apart, so the nodes with a 3D preview do not overlap.
- Known limits:
  - The CLI test of the guide graphs checks the triangle count and the bounds, not the hash.
    Round shapes can give another hash on another platform.

### Model workbench export, 2026-11-19

- Task D6: the `export` node of [workbenches/model](workbenches/model/README.md) writes the
  solid with a small writer in the `geometry` crate. The agent decisions are in
  [the workbenches plan](docs/workbenches-plan.md), task D6.
- Tool change: `patina_sdk::fired()` is true while a node runs for a trigger fire (9.4).
- Checked on 2026-11-19, on Windows only:
  - The CI clippy and `cargo test --workspace`. The CLI test runs the tower with no fire and
    sees no file. Then it adds a `trigger` and an order wire, and reads the `.glb` file.
  - The unit tests of `geometry` and `export` and their clippy, with the MSVC toolchain. They
    check the three formats, the sockets in the extras, the replace and the click rule.
- Checked on 2026-11-19, on Windows, with Blender 5.2 without a window: Blender imported the
  tower as `.glb`, `.obj` and `.stl` with no error, each with 144 triangles. The `.glb` has
  the `sockets` extra. The gun `.glb` gave 10484 triangles, as in the CLI, and 6 materials.
  Two city buildings gave their mesh, 9 and 7 materials and the `sockets` extra.
- The hidden model shot clicks **Export** in the node GUI of `export` (2026-11-19). The node
  shows "Wrote tower.glb, 144 triangles", and the shot undoes the click. The file goes into
  the git copy of the shot.
- Known limits:
  - The `.obj` and the `.stl` files have no sockets. A solid has no colors, so the `export`
    node writes no materials. The `.glb` writer gives each part with a color a material
    (2026-11-19), and the city and the gun exports use it.
  - No other program than Blender opened the files, and no test runs Blender.

### Model workbench 3D view, 2026-11-19

- Task D5: the `model-view` view of [workbenches/model](workbenches/model/README.md) opens in
  the second center pane. It draws the mesh of the selected node with wgpu, with its own depth
  texture and 4x MSAA, a light, a grid, part colors and a wireframe. The agent decisions are in
  [the workbenches plan](docs/workbenches-plan.md), task D5.
- Tool changes:
  - The first read of a batch output by a view starts a batch run (15.8 rule 3). Before this,
    the view saw no value until the next **Run**.
  - The view panel sends only the newest viewport place with each ask. Before this, a slow view
    drew at an old place, and the check saw 44 % of the viewport drawn.
  - The 3D view stayed frozen on the old picture after the open of another project. A surface
    on the editor window stays for seconds after the node process ends. Now each node process
    makes its surface on its own child window, and the editor waits 1 s for the drop of the views.
  - The thread of the child window takes the DPI awareness of the editor window. Before this,
    the picture moved up and to the left on a display at 175 %.
  - Each output of a batch node sends its value, also with no wire, so a view reads it
    (15.8 rule 3). A lone model node now shows in the 3D view. `Graph::outputs` gives the
    output keys of a node with a live value.
- Checked on 2026-11-19, on Windows only:
  - The CI clippy and `cargo test --workspace`. The CLI test checks the first-read rerun.
  - 13 unit tests of the workbench crates and their clippy, with the MSVC toolchain. CI does
    not run them.
  - The 203 frontend tests.
  - The hidden shot check: it opens the view in the second pane, selects `union`, captures the
    view, sets seed 8, sees a new mesh, and undo gives the first mesh again.
  - The hidden shot of `city` opens the `gun` workbench while the 3D view shows. The gun view
    draws its own images, and seed 8 gives a new image. The shots of `model`, `gun`, `hello`,
    `gpu` and `computer` also pass with the child window.
- Known limits:
  - During a batch run, the view keeps its last picture.
  - Linux and macOS did not run the view.

### Model workbench run model, 2026-11-19

- Task D4: the model nodes run in the batch domain by the domain rule of spec 9.1. A model
  builds at **Run** and after each change, not at each frame. The agent decisions are in
  [the workbenches plan](docs/workbenches-plan.md), task D4.
- Checked on 2026-11-19, on Windows only:
  - The CLI test adds a `delay` in the frame domain to the tower and runs 30 frames. The tower
    builds one time. 120 frames took 2.2 s, so the frames ran.
  - `measure_builds` in the `geometry` crate, a release build, 3 rounds: the tower took 0.6 to
    0.8 ms. A block of 500 houses with 3 windows each took 122 to 124 ms. Its mesh of 36000
    triangles took 5.2 to 5.6 ms with the sort. A game ran on the computer at the same time.
- Known limits:
  - In the editor, a batch run stops the frames, the node GUIs and the views until it ends. The
    view keeps its last picture. A model of 500 houses stops them for about 0.13 s.
  - The cluster libraries are not `Send`, so the batch domain cannot move to another thread
    without new `unsafe` code. Views that go on during a long batch run are a tool task before
    task E8.

### Model workbench types and seeds, 2026-11-19

- Tasks D1 and D2: [workbenches/model](workbenches/model/README.md) has the `geometry` type
  crate with `Solid`, `Mesh`, `Sockets` and `Seed`, and the nodes `cuboid` and `report`. The
  crate compiles the vendored Manifold and Clipper2 sources with `cc`.
- The **Inspector** shows a solid or a mesh as a small isometric picture, from a software raster.
- `Mesh::parts` gives a dense part index in the order of the sorted triangles, because the kernel
  IDs come from a global counter.
- Checked on 2026-11-19, on Windows only:
  - The CLI test runs the workbench and compares the mesh hash of the box.
  - 7 unit tests of the type crate passed with the MSVC toolchain. CI does not run them.
  - The MSVC build and a toolchain run (Zig, gnu target) gave the same mesh hashes: the box and a
    seeded model of 5 boxes. A cold build took 5.3 s with MSVC and 8.8 s with the own toolchain.
- Kernel exceptions (2026-11-19): a C++ exception of the kernel does not stop the program. A
  solid gets the error "the result is too large" for a `bad_alloc`, or else "the construction is
  not valid". The booleans keep the error. The copies of the crate in the city and the gun
  workbenches have the same guard, with new import records. The Rust calls delete their
  temporary kernel objects also after an exception.
- A sketch that the kernel cannot make is empty and keeps the error. A move and a copy keep it,
  and an extrude, a revolve or a loft of the sketch gives the same error. The circle, rectangle
  and polygon nodes show the error.
- Checked on 2026-11-19, on Windows only: a unit test catches a `bad_alloc` and keeps its error
  through a union. A unit test makes a sketch with a coordinate out of the Clipper2 range. It
  checks the error of the sketch, its move, its copy, and its extrude, revolve and loft.
- Measured on 2026-11-19, on Windows, in a debug build of the type crate: a loop of 2 million
  sketches with a coordinate out of the Clipper2 range, with their extrude, loft and a flat
  projection that fails. The private memory of the process stayed at 0.8 MB, so an exception
  leaves no memory. A test leak of 100 bytes in each loop step gave 55 MB after 500000 steps.
  The loop was a temporary test, and it is not in the repository.
- Known limits:
  - The memory check ran only on the model workbench crate. The copies in the city and the gun
    workbenches have the same source files (a diff on 2026-11-19). The `bad_alloc` path was not
    in the loop.
  - An allocation failure in the Rust code stops the program.
  - The `parallel` feature of `cc` adds `jobserver` and `libc` to the crate downloads.
  - Linux and macOS did not run.

### Groups, notes and the error display, 2026-11-18

- A group is a colored transparent frame with a title bar. A drag on the title bar moves the
  items in it. A note shows Markdown: headings, lists, tables, code, quotes and rules. Both are
  in the graph file as `[[group]]` and `[[note]]` (spec 5 and 15.12).
- The status bar shows **Errors: N** with a red dot, and **Ready** only with no error. The node
  and the error well show `line N: message`. The **Logs** show `Error in node 'name': line N:
  message`, with a time on each line (spec 15.5).
- Checked on 2026-11-18, on Windows only: the CI clippy and `cargo test --workspace` (256 Rust tests),
  the frontend check and tests (195), the selfcheck (325 steps, with the group and note
  step) and the shots of `computer`, `hello` and `gpu`. No person used the new GUI.
- Known limits:
  - The **Logs** have no "Compiling node" line for each node. The engine has no build event for
    each node (spec 15.5).
  - The error well of the **Code** tab was at the bottom of the panel (fixed 2026-11-19). With an
    error, the code box now has the height of the code, so the well is directly under it
    (mockup 3). The selfcheck shot `code-error` shows it.
  - The group check of the selfcheck waits 400 ms before its undo, so that the move edit arrives
    first (`shortcut:` in `check-groups.ts`).
  - Spec 5 says that the nodes of a graph file sort by ID and the wires by their `to` end
    (fixed 2026-11-19). **Save** now sorts the nodes, the wires and the order, collar and cable
    tables. The groups and the notes keep their order. The open graph keeps the order of the
    edits, which is the draw order, until it opens again. A unit test checks the sort.
  - A link in a note does not open. An image in a note shows its alt text only.

### Ember sound workbench, 2026-11-18

- The library has 34 node types and three patches: **PHRASE**, **ATMOSPHERE**, and **IMPACT**.
- Seven instrument panels and a general control knob use sizes that fit their controls.
- Sources, envelopes, modulation, mixing, filters, dynamics, distortion, chorus, reverb, and DC removal work together in the patches.
- Windows checks passed: workspace clippy and tests, frontend build and tests, and the example tests.
- The hidden editor check operated the waveform buttons, detune knob, sequencer transport, and all three scene buttons.
- It checked 62 cable routes across the three graphs, the Inspector, full panels, contrast, focus, and the small window layout.
- Edit and Bake produced matching phrase samples. All three scenes produced finite audio in both modes.
- The warm core DSP averaged 0.015 ms per block, with 0.016 ms p95. The 15-processor path averaged 0.030 ms, with 0.032 ms p95.
- Both DSP checks counted zero allocations after warmup. These timings exclude the graph scheduler, editor, and device.
- The CLI renders saved patches to stereo, 32-bit float WAV files at 48 kHz, without opening a sound device.
- Export checks cover exact sample counts, frame timing, Edit/Bake equality, graph selection, invalid audio, and existing file protection.
- Three 16-second renders contain finite audio. Speaker playback, listening quality, external sound-editor import, Linux, and macOS were not checked.
- Hardware MIDI and live recording remain open. The **song** graph now provides sample import, stereo processing, polyphonic notes, and arrangement.
- The [workbench notes](docs/sound-workbench.md) and [Ember guide](workbenches/synth/README.md) describe the modules, controls, and learning exercises.
- The **perform** view adds scene selection, phrase transport, sound controls, a filter XY pad, exact values, reset, and five learning experiments.
- The view uses the same parameters as the main patch. The shared rotary widget now works in both nodes and views.
- Windows workspace, frontend, and example checks passed. The view integration test covers Edit, Bake, saving, reopening, and grouped XY undo.
- A hidden GUI check operated the view's scenes, transport, XY pad, exact values, reset, and all five pages.
- Known limits: the view needs the original main patch's node IDs. Its sound controls shape **PHRASE**. Short docks need vertical scrolling.
- The shared view input path now groups a continuous canvas drag in one undo step.
- Nine dock tools add sound presets, lessons, modulation routes, pinned controls, output meters, continuous analysis, patterns, a chromatic keyboard, and WAV export.
- Preset preview uses a separate voice. Preset loading protects changed settings and uses graph undo.
- Analysis captures up to 8192 continuous samples. The existing scope node retains its short-block display path.
- Keyboard input is monophonic, with velocity and release protection. The phrase grid stays in C minor.
- Export reads the saved main patch through the CLI. It supports progress, cancellation, and existing-file protection.
- Presets require the original main patch's parameter nodes. Favorites, recent entries, and pinned controls last for one tab instance.
- Device selection, device connection status, modulation history, and spectrograms remain open.
- See the [dock implementation record](docs/synth-dock-design.md) for checks and remaining limits.
- On 2026-11-20, the hidden synth shot passed all 111 steps on Windows. The fixes were in the chorus and echo ring reads, export cancel, the Strip Inspector names, the **impact** layout and the spectrogram size.

### Ember song arrangement, 2026-11-19

- The second top graph, **song**, contains an original 32-bar study at 136 BPM with six synchronized tracks.
- The **arrange** dock edits hybrid note and WAV clips, a chromatic piano roll, track mixing, and automation.
- Clips support movement, trimming, duplication, splitting, source offsets, playback rate, and audio fades.
- Track creation adds a player and stereo routing. Track removal preserves the other mixer source.
- The score supports 32 tracks, 64 clips per track, and 32 voices per track. Eight tracks appear on each timeline page.
- A frame node validates the score and loads WAV files. One audio clock schedules every track at sample boundaries.
- WAV import supports mono and stereo PCM and float audio from 8 kHz to 192 kHz. Imported assets stay in the project.
- The player includes synth voices, drums, a pitched sampler, stereo delay, filtering, drive, and smoothed track gain and balance.
- Stereo adapters connect the song to the existing mono effects. The output protects peaks and provides separate channel captures.
- Windows workspace Clippy, workspace tests, frontend checks, and synth example tests passed.
- The graph test mixed stereo WAV audio and overlapping notes. Edit and Bake matched within 0.00001.
- Hidden GUI checks passed clip edits, undo, track creation, piano input, WAV import, saving, and graph switching.
- The six-track timeline fits a 450 px bottom dock. A 60-second stereo Bake render contains finite audio and stays below clipping.
- The warm six-track debug test measured 0.73 ms mean and 0.83 ms p95 per 256-frame block, with zero audio allocations.
- These timings exclude the editor, device, and graph scheduler. They do not prove the maximum track count on other systems.
- Microphone recording, hardware MIDI, time stretching, compressed audio import, VST hosting, and stem export remain open.
- The ruler uses 4/4. Separate side views for track and clip details remain open. Speaker listening, Linux, and macOS were not checked.
- The [arrangement notes](docs/synth-arranger.md) contain research, limits, and check details.

### Performance audit, 2026-11-18

- The [performance audit](docs/performance-audit.md) records 12 findings and the proposed implementation order.
- The Windows CLI ran the committed computer example in edit and bake modes, with identical output.
  The incremental frame estimates were 91.9 us and 17.9 us, including stdout. These are not editor timings.
- Done on 2026-11-18:
  - Finding 4: the canvas keeps the wire ends, the sources and the wired pins of each graph in a
    cache, so a live update does not find them again.
  - Finding 3: each tick measures its stages. The run text shows them as a tooltip, and `--shot`
    writes them into `check.json`.
  - Findings 2 and 6: a draw sends only the node GUI lines that changed. The page keeps its GUI
    map and its value maps when no line or value changed.
  - Finding 9: a GPU image keeps up to 4 spare read buffers and shrink objects again.
  - Finding 8: a quiet frame keeps no copy for a view. A large value costs one copy for each tick.
  - Finding 7: a new lane state, hover lane, color or dash writes only the flags of its wire, and
    only that range goes to the GPU. A new number of lanes or points still makes all instances.
    In Node, 2000 wires of 8 lanes (208000 instances): the full rebuild took 1.14 ms and a new
    1.6 MB array, and the patch of 3 wires took 0.004 ms with no new instance array.
- The computer example in the shot, with a game on the owner's computer: a tick of 54 events took
  2.08 ms for the frames, 0.38 ms for the values, 1.99 ms for the send and 0.01 ms for the node GUIs.
  The page frame was 16.7 ms (p95 16.8 ms), so no frame dropped. No number before the change exists.
- Known limits: the copies for a view stay after the view closes, until the graph loads again. A
  reopened view on a paused graph thus still shows the last value. Each drawn node GUI still makes
  its line text one time.
- No change for findings 1, 5, 10, 11 and 12 (agent decision of 2026-11-18, the owner can change it):
  - Finding 1: a filter by view can lose the values that a paused pan, the **Inspector** or a badge
    reads, and the numbers above show no dropped frame.
  - Finding 5: a quiet frame already sends no times, so only the last frame of a tick makes them.
    The gain is a part of the 2.08 ms of the frames.
  - Findings 10, 11 and 12 need a measurement or a design change first.
- Not checked in this audit: slot mode, allocation counts, Linux, and macOS.

### Top graphs, 2026-11-18

- A project can have more top graphs than `main`, for example an empty test graph next to a
  finished build. **New graph** in the **Graphs** group of the **Project** panel makes one.
- A top graph opens and runs in place of the last top graph. Save writes `top = true` in its file.
  A subgraph use or a clip cannot name a top graph.
- `patina-cli run` and `patina-cli package` take `--graph <name>`. A package records its top graph.
- Checked on 2026-11-18, on Windows only: `cargo test --workspace` (an engine test opens and runs
  a top graph and refuses a use of it, a clip test refuses a top graph, CLI parse tests of
  `--graph`), CI clippy, `npm test` (166 pass) and `npm run check`. The selfcheck (317 steps, on a
  hidden desktop) clicks **New graph**, sees the empty top graph run, opens `main` again and
  deletes the new graph. On a copy of `workbenches/hello` with an empty `bench` top graph,
  `patina-cli run --graph bench` ran, `--graph average` (a subgraph) and a missing name failed
  with a clear message, and `package --graph bench` wrote `graph = "bench"`. The package did not
  run.
- Known limits: a project always opens with `main` as the top graph that runs. A restore from the
  recovery file runs the restored open top graph, or `main` when a subgraph was open. The editor
  cannot rename a graph, or change a top graph into a subgraph.

### Copy, paste and libraries

- Known limits: the index has no libraries yet. A clip has 256 subgraphs at most.
- The **Library** tab (2026-11-18) lists the library nodes and subgraphs, with a search and a
  preview. The **Brings** list shows each subgraph and folder that a pick adds, and its state.
  A drag or **Add to graph** adds the item. An imported type now brings its connection kind.
- Checked on 2026-11-18: engine tests for the plan and the connection kind. The self-check
  shows the preview, drops the item, clicks **Add to graph** and undoes each add.
- Known limits: the tab reads the libraries at each open of the tab. A type that an import
  renames keeps the connection kind check of its old name.

### Gate speed

- Known limits: a gate net does not run in parallel. A light node that does much work runs in
  parallel only with `#[node(heavy)]`. The fast copy of each net makes the generated Rust and
  the bake build of the cluster about 11% larger and longer.

### Gate cycles, 2026-11-18

- A cycle of light gates with no state runs with no delay. Each gate runs one time in each frame,
  as in Digital Logic Sim 2, so a latch settles over some frames (spec 9.2). The `latch` graph of `workbenches/computer` is a D latch of two NOR
  gates. The CLI test runs it for 16 frames and checks that it keeps its value. It also checks
  that a NAND gate that reads its own output changes at each frame, with no **WARN** line.
- A node with state, for example `ram` or the new `rom`, can be part of a gate cycle. A light
  node can have a text parameter. The new `d-flip-flop` subgraph is two D latches. A CLI test runs a
  4-bit counter of these flip-flops with no delay node, a ROM and a RAM that read their own output,
  and the new `pulse` and `bus` nodes. See [docs/dls2-parity.md](docs/dls2-parity.md).
- The new `led`, `dot-display` and `rgb-display` nodes of `workbenches/computer` are the LED and the
  16 x 16 displays of Digital Logic Sim 2, with the same pins. The CLI test writes a pixel at a
  clock tick on each display. A hidden `--shot` drew the three nodes.
- Known limits: a rebuild of the cluster, for example after a graph edit, sets the values of a
  cycle back to the default, so a latch forgets its value. No check ran the latch in the editor.
- Known limits for the parts of Digital Logic Sim 2: no wire with many drivers (use `bus`), no
  Key chip, and no random order in a race. The `bus` node has no custom GUI, because
  its 16 inputs are the most that a node function takes (spec 4.1). Its contacts show the lanes.
- The `pulse` node has a custom GUI, as `clock`: an LED, a scope, the length and the **-** and
  **+** buttons (2026-11-19). A hidden `--shot` of a test graph drew `clock`, `pulse` and `bus`
  in the editor. A hidden `--shot` drew `rom` with ten words: the word map, the address and
  the word. No person clicked the buttons.
- The new `buzzer` node of `workbenches/computer` is the buzzer of Digital Logic Sim 2: a square
  wave in the audio domain (2026-11-19). **pitch** is a MIDI note number, and the low 4 bits of
  **volume** set the loudness. The CLI test renders a byte of the frame domain as the pitch.
  No person heard it on an audio device.

### Touchpad pan

- Known limits: a mouse with smooth scroll steps can give touchpad values, so its wheel pans. A
  touchpad driver that sends mouse wheel steps zooms. Linux and macOS are not checked.

### No save without Save

- Known limits: a new folder, **Import**, **Delete**, and an undo or a redo of a folder change
  act on the disk at once (spec 15.10).
- Checked on 2026-11-17: the engine tests and the self-check undo code edits with no file write.
- Checked on 2026-11-18: the engine and editor tests save the frame rate and keep it in the
  recovery file. The cargo test runs **Add crate** on unsaved text and leaves the file as it was.

### Snake on the computer

- Known limit: a step of the game is 26 to 38 instructions, and the clock gives at most one
  instruction each frame. At 60 frames each second, the snake moves about 2 cells each second.

### Bigger CPU of the computer

- Known limits: node state does not save, so the RAM is 0 after an open. The program memory and
  the RAM are apart, so a program cannot change its code.

### Speed, frame rate and random numbers of the computer

- Known limits: at a high rate, the frames of a tick stop after one tick period, so the CPU runs
  slower than the rate. Frames never queue.

### IO devices of the computer

- Known limits: the `cpu` has one **io** input, so each device that gives values to LD passes the
  values of the device before it (see the `random` node). Node state
  does not save, so the terminal, the frame and the key buffer are empty after an open. A
  hidden `--shot` run types a text into the keyboard and sees it in the key buffer (2026-11-18).
  The shot then picks `echo.asm` in **Program** and sees the keys on the terminal (2026-11-18).

### Start screen

- The start screen has two large buttons, **New project** and **Open project** (2026-11-18).
  **Open project** opens the folder dialog, and the selected folder opens at once. **New
  project** opens a dialog with **Name**, **Location** and a check of the name while the user
  types. The recent list has the columns **Name** and **Path**, a **Filter** field and the
  arrow keys. Ctrl+N and Ctrl+O click the two buttons.
- Checked on 2026-11-18, on Windows only: the frontend type check, 198 frontend unit tests with the new name test, the frontend build, the CI clippy command and the workspace tests passed. The selfcheck passed 326 steps, with the New project dialog, Ctrl+N, the name check, Create, the recent filter and Open project with a stubbed folder dialog.
- Known limits: no check opened the real folder dialog, because it waits for a person. Since
  2026-11-19, the selfcheck sends Ctrl+O and the arrow keys. The recent list has no time of the
  last open, because the list file has no times (13.3).
- The **x** button of a recent project removes it from the list or moves its folder to the Recycle
  Bin (2026-11-17). The self-check clicks **Close**, the **x** of the project and **Remove from
  list**, and runs the refusal of **Delete project** through the API. No check moved a project to
  the Recycle Bin.
- A new project has no node folders (2026-11-17). The engine test and the self-check open an empty
  new project, and the self-check then writes the hello nodes into it.
- The self-check uses the start screen as a person does (2026-11-18). It clicks **New project**, fills
  the dialog and clicks **Create**. It removes the trust of the project in **Settings**, clicks
  **Open project** with a folder in place of the folder dialog, and clicks **Trust project** in
  the banner. It also
  clicks **Pause** in the top bar, **Rename pin...** in the menu of a public pin and **Help** >
  **About Patina**. It clicks **Add library** in **Settings** and **Import** in the Project
  panel, and gives a folder in place of the folder dialog. The node menu clicks are **Cut**,
  **Fit node**, **Open subgraph** and **Expand subgraph**. The timeline clicks are the End key
  on the playhead and **Rewind**. The Logs panel click is **Clear**. The top bar clicks are
  **Save**, **Fit**, **Run**, **Bake**, **Undo** and **Redo**, and it types a rate in the **FPS**
  field. It clicks **Arrange** and **Add node...** in the canvas menu. The node menu of a
  selection clicks are **Fit selection**, **Delete selection** and **Collapse into subgraph...**.
  It clicks **Back** of the full size GUI, **Move down** of a public pin, and **M** and **S** of a
  timeline lane. It sets the **Corners** and **Default route** fields, the pivot and **Aspect
  lock** of a transform, a slider, a dropdown and the **Name** of a bus wire. It closes a panel
  with its **Close** button and with **Close** in its tab menu, and shows it again from the
  **Views** menu. It clicks a bar in the **Debug** tab, the **1 compile error** link of the
  status bar, the error line in the **Logs** tab of the **Inspector**, and the middle crumb of
  the path bar in a nested subgraph. On an automation lane, a double click adds a point and the Delete key removes it.
  It clicks **Expand subgraphs** on two uses of one subgraph, and moves a view with **Move to
  the right area** and **Move to the center** in its tab menu (2026-11-18).
  The keys are Ctrl+Z, Ctrl+Y, Ctrl+A, Ctrl+C, Ctrl+V, Ctrl+G, Ctrl+Shift+G, Shift+L and F.
- The self-check saves and clicks **Package** with a folder in place of the folder dialog. It
  waits for the log line of the package (2026-11-18). The local check copies `patina-cli` next
  to the editor, so the full package build runs. Then it clicks **Close** and opens the package
  with **Open project**, and sees the original project with the trust banner (2026-11-18).
- The self-check clicks **Float in a window** in the tab menu of a second instance. The float
  window stays hidden, and the check docks the view again (2026-11-18).
- Known limits: the self-check does not click **Open GitHub page**, because it opens the web
  browser.
- The self-check opens the **Import** dialog and clicks **Cancel**. Then it imports `nodes/number`
  of the library with **Rename**, clicks the copy in the **Imports** list, and sees that
  **Update** is off. Then it deletes the copy from outside the editor (2026-11-18). A second step
  changes `src/lib.rs` in a copy and in the library. It picks **Other project** in the **Update**
  dialog, and the copy gets that file (2026-11-18).
- Known limits: **Delete project** shows an error on Linux and macOS. It has no trash there yet.
- The first start shows an **Install the toolchain** card, and the install shows a progress bar
  (2026-11-18). A unit test checks the percent parse and the step math. No check ran a full
  install with the bar, because each download needs the approval of the owner.
- A right-click **Delete** on the **Project** panel items moves a folder to the Recycle Bin
  (2026-11-18). A unit test checks the folder names that can go. The self-check clicks **Delete**
  on a node folder and **Cancel** in the question. No check moved a folder to the Recycle Bin.
- **Delete** on the open graph or on the top graph that runs first opens `main` or the top graph
  that runs (2026-11-19). The self-check (326 steps, on a hidden desktop) deletes a new top graph
  while it is open and runs, and sees `main` open and run again. An engine test checks that the
  graph that runs goes only after `main` opens. No check deleted a saved graph file.
- The **Logs** panel shows the full compile errors, with highlight and **Fix:** lines from a list
  of common errors (2026-11-18). The **Output** panel has the same highlight. A unit test checks
  the highlight parts. The self-check reads the highlight and the **Fix:** line of a type error.
  The **Debug** panel is a timeline with no text, so it has no highlight.
- A lost parameter does not block a load, a paste or a run (2026-11-18). The node shows it with
  an **x** that deletes it. An engine test checks that it is no problem. The self-check clicks the
  **x** on a free knob and undoes the delete.
- The **Code** tab highlights TOML, and the **About** dialog has the GitHub page (2026-11-18). The
  self-check sees more than two token colors in a `Cargo.toml`.
- Known limits: **Delete** in the **Project** panel shows an error on Linux and macOS.

### Canvas editing

- **Delete node** in the right-click menu asks first (2026-11-18). The Delete key does not ask.
  The self-check clicks the menu entry and the **Delete** answer.
- The doc comment of a node is its description (2026-11-18). The node header, the palette and
  the **Project** panel show it as a tooltip, and the **Inspector** shows it as text. The engine
  test reads it. The self-check reads the tooltips of a free node caption and of its **Project**
  item and of a palette entry. The text kept each line break of the comment, so the
  **Inspector** broke the lines in the wrong places (fixed 2026-11-18). The engine now joins
  the lines of a paragraph as Markdown does. The **Inspector** showed the `**` marks of bold
  words (fixed 2026-11-18). It now shows bold words and code spans, and a tooltip drops the
  marks. A frontend test reads the parts, and the self-check reads a tooltip without the marks.
- The **Code** tab has a **Save** button (2026-11-18). The self-check fixes a slot error with it,
  and checks that it is off when no file has changes.
- When the rustup toolchain has no rust-analyzer, the editor adds it with rustup (2026-11-18).
  No check ran this install.
- `#[node(free_layout)]` makes the GUI the whole node, with its own shape and pin places
  (2026-11-18). `Ui::pin`, `Ui::place` and `Canvas::path` are new. The SDK, engine and frontend
  unit tests read them. A self-check step adds a free knob, checks its pin places and drags
  it on its canvas. The example is `workbenches/computer/nodes/button`.
- The parameter writes of one node GUI input are one undo step (2026-11-18). A canvas drag from
  the down to the up is one input. The writes go out in order. An engine test and the free knob
  step of the self-check read it.
- Known limits: the self-check drop is a page event, not a drag of the operating system. A wire
  leaves a free pin only to the left or the right.
- A bus wire meets a bar on each pin, also on circuit symbols (2026-11-18). A bus pin is a bar
  of n x 5 + 4 px. A box socket has one contact for each lane, and a symbol draws each bus lead
  as parallel lanes. A wire drag from a bus pin is a ribbon, and a lost bus pin is a red bar.
  Frontend unit tests and a shot check of the self-check cover it.
- `#[node(symbol = "<shape>")]` shows a gate or block symbol in place of the box (2026-11-18).
  A subgraph sets **Shape** and **Label** in the **Look** section, with undo. The engine, CLI
  and frontend tests read it. The self-check gives a subgraph the chip shape and undoes it. It
  also adds a node with the AND symbol.
- A subgraph can have a **Custom outline** and a body **Color** (2026-11-19). The look editor in
  the **Inspector** moves, adds and removes corners, with undo. **Edit look** on a use opens the
  subgraph. Engine and frontend unit tests cover it. The self-check draws an outline with a
  color and reads it on the use. Known limit: no hand test of the mouse drags of the editor.
- An open subgraph that no graph uses runs alone as a test (2026-11-19), so its switches and
  LEDs work. The self-check sets a number in such a subgraph and reads its value.
- A node GUI in the open subgraph writes its parameters (2026-11-19): through a use, in the
  **Test** mode and when the subgraph runs alone. The self-check clicks a switch that drives a
  NAND gate and an LED in each of the three modes. Without the fix, the same check fails.
- A subgraph can have the **Code** shape (2026-11-19). A face node with Rust code draws each
  use, and the **Look code** section of the **Inspector** edits its code. Engine and frontend
  unit tests cover it. The self-check writes face code with an input from the NAND gate, saves
  it and reads the face of the use on the main graph after a click on the switch. Known limit:
  the face GUI only draws, so a click on a use does not go to the face node.
- A node canvas can grab only some rectangles (`grab_rect`, 2026-11-19). The button and the
  joystick of computer grab their caps, and a drag on the board moves the node. The self-check
  drags a free knob below its grab area and the node moves.
- Each example has a style crate (2026-11-18): `widgets/parts` (a circuit board) in computer,
  `widgets/calc` (a pocket calculator) in hello and `widgets/scope` (a lab instrument) in gpu.
  A placed image or field fills the width to the same right margin. A free node shows no canvas
  label row, so its pins and placed widgets line up with the drawing. The tests of each crate pass.
- The **Inspector** put a placed widget at the wrong place, for example the **Type** field of
  the keyboard on the canvas title (fixed 2026-11-18). A GUI with placed widgets now has no
  padding and no canvas label there too. The shot checks that each placed widget is on its canvas.
- A node covered the full badge of a selected wire, for example `64 x 16` in gpu (fixed
  2026-11-18). The badge and the label of a selected wire are now above the nodes. The shot
  checks that no node covers the badge.
- The shot checks the layout of each graph that it opens: no widget is out of its node or the
  **Inspector**, no label cuts its text, no two nodes overlap, no wire goes through a node body
  and no wire has a route conflict mark (added 2026-11-18). It also selects each node of the
  main graph, and the first node of each new kind in a subgraph. Its **Inspector** must not
  scroll to the side, cut a label, show an error or show a GUI that cannot draw (added
  2026-11-18).
- Since 2026-11-19, the shot also opens each top graph of the project and does the same checks.
  A wire with a manual route (curved, orthogonal or polyline) keeps its path, so the shot does
  not check it for node bodies. A graph with a saved pan and zoom does not get the fit test.
- The order pins of a free node covered the first letters of its name, for example
  **Gradient** in gpu (fixed 2026-11-18). The name now starts after the left pin. The shot checks
  that no pin covers the title of a node.
- The shot checks that no button, tab, title or label of the panels and the bars cuts its text
  or is out of the window. It does this at 1440 x 900 in the main graph and in each subgraph.
  It does it again at 1280 x 720 with the node fit check, and captures `main-small.png` (added
  2026-11-18).
- A node with a free layout and a full size, for example **ram** in computer, had no expand
  button on the canvas (fixed 2026-11-18). The button is now after its name. The shot opens the
  full size of each node in the main graph, and of each new kind in a subgraph. A full size must
  not scroll to the side, cut a label or show a GUI that cannot draw.
- The shot checks that each button, field and tab of the page, of each **Inspector** and of each
  full size has an accessible name for a screen reader (added 2026-11-18). All three examples
  pass.
- The shot opens the menus of the pane, a node, a wire, **Views**, a view tab and the
  **Project** panel (added 2026-11-18). Each menu must be in the window, must not cut a text and
  must name each button. It does this in the main graph, at 1280 x 720 and in each subgraph.
- The shot checks the contrast of each text of the page, of each menu and of each view: 4.5:1,
  or 3:1 for a large text (added 2026-11-18). It found the **None** item of the **Project**
  panel at 3.4:1 (fixed 2026-11-18). The **None** item, the **No match** text of the palette and
  the line numbers of the logs now use the subtle gray. A frontend test checks the ratio math.
- The shot checks that each button, field, tab and node of the page shows a mark at the
  keyboard focus (added 2026-11-18). A hidden window never gets the focus, so the check reads the
  focus rules of the style sheets, not the pixels. It found that a node showed no mark, because
  React Flow removes its outline (fixed 2026-11-18). The node box now shows the accent outline.
  A frontend test reads the selector parts.
- The shot checks that each text of a node GUI drawing or a symbol stays in its drawing and does
  not cover another text (added 2026-11-18). It reads more than 1000 drawn texts in the three
  examples on the canvas, in the **Inspector** and in the full size, and found no problem. A
  frontend test checks the box rule.
- The self-check records each button, menu item and tab that shows, and each click (added
  2026-11-18). Its last step lists the controls with no click. The node GUIs and the code
  editor are out. The list found **New subgraph**, **New view**, **Add input**, **Remove** of a
  public pin and the problem link of the status bar. The self-check now clicks them. Known
  limits: the list names each control by its label, so it also lists the items of the project
  and the log lines. **Browse**, **Add from Git** and **Install toolchain** need a dialog or the
  network, so the self-check does not click them.
- The **Inspector** showed the thumbnail of a wire as wide as the panel, larger than the widgets
  of the viewer (fixed 2026-11-18). The thumbnail is now at most 128 px high.
- Known limits: the style crates are copies of one pattern, not a shared library. A symbol
  node has no GUI widgets. Only the hidden shots checked the look of the examples.
- An unwired input whose type has no parameter support gives a graph problem on its node, not
  a compile error (2026-11-18). The engine test checks it. A hidden shot showed three problems.
  The self-check adds a node with such an input and reads the problem on the node.
- An unwired generic input gets the same check with the type that its wires give (2026-11-18).
  The engine test checks it.
- Known limits: the check does not find an `impl Param` that a macro makes.
- An input of a node function can have `#[param(default = 2.5, min = 0, max = 10)]`
  (2026-11-19). The engine sends the default when the parameter has no text. A number out of
  the range gives a parameter error on the node. The **Inspector** shows a slider next to the
  field of an input with `min` and `max`. Engine unit tests and a CLI test cover it.
- An input of `process` of a stateful node can have `#[param]` too (2026-11-19). A bare
  `#[node]` on the `impl` block removes it. An engine unit test and the CLI test cover it.
- The model shot moves the **taper** slider of an `extrude` node in the **Inspector** to 0.5
  (2026-11-19). The field shows 0.5, the graph runs, the 3D view draws a new image, and undo
  clears the field. A hidden shot on Windows showed the tapered solid.
- Known limits: no person dragged the slider by hand. The check sets the value as a drag does.
- The model nodes have `#[param]` defaults and ranges (2026-11-19). A new `cuboid` is a box of
  1 x 1 x 1, `taper` has a slider from 0 to 1 and `gap` has a slider from 0 to 360. The city
  has the new copies of `cuboid`, `cylinder` and `sphere`. The model and city CLI tests pass.
- The hello example has a pocket calculator (2026-11-18): two **keypad** nodes, an
  **operation** with four keys, two paper **tape** nodes and an `average` subgraph with the
  **AVG** chip. The unit tests of the nodes and the CLI tests pass. A hidden shot showed 36
  and 7.5 on the tapes.
- Known limits: a click on a key gives a new run, so the tape prints each edit, also an edit
  that the user undoes. Nobody clicked the keys by hand.
- The numbers 7.5 and 1.5 on a **tape** ended 4 px left of the integer rows (fixed 2026-11-18).
  A point and a minus sign now count narrower than a digit. A hidden shot showed one right edge.
- A hidden `--shot` run taps the node GUIs of the examples (2026-11-18): **C** and **7** on a
  keypad and the **+** key of the operation in hello, a drag of the knob and of the fader and a
  tap on **INV** in gpu, and a switch and a typed text in the keyboard in computer. It waits for
  the new canvas labels. A node out of the view gets **Fit node** first. In computer, it opens
  the full size of **Program** and waits for the line numbers and the band on the current line.
- The gpu example has a second row of lab nodes (2026-11-18): a **knob**, GPU color **bars**
  with a WGSL `override` level, a GPU **mix** with a fader, a second monitor and a **waveform**
  monitor. **gradient** has an **INV** switch, and **brightness** shows a histogram. The unit
  tests of the nodes and the gpu CLI test pass. A hidden shot showed the bars, the mix and the
  waveform.
- The bars, mix and gradient nodes keep their pipeline while the WGSL and the `override`
  constant stay the same (2026-11-18). A bars run took 0.6 ms with the same level and 15.8 ms
  with a new level (release, 20 runs each, with a game open on the computer).
- Known limits: a new level or a new fader place makes a new pipeline, because the GPU fixes an
  `override` constant in the pipeline. Nobody dragged the knob or the fader by hand.

### Canvas speed

- A pan of 2000 nodes takes about 70 ms for each frame, and a drag about 155 ms (see Speed
  measurements). A game ran in the background, so the times without load are lower.
- The bench ran only on Windows 11, with the debug editor and the built page. It did not run on
  Linux or macOS, or in a release build.

### Edit speed

- An edit of only the layout keeps the domains and the problems of the last check. A change to a
  code or graph file from outside the editor shows in about 2 s, or at the window focus (fixed
  2026-11-17). A graph that is open in memory keeps its version.
- An edit or a save before that poll also takes the outside change (fixed 2026-11-19). Before,
  its view kept the new file times, so a node folder that was deleted outside the editor stayed
  in the Code tab and got no build. A unit test deletes a node folder of a copy of
  `workbenches/hello`. The Code tab file list also reads the folder again after an outside change.
- The parse cache of the node folders keeps at most 4096 folders (fixed 2026-11-17). When it is
  full, it drops all entries, and the next check parses each node folder again.
- The edit times ran only on Windows 11. The page times are from the debug editor.

### Process isolation

- The crash handler is only on Windows. It names the node that runs on the thread that crashed,
  also in a parallel run (fixed 2026-11-17). A unit test checks the parallel rule. The crashes of
  the self-check run on one thread.
  A thread that a node starts is not in a run. A Rust abort skips the handler.
- The shot captures the viewport of the node process in the composed window (fixed 2026-11-18).
  It ran on a hidden desktop. Nobody looked at it on the screen of the owner.
- A closed project ends its node process and rust-analyzer with their child processes, so no
  file of the project stays open (fixed 2026-11-17). The lock of a project folder that the owner
  saw did not reproduce here, so its cause is not proven.
- The editor and the node process both check the graph of each run.
- A crash outside a run shows on the serial of the last run or open.
- Node code that nests deeper than about 1000 levels still overflows the parse stack in a debug
  build. A child process for the parse is the full fix.
- On Linux and macOS, the processes that a node starts do not end with the node process, and the
  memory has no limit.

### Milestone 12: the event domain

- These shortcuts are open in the code:
  - `events/watch.rs` of `patina-engine`: `file_change` polls its folder.
  - `engine.rs` of `patina-editor`: no event runs when the slot keeps a value that is not
    `Sync` and holds an event node and a node of another domain.
- In the editor, a change, **Pause** and a failed frame wait for the active event runs. A code
  swap does not keep the active runs on the old builds (9.4).
- While the event domain runs, a view reads no output of an event node. The GUI of an event
  node draws only when the event and the slot libraries are parallel. A GUI input waits while
  a run holds the node, and applies at the next GUI call after the run. Past 64 KiB of waiting
  input, a new input is lost.
- The first build of a project with an async node needs `tokio` from crates.io or the cargo
  cache.

### Milestone 11: the audio domain

- These shortcuts are open in the code:
  - `engine.rs` of `patina-editor`: when a library is not parallel, the blocks run on the engine
    thread between the frames, so a slow frame or node GUI makes them late.
- While the audio thread of the editor runs, a view reads no output of an audio node. The GUI of
  an audio node draws only its display value (4.3, since 2026-11-20). The badges still show.
- No check played a sound in the editor. The editor uses the same device code as the CLI.
- When an audio node keeps a value that is not `Sync`, the blocks run on the thread of the
  frames. A slow frame then makes the blocks late. The device queue of 43 ms hides a short delay
  only.
- The device must take 48 kHz f32 samples. There is no resampler. On Windows, cpal 0.17 lists
  each rate in f32 for the default device, because it opens the stream with AUTOCONVERTPCM: a check
  on 2026-11-16 found 48 kHz f32 on a Windows 11 computer. A device on Linux or macOS
  without 48 kHz f32 plays no sound.
- Linux needs the ALSA library. The device output was not run on Linux or macOS.
- On Windows, the audio thread has the priority `THREAD_PRIORITY_HIGHEST` (fixed 2026-11-16). A
  unit test checks it. On Linux and macOS, it has the normal priority.

### Milestone 10: views

- The limits of version 1 are in spec 15.8: four instances, typed values only of wired outputs,
  and one viewport on Windows only.
- The **monitor** view of computer showed **No frame yet** while the graph ran (fixed
  2026-11-18). The fused cluster was a bake build and the view an edit build, so the two `Frame`
  types had different type IDs. A view now builds in the mode of the loaded cluster. The
  computer CLI test reads a frame with a bake cluster and a bake view. After a fuse, the view
  builds again, and its panel keeps the last frame until the build ends.
- The `Graph` handle of a view is the open graph (15.8). In a subgraph, the **monitor** view has
  no screen node, so it shows no frame. It now tells the user to open the main graph. A view
  that reads the main graph from a subgraph is not done.
- The graph showed through the transparent viewport of a view in the center (fixed 2026-11-18).
  The graph now hides under a view with a viewport. React Flow sets the visibility of each node,
  so the first fix still showed a node there. The shot opens each view of the example, checks
  that only the parents of the viewport are under 49 points of it, and captures
  `view-<name>.png`. The shot has a limit of 360 s. Computer took 163 s, and one run under load
  took more than 180 s. All three examples pass.
- No viewport showed in the editor (fixed 2026-11-18). The `#root` background of the page was
  opaque and covered the surface since the first editor. The shot now checks that no parent of
  the viewport has a background. It captures the composed window with PrintWindow and checks
  that the view drew 90 % or more of its viewport. The scene of gpu and the monitor of computer
  pass. The resize and the scroll clip need a check on screen.
- A viewport draws in the `ui` call on the engine thread, so a slow run delays its frames.
- On 2026-11-15, the in-process run of `the_gpu_example_draws_a_gradient_with_a_wesl_shader`
  failed one time in a parallel test run. Four more parallel runs passed. A second failure gave
  the cause: **Patina could not read the image from the GPU**. Hypothesis: a poll on another
  thread ran the map callback late. The read now polls until the callback ran. Watch for a
  third failure.
- The view did not run on Linux or macOS. Only CI built it there.
- These shortcuts are open in the code:
  - A viewport surface works only on Windows (`crates/patina-gpu/src/viewport.rs`).
- The first open of a view builds it in a background thread, so the frames go on (fixed
  2026-11-17). The panel shows the view after the build. Nobody measured the frames during a
  view build.

### Milestone 9: the GPU

- Each cluster links its own copy of `wgpu`. This is acceptable because the engine never
  unloads a library.
- A project with GPU nodes needs Rust 1.97.1 or later. All examples use Rust 1.100.0.
- NVRTC comes with the CUDA Toolkit, not with the driver. A package does not carry it yet.
- CI has no NVIDIA driver, so it checks only the error path of `patina-cuda`.
- In `workbenches/gpu`, `cargo test --workspace` does not link the forge crates (`LNK1104`,
  `atls.lib`). The feature `static-dxc` of `patina-gpu` applies to all crates, but only
  `patina-gpu` adds the ATL folder. Test each crate with `-p`.
- These shortcuts are open in the code:
  - A project root that is not UTF-8 goes to the clusters with lossy text
    (`crates/patina-engine/src/library.rs`).
  - The CLI test of `workbenches/gpu` checks only the build when there is no adapter
    (`crates/patina-cli/tests/cli/run.rs`).

### Milestone 8: packaging

- A package is a folder, not one file.
- The notices of the crates of the runner are only in the portable zip. A runner from a
  checkout gives a warning.
- **Open project** on a package opens its original project. A build there writes a `build/`
  folder into the package. The self-check clicks **Package** and opens the package with
  **Open project** (2026-11-18). **Package** needs `patina-cli` next to the editor, as
  in the portable folder.
- A test builds and loads the `patina_capi` library. It checks each function of
  `patina_capi.h` and the error texts (fixed 2026-11-17). A full package run through the C API
  is only in the Rust test of the crate and in the manual `ctypes` check on Windows.
- A package from a checkout copies the library from `target/debug`. When no `cargo test` or
  `cargo build` of the workspace ran after a change of the C API, this library is old. Run
  `cargo build -p patina-capi` before such a package.
- The Python module is not done. It needs the `pyo3` crate (a download) and a Python install.
- The package ran only on Windows.
- A package builds in a background thread, so the graph runs on (fixed 2026-11-17). A change
  that needs a build waits until the package ends, because one build runs at a time.

### Milestone 7: subgraphs and reuse

- The editor shows a use of a subgraph as one node with the public pins, and **Add node** lists
  the subgraphs. The output pins of a use show the live values of their inner outputs, and a run
  error of an inner node shows on the use.
- Checked: the engine test `the_use_paths_go_through_the_uses_inside_uses`, the frontend test of
  the path bar, and a selfcheck step that opens `fire_check` from the **Project** panel and finds
  the domain of its only use.
- A removed public pin keeps the wires of the uses to it. The check shows a problem for each,
  and the use shows a lost pin with the wire, so the user can delete it (fixed 2026-11-17, before
  the wire was not visible). The frontend test checks the lost pins, and the selfcheck removes a
  public output and finds the lost pin and its wire on the use.
- Ctrl+G collapses the selected nodes into a new subgraph, with public pins for the wires that
  cross. Save writes its file while a graph uses it. An undo keeps it for a redo. Ctrl+Shift+G expands
  the selected uses.
- A subgraph that opens with a double-click on a use shows the live values of that use. A
  subgraph that opens from the **Project** panel shows the live values of its only use (fixed
  2026-11-17). With more than one use, it shows none, because no use is selected. The frame of
  **Show inner nodes** shows the values of one use.
- The node GUIs in an open use show the state of that use and take input, on the canvas and in
  the **Inspector** (fixed 2026-11-18, before they used the IDs of the main graph). The reports,
  logs and times of the inner nodes show on the inner nodes. A use has no **Code** tab (before,
  it tried to read `nodes/<subgraph>`). The **Nodes** group of the **Project** panel lists no
  built-in nodes. The selfcheck adds a node with a switch to a use, clicks the switch on the
  canvas and in the **Inspector**, and reads the text of the GUI.
- **Import** and **Update** work only from a trusted project. A renamed folder keeps the comments
  and the key order of its `Cargo.toml` (fixed 2026-11-17, before they were lost). The import
  test checks a comment. A CLI test with the real toolchain imports a node and builds it. It
  puts back an update that does not compile, then builds and runs a good update. The self-check
  runs the two dialogs in the window (2026-11-18). A deleted imported folder kept its line in the
  **Imports** list (fixed 2026-11-18). Its record stays, so a restore shows it again.

### Milestone 6: layers

- The rpath on Linux and macOS ran only in CI. Nobody compared the bytes of two builds yet.
- The state move depends on a hypothesis: rustc gives the same layout to the same type with the
  same compiler and cfg values, also at another optimization level (7.2). A slot test moves a
  state that holds a type of a type folder from an edit build to a bake build. A feature of the
  type folder that another node turns on resets that state. These checks do not prove the
  hypothesis for all types.
- There is one cluster for each clock domain. No cluster boundary is at a large node yet.
- The reader cluster depends on the crates of the producer and of the steps before it, so a
  change of a batch node builds the frame cluster again too.
- The editor selfcheck clicks **Bake** after a click in a node GUI. The fuse keeps the state of
  that node from the slot. Then it wires a delay after the node, and the new build keeps the
  output and the transform of the node GUI (2026-11-17).
- The modes have no debug assertions, so a `debug_assert!` in a node does not run (8.3).
- The fuse builds in a background thread, so the graph runs on during a fuse (fixed
  2026-11-17). The engine loads the fused clusters only when no change, pause or step came after
  the start of the fuse. Nobody measured the frames during a fuse.
- A change during a fuse waited until the fuse ended (fixed 2026-11-17). Now a change, an
  unload or a pause stops the fuse, and the engine kills its cargo run (spec 7.2). The engine
  test `a_stop_ends_cargo_and_its_build_script` checks that the build script ends too on
  Windows. On Linux and macOS, the processes of a killed cargo run on until they end.
- A view build waited for a running fuse (fixed 2026-11-17). Now it stops the fuse, and the fuse
  starts again after 30 s. A **Package** waits for the fuse, because it builds the same bake
  clusters and uses their crates. The unit test of `engine/fuse.rs` checks the restart time.
  Nobody opened a view during a fuse in the editor.

### Milestone 5: stateful nodes, node GUIs and generic nodes

- Branding: nobody made a real `.msi`, so the installer bitmaps and the uninstall icon were not
  seen in Windows. `project-file.ico` has no file association yet.
- macOS CI runs 43751314457, 43754038780 and 43755389510 counted 2, 1 and 2 allocations in 100
  frames in the `hello` test of `tests/run.rs` of `patina-cli` (0 expected). The other macOS runs passed.
  Windows and Linux pass. NOT_ENOUGH_INFO: no Mac is available to find the cause. A hypothesis:
  a lock of the worker pool allocates the first time this thread waits.
- The editor draws the node GUIs in the node body, and the large size of the selected node in
  the **Inspector**. It draws them again after each run and each frame. A change in a live
  graph runs the graph again (4.3). The editor selfcheck clicks a button of a node GUI, sees each
  value widget, selects a table row, steps a number field, selects a tab, opens a section, moves a trim
  handle with a key, reads the axis labels, the scatter dots and the curve histogram, clicks an icon button and reads the progress bar, adds a ramp stop with a click, removes it with Delete, sets the ramp blend, hides a layer and moves it with the down arrow, types a file path, picks a project file in the asset picker, clicks the canvas, shows the large size in the **Inspector**, and opens the full size and closes it with Esc. It drags a curve point, a trim handle, a ramp stop, a layer, a canvas, the box of a 2D transform gizmo and a timeline clip with page pointer events, not with the real mouse.
  Chromium ignores the pointer capture of these events, so the drags do not check the capture. A
  layer drag down could lose its capture, because React moves the row of the handle. The list
  now holds the capture (fixed 2026-11-17 from the source, not tried with a real mouse). The
  selfcheck sets a color with the change event that the closed picker sends (2026-11-18). No check
  opens the color picker or clicks **Browse**. The dialogs of the platform need a person.
- The timeline widget (4.3, mockup 5) has a transport, a ruler with the loop and the playhead,
  clip lanes and automation lanes. The SDK tests check the widget line, the edits, `advance` and
  `Automation::value`. The frontend tests check the parse and the limits of each drag. The
  selfcheck drags a clip, and clicks **Play**, **Pause**, **Loop** and a **Snap** value (2026-11-18).
  It moves the loop end with an arrow key. A ruler shorter than the default loop range of 0 - 4 s
  did not loop, and its loop band went past the ruler end (fixed 2026-11-18). The SDK now keeps
  the loop range inside the ruler. No person tried it with a real mouse. A lane reorder is not in this version.
- A node with `#[node(full_gui)]` has an expand button in its header and in the **Inspector**
  header. The button opens the full size over the graph area. **Back** or Esc closes it. The
  parse test and the macro test check `full_gui`. The editor selfcheck opens the full size,
  reads a label that the node shows only in the full mode, and closes it with Esc.
- Each build makes a new cluster. Milestone 6 keeps the state of each node whose code did not
  change (7.2).
- A state that is not `Send` makes all the tasks of the cluster run on the thread of the run,
  not only the task of the node (9.2).
- The editor selfcheck wires the built-in delay, a generic node, from A to B in the live graph.
  Both wires show 2.5. The editor uses the same engine code as the CLI.
- Only stateful nodes can have a GUI. An audio GUI has no `#[settings]` and no messages yet (4.3).
- The **Widgets** group of the **Project** tab makes widget folders with a start widget. **Add
  crate** with a widget name adds the widget by its path. A manual check built a node that shows
  a widget folder in `patina-cli`. The editor self-check makes a widget folder. Then it builds a
  node that shows the start widget, and a press on the widget sets the value (2026-11-18).
- A node GUI can have a canvas with lines, rectangles, circles and text. The canvas sends the
  pointer down, the moves of a drag (one for each frame at most) and the up. A custom widget
  implements the `Widget` trait. The editor selfcheck clicks a canvas and drags on it, and the
  node draws a dot at each point.
- These shortcuts are open in the code:
  - The **Add node** palette compares the type text with no module paths, so two types of one
    name in two modules match (`crates/patina-editor/frontend/src/canvas/palette.ts`). rustc checks the
    types at build time.
- A loop of only generic nodes and delays has no type, so the engine refuses it.
- A generic node with no input gets its type from the readers of its output (4.5). The hello
  test runs a generic zero node in the batch domain, and a number node of the frame domain gives
  its type. The slot test runs it in the slot. The event slot test gives it an async reader, with
  the async node in the slot and out of it.
- Two Linux CI runs counted 1 allocation in 100 frames of the hello test. A slow frame can start
  the worker pool for the first time inside the counted frames. A parallel cluster now prepares
  the pool when it loads. One CI run with this fix passed on all three platforms.
- A generic node with no wire that gives its type is unresolved, with gray pins (fixed
  2026-11-16). The editor selfcheck adds a delay with no wires, sees the unresolved state, and
  sees the state go when a wire gives the type. A generic pin shows the name of its type
  parameter in its tooltip. The Add node palette joins a wire to a generic pin of any type. Its list has the built-in delay node, after
  the nodes of the project. The editor selfcheck adds the delay from the list of a pin drag.
- The frames draw only the node GUIs in the view of the graph (fixed 2026-11-16, 4.3 rule 3).
  The page sends the nodes in view. The test `the_frames_draw_only_the_node_guis_in_view` checks
  the choice of the engine. No check measured the time on a large graph.
- The delay runs in the audio domain when its input comes from there (fixed 2026-11-16). The
  hello test plays a tone through a delay and hears it one block later with no frame between.
  The plan test puts the delay of an audio cycle in the audio domain. The editor selfcheck wires
  a delay after an audio node and sees the audio domain icon. No check played it on a real
  device.

### Milestone 4: automatic parallelism and the frame domain

Milestone 5 added the delay node, and milestone 11 made the latest-value wires of the audio
domain free of locks. These limits are still open:

- The pool rule uses the node times of the last run. Thus, after a small run, the next run
  stays on one thread also when it is heavy. The measurement ran only on Windows.
- Measured on 2026-11-19, on Windows, in a debug build, with a copy of the computer graph in
  the edit mode: the engine part gave 48 allocations in each frame with values after a quiet
  frame. They came from the lanes of the values. Now each value gets its old item again, so the
  engine part gives 0 (fixed 2026-11-19). The measurement was a temporary test.
- Each call of a node GUI copies its widget lines one time in the engine. The SDK keeps the
  widget buffer of the last GUI, so the GUI itself does not grow a new buffer.
- The canvas texts of the computer nodes use `format_args!`, so they do not allocate (fixed
  2026-11-19). The labels, the canvas names, the digits of the displays and the paths of `ram`
  still allocate at each frame with `format!`, `collect` and `vec!`. The allocation test does
  not see the cluster library, so nobody measured these allocations.
- The allocation test counts only the thread of the run. In the test graph, this thread runs
  all the frame tasks, because the second task waits for the first.
- A text that is longer than the text of the last frame grows its buffer one time.
- The editor sends an event to the page at each frame, also when no value changed.
- A change while the frames run runs the batch domain again before the next frame.
- A latest-value wire between the batch and the frame domain uses a slot with a lock (9.1).
  The batch run ends before the frames start, so the lock never waits.
- A batch value that a frame node reads stays after the frames end, until the next batch run.
- The worker threads of a parallel run wake about 60 us after the run starts. The calling thread
  spins for up to 200 us at the end, so it does not wait for its own wake (fixed 2026-11-16).
  Eight separate tasks of 22 us now run 1.7 to 1.8 times faster than one task, and eight tasks
  of 180 us 3.7 to 5.7 times faster, not 8 times. The measurement ran only on Windows.

### Milestone 2: editor

- **Add crate** has no search of crates.io and no list of versions. The self-check clicks **Add
  crate** for a widget folder and for the `log` crate, and saves Cargo.toml (2026-11-18). The
  local check runs offline with `log` in the cargo cache. CI does not run the self-check.
- The self-check does not click **Install toolchain**, because CI must not download the
  toolchain. The self-check clicks **Remove** of a trusted project and of a library in
  **Settings** (2026-11-18).
- No check opened the folder dialog, the fatal message box or the WebView2 message. A person
  must click them. The message of a missing WebView2 Runtime did not run.
- The check drags one node with mouse events of the page, not with the real mouse. A person
  did not use the editor yet.
  The owner check: make the first workflow of 15.10 with the mouse.
- The capture works only on Windows. The check did not run on Linux or macOS.
- rust-analyzer gives completion and hover in the **Code** tab (15.1, added 2026-11-17). A Rust
  unit test checks the message frames. A frontend unit test checks the conversion of the results.
  A script sent the messages of the client to rust-analyzer 1.100 on a copy of `workbenches/hello`.
  Completion, resolve and hover gave correct results. The editor did not run with rust-analyzer,
  because the selfcheck toolchain has none. Nobody typed in the **Code** tab with it.
- A view opens as a tab in the center, after the fixed **Graph** tab (15.2, added 2026-11-17).
  The active view covers the graph, so the graph keeps its state. A layout unit test checks the
  center. The selfcheck checks that a view opens in the center and that **Graph** shows the
  graph again. Nobody dragged a view tab into the center by hand. A layout file from before
  keeps its view tabs in the right area.
- A change of the open graph kept a view tab over the new graph (fixed 2026-11-18). Thus the
  shot captured the **monitor** view in place of each subgraph of computer, and its checks did
  not see it. A change of graph now shows the **Graph** tab. The shot checks that no view covers
  each subgraph.
- At open, the graph fit before the node GUIs had their size, so the shot cut the bottom keypad
  of hello (fixed 2026-11-18). The graph now fits again when a node changes size, until the user
  clicks, types, pans or zooms in the canvas. The shot checks that each node is in the canvas
  when the main graph and each subgraph open.
- A wire badge cut short values to `12...` or `0...` (fixed 2026-11-18). A badge now keeps at
  least 50 px, so a value of 6 characters shows whole. The shot checks that no badge of 6 or
  fewer characters cuts its text.
- A badge of 50 px on a short wire went under the pin of the node, for example `GpuIm...` from
  **bars** to **mix** in gpu (fixed 2026-11-18). A badge that is not selected now hides within
  6 px of a node body. The shot checks that no pin or node covers a badge.
- A badge covered the crossing of two wires, so a crossing looked like a junction, for example
  `3.0` in hello and `false` in counter-4 of computer (fixed 2026-11-18). A badge that is not
  selected now hides when a routed wire of another output goes through it. The shot checks
  that no routed wire of another output goes through a badge.
- The **value** output of counter-4 in computer showed its name two times, in the header and
  in the bus socket (fixed 2026-11-18). The socket of a public pin now shows only `[7:0]`.
- The chip drawings of computer, for example **gpu**, **ram** and **sprites**, showed bus pins as
  dots with no contacts (fixed 2026-11-18). A node with a free layout and a bus pin now shows a
  socket column on each side of the drawing (4.3). The drawings no longer draw pin names. The
  shot checks that each bus pin of such a node has its contacts and that the drawing does not
  cover a column. Each chip is now a package with gold fingers, copper traces and legs at the
  contact rows, and a lead goes from each contact to its finger. The bottom row of the main graph
  moved to the right, so no wire goes through the bigger chips.
- The memory map of **ram** in computer showed no empty cells (fixed 2026-11-18). The path of
  the empty cells had more than 4096 characters, and the editor skips such a path with no
  message. The map now draws one path for each row. The shot now fails when a canvas has a
  skipped shape.
- Two equal bus labels `address[7:0] / 8` on one trunk ran together with no space in computer
  (fixed 2026-11-18). A label now hides when a shown label is nearer than 4 px. The shot
  checks that no two shown labels touch.
- The **File** select of the program node in computer cut `programs/fibonacci.asm` with no
  "..." (fixed 2026-11-18). A field now cuts its text with "...", and a select of a node GUI
  shows its full option in the tooltip. Only an eye check of the shot checks this.
- Three buses shared a gap of 30 px between two register nodes in **register-4** of computer,
  so an input bus ran on the pin line of an output bus (fixed 2026-11-18). The register nodes
  now have the pitch of the mux nodes.
- In **mux-8** of computer, an output row of a mux node was 2 px from a pin row of the **JOIN**,
  so a crossing went into a corner (fixed 2026-11-18). The **JOIN** moved 8 px down, so each row
  pair is half a pitch apart. The shot check of such pin rows now reads the row of the straight
  wire.
- The Inspector showed the random keys of the public pins of a subgraph, for example `dcc47589`
  for **sel** of a **mux** use (fixed 2026-11-18). The parameter rows and the wire **From** and
  **To** rows now show the pin names. The shot checks that no parameter row and no wire row of
  a subgraph shows a pin key.
- The state dot of a bus join symbol went under its order pin, for example in **counter-4** of
  computer (fixed 2026-11-18). A join was 50 px wide, so its caption icons did not fit between
  the pins. A bus stub is now at least 36 px. The shot checks that no order pin covers an icon.
- Wires crossed the text of the subgraph path bar at the top left of the canvas (fixed
  2026-11-18). The bar now has a background of the canvas color.
- The caption of a narrow symbol use went past its node, for example the AND gate of
  **half-adder** in computer (fixed 2026-11-18). The caption icons now wrap to rows above the
  name. The shot checks that no caption leaves its node and no order pin covers an icon.
- **Reset route** and **Lock bends**, and **Import** and **Package**, touched with no gap, so
  each pair looked like one control (fixed 2026-11-18). The shot checks that two buttons side by
  side have a gap.
- A bus label cut its text, for example `value[7...` in **ram-4x4** of computer (fixed
  2026-11-18). With too little room, a bus label now shows `name / n`, else `/ n`. The shot
  checks that no bus label cuts its text.
- A bus label beside a small S-bend showed only `/ 8`, for example in **ram-4x4** of computer at a
  near zoom (fixed 2026-11-18). A corner that rises less than 10 px no longer takes room from the
  label.
- A value badge covered a junction dot, for example at the **JOIN** of **register-8** in computer
  (fixed 2026-11-18). A badge that is not selected now hides on a junction dot. The shot checks
  that no badge covers a junction dot.
- A badge sometimes covered the ring of a junction in **mux-4** of computer (fixed 2026-11-18).
  A wire with a live value has lanes, so its junction is a ring larger than a dot. The badge
  rule used the size of a dot. Each junction now gives the rule its drawn radius.
- Two wires of different outputs crossed inside their rounded corners, so the crossing had no gap
  and looked like a junction, for example at the inputs of **MIX** in gpu (fixed 2026-11-18). A
  corner near a crossing now gets a smaller radius. The shot checks that no two automatic wires of
  different outputs cross at a bend or overlap.
- Many wires of different outputs went on one line in a full channel, for example at the inputs
  of **register-4** in computer (fixed 2026-11-18). Only segments that overlap along the line now
  keep a space from each other, and a narrow channel gets smaller spaces. A wire that turns before
  the pin line of another output now goes before it. When only one order of two segments fits,
  they take that order. On the main graph of computer, the wires on one line went from 320 to 0.
  Five frontend unit tests check the nudge.
- Two decoder outputs of **hex-7seg** in computer ran on one pin line for 50 px (fixed
  2026-11-18). The nudge moved a segment before another and passed a third segment, which then
  shared its pin line. Each move now counts all the segments that it passes, and a segment can
  also move later in the order.
- Another wire went through a type label, for example **f64** at the **A x B** node of hello (fixed
  2026-11-18). A label now hides where another wire with the automatic route goes through it. The
  shot checks that no wire goes through a label. A wire with its own route or a curve does not
  hide a label yet.
- The parameter field in a node row covered the output name, for example **file** over **out1** of the
  **rom** node in computer (fixed 2026-11-18). The field now keeps its width of 70 px at most. The
  shot checks that the pin names, the fields and the bus socket heads of a node do not cover each other.
- A type label stayed when its badge hid, so two `bool` labels stood over one badge, for example
  in **alu-8** of computer (fixed 2026-11-18). A type label now hides with its badge.
- The **Debug** tab shows a timeline of the node calls of each domain and thread (15.7, added
  2026-11-17). The load, CLI run, frontend and selfcheck tests check the start and the thread.
  A pool unit test sees a task on a worker give its pool index above 0. A CLI test and the
  selfcheck run two `heavy` naps with no wire and see them on two threads (2026-11-18). Two light
  naps share one task, so they run on one thread (9.2). Nobody looked at the tab by hand.
- The palette of a drag from a pin compares the type text with no module paths (15.2). A type
  alias does not match its type, and two types of one name in two modules match.
- `stop_requested()` is false on a thread that a node starts. The limits of milestone 12 tell how
  the waits of async nodes (4.6) stop.
- The self-check builds a node with the `log` crate and reads its line in the **Logs** tab of the
  **Inspector** and in the **Logs** panel (2026-11-18). The crate must be in the cargo cache or on
  crates.io. A CLI test builds and runs two such nodes and reads their log lines (2026-11-17).
- The `Viewer` gives text, a thumbnail of 64 x 64 pixels at most, and the widgets of a node GUI
  (5.3, fixed 2026-11-17). `Viewer::show` gives the plot, the waveform and the table with the
  `Ui` widgets. The **Inspector** reads the full text and the widgets one time after each run.
  The glue, load, sink and frame tests check the widget lines. The cluster makes the widget lines
  of each probed value at each run, also when no **Inspector** shows them. The selfcheck selects a
  wire with a `Viewer` value and sees its text, thumbnail and waveform in the **Inspector**
  (2026-11-18).
- The `frame` type of the computer example gives a thumbnail, the screen as an image and a plot
  of the colors (2026-11-18). A hidden `--shot` run selects a frame wire in computer and an image
  wire in gpu, and waits for the thumbnail and the screen image in the **Inspector**. A node with
  a free layout or a symbol shows no thumbnail. No hand test showed the widgets.
- In a live run, the **Inspector** keeps the newest answer of its value requests (fixed
  2026-11-18). Before, each frame cancelled the request, so a fast graph showed only the badge
  text and no widgets.
- An audio wire draws the wave of its `AudioBlock` and moves with each frame (5.3, 15.9,
  2026-11-17). The glue, outcome, frame and page tests check the wave text and the path. The
  editor selfcheck sees the wave change between two frames. No person looked at it on screen. A
  note wire does not flash for each note yet.
- The node GUIs, the row text, the pins and the bus lanes draw down to a zoom of 25 percent
  (2026-11-17). All change to the simple form at the same zoom.
- **Lock bends** keeps at most 12 bends of a longer route. Between two kept bends, the router
  can then take a different path.
- The automatic route and **Arrange** have no undo of a route change, because a route is not
  saved. The route changes when a node moves, and the wires of a large graph can jump.
- The recovery file does not have the typing of the last 300 ms before a crash. The check did
  not crash Patina. The selfcheck clicks **Discard** and sees the saved graph (2026-11-18). A
  project that is not trusted keeps its old recovery file, and its new changes do not go into
  the file until the user answers.
- A connection kind matches a type by the type text that the node code writes, not by the type
  crates of 5.1. A panic in a kind gives no lanes. Its message shows only in the **Output**
  panel, from the standard error (4.7). A compile error in a kind
  stops the build of all of the graph.
- The lanes have no groups. The lane hover works only on the WebGL wires, at a zoom of 0.25 or
  more. The badge is always the viewer text.
- The self-check makes a connection kind with **New connection** and wires a `u16` of that kind
  (2026-11-18). It checks the bus label before and after a run, the **Lanes** row of the
  **Inspector**, a lane tooltip with its live state and the "/ 16" label below a zoom of 0.25.
  The **Lanes** row uses the bus name of 15.9 (fixed 2026-11-18). Nobody looked at the lane
  colors of a real run by hand.
- The editor selfcheck drags a tab to another area and drags a splitter with page events, not
  with the real mouse. The selfcheck also ends a drag of a view tab out of the window, and the
  view floats (2026-11-18). These are page events, not a drag of the operating system.
- Vite shows a warning for the `"use client"` lines of React Flow, and a warning for the size
  of the page script. The build is correct.
- `package.json` sets `dompurify` to 3.4.17 with an override. Monaco 0.57.0 asks for 3.4.15,
  which has the critical advisory GHSA-p98j-92pf-mc4p. `npm audit` now finds 0 problems.
- These shortcuts are open in the code:
  - The capture works only on Windows (`src/session/check.rs`).
  - One texture row holds the points of each wire. Above `MAX_TEXTURE_SIZE` wires (2048 or
    more, by GPU), SVG draws the wires (`frontend/src/wires/gl.ts`).
  - A route of more than 62 points draws only its first 62 points. Only a graph file that a
    person edited has such a route (`frontend/src/wires/wires.ts`).
  - A new lane state builds the instances of all wires again (`frontend/src/wires/wires.ts`). On
    2026-11-17 in Node 22, one build took 0.3 ms for 500 wires, 1.0 ms for 2000 wires and 3.6 ms
    for 8000 wires, with a bus of 8 lanes on each fourth wire. The upload to the GPU was not
    measured.
  - The SVG wires draw a bus as one line, and have no gap at a crossing (`frontend/src/WireLayer.tsx`).
  - A wire with more than 62 points after its gaps draws without its last gaps (`frontend/src/wires/wires.ts`).
  - A graph with a route grid of more than 2 million cells gets the simple orthogonal paths
    (`frontend/src/wires/route.ts`).
- The route points of the wires in a frame move with the use during a drag (fixed 2026-11-17).
  A frontend unit test checks the move. Nobody dragged an open frame by hand. The frame is
  read-only. Open the subgraph to edit its nodes.
- A `log` crate with another name or in a target table now turns on the logger (fixed
  2026-11-16). The plan test reads four manifests. A CLI test builds and runs a node of each
  form (2026-11-17).
- The editor has no bench with 500 nodes, so the far zoom form was not measured for frame
  drops. After each frame, the engine copies each changed value one time into the kept values,
  and makes a new badge text for the run event (fixed 2026-11-17).
- The segments at the two pins of an orthogonal wire cannot move, and a wire with the graph
  default style has no segment bars (15.9). The **Default route** of the **Inspector** sets the
  graph default style (fixed 2026-11-17).
  **Bus** and the crossing bridges are not made. A wire with a fixed style can go under a node.
- The segment at a pin stays on the line of the pin. Two buses at pins nearer than their width
  overlap there, for example two buses of 40 px at pins 16 px apart in the main graph of computer.
  Two pin lines of different outputs nearer than the gap can also cross at a bend, for example
  at 2756, 120 in incrementer-8 of computer. The nudge cannot move them, and a cost in the A*
  router for such lines made the other graphs worse. The shot does not report this case yet.
  At the input splitter of incrementer-8, the wires of bits 2 to 8 turn within 4 px of the pins
  and cross at the pin stubs, because the lanes go to targets in a different order.
- **Arrange** splits a tall column into up to 4 columns when the layout is more than twice as
  tall as wide (fixed 2026-11-17, before a wide layer made one tall column). With estimated node
  sizes, `rom-16x7` went from 2006 x 11472 px to 2596 x 5433 px. The router of the editor found a
  path around the nodes for each wire. The wires were 17% shorter, with 12% more bends. A
  frontend unit test checks the split. A `--shot` run on a hidden desktop pressed Shift+L in the
  editor on `rom-16x7`. The constant column split into 3 columns with no overlap. Nobody used it
  by hand yet.

### Milestone 1: engine and CLI

The later milestones closed the limits of one cluster, a static `std`, function nodes only, the
library in the target folder and the single run of the CLI. These limits are still open:

- The toolchain comes from the toolchain folder (13.3), or else from the `CARGO` variable or
  `PATH`. Its version must be the `rust` version of `patina.toml` or newer (8.1).
- **Install toolchain** works only on Windows. It installs rust-analyzer and, since
  2026-11-17, the source of `std` (`rust-src`, 13.2). An install of 2026-11-17 on the computer
  of the owner has rust-analyzer but no `rust-src`. **Update toolchain** adds `rust-src`. No
  install or update with `rust-src` ran yet, because it downloads. A unit test reads the
  `rust-src` table of the channel manifest.
- No GitHub release makes the portable zip yet. The **Package** workflow keeps the zips only as
  artifacts of the run.
- A `Result` output is found by the type name `Result` only. A type alias with another name is
  not found, and then an `Err` is a value, not a failure.
- The editor compares the pin types only for the ring of a wire drag (15.9). A drop on a socket
  of another type still adds the wire, and the next build gives the compile error. The type
  label is the text of the code, so `String` and `std::string::String` show differently.
- The CLI finds the toolchain folder of the installed edition too (fixed 2026-11-16). A unit
  test checks the rule. No installer ran, so no installed CLI ran with it.

### Rust types across clusters

- The loader supports only Windows. On Linux and macOS, the test does not run.
  [Issue 1](https://github.com/bhowiebkr/Patina/issues/1) tracks this gap.
- The test did not run on a clean Windows computer.
- The only generic type is `Vec<T>`. The type crates have no dependencies, so the build ID
  does not include resolved dependencies yet.
- The harness is not the engine. It shows the method, and the engine will use the same method.

### Hot reload

- The test ran only on Windows, because the loader supports only Windows.
- The memory of one row is noisy, because the test and the Windows loader also allocate. Use
  the totals of a long run.
- `/Brepro` is a flag of the MSVC linker. With `rust-lld` on `x86_64-pc-windows-gnu`, the flag
  is `--no-insert-timestamp` (see Toolchain). The hot reload test did not run on that target.
- The cleanup of the cargo target folder is not designed yet.
- The harness is not the engine. It shows the method, and the engine will use the same method.

### Toolchain

- The prototype did not run on a clean computer. This computer has Visual Studio and a Rust
  install. The clean environment hid them, and the build did not use MSVC.
- The prototype is for Windows only.
- C code that needs functions of the UCRT only was not tested.
- rust-analyzer and NVRTC (13.2, items 3 and 4) were not installed.
- The update of an installed toolchain was not tested.
- The fixture is a C ABI call, not a node in a graph.
- These shortcuts are open in the code:
  - `curl.exe` downloads all the Rust files, because the firewall can block a new
    `rustup-init.exe` (`src/install.rs`).
  - `PATINA_FETCH_CARGO` names a `cargo.exe` that the firewall allows, for the crate download
    (`src/main.rs`).

### Viewport

- The bench ran only on Windows 11, with one monitor at 60 Hz. It did not run on Linux or
  macOS. The shared path, the capture and the sampler use Windows APIs.
- The bench window is off-screen, and the checks read `PrintWindow` captures, not the screen.
- The zoom of the web view emulated the display scale. Two monitors with different scales were
  not checked.
- The owner check: run `cargo run --release -p viewport -- --mode native`. Drag, resize, float
  and dock viewport 1, and open the menu. Change the Windows display scale, and look at the
  viewport edges.
- A real device loss between the loss check and the present keeps the texture held. Then the
  Vulkan surface drop panics (`src/render.rs`).
- These shortcuts are open in the code:
  - The texture release after a device loss uses a caught unwind (`src/render.rs`).
  - The bench code is a copy of the connections bench (`src/bench.rs`).
  - The CPU and GPU sampler and the probes work only on Windows (`src/bench.rs`,
    `src/main.rs`).
  - The page and the reports use paths in the source tree, so the prototype runs only from a
    checkout (`src/main.rs`, `src/bench.rs`).
  - The scene file has no WESL features, so `wgpu` reads it as WGSL (`src/gpu.rs`).
  - The channel path allocates for each frame (`src/render.rs`).
  - A resize of the shared path makes a new buffer (`src/shared.rs`).

### Connections

- The bench ran only on Windows 11, with one monitor at 60 Hz. It did not run on Linux or macOS.
- The zoom of the web view emulated the display scale. A live change of the Windows display
  scale is an owner check. Open the graph view, change the scale in Windows Settings, and look
  at the wires, the badges and the wire selection.
- The bench moves the view through the React Flow API. The mouse path of the pan, the zoom, the
  selection and the route drag was not measured.
- The nodes do not move, so a node drag was not measured. A bus socket does not show a contact
  for each lane.
- React Flow hides an SVG edge outside the box of its two nodes. A route point outside that box
  was not checked.
- Svelte Flow was not measured.
- These shortcuts are open in the code:
  - The CPU and GPU sampler and the screenshots work only on Windows (`src/bench.rs`,
    `src/main.rs`).
  - The page and the reports use paths in the source tree, so the prototype runs only from a
    checkout (`src/main.rs`, `src/bench.rs`).
  - Each send allocates. A send with no allocation needs shared memory (`src/feed.rs`, 15.6).

## Shortcuts in the code

Each `shortcut:` marker of the code (AGENTS.md). Change the code and this list together.

- `crates/patina-cli/tests/cli/run.rs:508`: The CLI test of `workbenches/gpu` checks only the build when no adapter exists. Add an adapter to CI when one is available.
- `crates/patina-editor/frontend/src/canvas/NodeView.tsx:358`: A wire leaves a free pin only to the left or the right. Add a side to `Ui::pin` when a node needs it.
- `crates/patina-editor/frontend/src/canvas/NodeView.tsx:371`: A face only draws. It takes no pointer input, because a node in a use cannot write. Change it when a face needs input.
- `crates/patina-editor/frontend/src/canvas/SymbolView.tsx:24`: A lane of a symbol has no hover tooltip, as the contacts of a box socket have. Add one when a symbol needs it.
- `crates/patina-editor/frontend/src/canvas/palette.ts:38`: The palette drops module paths, so two types of one name match. Rustc checks the types at build time.
- `crates/patina-editor/frontend/src/check/check-groups.ts:73`: A short wait lets the move edit arrive before the undo. Change it when the canvas shows the edit count.
- `crates/patina-editor/frontend/src/check/shot-layout.ts:273`: The pin rows of two outputs nearer than the gap cross at a bend, and the nudge cannot fix this. Fix it in the A* router.
- `crates/patina-editor/frontend/src/wires/WireEdge.tsx:156`: The SVG fallback draws a bus as one wide line with no lanes. Draw its lanes when a GPU without WebGL2 matters.
- `crates/patina-editor/frontend/src/wires/WireEdge.tsx:157`: The SVG fallback has no crossing gaps. Add a mask when the fallback is more than a lost context.
- `crates/patina-editor/frontend/src/wires/gl.ts:275`: One texture row holds each wire. Above `MAX_TEXTURE_SIZE` wires, SVG draws them. Pack the rows for larger graphs.
- `crates/patina-editor/frontend/src/wires/route.ts:36`: A graph with a larger route grid gets simple orthogonal paths. Use a sparser grid when such graphs matter.
- `crates/patina-editor/frontend/src/wires/wires.ts:368`: A path with more than `MAX_POINTS` - 2 bends draws only its first bends. Split the row when this matters.
- `crates/patina-editor/src/engine.rs:525`: No event runs when the slot holds a value that is not `Sync`, an event node and a node of another domain. Give event nodes their own slot library.
- `crates/patina-editor/src/engine.rs:621`: When a library is not parallel, the blocks run between the frames, so a slow frame makes them late. Give such a graph its own thread.
- `crates/patina-editor/src/session/check.rs:78`: Screenshots work only on Windows. Add a path for Linux when the check runs there.
- `crates/patina-editor/src/session/start.rs:170`: Linux and macOS have no trash, so **Delete** shows an error there. Add the freedesktop trash when a user asks.
- `crates/patina-engine/src/device.rs:196`: Only Windows raises the audio thread priority. Add it for Linux when a user there hears gaps.
- `crates/patina-engine/src/events/watch.rs:36`: `file_change` polls its folder. Use the change events of the system when folders get large.
- `crates/patina-engine/src/generate.rs:23`: Each domain has one cluster, with no boundary at large nodes. Split at large nodes and subgraphs when a measurement shows the need.
- `crates/patina-engine/src/library.rs:77`: A project root that is not UTF-8 changes, so its assets are not found. Pass the bytes of the system when a user needs it.
- `crates/patina-engine/src/load.rs:416`: A node with no cluster or index loses its state. Only a defective plan causes this, so no change is needed.
- `crates/patina-engine/src/node.rs:153`: Code that nests deeper than the 64 MB stack allows still overflows. Parse in a child process if a real project needs it.
- `crates/patina-event/src/lib.rs:20`: Each cluster build makes its own runtime. Share one runtime when many builds load.
- `crates/patina-event/src/lib.rs:75`: The run thread blocks while it waits. Change this when event runs share the worker threads.
- `crates/patina-gpu/src/viewport.rs:348`: A viewport surface works only on Windows. Add the X11 and Wayland handles when the editor needs them.
- `prototypes/connections/src/bench.rs:167`: The path comes from the source tree, so the bench runs only from a checkout.
- `prototypes/connections/src/bench.rs:236`: CPU and GPU use are measured on Windows only. Add `/proc` sampling for Linux when needed.
- `prototypes/connections/src/feed.rs:62`: Each send allocates, because Tauri takes the frame by value. A pooled path needs shared memory.
- `prototypes/connections/src/main.rs:91`: Screenshots work only on Windows. Add a path for Linux when the bench runs there.
- `prototypes/connections/src/main.rs:116`: The page is read from the source tree. Embed it when the editor ships.
- `prototypes/geometry/src/lib.rs:125`: A prism takes only convex outlines. Add ear clipping when a node needs L shapes.
- `prototypes/toolchain/src/install.rs:157`: `curl.exe` downloads all Rust files, because a firewall can block a new `rustup-init.exe`.
- `prototypes/toolchain/src/main.rs:161`: `PATINA_FETCH_CARGO` names a `cargo.exe` that the firewall allows, for the crate download.
- `prototypes/viewport/src/bench.rs:3`: The bench code is a copy of the connections bench. Merge both into one crate when a third prototype needs it.
- `prototypes/viewport/src/bench.rs:132`: The path comes from the source tree, so the bench runs only from a checkout.
- `prototypes/viewport/src/bench.rs:193`: CPU and GPU use are measured on Windows only. Add `/proc` sampling for Linux when needed.
- `prototypes/viewport/src/gpu.rs:18`: The scene file has no WESL features, so `wgpu` reads it as WGSL. Link it with the `wesl` crate when it imports.
- `prototypes/viewport/src/main.rs:146`: The probes work only on Windows. Add a capture for Linux when the bench runs there.
- `prototypes/viewport/src/main.rs:223`: The page is read from the source tree. Embed it when the editor ships.
- `prototypes/viewport/src/render.rs:299`: A drop in an unwind releases the texture (gfx-rs/wgpu 8243). Remove it when wgpu fixes this.
- `prototypes/viewport/src/render.rs:447`: A real device loss after the check keeps the texture, and the Vulkan surface drop then panics.
- `prototypes/viewport/src/render.rs:566`: Each frame of the channel path allocates, because Tauri takes an owned `Vec`. The shared path does not.
- `prototypes/viewport/src/shared.rs:45`: A resize makes a new buffer and waits for the main thread. Keep spare room when resizes are frequent.
- `workbenches/gpu/nodes/bars/src/lib.rs:120`: A computer without a GPU adapter checks nothing in the bars test. CI on Linux has no adapter.
