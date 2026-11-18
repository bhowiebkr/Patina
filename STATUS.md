# Status

All milestones of the roadmap (spec section 16) ended. Milestone 12, the event domain, ended on
2026-11-16. The [quality plan](docs/quality-plan.md) of 2026-11-17 is done, except two items
that need the owner. The next work is the open items of the known limits and of
[docs/before-building.md](docs/before-building.md).
Milestone 11, the audio domain, ended on 2026-11-16. Its step 3, the GUI of audio nodes, stays
deferred by the owner.
The circuit graphs of question 13 (spec section 18) work: the computer example of NAND gates is
complete since 2026-11-16. Milestone 10, views,
ended on 2026-11-15. Milestone 9, the GPU, ended on 2026-11-15. Milestone 8, packaging, ended on 2026-11-15 without the Python module, which needs
a download that the owner did not approve yet. Milestones 2 to 10 work, with the open items in their known limits. The installers of
milestone 3 are not made yet: the WiX tools need a download that the owner did not approve yet.
Milestone 1, the engine and the CLI, works. Milestone 0 ended on 2026-11-15, with three owner
checks open.


The records of the ended milestones and of the finished work are in [docs/history.md](docs/history.md).

## What works

- Rust version of a project: The `rust` of `patina.toml` is the oldest Rust version that builds the project. **Update toolchain** raises the toolchain.
- Copy, paste and libraries: **Copy**, **Cut** and **Paste** of nodes work. **Settings** > **Libraries** and **Add from Git** give nodes from other projects.
- Gate speed: Light nodes share one task, nets share one check and skip the `Option` checks when their inputs have values, and the frame rate goes up to 100000 FPS.
- Touchpad pan: A two-finger touchpad scroll pans the graph. The mouse wheel and a pinch zoom.
- Arrange with node sizes and bus widths: **Arrange** uses node sizes and wire widths. The 25 graphs of `examples/computer` have 0 node overlaps.
- No save without Save: Only Ctrl+S or **Save** writes graph files. The recovery file keeps the unsaved graphs. An undo of a code edit puts the text into the **Code** tab and writes no file.
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
- Milestone 11: the audio domain: The audio domain runs blocks on an audio thread and plays them with `cpal`. The GUI of audio nodes is deferred.
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

### Copy, paste and libraries

- Known limits: the index has no libraries yet. A clip has 256 subgraphs at most.

### Gate speed

- Known limits: a gate net does not run in parallel. A light node that does much work runs in
  parallel only with `#[node(heavy)]`. The fast copy of each net makes the generated Rust and
  the bake build of the cluster about 11% larger and longer.

### Touchpad pan

- Known limits: a mouse with smooth scroll steps can give touchpad values, so its wheel pans. A
  touchpad driver that sends mouse wheel steps zooms. Linux and macOS are not checked.

### No save without Save

- Known limits: a new folder, **Import**, **Add crate**, and an undo or a redo of a folder change
  still write their files at once.
- Checked on 2026-11-17: the engine tests and the self-check undo code edits with no file write.

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
  does not save, so the terminal, the frame and the key buffer are empty after an open. The
  keyboard path to the terminal is checked only by unit tests, because it needs typed input.

### Canvas editing

- Known limits: the self-check drop is a page event, not a drag of the operating system.

### Canvas speed

- A pan of 2000 nodes takes about 70 ms for each frame, and a drag about 155 ms (see Speed
  measurements). A game ran in the background, so the times without load are lower.
- The bench ran only on Windows 11, with the debug editor and the built page. It did not run on
  Linux or macOS, or in a release build.

### Edit speed

- An edit of only the layout keeps the domains and the problems of the last check. A change to a
  node or subgraph file from outside the editor shows at the next edit that runs the check.
- The parse cache of the node folders keeps at most 4096 folders (fixed 2026-11-17). When it is
  full, it drops all entries, and the next check parses each node folder again.
- The edit times ran only on Windows 11. The page times are from the debug editor.

### Process isolation

- The crash handler is only on Windows. It names the node that runs on the thread that crashed,
  also in a parallel run (fixed 2026-11-17). A unit test checks the parallel rule. The crashes of
  the self-check run on one thread.
  A thread that a node starts is not in a run. A Rust abort skips the handler.
- Nobody looked at a viewport of the node process on screen. The probe ran on a hidden desktop.
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
  node draws only when the event and the slot libraries are parallel. A GUI input is lost when
  a run holds the node at that time.
- The first build of a project with an async node needs `tokio` from crates.io or the cargo
  cache.

### Milestone 11: the audio domain

- These shortcuts are open in the code:
  - `engine.rs` of `patina-editor`: when a library is not parallel, the blocks run on the engine
    thread between the frames, so a slow frame or node GUI makes them late.
- While the audio thread of the editor runs, a view reads no output of an audio node, and the
  GUI of an audio node does not draw. The badges of the audio outputs still show.
- The audio thread has its own `Control`. So the editor does not show an audio node as the node
  that runs, and **Stop** stops the audio thread only at the end of a block (15.2).
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
  one viewport on Windows only, and no bake build.
- Nobody looked at a viewport in the editor yet. The self-check keeps its window hidden, so it
  cannot show the surface. The colors, the resize and the scroll clip need a check on screen.
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
- These shortcuts are open in the code:
  - A project root that is not UTF-8 goes to the clusters with lossy text
    (`crates/patina-engine/src/library.rs`).
  - The CLI test of `examples/gpu` checks only the build when there is no adapter
    (`crates/patina-cli/tests/run.rs`).

### Milestone 8: packaging

- A package is a folder, not one file.
- The notices of the crates of the runner are only in the portable zip. A runner from a
  checkout gives a warning.
- **Open project** on a package opens its original project. A build there writes a `build/`
  folder into the package. The **Package** button and the opening did not run in the window yet.
  It needs `patina-cli` next to the editor, as in the portable folder.
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
- A removed public pin keeps the wires of the uses to it. The check shows a problem for each.
- Ctrl+G collapses the selected nodes into a new subgraph, with public pins for the wires that
  cross. Save writes its file while a graph uses it. An undo keeps it for a redo. Ctrl+Shift+G expands
  the selected uses.
- A subgraph that opens with a double-click on a use shows the live values of that use. A
  subgraph that opens from the **Project** panel shows the live values of its only use (fixed
  2026-11-17). With more than one use, it shows none, because no use is selected. The frame of
  **Show inner nodes** shows the values of one use.
- **Import** and **Update** work only from a trusted project. A renamed folder keeps the comments
  and the key order of its `Cargo.toml` (fixed 2026-11-17, before they were lost). The import
  test checks a comment. A temporary test with the real toolchain on Windows imported a node and
  built it, put back an update that did not compile, and built a good update. The test is not
  in the repository. The two dialogs did not run in the window yet.

### Milestone 6: layers

- The rpath on Linux and macOS ran only in CI. Nobody compared the bytes of two builds yet.
- The state move depends on a hypothesis: rustc gives the same layout to the same type with the
  same compiler and cfg values, also at another optimization level (7.2). The test
  `a_feature_change_gives_a_new_state_key` checks that a feature change of a dependency resets
  the states. No build test moves a state that holds a type of such a dependency.
- There is one cluster for each clock domain. No cluster boundary is at a large node yet.
- The reader cluster depends on the crates of the producer and of the steps before it, so a
  change of a batch node builds the frame cluster again too.
- Nobody ran the state move in the editor. The editor calls the same engine code as the test.
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
  handle with a key, reads the axis labels, the scatter dots and the curve histogram, clicks an icon button and reads the progress bar, adds a ramp stop with a click, removes it with Delete, sets the ramp blend, hides a layer and moves it with the down arrow, types a file path, picks a project file in the asset picker, clicks the canvas, shows the large size in the **Inspector**, and opens the full size and closes it with Esc. It drags a curve point, a trim handle, a ramp stop, a layer and a canvas with page pointer events, not with the real mouse.
  Chromium ignores the pointer capture of these events, so the drags do not check the capture. A
  layer drag down could lose its capture, because React moves the row of the handle. The list
  now holds the capture (fixed 2026-11-17 from the source, not tried with a real mouse). No
  check picks a color or clicks **Browse** in the editor. The dialog of the platform needs a person.
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
- Only stateful nodes can have a GUI. The audio `ui` with `#[settings]` comes with the audio
  domain (milestone 11).
- The **Widgets** group of the **Project** tab makes widget folders with a start widget. **Add
  crate** with a widget name adds the widget by its path. A manual check built a node that shows
  a widget folder in `patina-cli`. The editor self-check makes a widget folder, but does not
  build a node that shows it.
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
  its type. The slot test runs it in the slot. No check runs it with an async reader.
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
- The frame loop of the editor allocates about 12 times for each shown frame on the computer
  example, in the frame of the engine part. The list of changed values uses its buffer again
  (fixed 2026-11-17). The CLI test graph gives 0. The `ui` functions of the computer nodes make
  their labels with `format!` and `collect`, for example `ram`, `readout` and `analyzer`. So the
  12 are in the node code. This is from the source, not from a measurement. The frames of the
  CLI do not allocate. Nobody measured the editor again on the computer example.
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

- **Add crate** has no search of crates.io and no list of versions. The self-check does not run
  `cargo add`, because CI must not need crates.io for the UI check.
- The self-check does not click **Install toolchain**, because CI must not download the
  toolchain. The self-check does not click **Remove**. The data test covers the removal.
- No check opened the folder dialog, the fatal message box or the WebView2 message. A person
  must click them. The message of a missing WebView2 Runtime did not run.
- The check drags one node with mouse events of the page, not with the real mouse. A person
  did not use the editor yet.
  The owner check: make the first workflow of 15.10 with the mouse.
- The capture works only on Windows. The check did not run on Linux or macOS.
- rust-analyzer gives completion and hover in the **Code** tab (15.1, added 2026-11-17). A Rust
  unit test checks the message frames. A frontend unit test checks the conversion of the results.
  A script sent the messages of the client to rust-analyzer 1.100 on a copy of `examples/hello`.
  Completion, resolve and hover gave correct results. The editor did not run with rust-analyzer,
  because the selfcheck toolchain has none. Nobody typed in the **Code** tab with it.
- A view opens as a tab in the center, after the fixed **Graph** tab (15.2, added 2026-11-17).
  The active view covers the graph, so the graph keeps its state. A layout unit test checks the
  center. The selfcheck checks that a view opens in the center and that **Graph** shows the
  graph again. Nobody dragged a view tab into the center by hand. A layout file from before
  keeps its view tabs in the right area.
- The **Debug** tab shows a timeline of the node calls of each domain and thread (15.7, added
  2026-11-17). The load, CLI run, frontend and selfcheck tests check the start and the thread.
  A pool unit test sees a task on a worker give its pool index above 0. No graph check showed
  such a call, and nobody looked at the tab by hand.
- The palette of a drag from a pin compares the type text with no module paths (15.2). A type
  alias does not match its type, and two types of one name in two modules match.
- `stop_requested()` is false on a thread that a node starts. The limits of milestone 12 tell how
  the waits of async nodes (4.6) stop.
- The self-check does not run a node with the `log` crate, because that needs crates.io. The
  type check covers the log lines of the page, but no check showed them in the editor yet.
- The `Viewer` gives text, a thumbnail of 64 x 64 pixels at most, and the widgets of a node GUI
  (5.3, fixed 2026-11-17). `Viewer::show` gives the plot, the waveform and the table with the
  `Ui` widgets. The **Inspector** reads the full text and the widgets one time after each run.
  The glue, load, sink and frame tests check the widget lines. The cluster makes the widget lines
  of each probed value at each run, also when no **Inspector** shows them. No example type has
  widgets yet, so no selfcheck or hand test showed them in the **Inspector**.
- The node GUIs, the row text, the pins and the bus lanes draw down to a zoom of 25 percent
  (2026-11-17). All change to the simple form at the same zoom.
- **Lock bends** keeps at most 12 bends of a longer route. Between two kept bends, the router
  can then take a different path.
- The automatic route and **Arrange** have no undo of a route change, because a route is not
  saved. The route changes when a node moves, and the wires of a large graph can jump.
- The recovery file does not have the typing of the last 300 ms before a crash. The check did
  not crash Patina, and it did not click **Discard**. A project that is not trusted keeps its
  old recovery file, and its new changes do not go into the file until the user answers.
- A connection kind matches a type by the type text that the node code writes, not by the type
  crates of 5.1. A panic in a kind gives no lanes. Its message shows only in the **Output**
  panel, from the standard error (4.7). A compile error in a kind
  stops the build of all of the graph.
- The lanes have no groups. The lane hover works only on the WebGL wires, at a zoom of 0.25 or
  more. The badge is always the viewer text.
- The self-check makes a connection kind with **New connection**, but no value of the self-check
  uses it. The lanes ran in the CLI test, and the look
  had one manual check with fake lanes in the page. No check zoomed a bus out below 0.25.
- The check did not drag a tab or a splitter with the mouse.
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
  2026-11-16). The plan test reads four manifests. No build ran with such a node.
- The editor has no bench with 500 nodes, so the far zoom form was not measured for frame
  drops. After each frame, the engine copies each changed value one time into the kept values,
  and makes a new badge text for the run event (fixed 2026-11-17).
- The segments at the two pins of an orthogonal wire cannot move, and a wire with the graph
  default style has no segment bars (15.9). The **Default route** of the **Inspector** sets the
  graph default style (fixed 2026-11-17).
  **Bus** and the crossing bridges are not made. A wire with a fixed style can go under a node.
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

- `crates/patina-cli/tests/run.rs:475`: The CLI test of `examples/gpu` checks only the build when no adapter exists. Add an adapter to CI when one is available.
- `crates/patina-editor/frontend/src/wires/gl.ts:266`: One texture row holds each wire. Above `MAX_TEXTURE_SIZE` wires, SVG draws them. Pack the rows for larger graphs.
- `crates/patina-editor/frontend/src/canvas/palette.ts:37`: The palette drops module paths, so two types of one name match. Rustc checks the types at build time.
- `crates/patina-editor/frontend/src/wires/route.ts:36`: A graph with a larger route grid gets simple orthogonal paths. Use a sparser grid when such graphs matter.
- `crates/patina-editor/frontend/src/wires/WireEdge.tsx:149`: The SVG fallback draws a bus as one line. Draw its lanes when a GPU without WebGL2 matters.
- `crates/patina-editor/frontend/src/wires/WireEdge.tsx:150`: The SVG fallback has no crossing gaps. Add a mask when the fallback is more than a lost context.
- `crates/patina-editor/frontend/src/wires/wires.ts:359`: A path with more than `MAX_POINTS` - 2 bends draws only its first bends. Split the row when this matters.
- `crates/patina-editor/frontend/src/wires/wires.ts:561`: A new lane state builds the instances of all wires again. Put the lane states in a texture for larger graphs.
- `crates/patina-editor/src/engine.rs:518`: No event runs when the slot holds a value that is not `Sync`, an event node and a node of another domain. Give event nodes their own slot library.
- `crates/patina-editor/src/engine.rs:614`: When a library is not parallel, the blocks run between the frames, so a slow frame makes them late. Give such a graph its own thread.
- `crates/patina-editor/src/session/check.rs:47`: Screenshots work only on Windows. Add a path for Linux when the check runs there.
- `crates/patina-engine/src/device.rs:186`: Only Windows raises the audio thread priority. Add it for Linux when a user there hears gaps.
- `crates/patina-engine/src/events/watch.rs:36`: `file_change` polls its folder. Use the change events of the system when folders get large.
- `crates/patina-engine/src/generate.rs:21`: Each domain has one cluster, with no boundary at large nodes. Split at large nodes and subgraphs when a measurement shows the need.
- `crates/patina-engine/src/library.rs:73`: A project root that is not UTF-8 changes, so its assets are not found. Pass the bytes of the system when a user needs it.
- `crates/patina-engine/src/load.rs:386`: A node with no cluster or index loses its state. Only a defective plan causes this, so no change is needed.
- `crates/patina-engine/src/node.rs:130`: Code that nests deeper than the 64 MB stack allows still overflows. Parse in a child process if a real project needs it.
- `crates/patina-event/src/lib.rs:20`: Each cluster build makes its own runtime. Share one runtime when many builds load.
- `crates/patina-event/src/lib.rs:75`: The run thread blocks while it waits. Change this when event runs share the worker threads.
- `crates/patina-gpu/src/viewport.rs:160`: A viewport surface works only on Windows. Add the X11 and Wayland handles when the editor needs them.
- `prototypes/connections/src/bench.rs:167`: The path comes from the source tree, so the bench runs only from a checkout.
- `prototypes/connections/src/bench.rs:236`: CPU and GPU use are measured on Windows only. Add `/proc` sampling for Linux when needed.
- `prototypes/connections/src/feed.rs:62`: Each send allocates, because Tauri takes the frame by value. A pooled path needs shared memory.
- `prototypes/connections/src/main.rs:91`: Screenshots work only on Windows. Add a path for Linux when the bench runs there.
- `prototypes/connections/src/main.rs:116`: The page is read from the source tree. Embed it when the editor ships.
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
