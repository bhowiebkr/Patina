# Status

All milestones of the roadmap (spec section 16) ended. Milestone 12, the event domain, ended on
2026-11-16. The current work is the open items of the known limits and of
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

## Process isolation

Process isolation (spec 16.1) works in the editor since 2026-11-16. The WASM sandbox (16.2) is
specified only: it needs a download of wasmtime that the owner did not approve yet.

### What works

- The editor runs the engine in a node process: the same `patina` executable with the hidden
  argument `--node-process`. The messages are lines of JSON on its standard input and output.
- The output of the nodes goes to **Output** as messages. The standard error of the node process
  goes to **Output** directly, and the editor keeps its last 1000 bytes for a crash.
- A crash of a node ends only the node process. **Output**, the Logs and the status bar show the
  exit code and the last error text. On Windows, a crash handler adds the node that ran.
- The next run or edit of a live graph starts a new node process.
- After "Node X does not stop", a second **Stop** ends the node process.
- The views, the node GUIs, the triggers, the values and the package job cross the pipe.

### What was checked

- `cargo test -p patina-editor`: the messages cross a pipe, and the error tail keeps the last
  text.
- The hidden selfcheck on Windows: a node writes to a bad address. The editor shows the exit code
  `0xc0000005` and the node, and the undo runs in a new node process. A node that does not read
  the stop request ends at the second **Stop**, and the next **Run** works.
- Linux and macOS did not run.

### Known limits

- The crash handler is only on Windows, and it names only the node of the engine thread. A Rust
  abort and a stack overflow skip it.
- A native viewport across the two processes is not checked.
- The editor and the node process both check the graph of each run.
- A crash outside a run shows on the serial of the last run or open.

## Circuit graphs

The model is synchronous: one frame is one clock step, and a delay node is a D flip-flop (spec
18, question 13). The model needs no engine change.

### Plan

1. Done: `examples/computer`. The gates, the adders and a 4-bit counter are subgraphs of one
   `nand` node. A `readout` node shows 4 bits as lamps in its GUI.
2. Done: a multiplexer and a 4-bit register with a load input. An accumulator adds 5 to its
   register at each step. The **Inspector** shows a check box for a `bool` parameter.
3. Done: a memory of 4 words of 4 bits, with a decoder of the address. It writes the
   accumulator at each step, and readout D shows the word that it wrote 4 steps before.
4. Done: a CPU of NAND gates and delays. It runs one instruction at each step from a ROM of 8
   words of 7 bits. Each ROM bit is a switch. The 8 instructions are LDI, ADD, OUT, JMP, JC, ST,
   LD and JZ. JC uses a carry flag, JZ a zero flag, and ST and LD use the RAM of step 3. The CPU has no
   instruction register, because each instruction takes one step.
5. Done: look at the circuit in the editor. A 1-bit wire is lit when it is true. A subgraph
   that opens with a double-click on a use shows the live values of that use, also for a use
   inside a use. The generator stacks the nodes by their pin count, so they do not overlap.
6. Done: the generator puts each node in a column by its signal depth. An unwired `bool`
   input shows a check box in the node row, so a click sets a switch (4.8).
7. Done: a `display` node shows 7 segments. The `hex-7seg` subgraph of NAND gates decodes the
   count to the segments of its hex digit.
8. Done: a second display shows the OUT register of the CPU.
9. Done: a zero flag and a JZ instruction, with opcode 7. The new program uses all 8 opcodes.
10. Done: the **Test** button of an open subgraph runs it alone. Switches toggle its `bool`
    inputs and lamps show its outputs. The **Graphs** group of the **Project** panel lists the
    subgraphs. A click opens one, and a drag adds a use.
11. Done: an `lcd` node shows 2 lines of 16 characters. The CPU writes the character '0' plus
    OUT at each step. A node GUI keeps its size: a canvas sets the node width, a label clips on
    one line, and nothing draws outside the node (4.3).
12. Done: the **Step** button runs one frame of the paused graph, as the clock step of a
    breadboard computer. The start screen network is blurred and fills large screens.
13. Done: an 8-bit data path. A, OUT and the RAM words have 8 bits, the PC has 4 bits and the
    ROM keeps 16 instructions. ADD adds the RAM word at address k. The program writes the
    Fibonacci numbers to OUT, and a third display shows the high hex digit of OUT.
14. Done: after each OUT instruction, the `lcd-writer` subgraph writes the two hex digits of OUT
    and a space to the LCD. The `hex-ascii` subgraph gives the ASCII code of a hex digit. An LED
    bar node is not necessary, because a readout shows 8 lamps.
15. Done: a `clock` node gives the ticks of the CPU, one tick each **every** frames. **halt**
    stops it, and its **Step** button then gives one tick. Each load, write and PC change of the
    CPU is "and clk". The example ticks each 4 frames. The LED is lit for the first half of
    each period, so a slow clock blinks visibly.
16. Done: an `analyzer` node draws the last 96 frames of 8 signals as waveforms. In the
    example, it shows the clock, the strobe, the PC bits, A0 and OUT0.
17. Done: byte buses. A `join` node puts 8 bits into a byte, and the `byte` connection kind
    draws a byte wire as a ribbon of 8 lanes. The 4-bit subgraphs, `register-8` and the `cpu`
    give byte outputs, so each readout gets one ribbon in place of 4 or 8 wires.
18. Done: byte ribbons in the data path of the `cpu`. `register-8`, `ram-4x8`, `adder-8` and
    `mux-8` have byte pins, with `split` and `join` nodes inside. One ribbon replaces each group
    of 8 bit wires, so the `cpu` graph has 103 wires in place of 161.
19. Done: node header colors (15.3). `#[node(color = "#rrggbb")]` colors the header, tints the
    border and gives the selection a lighter shade. The **Project** panel shows a swatch. The
    example colors its gates, inputs, outputs and bus nodes by the families of the GUI guide.
20. Done: readable layouts (15.9). The default wire is now the automatic route: it goes around
    the nodes, the wires of one output share a trunk, and parallel wires keep a gap. Route
    points are waypoints. **Shift+L** arranges the nodes in columns by the signal flow, and
    the example graphs are arranged.
21. Done: lane identity (15.9). `#[connection(lanes = [...], label = "...", color = "...")]`
    gives each lane a name and the bus a color family. A pin of such a kind is a bus socket: it
    shows the pin name, the lane range, and a contact with an LED and a name for each lane. The
    leads fan in from the contacts to the lanes of the ribbon. A pointer on a lane or a contact
    marks the lane along the wire and at the two sockets, with a tooltip such as
    `ctrl[15] HLT = 1`. The **Inspector** gives a wire a **Name**. The example `byte` kind is
    green. A public pin of such a kind in an open subgraph is a bus socket too. A short S-bend of a ribbon is two flat arcs, so no lane folds back.
22. Done: a segment bus with named lanes. The `segments` type crate keeps segments a to g in
    one value. It is the first type crate of the example (5.1). The `segment-bus` kind draws it
    as a copper ribbon of 7 lanes with the names a to g. A `seg-join` node in `hex-7seg` joins
    the segments, so each display gets one bus in place of 7 wires.
23. Done: wire names on one line. A named wire of one line shows its name in place of the
    type label. In the `cpu` graph, the 8 opcode lines of the decoder have the names `LDI` to
    `JZ`.
24. Done: connection work in the editor (15.2). The **Project** panel has the groups
    **Connections** and **Types**, each with a **+** button and a start crate. A click opens
    the code in the **Inspector**. **Add crate** adds a type folder by its path.
25. Done: each example has a `README.md` with images from `patina --shot`. The `computer` and
    `gpu` READMEs are first versions.
26. Done: a value badge cuts its text to the room between the nodes, as the label does, but
    keeps at least 32 px. Before, a long badge on a short wire covered the pin names. The
    `hello` and `gpu` graphs now have the column gap of **Arrange**.
27. Done: a wide main graph. The CPU, its displays and the LCD are at the left. The counter and
    the adder are at the upper right, and the accumulator and the RAM at the lower right. The
    CPU bits go straight to the `lcd-writer`, and the `lcd` takes d0 first. A badge hides when
    the badge of another wire covers it. A delay output shows the type of its input, so a
    wire from a register shows `bool` and not `T`.

### Checked

- On Windows 11 with Rust 1.100.0: the CLI test runs `examples/computer` for 560 frames. Readout
  A shows the count modulo 16, and readout B the count plus 3, modulo 16. Readout C shows the
  accumulator: 0, 5, 10, 15, 4 and so on. Readout D shows the memory: 0 for 4 steps, then the
  accumulator of 4 steps before. Readouts E, F and G show the PC, A and OUT of the 8-bit CPU.
  The test compares them with a small emulator of the 8 instructions, which runs one
  instruction each 4 frames, as the clock does. OUT shows 1, 1, 2, 3, 5 to
  233, then 0, 1, 1 after the carry. The first display gives the
  segments of the 16 hex digits. The second and third displays give the two hex digits of OUT.
  The LCD shows the last OUT values as hex digits, as the emulator gives them.
- The 8-bit CPU has 1751 NAND gates. A clean build of its clusters took about 150 s, because
  each node call was a closure, and rustc checks all closures of one function together. Now
  the cluster calls each node through `glue::run` with a tuple of its arguments (4.1, 4.7). The
  same build took 43 s, with the SDK. `patina-cli run` gave the 1400 values of the emulator.
- On Linux and macOS, rustc crashed in the DWARF debug info of the large frame cluster
  (CI run 43803351311). The clusters now build with no debug info in edit mode (8.3). The
  frame cluster rebuild of the CPU went from 22.2 s to 19.9 s on Windows.
- The type check of the CPU frame cluster took 12.4 of 17.8 s, because each shared value had
  an open type. Now dead code gives each value its type where the cluster makes it (8.2). The
  clean build of the frame cluster took 8.5 s, with a type check of 2.4 s. A `patina-cli`
  build of the CPU after an SDK change took 11.5 s, against 19.9 s before. The 1400 values
  still agree with the emulator.
- The editor self-check clicks the switch of a bool input in a node and the check box in the
  **Inspector**. The live graph runs again after each click.
- `patina --shot <project> <folder>` runs a project in a hidden window, and writes a
  screenshot of the main graph and of each subgraph. It opens each subgraph through its first
  use. On `examples/computer`, it wrote 18 screenshots in about 25 s. They show the lit wires
  in the subgraphs and the lamps of the readouts at a near zoom.
- A unit test runs one use of an open subgraph alone with the input parameters. The editor
  self-check moves a node into a subgraph, adds a `bool` input, clicks **Test** and clicks the
  switch of the input. The output of the subgraph shows the new value.
- The package tool keeps `data/` of `target/package/Patina`, so the recent projects stay. The
  tool did not run after this change.
- The readout lamps and the pin names hide at a far zoom, below 50 percent (15.1).
- A `patina --shot` screenshot shows the check box in the row of each `switch` node. The editor
  self-check clicks a switch of the same kind in a node row.
- A `patina --shot` screenshot shows the `clock` node and the waveforms of the `analyzer`
  node: one clock pulse each 4 frames, and PC0 toggles at each pulse. A unit test checks the
  samples of the analyzer. No test checks its drawing.
- A `patina --shot` screenshot shows the byte wires of the `cpu` outputs **pc**, **a** and
  **out** as ribbons of lanes, with the label `bus[7:0] / 8`. The CLI test gives the same
  readout values with the byte wires. A unit test checks the bit order of `join`.
- With the byte ribbons in the `cpu` data path, `patina-cli` ran the example for 560 frames.
  All 5600 readout values and the LCD text are the same as before. A unit test checks the bit
  order of `split`.
- With the node header colors, `patina-cli` ran the example for 560 frames. All 5600 readout
  values and the LCD text are the same. A `patina --shot` screenshot shows the colored headers
  and the swatches in the **Project** panel. Unit tests check the color attribute in the engine
  and the macro. No test checks the colors on the screen.
- Unit tests check the automatic route: it goes around a node, parallel wires keep a gap, the
  wires of one output share a trunk, and a route goes through its route points. Unit tests check
  **Arrange**: the columns go from left to right, the nodes of a column do not overlap, and a
  gap gets a lane only for a wire with a bend. A `patina --shot` run with **Shift+L** arranged
  the 25 graphs of `examples/computer`. In Node, a test graph of 217 nodes and 210 wires took
  5 ms to arrange and 27 ms to route. A graph of 400 nodes and 500 wires took 84 ms to route. The
  editor self-check drags a node, and the wire follows it. No check measured the route time in
  the window.

## Milestone 12: the event domain

Milestone 12 started and ended on 2026-11-16. It adds the event domain: async nodes and trigger nodes
(spec 4.6 and 9.4).

### Plan

1. Done on 2026-11-16: the engine and the CLI: `Domain::Event`, plain async nodes, the
   built-in triggers `schedule` and `trigger`, a run thread for each event run, and
   `patina-cli run --events <count>`.
2. Done on 2026-11-16: the triggers `file_change` and `webhook`.
3. Done on 2026-11-16: the editor runs the event domain with **Run**, **Pause** and **Stop**.
   The **Fire** button of a `trigger` node fires it. The selfcheck has a step for it.
4. Done on 2026-11-16: a view fires a `trigger` node with `Graph::fire`. The limit of a code
   swap during a run is in the known limits.
5. Done on 2026-11-16: the editor shows and edits order wires (9.2), so an async node with no
   inputs can get its trigger in the editor.

### Checked

- On Windows 11 with Rust 1.100.0: the CLI test `events` adds the async node `wait` and the
  node `say` to a copy of the hello example. A `schedule` node gives the run ID to wait, and
  say shows the value of wait. Say has no domain, so it runs in the event domain.
- With `overlap = both`, runs 2 and 3 started while run 1 waited 1 s. With `skip`, each run
  was alone. With `queue`, the trigger dropped a fire at 16 runs and reported it, and the 17
  runs came in order.
- The same test checks a bad `every_ms` (a bad parameter before a run), a `trigger` node that
  fires one time with an empty text, the **Run 1:** start of a report, and the graph error of
  an event node with no trigger.
- A unit test checks the settings of a trigger. A plan test checks the trigger of each event
  step.
- An engine test fires a `trigger` node through a remote, and gets one run.
  A fire with a wrong ID gives a report. After the end, no new run starts.
- The editor selfcheck makes the number node an event node with a `trigger` node in front of
  it. **Run** starts no event run. A click on **Fire** gave run 1, which printed 2.5. A fire
  with the text `abc` gave run 2, which printed 5.5. After **Stop**, a fire failed.
- An editor unit test checks that an event run replaces only the reports of the nodes that it
  reached.
- The selfcheck view has a **Fire** button that calls `graph.fire` with the text `abcd`. In a
  live run, a click gave an event run that printed 6.5. Unit tests check the fire line of the
  SDK and of the engine.
- The same CLI test changes a file in a watched folder two times, and gets two runs with the
  path of the file. A `file_change` node with no path gives an error before a run.
- The same CLI test sends a `POST` to a wrong path, which got 404, then a `POST` with the body
  `hello`, which got 202 and a run with `hello`. A port in use gives an error before a run.
- Unit tests check the scan of a file and of a folder, and the status code of each bad request.
- An engine unit test checks the order-wire edits: a second wire, a wire to the same node, a
  missing node, a cycle with its node names, a bad removal, undo, and the removal of a node.
- The editor selfcheck drags from the order socket of one node to the order socket of another.
  The order wire showed, Delete removed it, and two undos removed it again. 88 steps passed.
- A screenshot of the hello example with one order wire showed the dotted line between the
  two hollow diamonds.
- In a subgraph that opened through a use, **Fire** sends `<use ID>/<node ID>`, the ID of the
  flat graph. An engine test checks these IDs. A selfcheck step moves a `trigger` node into a
  subgraph, opens the subgraph with a double-click on the use, and clicks **Fire**. The event
  run has the trigger `<use ID>/<node ID>`.
- The test machine has no network for the tests, so the local test run used
  `CARGO_NET_OFFLINE=true`. The `tokio` crate came from the cargo cache.

### Known limits

- These shortcuts are open in the code:
  - `events.rs` of `patina-engine`: one thread for each run.
  - `patina-event`: each cluster build makes its own runtime, and each wait of an async node
    allocates its future and blocks the thread of its run.
  - `main.rs` of `patina-cli`: the event runs come before the frames and the blocks.
  - `checked.rs` of `patina-engine`: an event node does not go into the slot (7.1), so an edit
    of an event node rebuilds its cluster.
  - `events/watch.rs` of `patina-engine`: `file_change` polls its folder.
  - `events/webhook.rs` of `patina-engine`: `webhook` reads one request at a time.
  - `engine.rs` of `patina-editor`: no event runs when the slot or the audio domain keeps a
    value that is not `Sync`.
- `patina-capi` does not run the event domain.
- In the editor, a change, **Pause** and a failed frame wait for the active event runs. A code
  swap does not keep the active runs on the old builds (9.4).
- While the event domain runs, a view reads no output of an event node, and the GUI of an event
  node does not draw.
- The first build of a project with an async node needs `tokio` from crates.io or the cargo
  cache.

## Milestone 11: the audio domain

Milestone 11 started on 2026-11-15. It adds the audio domain: nodes that run on an audio thread
at each block of samples (spec 9.3).

### Plan

1. Done: the audio domain without a device: `domain = audio`, `AudioBlock` from a pool, and
   latest-value wires without a lock. The CLI runs the blocks on a timer, between the frames.
2. Done: audio output to the default device with `cpal`. A node plays a block with
   `patina_sdk::audio::play`. The timer stays when there is no device.
3. Deferred by the owner on 2026-11-15: node GUIs of audio nodes (4.3).
4. Done on 2026-11-16: the audio thread, apart from the frames (`Cluster::split`). The node GUIs
   of audio nodes are deferred (step 3), so the milestone check uses a slow frame node: it does
   not make a block late.
5. Done on 2026-11-16: the editor runs the audio domain with the frames. **Stop** stops it.
   The editor splits the cluster as the CLI does. The engine thread runs the frames, the node
   GUIs and the views with the frame part (`Cluster::frame_part`). The device code moved from
   `patina-cli` to `patina-engine`.

Milestone 11 ended on 2026-11-16 without step 3. Audio nodes have no GUI yet, so the milestone
check used a slow `ui` function of a frame node while the audio thread ran.

### Checked

- On Windows 11 with Rust 1.100.0: the CLI test adds the audio node `tone` and the frame node
  `meter` to a copy of the hello example. Tone reads a frame value, and meter reads the block of
  tone. With `--frames 3 --blocks 9`, the frames and the blocks run on two threads. Frame 0
  starts before tone has a value, so meter skips it. Meter shows only the tones 15 and 19.
- The same test adds the audio node `hold`, which keeps an `Rc`. The audio library is then not
  parallel, so `Cluster::split` gives no split and the CLI runs the blocks between the frames.
  Meter then shows the tones of frames 0 and 1, in a fixed order.
- The same test runs `--blocks 9` without `--frames`. The frames stop when the blocks are done.
- The same test splits the cluster. An audio thread runs blocks while the main thread runs one
  frame, in which meter sleeps 20 ms. Blocks ran during the frame.
- The same test calls the `ui` function of meter on the frame part during the split. Meter
  sleeps 20 ms in its `ui` function, and blocks ran during the call. The audio thread takes no
  lock of the frame part. The editor calls the same `Frames::ui`.
- Unit tests check the buffer of three values, the pool of `AudioBlock`, the order of the
  frames and the blocks, and the domain of a node with a frame input and the audio domain.
- The witness block now builds with a delay before an import. A delay gets the type of its
  input at the end of the block.
- The CLI test adds the audio node `speaker`, which plays tone. After a frame and a block, the
  engine reads the mix with the sample of tone. Unit tests check the mix and the device queue.
- On Windows 11, a manual run of a node that plays silence opened the default device. 1500
  blocks took 8.009 s on the device clock (8.000 s expected). This run was silent.
- CI does not open a device: no test graph plays in the CLI.
- On Windows 11, the editor selfcheck makes the node of the hello example an audio node and
  runs it. The cluster splits, so the blocks run on the audio thread. The blocks print with the
  frames, the status bar shows the frame number, and no block
  or frame runs after **Stop**. The node does not play, so no device opened.
- A unit test of the editor checks that the blocks since a frame keep all problems and logs and
  the last value of each output.
- The audio thread of the editor gives its results to the frames in lists that keep their old
  items and texts. The frames swap their emptied lists with the lists of the audio thread. A
  unit test counted 0 allocations in 100 rounds of two blocks, a hand-over and a swap, after two
  first rounds (2026-11-16).

### Known limits

- These shortcuts are open in the code:
  - `device.rs` of `patina-engine`: a fixed wait of 50 ms for the buffer of the device at the end.
  - `engine.rs` of `patina-editor`: when a library is not parallel, the blocks run on the engine
    thread between the frames, so a slow frame or node GUI makes them late.
- While the audio thread of the editor runs, a view reads no output of an audio node, and the
  GUI of an audio node does not draw. The badges of the audio outputs still show.
- The audio thread has its own `Control`. So the editor does not show an audio node as the node
  that runs, and **Stop** stops the audio thread only at the end of a block (15.2).
- `patina-capi` does not run audio blocks yet.
- No check played a sound in the editor. The editor uses the same device code as the CLI.
- When an audio node keeps a value that is not `Sync`, the blocks run on the thread of the
  frames. A slow frame then makes the blocks late. The device queue of 43 ms hides a short delay
  only.
- The audio thread has the normal priority of the operating system, not a real-time priority.
- The device must take 48 kHz f32 samples. There is no resampler. On Windows, cpal 0.17 lists
  each rate in f32 for the default device, because it opens the stream with AUTOCONVERTPCM: a check
  on 2026-11-16 found 48 kHz f32 on a Windows 11 computer. A device on Linux or macOS
  without 48 kHz f32 plays no sound.
- Linux needs the ALSA library. The device output was not run on Linux or macOS.

## Milestone 10: views

Milestone 10 started on 2026-11-15 and ended on 2026-11-15. It adds views: custom panels of the editor that read and
change the graph (spec 15.8).

### Plan

1. Version 1 of views: the `#[view]` ABI, the `Graph` handle, the build of a view and the
   **View** panel. This part works.
2. More instances of a view, and float windows. This part works. The self-check floats a view,
   waits for its window and docks it again. Nobody looked at a float window on screen yet.
3. More edits from a view: add a node, connect a wire, start a trigger and select nodes. Add,
   remove, connect, disconnect and select work. Trigger waits for the event domain of
   milestone 12 (9.4).
4. Typed outputs through type crates (15.8 rule 3). This part works for outputs with a wire.
5. A viewport in a view (15.8), on the native path of the viewport prototype. Version 1
   works on Windows. A render thread and more viewports in a window are limits of version 1.

### What works

- `#[view]` compiles a struct with `init` and `ui` into a view library.
- `patina_engine::view::View` loads the library and calls `ui` with the graph lines. A panic
  frees the state, and the next call makes a new state.
- `Project::build_view` builds `views/<name>` in edit mode into `build/loaded`.
- The editor lists the views in the **Project** panel and in the **Views** menu. **New view**
  makes a view folder. The **View** panel shows the widgets of the open view.
- A view reads the wires. It sets parameters, adds and removes nodes, and connects and
  disconnects wires. The writes of one call go through the edit path as one step of undo.
- A view reads the selected nodes and selects nodes in the graph view. It reads the node
  errors of the last run or frame.
- A save of the code of a view builds only the view again.
- `Ui::viewport` and `patina_gpu::ViewportSurface` draw a viewport into the editor window under
  the transparent page, on Windows. The `scene` view of `examples/gpu` colors its viewport with
  the pointer and the wheel.

### What was checked

- `cargo test --workspace` on Windows: the test
  `a_view_reads_the_graph_and_sets_a_parameter` builds a view, reads the graph, clicks a button
  and gets the edit. A second instance has its own state. Then it reads the sum of a run as an
  `f64`: none at the first read, and 6.5 after the next run.
- `cargo test --workspace` on Windows: `the_gpu_example_draws_a_gradient_with_a_wesl_shader`
  builds the `scene` view and reads its viewport line and the wheel. The sdk and gpu tests read
  the place events and skip a draw before the place.
- The editor self-check on Windows: it makes a view, opens it from the **Views** menu, clicks
  **Set**, sees the new value and undoes the write. Then it opens a second instance, **panel 2**.
- `npm test`: the layout opens view instances in free panels, keeps the view of a moved panel and
  drops the view of a closed panel.
- CI builds and tests on Windows, Linux and macOS.

### Known limits

- The limits of version 1 are in spec 15.8: four instances, typed values only of wired outputs,
  no trigger, one viewport on Windows only, and no bake build.
- Nobody looked at a viewport in the editor yet. The self-check keeps its window hidden, so it
  cannot show the surface. The colors, the resize and the scroll clip need a check on screen.
- A viewport draws in the `ui` call on the engine thread, so a slow run delays its frames.
- On 2026-11-15, the in-process run of `the_gpu_example_draws_a_gradient_with_a_wesl_shader`
  failed one time in a parallel test run. Four more parallel runs passed. A second failure gave
  the cause: **Patina could not read the image from the GPU**. Hypothesis: a poll on another
  thread ran the map callback late. The read now polls until the callback ran. Watch for a
  third failure.
- The first open of a view builds it on the engine thread. A frame run waits for this build.
- The view did not run on Linux or macOS. Only CI built it there.
- These shortcuts are open in the code:
  - A viewport surface works only on Windows (`crates/patina-gpu/src/viewport.rs`).

## Milestone 9: the GPU

Milestone 9 started on 2026-11-15 and ended on 2026-11-15. It adds the shared GPU context, GPU
types and shader nodes with WESL (spec 10).

### Plan

1. Share one value across the clusters of a project (ABI 13). This part works.
2. The `patina-gpu` crate: the shared `wgpu` device, `GpuImage` and the WESL linker. This part
   works.
3. Shader assets and the project root for each cluster (ABI 14). This part works.
4. A smaller copy of a GPU image for the node GUI (10.2). This part works. The wire viewer
   of the editor shows only the text of an image.
5. CUDA through `cudarc` and NVRTC (10.1). This part works.
6. The WESL package `patina_gpu` with shared image functions (10.3). This part works.

### What works

- `patina-gpu` opens one `wgpu` device for each project. The edit slot and the bake clusters
  share it.
- `patina_gpu::link` links WESL files with imports and `@if` features. `Gpu::shader` gives the
  validation errors as text.
- `GpuImage` makes a texture and reads it back to the CPU.
- `asset!` gives the path of a node asset in a project and in a package.
- `examples/gpu` draws a gradient with a compute shader, and a second node reads its mean.
- `patina-cuda` opens one CUDA context for each project and compiles CUDA C++ with NVRTC.
- `GpuImage::preview` gives a copy that fits in a box. The preview node of `examples/gpu`
  shows it in its GUI.
- Each node shader can import `patina_gpu::image`. The gradient of `examples/gpu` uses
  `inside` from it.
- `patina_gpu::link_source` links WESL text, for example a string from another node. The text
  can import only packages.

### What was checked

- On Windows 11 with the real toolchain, Rust 1.100.0: the CLI test builds `examples/gpu`. It
  gets a mean of 0.336, then 0.664 after the `invert` parameter changes the WESL feature. The
  GUI of the preview node gives an image of 64 x 16 pixels.
- The unit tests of `patina-gpu` check the link with an import and a feature, a bad import, the
  image read-back and a bad shader. They also link an import of the `patina_gpu` package, and
  get the WESL file and line of a type error in an imported file and in the main file. A test
  links WESL text with a package import.
- A glue test checks the asset path with and without a project root.
- On Windows 11 with an RTX 4090 and the CUDA Toolkit 12.6: the unit test of `patina-cuda`
  compiles a kernel, runs it and gets the compile log of a bad kernel. A project in the
  scratch folder with a CUDA node built with the CLI and gave 12. No CLI test covers it, to
  keep the CI time low.
- Linux and macOS ran only in CI. CI on Linux has no GPU adapter, so the test checks only the
  build there.

### Known limits

- A shader error gives the line of the statement in the WESL file (fixed 2026-11-16). An error
  in a function header or in a global gives the line of the declaration. The link tests check
  an imported file and two equal lines.
- Each cluster links its own copy of `wgpu`. This is acceptable because the engine never
  unloads a library.
- A project with GPU nodes needs Rust 1.97.1 or later.
- NVRTC comes with the CUDA Toolkit, not with the driver. A package does not carry it yet.
- CI has no NVIDIA driver, so it checks only the error path of `patina-cuda`.
- The wire viewer of the editor shows only the text of a GPU image. It needs the image
  `Viewer` of spec 5.3, which comes later. A node GUI can show the image now.
- These shortcuts are open in the code:
  - A project root that is not UTF-8 goes to the clusters with lossy text
    (`crates/patina-engine/src/library.rs`).
  - `GpuImage::preview` reads the full image and shrinks it on the CPU
    (`crates/patina-gpu/src/image.rs`).
  - The CLI test of `examples/gpu` checks only the build when there is no adapter
    (`crates/patina-cli/tests/run.rs`).

## Milestone 8: packaging

Milestone 8 started on 2026-11-15 and ended on 2026-11-15. It makes packages of a project
(spec 14). Milestone 8 added no `shortcut:` comments.

### Plan

1. The executable package (14.1). This part works.
2. The **Package** button of the editor. This part works.
3. The editor opens a package and shows the original project. This part works.
4. The library (C API) (14.2). This part works.
5. The Python module. This part waits for the approval of a download.

### What works

- `patina-cli package` builds a project and writes the package folder: the runner, the header,
  the clusters with the Rust runtime, the project source and the license notices.
- The program of the package runs the graph alone and takes `--frames` and parameter arguments.
- A CLI test with the real toolchain on Windows makes a package of `examples/hello`, runs it with
  a parameter and gets `5`. It also checks the notices, that `build/` stays out, and that a bad
  parameter fails.
- A unit test checks the parameter arguments, the notice file names, the version check and the
  wiring check.
- The **Package** button of the **Project** panel saves the project and makes the package on the
  engine thread, with the `patina-cli` next to the editor as the runner.
- The crate `patina-capi` gives the C library `patina_capi` of 14.2. The package build copies it
  and writes `patina_capi.h`. The CLI test opens the package through the C functions, sets a
  parameter, runs the graph and reads the value `5.0`. A unit test checks the error results.
- A manual check on 2026-11-15 loaded `patina_capi.dll` of the test package with Python
  `ctypes`, a program that is not Rust. `patina_set`, `patina_run` and `patina_value` gave 0, 0
  and `5.0`.

### Known limits

- A package is a folder, not one file.
- The notices of the crates of the runner are only in the portable zip. A runner from a
  checkout gives a warning.
- **Open project** on a package opens its original project. A build there writes a `build/`
  folder into the package. The **Package** button and the opening did not run in the window yet.
  It needs `patina-cli` next to the editor, as in the portable folder.
- During a package build, the editor does not run the graph.
- No automated test loads `patina_capi.dll`. The test calls the functions through the Rust
  library of the crate. Only the manual `ctypes` check loaded the DLL, on Windows.
- The Python module is not done. It needs the `pyo3` crate (a download) and a Python install.
- The package ran only on Windows.

## Milestone 7: subgraphs and reuse

Milestone 7 started on 2026-11-15 and ended on 2026-11-15. It adds subgraphs, **Import** and **Update** (spec 6.1, 6.2
and 6.4).

### Plan

1. The engine replaces each use of a subgraph with its nodes before the check (6.4). This part
   works.
2. The editor shows a use as one node, opens a subgraph, collapses a selection into a subgraph
   and expands it, and shows the live values of the public outputs.
3. **Import**: copy the node folders and their dependencies, rename on a name conflict, record
   the import with its hash, and build before the change applies.
4. **Update** of an imported folder.

### What works

- The CLI runs a graph with two uses of one subgraph. Each node of a use gets the ID path
  `<use ID>/<node ID>`. A parameter of a use goes to the inner inputs of its public input pin.
- A subgraph that uses itself and a wire to a pin that the subgraph does not have give problems.
- The engine gives each subgraph to the node list with its public pins and their types. The open
  subgraph gives the pins of its unsaved edits, so **Test** finds a new `bool` input (fixed
  2026-11-16).
- In the Test mode, each public output of the subgraph sends its value, also when no wire
  inside reads it (fixed 2026-11-16).
- The engine opens a subgraph for edits, saves it to its own file and runs the main graph with
  the unsaved changes of the open subgraph. A unit test checks this.
- The engine collapses a selection into a new subgraph and expands a use. Unit tests check the
  public pins, the order wires, the cycle check, the ID collisions and the file.
- A double-click on a use opens the subgraph in the editor. The public pins show as small nodes
  with their wires, and the path bar goes back to the main graph. A change of graph saves first.
- In an open subgraph, the editor adds, moves, connects, renames and removes the public pins.
  Unit tests check the pin edits and an output with no source.
- The **Subgraph interface** of the **Inspector** lists the public pins in pin order, with the
  edit `OrderPin`. A unit test checks the order and its undo. The selfcheck adds, moves, renames
  and undoes a public output.
- The engine plans an **Import** with the type and widget folders that the folder depends on, and
  finds the name conflicts. It copies, renames with the `package` key, replaces and records the
  hash. A failed build check puts the project back. A unit test checks a rename, a replace and
  the undo of each, without a build.
- The **Import** button of the **Project** panel opens a dialog with the folders of the other
  project, the conflicts and a choice for each.
- The engine compares an imported folder with the other project, with the renames of the import,
  and finds the changes of each side with the two record hashes. **Update** writes the new
  version with the picks of the user and puts it back when the build fails. A unit test checks
  a rename, an update, an undo, the picks and a new dependency.
- The import record has a hash for each copied file. **Update** takes the version of the side
  that changed a file, and asks only for the files that the two sides changed (2026-11-16). The
  unit test checks each side, a kept file and a record from before the file hashes.
- The **Imports** list of the **Project** panel opens the **Update** dialog.

### Known limits

- The editor shows a use of a subgraph as one node with the public pins, and **Add node** lists
  the subgraphs. The output pins of a use show the live values of their inner outputs, and a run
  error of an inner node shows on the use.
- In an open subgraph, the inner nodes show no live values and no run errors, because a
  subgraph can have many uses. The frame of **Show inner nodes** shows them for one use.
- A removed public pin keeps the wires of the uses to it. The check shows a problem for each.
- Ctrl+G collapses the selected nodes into a new subgraph file, with public pins for the wires
  that cross. An undo keeps the new file. Ctrl+Shift+G expands the selected uses.
- A public pin that ends at an input of a generic node shows the type parameter as its type.
  The use is generic in that parameter too, so the **Add node** palette offers it for each
  type (fixed 2026-11-16). A pin that ends at `&bool` shows `bool`.
- The engine checks the key of a new public pin. The check of a use gives a problem for a bad
  key in the subgraph file (fixed 2026-11-16).
- **Import** and **Update** work only from a trusted project. A renamed folder loses the comments
  of its `Cargo.toml`. A temporary test with the real toolchain on Windows imported a node and
  built it, put back an update that did not compile, and built a good update. The test is not
  in the repository. The two dialogs did not run in the window yet.

## Milestone 6: layers

Milestone 6 started on 2026-11-15. It adds the base, the slot, fuse back and **Bake** (spec 7).

### Plan

1. A shared `std` and reproducible links. This part works.
2. The state across swaps: the cluster gives the state of each node whose key did not change to
   the next cluster. This part works.
3. More than one cluster in a build, and swaps of all of them or of none. This part works.
4. The slot: the base calls the node in edit mode through a function pointer. This part works.
5. Fuse back, the bake profile and **Bake**, with the decision of the idle time (Q7). This part
   works.
6. The **Layers** panel, the slot icon and the status bar. This part works.
7. The check of the roadmap, with the measured fuse time. This part works.

### What works

- Each build links `std` as a shared library (`-C prefer-dynamic`), so all clusters use one
  copy of `std`. The engine copies the `std` library of the toolchain to `build/loaded/`.
- On the gnu target of Windows, `std` needs `libgcc_s_seh-1.dll` and `libwinpthread-1.dll`. The
  engine copies them from the `bin` folder of the target to `build/loaded/` too.
- The links are reproducible: `/Brepro` with MSVC, and `--no-insert-timestamp` with the
  rust-lld of the own toolchain. On Linux and macOS, a cluster finds `std` in its own folder
  through an rpath.
- rustc makes the import libraries of `std` with dlltool. The dlltool of binutils cannot open a
  path with CJK letters, so the own toolchain uses `zig-dlltool.exe`, a new Zig wrapper.
- Each build gives a state key to each stateful step (7.2). After a build, the new cluster takes
  the state of each node with the same ID and the same key from the old cluster. The editor
  does this between two runs. The C ABI has `patina_cluster_take` and `patina_cluster_put`
  (ABI version 10).
- Each build makes one cluster for each clock domain with a step: `build/clusters/batch` and
  `build/clusters/frame` (8.2). One cargo run builds all of them. The engine loads all
  clusters of a build before it uses one, so a swap changes all clusters or none.
- A frame node that reads a batch value reads it through a link: the batch cluster keeps the
  value in a shared `Arc`, and the engine gives the `Arc` to the frame cluster after the load
  (ABI version 11).
- `Checked::set_slot` puts one node folder in the slot (7.1). Each use of the node calls the
  slot library, `build/clusters/slot`, which keeps the state of each use (ABI version 12).
  `Checked::build_slot` builds and loads the slot library alone, while the base stays loaded.
  It refuses, with false, when the code of the base or the signature of the node changed.
- The editor puts a node in the slot at the first save of a Rust file of its folder. The next
  saves build the slot alone. The node shows a slot icon, and the status bar shows
  **Slot: name**, **Fused** or **Edit build**.
- After 30 s with no change, the editor fuses the live graph in bake mode (Q7). The **Bake**
  button fuses at once.
- The **Layers** tab of the left area shows a **Base** row and a **Slot** row. Each row shows
  its build state and the time of its last build that compiled (7.3). Before each build, the
  engine tells which layers it builds, and only their rows show **Building** (2026-11-16).
- When a newer build waits after a build, the editor drops the older build. It does not run
  it, and its errors do not show (7.2).

### What was checked

- The own toolchain (Rust 1.100.0, x86_64-pc-windows-gnu) built and ran `hello` in a folder
  with a space, an accent and a CJK letter in its name. It gave 6.5. `build/loaded/` had the cluster, `std`, libgcc_s and
  winpthreads.
- With the own toolchain and `-C prefer-dynamic`, the build scripts of `cmix` and
  `libsqlite3-sys` ran, and the fixture of the toolchain prototype built.
- The link flag test checks the flags of each host. The runtime test checks that the engine
  copies `std` and the libraries of `bin` one time.
- The hello test edits mix and builds again: the generic tick node keeps its count 7. Then it
  edits tick and builds again: the count starts at 0. The glue test moves a state from one
  cluster to another.
- The hello test now builds and runs two clusters. Its frame nodes read the batch values
  through links, and the tick state moves into the new frame cluster. The glue test checks
  that a link reads the value of the other cluster and takes one value only.
- The slot test puts a stateful node with a `&` input, a `&str`, a moved input and a `&mut`
  input in the slot. Two runs keep its state. An edit of its body builds the slot alone and
  resets the state. A fused build in bake mode takes the state from the slot library. A change
  of the signature makes `build_slot` refuse. The glue test calls a node in a slot library.
- The slot test draws the node GUI through the slot library. The edited GUI shows the new
  slider name and total, and the fused cluster shows the adopted total (2026-11-16).
- The CI run of the slot change failed one time on Linux: a frame allocated. The time text of
  a slow node had more digits, so the report buffer grew. The time text now has a fixed width.
- The CI run of the slot change also failed on Linux in a test of the cluster folders: the
  order of `read_dir` is different there. The test now sorts the names.
- The roadmap check: a test changes the output type of `number`, with a use in the batch
  cluster and a use in the frame cluster, to f32. It changes the type to i32 while the first
  build runs. After the next build, both uses give the i32 value. It ran on Windows.
- On Windows, with the crates built before, the slot test measured 0.4 s for the build of the
  slot alone and 0.4 s for the fuse in bake mode. The test prints the times with
  `--nocapture`. Nobody measured a fuse with no cache.
- The editor selfcheck saves two compile errors, the second during the build of the first. No
  run shows the errors of the first save. It saves the code of a node and expects the node in
  the slot, with the slot icon. It does not click **Bake** or wait for the fuse. The CLI slot test builds in bake mode.
- The selfcheck then saves a compile error in the node of the slot, and saves the fix. The
  fix builds the slot alone, because a failed slot build keeps the base (2026-11-16).

### Known limits

- A toolchain folder from before this change has no `zig-dlltool.exe`. The editor adds the
  missing wrappers at each start (fixed 2026-11-16). A unit test checks the copy. The editor
  selfcheck started with a toolchain folder that had only `zig.exe`, and the editor added the
  three wrappers. The CLI adds them before each build. A check started a copy of the CLI
  next to such a folder: the CLI added the three wrappers, and the `zig-dlltool.exe` copy ran
  as a wrapper.
- The rpath on Linux and macOS ran only in CI. Nobody compared the bytes of two builds yet.
- The state move depends on a hypothesis: rustc gives the same layout to the same type with the
  same compiler and cfg values, also at another optimization level (7.2). No test checks a state that holds a type of a
  dependency with changed features.
- When a parameter loses its text, `set_params` makes all the states of all clusters again,
  and links them again.
- There is one cluster for each clock domain. No cluster boundary is at a large node yet.
- The reader cluster depends on the crates of the producer and of the steps before it, so a
  change of a batch node builds the frame cluster again too.
- Nobody ran the state move in the editor. The editor calls the same engine code as the test.
- A layout that an older editor saved now opens the **Layers** tab in the left area (fixed
  2026-11-16). The layout file lists the known panels, so a closed panel stays closed. The
  frontend test reads an old file, a file with a short list and a file with a closed panel.
- The fuse builds on the engine thread, so the graph does not run during a fuse
  (`crates/patina-editor/src/engine.rs`).
- A generic node does not go into the slot (`crates/patina-engine/src/lib.rs`). A change of a
  type alias in the node folder needs a full build, because the base compares the signature
  text.
- The modes have no debug assertions, so a `debug_assert!` in a node does not run (8.3).

## Milestone 5: stateful nodes, node GUIs and generic nodes

Milestone 5 ended on 2026-11-15. Stateful nodes (4.2), generic nodes (4.5), the delay node
(9.2) and node GUIs (4.3) work in the engine and in the editor. They ran on Windows 11 Pro,
with Rust 1.94.0. CI builds and tests them on Windows, Linux and macOS.

### What works

- A node crate can have a `#[node]` struct with `pub fn init() -> Self` and
  `pub fn process(&mut self, ...)` in an `impl` block. The pins come from `process`.
- Each use of the node in a graph has its own state. The cluster calls `init` before the first
  call of `process`, and keeps the state until the cluster drops.
- A panic in `init` or `process` drops the state, so the next run calls `init` again (4.7).
  An `Err` of `process` keeps the state.
- The state is in a lock of its own. A state that is `Send` lets the cluster run in parallel.
- A node function or a state struct can have type parameters. The engine gets them from the
  types of the input wires, and finds the connection kind of each output from its real type.
  A parameter that no input wire gives makes an error on the node.
- A stateful node can have `pub fn ui(&mut self, ui: &mut Ui)`. `Cluster::ui` calls it between
  two runs, with the user input as events, and gives the widget lines. The `Ui` has the basic
  widgets (a label, a button, an icon button, a toggle, a slider, a number field with an optional unit and step
  buttons, a text field, a file path field with a **Browse** button, a dropdown, tabs and a
  section that opens and closes)
  and the value widgets (a color picker, a color ramp, a layer stack, an asset picker, a plot with optional axes and units, a scatter plot, a
  waveform with optional trim handles, a table, a curve with an optional histogram, a progress
  bar and an image).
  After the events, `ui` runs one more time, so the widget lines show the new state. A panic in `ui` drops the state, as in `process`.
- A graph can have a feedback loop in the frame domain through a delay node
  (`builtin = "delay"`). The delay gives the value of its input in the previous frame, and
  `Default::default()` in the first frame.
- The editor uses the Aged Dovetail branding (`assets/branding`): the executable and window icon,
  the title accent and the **Help** menu with **About Patina** in the top bar, the splash on the
  start screen, the **About** dialog and the `.msi` bitmaps in `tauri.conf.json`.

### What was checked

- The parser test reads the pins of a stateful node from `process` in a second `impl` block.
  It refuses 10 wrong forms, for example a private struct, no `init`, and `process(&self)`.
- The glue test gives a stateful call an `Err` and then a panic. The `Err` kept the state, and
  the panic dropped it. The two reports were an error and a panic.
- The hello test has a stateful frame node that adds its input to a count at each frame. With
  `--frames 3`, the CLI wrote `6.5`, `7.5`, `8.5` and `9.5`. Thus the state stayed between
  frames. 100 frames with the stateful node made no allocation.
- In the same test, the tick node and the mix node are generic (`Tick<T>` and `mix<T>`). The
  cluster compiled them with `f64` from the wires, and the output was the same.
- The plan test resolves `pick<T>` to `u8` and `Vec<u8>`, and finds the kind of `u8`. It refuses
  two wires that give `u8` and `f64` for `T`, and a `T` that no wire gives.
- The hello test has a loop: mix to tick to a delay, and back to mix. The CLI wrote `6.5`,
  `7.5`, `9.5` and `13.5`, so the delay gave the tick value of the previous frame. 100 frames
  made no allocation.
- The hello test calls the `ui` of the tick node. It got the widget lines. A click event on
  **Reset** set the count to 0. A node with no `ui` gave no lines.
- The editor selfcheck makes the number node stateful, with an **Add 1** button in its `ui`.
  The node body showed the button. A click ran the live graph again without a build, and the
  wire showed `3.5` instead of `2.5`.
- The parser test finds `ui`, and refuses `ui(&self, ...)`, `ui: &Ui` and a private `ui`.
- The `Ui` test reads the events of a button, a toggle and a slider, and clamps the slider.
- The `Ui` test writes the lines of each value widget. A plot of 3000 values sent 1024 values
  and kept its peak. A waveform sent 256 columns. A curve sorted its points after a move. A
  bad event changed no value. An image with a bad size sent no pixels.
- The frontend test reads each value widget line, and drops 5 bad lines but keeps their index.
- The `Ui` test writes the lines of a unit, tabs and two sections. The closed section kept an
  empty line for its hidden widget, so the button after it kept its index. The frontend test
  puts the widgets into their sections, ignores an extra end line and stops at 8 levels.
- The `Ui` test sorts a trim event and keeps it from 0 to 1, and a bad trim event changes no
  value. The frontend test reads a trim range and drops a range with the start after the end.
- The `Ui` test writes the lines of plot axes, a scatter plot and a curve histogram. A
  histogram of 1000 bins sent 256 bars with the same sum. 3000 scatter points sent 1000 points.
  The frontend test reads the axes and the histogram, and drops a plot with a bad axis.
- The `Ui` test writes an icon button and a progress bar, and keeps a bad progress value at 0.
  The frontend test puts icon buttons on adjacent lines into one row and drops a progress
  value above 1.
- The `Ui` test sorts the stops of a color ramp, keeps the selected stop on a move, and adds,
  colors, removes and selects stops. A bad ramp event changes nothing, and a ramp keeps one
  stop. The frontend test drops a ramp line with a bad blend or color, and draws the gradient.
- The `Ui` test moves, selects and hides layers of a layer stack, and sets an amount in the
  limits. A bad layer event and an amount for a layer without an amount change nothing. The
  frontend test reads a base without an amount and drops a bad layer row.
- The `Ui` test writes the kind of a file path field and takes a new path. The editor test makes
  a path in the project relative, also with another case on Windows. The frontend test drops a
  path line with an unknown kind.
- The `Ui` test writes the extensions of an asset picker and takes a new path. The engine test
  lists the project files with an extension, without a case, and skips `build/`. The frontend
  test reads the extensions.
- An engine test finds each source file of the SDK crates in the files that the engine writes
  into a project. A new SDK file that is not in the list fails the test, not the project build.
- The hello test moves the **Count** slider of the tick node to 7. The widget lines after the
  event showed 7, not the old count.
- The editor selfcheck shows the value widgets in the node body. A click on a table row
  selected the row. The **Parameters** tab of the **Inspector** showed the large size.
- The plan test orders a loop through a delay and gets its type from the other wire. It refuses
  a delay with no input wire, an unknown built-in node, and a loop of only generic nodes.

### Known limits

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
  handle with a key, reads the axis labels, the scatter dots and the curve histogram, clicks an icon button and reads the progress bar, adds a ramp stop with a click, removes it with Delete, sets the ramp blend, hides a layer and moves it with the down arrow, types a file path, picks a project file in the asset picker, clicks the canvas, shows the large size in the **Inspector**, and opens the full size and closes it with Esc. No check
  drags a curve point, a trim handle, a ramp stop or a layer, picks a color, or clicks
  **Browse** in the editor. The dialog of the platform needs a person.
- A node with `#[node(full_gui)]` has an expand button in its header and in the **Inspector**
  header. The button opens the full size over the graph area. **Back** or Esc closes it. The
  parse test and the macro test check `full_gui`. The editor selfcheck opens the full size,
  reads a label that the node shows only in the full mode, and closes it with Esc.
- Each build makes a new cluster. Milestone 6 keeps the state of each node whose code did not
  change (7.2).
- A state that is not `Send` makes all the tasks of the cluster run on the thread of the run,
  not only the task of the node (9.2).
- No test makes a real cluster panic in `process`. The glue test checks the rule.
- The editor selfcheck wires the built-in delay, a generic node, from A to B in the live graph.
  Both wires show 2.5. The editor uses the same engine code as the CLI.
- A generic node with no wire that gives its type is unresolved, with gray pins (fixed
  2026-11-16). The editor selfcheck adds a delay with no wires, sees the unresolved state, and
  sees the state go when a wire gives the type. A generic pin shows the name of its type
  parameter in its tooltip. The Add node palette joins a wire to a generic pin of any type. Its list has the built-in delay node, after
  the nodes of the project. The editor selfcheck sees the delay in the list, but does not add it.
- Only stateful nodes can have a GUI. The audio `ui` with `#[settings]` comes with the audio
  domain (milestone 11).
- The **Widgets** group of the **Project** tab makes widget folders with a start widget. **Add
  crate** with a widget name adds the widget by its path. A manual check built a node that shows
  a widget folder in `patina-cli`. The editor self-check makes a widget folder, but does not
  build a node that shows it.
- A node GUI can have a canvas with lines, rectangles, circles and text. The canvas sends the
  pointer down, the moves of a drag (one for each frame at most) and the up. A custom widget
  implements the `Widget` trait. The editor selfcheck clicks a canvas, and the node draws a dot
  at the point. No check drags on a canvas in the editor.
- These shortcuts are open in the code:
  - The engine draws all node GUIs after each run and frame, also the nodes out of view (4.3,
    `crates/patina-editor/src/engine.rs`). Send the visible nodes from the page when graphs get
    large.
  - The **Add node** palette compares the type text with no module paths, so two types of one
    name in two modules match (`crates/patina-editor/frontend/src/palette.ts`). rustc checks the
    types at build time.
- The delay runs in the audio domain when its input comes from there (fixed 2026-11-16). The
  hello test plays a tone through a delay and hears it one block later with no frame between.
  The plan test puts the delay of an audio cycle in the audio domain. The editor selfcheck wires
  a delay after an audio node and sees the audio domain icon. No check played it on a real
  device.
- A loop of only generic nodes and delays has no type, so the engine refuses it.
- A generic node with no input gets its type from the readers of its output (4.5). The hello
  test runs a generic zero node in the batch domain, and a number node of the frame domain gives
  its type. No check runs this with the node in the slot or with an async reader.
- Two Linux CI runs counted 1 allocation in 100 frames of the hello test. A slow frame can start
  the worker pool for the first time inside the counted frames. A parallel cluster now prepares
  the pool when it loads. One CI run with this fix passed on all three platforms.

## Milestone 4: automatic parallelism and the frame domain

Milestone 4 ended on 2026-11-15. The engine splits a graph into tasks and runs them on a worker
pool (9.2).
The CLI and the editor run the frame domain at 60 FPS (9.1), and a frame node can read batch
values. A frame of the engine does not allocate after the first frames. All ran on 2026-11-15 on
Windows 11 Pro, with Rust 1.94.0. CI ran the tests of the first four slices on Windows, Ubuntu
and macOS. The pool was measured, and a small run stays on one thread. The delay node moved to
milestone 5 (9.2).

### What works

- The plan puts a step into the task of its input when the step has one input step, and that
  step has no other next step. Thus a chain of nodes is one task, and each branch is a task.
- The cluster ABI is version 8. A cluster has one call for each task, a call that ends the run
  of a domain, and a call that tells if its tasks can run on many threads at a time.
- A value that goes from one task to another is in a shared slot with a lock. A value inside one
  task is a local value of the task. The parameters are in shared slots.
- The cluster can run on many threads only when each shared value is `Sync`. Else all tasks run
  in index order on the thread of the run.
- One worker pool for each process has one thread for each CPU core. The thread of the run is
  one of them. A second run at the same time runs its tasks in index order on its own thread.
- The log lines, the values and the run times come in task order, so a run gives the same
  reports each time.
- A node declares its domain with `#[node(domain = batch)]` or `#[node(domain = frame)]`. A
  node with no domain takes the domain of its inputs. A node with batch and frame inputs takes
  the frame domain. The plan reports a wire from the frame domain to the batch domain.
- A wire from a batch node to a frame node is a latest-value wire. The frame node reads the
  batch value at each frame. The value stays until the next batch run. A task holds the steps of
  one domain.
- The engine keeps the tasks of each domain apart. `Cluster::run` runs the batch domain, and
  `Cluster::frame` runs one frame. The end of a domain drops the shared values of its steps.
- The pool runs the tasks of a domain only when its nodes took 0.1 ms or more in the last run
  (9.2). A smaller run runs its tasks in order on the calling thread.
- After the first frames, `Cluster::frame` does not allocate. The reports of a call go into one
  text buffer, and a frame fills the lists of the last outcome again with their old texts.
- `patina-cli run <folder> --frames <count>` runs the batch domain one time and then the frames
  at 60 FPS. Without `--frames`, it runs frames until Ctrl+C. A late frame skips the missed
  ticks.
- The first Ctrl+C stops the runs of the CLI. The values that the graph keeps drop, and the
  exit code is 130. A second Ctrl+C ends the process at once (fixed 2026-11-16). On Windows, a
  probe sent Ctrl+C to a run without `--frames`. The drop of a frame node ran, and the CLI
  ended after 0.2 s. The Unix path and a run with audio blocks were not probed.
- In the editor, **Run** runs the batch domain and then the frames. **Pause**, **Stop**, a
  failed frame or a change that runs the graph again ends the frames. **Step** runs one frame
  of the paused graph. The engine sends an event
  for each frame. The status bar shows the frame number, and the badges and the run times of
  the frame nodes change at each frame.
- The editor refuses a connection that makes a cycle of wires or order wires, and names the
  nodes of the cycle (9.2). The graph does not change.

### What was checked

- `cargo test --workspace --locked` passed 65 tests on Windows. New tests:
  - The plan makes one task of a chain and splits the branches.
  - Only a parallel cluster with more than one task and 0.1 ms of node time uses the pool.
  - The pool starts a task only after its inputs, also with a second run inside a task.
  - Only `Sync` values let a cluster run in parallel (`Cell` and `Rc` do not).
  - The plan gives the frame domain to the node after a frame node, and to a node with batch
    and frame inputs. It reports a wire from the frame domain to the batch domain. A batch node
    and the frame node after it go into two tasks.
  - The frame clock skips the ticks that a late frame missed.
  - A frame replaces only the live values of its outputs. The batch values stay.
  - A connection that makes a cycle gives an error with the nodes of the cycle, and the graph
    does not change. This also applies to a wire from a node to itself.
- The CLI tests ran the hello example (3 tasks, with shared values) and the fail project (2
  branches) through the pool. The hello test also added a batch number, a frame node that reads
  it, an add node that reads the batch sum and the frame node, and a show node. With
  `--frames 3`, it wrote `6.5` one time and then `8.5` three times. Thus the frame node read the
  batch value again at each frame, also with no other reader.
- The hello test counts the allocations of its thread in 100 frames, after 3 frames that fill
  the buffers. Each frame sends values with lanes and run times. The count was 0. The old code
  made 2000 allocations in the same 100 frames, thus 20 in each frame.
- A manual CLI run of 5 frames took 0.29 s with the build check.
- `measure_split` in `pool.rs` measured the pool in a `release` build on Windows with 32 cores.
  It gives the median of 201 runs of 8 tasks, in us:

  | Loop steps of each task | One task | Chain of 8 | 8 separate |
  | ---: | ---: | ---: | ---: |
  | 0 | 0.1 to 0.9 | 0.3 to 1.8 | 1.4 to 3.0 |
  | 1,000 | 2.7 to 3.1 | 4.7 to 5.3 | 6.7 to 16.5 |
  | 10,000 | 22.9 to 23.2 | 23.6 to 24.5 | 41.8 to 55.1 |
  | 100,000 | 175.7 to 175.8 | 176.2 to 176.6 | 105.8 to 120.8 |
  | 1,000,000 | 1443.6 to 1447.8 | 1445.5 to 1455.7 | 280.6 to 567.7 |

  The table shows the range of 3 runs. A trace of 8 tasks of 180 us showed that all tasks
  started in 8 to 41 us, and the calling thread woke 56 to 65 us after the last task ended.
- `measure_live_values` in `load.rs` sent 1000 live values of `f64` through the reporter into
  the outcome, 100 times, in a `release` build. Each value took 71 to 76 ns in 3 runs. This
  supports the agent decision for question 12 (15.9). The decision for question 11 is in 9.1.
- The editor self-check passed its 57 steps in 15.0 s. A new step makes the number node a frame
  node and clicks **Run**. It waits for 3 frames that write `2.5`, and for the frame number in
  the status bar. Then it clicks **Stop**. The end event counted the 3 frames, and no frame ran
  after it.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` passed with Rust 1.100.0.
- CI run 43741563738 of the editor frame run passed on `windows-latest`, `ubuntu-latest` and
  `macos-latest`. So did CI run 43742036575 of the cycle check and CI run 43742347402 of the
  latest-value wire.

### Known limits

Milestone 5 added the delay node, and milestone 11 made the latest-value wires of the audio
domain free of locks. These limits are still open:

- The calling thread of a parallel run wakes about 60 us after the last task ends. Eight
  separate tasks of 180 us thus ran only 2.5 to 5 times faster than one task, not 8 times.
- The pool rule uses the node times of the last run. Thus, after a small run, the next run
  stays on one thread also when it is heavy. The measurement ran only on Windows.
- The editor copies each frame into a JSON event for the page, so the frame loop of the editor
  allocates. The frames of the engine and of the CLI do not allocate.
- The allocation test counts only the thread of the run. In the test graph, this thread runs
  all the frame tasks, because the second task waits for the first.
- A text that is longer than the text of the last frame grows its buffer one time.
- The editor sends an event to the page at each frame, also when no value changed.
- A change while the frames run runs the batch domain again before the next frame.
- A latest-value wire between the batch and the frame domain uses a slot with a lock (9.1).
  The batch run ends before the frames start, so the lock never waits.
- A batch value that a frame node reads stays after the frames end, until the next batch run.
- These shortcuts are open in the code:
  - The editor copies each frame into a JSON event (`crates/patina-editor/src/engine.rs`).

## Milestone 2: editor

The first part of item 2 of section 19 works. The editor `patina` makes or opens a project,
edits the main graph, runs it and saves it. This is the first workflow of spec 15.10. It ran on
2026-11-15 on Windows 11 Pro, with Rust 1.94.0, Tauri 2.12.5, WebView2 157, React 19.3.1 and
React Flow 12.12.1. It did not run on Linux or macOS.

To run the editor from a checkout:

1. In `crates/patina-editor/frontend`, run `npm ci` and `npm run build`.
2. In the root folder, run `cargo run -p patina-editor`.

### What works

- The start screen has **New project**, **Open project** and the recent projects. A new project
  has the `number` and `show` node folders, and goes on the list of trusted projects (11).
- The graph shows the nodes, their pins, their parameter values and the wires. Drag a node
  folder from the **Project** panel into the graph to add a node.
- A double-click on an empty area of the graph, or Shift+A, opens the **Add node** palette
  (15.2). Type to filter the nodes, and push Enter or click a node to add it at that point.
- A drag from a pin into an empty area opens the palette with only the nodes that fit the pin.
  The new node gets a wire to the pin, and one undo removes the node and the wire.
- The **+** button of the **Nodes** group in the **Project** panel makes a node folder (15.2).
  Type a name and push Enter. The **Inspector** shows the code of the start node. A used or
  reserved name shows its problem under the field.
- **Add crate** in the **Code** tab adds a crate of crates.io to the node (15.4). Type `rand` or
  `rand@0.9` and push Enter. Patina runs `cargo add`, and undo removes the crate.
- **Settings** on the start screen shows the Rust version, the path of `cargo`, the data
  folders and the trusted projects (15.10). **Remove** takes a folder off the trusted list.
- The canvas has no animation. The mouse wheel zooms. The middle button, or Space and a drag,
  pans. A drag on the empty canvas selects. Shift or Ctrl adds to the selection.
- The wires draw on one WebGL2 canvas under the nodes (15.1). A selected wire has an amber
  band. The hit area of a wire is 12 screen pixels wide at each zoom. Without WebGL2, or after
  a lost context, the wires draw as SVG.
- A double-click on a wire adds a route point (15.9). A selected wire shows square handles on
  its points. Drag a handle to move its point. Delete or a double-click on a handle removes the
  point. The arrow keys move a focused handle by 10 px, or by 1 px with Shift. Esc cancels a
  drag. A route has rounded corners. The + or Insert key on a focused wire adds a route point
  at its middle. The selfcheck presses + on the wire and checks the new point and its undo.
- A selected orthogonal wire shows a bar on each middle segment (15.9). A drag on a bar moves
  the segment sideways, and the segments next to it stretch. A unit test checks the moves. The
  selfcheck drags a bar 40 px and pushes an arrow key on it, and checks each undo.
- Esc during a drag of a route point or a segment bar puts it back and sends no edit (15.9).
  The selfcheck presses Esc during each drag and checks the place and the undo history. It also
  pushes an arrow key on a route point.
- A wire with the graph default look and no clear path shows a yellow conflict mark on the
  first segment through a node body (15.9). A unit test checks the mark and its removal. The
  selfcheck puts a route point in a node body, and checks the mark and that the undo removes it.
- Each use of a subgraph has an **Open subgraph** button, and the path bar shows each use of a
  nested path (15.9). Unit tests check the use names and the use path. The selfcheck opens a use
  with the button, checks the path bar, and goes back with its **main** button.
- **Show inner nodes** on a use shows the subgraph nodes in place, in a read-only frame with
  live values (15.3). The frame moves the nodes at its right and below it in the view only.
  Unit tests check the frame layout, the toggle, the view map and its inverse for edits. The
  selfcheck shows and hides the frame. It checks the inner node, its wire to the frame pin, that
  it has no edits, that the frame covers no node and that the nodes come back.
- A drag on the end of a wire moves it to another socket in one edit, and keeps the wire name
  and look (15.9). The selfcheck drags the end of the wire from B to a delay, checks the new
  wire, and checks that one undo gives the wire to B.
- During a wire drag, the sockets that fit get an accent ring and the other sockets dim (15.9).
  A unit test checks the type keys and the style. The selfcheck checks the ring on the delay
  input and the dim order socket during the drag, and that the drop removes the dim.
- A dot marks each point where the wires of one output part, and a ring marks it on a bus
  (15.9). Unit tests check the junction points and the router marks. The selfcheck adds a
  second wire from one output and checks one dot, and that undo removes it.
- Where two automatic routes of different outputs cross, the narrower wire has a gap, and the
  vertical wire goes under a wire of the same width (15.9). Unit tests check the crossing gaps
  of the router and the segments that the WebGL layer does not draw.
- Each node header shows a kind icon and a clock-domain icon, and a wire between two domains is
  dashed (15.3). A unit test checks the kinds of the engine. Frontend tests check the domains
  of a scope and the dash flag of the wire store. The selfcheck connects a batch value to a
  `delay`, and checks the icons, the dashed wire and the undo. It checks the trigger icons too.
- The **Look** section of the **Inspector** sets the look of the selected wire (15.9). **Route**
  is **Graph default**, **Curved**, **Orthogonal** or **Polyline**. **Corners** is **Rounded**
  or **Sharp**. **Reset route** removes the route points. The graph file keeps the style, and
  each change has undo.
- The graph file keeps the route points. Undo and redo work for each route edit. A route edit
  does not make the result of the run stale.
- After each run, each wire shows the value that it carries in a badge (15.9). The badge shows
  the `Viewer` text of the value, the `Debug` text, or the type name and the size (5.3). A
  selected wire shows all of the badge text. After an edit, the badge is stale: its text is dim
  and its border is dashed.
- Each wire shows the Rust type of its value in a small label above the badge (15.9), for
  example `f64`. A pin shows its type in a tooltip. The **Inspector** shows the type of the
  selected wire. A label hides when a node body, the badge of another wire or a label of a
  higher rank covers it. A selected wire always shows its label.
- A project can have connection kinds in `connections/<name>/` (6.3). A `#[connection]`
  function takes the value of a type and gives its lanes and a label. After a run, a wire of
  that type draws as a bus of lanes, the most significant lane first, with a label such as
  `whole[7:0] / 8` (15.9). A lit lane is light, and a stale bus is dim. Below a zoom of 0.5,
  the bus is one line with the label `/ 8`. The **Inspector** shows the lanes as bits.
- Below a zoom of 0.5, the nodes show no pin names and no parameter text, and the wires show no
  badges and type labels (15.1). A selected wire keeps its badge.
- Select a wire to see its ends and the full text of its value in the **Inspector**, up to 4096
  bytes (15.7). A type crate implements the `Viewer` trait of `patina-sdk` to give its own text.
- Delete removes the selected nodes and wires. F and **Fit** fit the graph. Esc clears the
  selection.
- The **Inspector** has the **Parameters**, **Code** and **Logs** tabs of the selected node. A
  parameter change applies when the field loses the focus.
- The **Code** tab edits the files of the node folder in Monaco, with Rust colors. An unsaved
  file has a dot on its tab. The tab shows the build state of the saved code (15.10). It keeps
  the cursor and the scroll position of each file, and shows the last file of the folder again.
- Each graph change is one edit of the engine (7.4): add, remove, move, connect, disconnect, set
  a parameter, or a group. Undo and redo work for each edit.
- Ctrl+S saves all: one `SetCode` edit for each changed code file, then the graph files. Undo
  and redo of a `SetCode` edit write the file and reload its buffer.
- A compile error shows in the four places of 15.5: the underline and the gutter mark, the node
  border, a line in **Logs** and the count in the status bar. A click on a **Logs** line or on
  the count selects the node and shows the line in the **Code** tab.
- When the saved code does not compile, the graph runs the last build that compiled. The **Code**
  tab and the status bar show **Running the last build that compiled**.
- A close or a quit with unsaved changes asks **Save**, **Discard** or **Cancel** (15.10).
- **Stop** pauses and asks the active run to stop (15.2). The engine starts no new node call.
  A node with a long loop can read `patina_sdk::stop_requested()` and return early. When a
  node call runs for 2 s after **Stop**, the status bar shows **Node** `<id>` **does not stop**
  and the time.
- The unsaved graph and code go to a recovery file in `data/recovery/` (15.10). When a project
  opens with a recovery file, a dialog asks **Restore** or **Discard**. A save, or a close after
  the question, deletes the file.
- **Run** builds the graph on the engine thread and runs it. While the graph runs, a parameter
  change runs it again with no build. A wiring change builds it again. A move does neither.
- The **Output** tab shows the text that the nodes write. The **Logs** tab shows the build
  time, the run time, the log lines and the reports of the nodes.
- Each node shows its state with an icon and a border: ready, compiling, compile error, run
  error, stale, skipped or unresolved (15.3). A header color never shows a state. A node after
  a failed node shows **Skipped**. After **Stop**, only the nodes that did not start show
  **Stale**. `patina-cli` writes a `skipped` line for each skipped node.
- After a run, each node that ran shows its run time in a footer, for example **Run 0.30 us**
  (15.7). The cluster measures each node call and sends the time to the engine.
- A node with the `log` crate writes log lines with `log::info!`, `log::warn!` and
  `log::error!` (15.7). The **Logs** panel shows each line with the node folder and ID. A WARN
  line has the warning color, and an ERROR line has the error color. The **Logs** tab of the
  **Inspector** shows the lines of the node. `patina-cli` writes them to stderr.
- The window is a dock (15.2). The **Project**, **Inspector**, **Logs** and **Output** panels
  are tabs in the left, right and bottom areas. Drag a tab to another area, or use its
  right-click menu to move or close it. A splitter changes the size of an area, with the mouse
  or the arrow keys.
- The navigation rail has an icon for each tab of the left area (mockup 6). The icon of the
  active panel folds and unfolds the left area. The layout file keeps the fold. A unit test
  checks the fold, and the selfcheck folds and unfolds the area with the rail.
- The **Views** menu shows and hides each panel, and **Reset layout** gives the default layout.
  The layout and the pan and zoom of the graph go to `data/layouts/<project id>.json`, 300 ms
  after a change and before a close or a quit.
- A close of the window waits for a layout change that is not saved yet, also when the project
  has no unsaved changes (fixed 2026-11-16). A check on a hidden desktop set the save delay to
  60 s, folded the left area and sent `WM_CLOSE` to the window. The editor exited with code 0,
  and the layout file had the fold.
- Patina does not build a project that is not trusted. It shows a banner with **Trust
  project**.
- The page loads only its own files, with a content security policy. A request for a path
  outside the page folder gets an error.
- The data folder holds the lists of recent and trusted projects. With a `portable` file next
  to the program, or with `--portable <folder>`, the data stays in that folder (13.3).

### What was checked

- `patina --check <folder>` runs the first workflow in the real window. The window is
  off-screen and does not take the focus. It passed these 53 steps in 12.8 s:
  - Open **Settings**, find the Rust version and the trusted list, and capture the window into
    `settings.png`. Click **Back**.
  - New project, add 2 nodes and a wire, and set a parameter.
  - Run, and wait for `2.5` in the output. Set the parameter to 7 and wait for `7`, with no
    build. Undo, and wait for `2.5`.
  - Save a function with a type error into `nodes/number/src/lib.rs`. The old build still
    writes `2.5`. Find the error on the node, in Monaco, in the gutter and in the status bar.
    Capture the window into `code-error.png`.
  - Open `Cargo.toml` and then `src/lib.rs`: the cursor is at the error again. Put it on line 2,
    close the **Code** tab and open it again: the cursor is on line 2.
  - Replace the error in the Monaco buffer with the original code, a panic at 13 and a loop
    that waits while the value is 99. Push Ctrl+S. The build passes, the file on disk has that code and no file
    is unsaved.
  - Set the parameter to 13. The run fails, and the engine reports `show` as skipped. `number`
    shows the run error, `show` shows **Skipped** and the status bar shows **1 skipped node**.
    Undo, and wait for `2.5`.
  - Set the parameter to 99. Wait until the engine names `number` as the active node, and
    click **Stop**. The run ends with no error, and the engine reports that `show` did not
    start. The output has no `99`. **Run** is enabled, the status bar shows **1 stale node**,
    `show` is stale, `number` is ready and no node is active. Undo, and the graph stays paused.
  - Save and pause. Move the **Logs** tab to the right area with its menu. Push Shift and the
    right arrow on the left splitter, and zoom the graph with the wheel. Read the layout file:
    it has the tab, the size 324 and the pan and zoom.
  - Close. The recent list starts with the project.
  - Open again. The graph is the saved graph. **Logs** is in the right area, and the pan and
    zoom are the same. **Reset layout** moves **Logs** back and keeps the pan and zoom.
  - Run again, and find 2 ready nodes in the graph. Find a run time in the footer of each
    node: **Run 0.30 us** for `number` and **Run 9.00 us** for `show`.
    Find `2.5` in the badge of the wire, not stale, and `f64` in its type label.
  - Read the pixels of the wire from the WebGL canvas. They are gray, with no amber. The
    point under the wire is in the edge. Click the wire, and find the amber band. Find `2.5`
    in the **Inspector** and in the full text that the engine keeps.
  - Double-click the wire, and find 1 route handle. Drag the handle down by 60 px. After the
    edit, the handle is under the pointer and the old place of the point has no wire pixels.
  - Undo and redo the drag. Push Delete on the focused handle, and find no handle. Undo, and
    find 1 wire with 1 route point.
  - Pause, and set the parameter to 3. Find a stale badge with `2.5`. Undo, save, and find no
    unsaved changes.
  - Move a node and change the code in Monaco, with no save. Open the project again with no
    close, as after a crash. Click **Restore**, and find the changed code in Monaco and the
    unsaved dot in the top bar. Push Ctrl+S. The file on disk has the changed code. Open again:
    no dialog, and the graph has the moved node.
  - Capture the window into `check.png`.
- The first capture with a route showed a stale segment from the old curve. The segment lists
  did not change when the number of points changed. The fix makes new lists, and the check
  failed without the fix.
- The capture showed the two nodes with the ready state, the wire, the parameter value `2.5`
  and the **Output** tab with `2.5`. **Run** was disabled while the graph ran. The selected wire
  had one rounded corner at its amber route handle.
- The first capture with a badge showed the route handle on the badge text. The badge was at
  half of the length of the route, on the route point. It is now at the middle of the longest
  segment. The stale badge also showed the wire through its background, so only its text is
  dim now.
- The capture showed the dock with the default layout: the **Project** and **Inspector** tabs
  with a close button, the **Logs** and **Output** tabs with **Clear**, and the **Views** menu
  button in the top bar.
- The capture showed the **Wire** view of the **Inspector**: `number.out`, `show.value` and the
  stale text `2.5`.
- The capture `code-error.png` showed the red node, the gutter dot and the underline on line 10,
  the error well of the **Code** tab and **1 compile error** in the status bar.
- A manual check gave fake lanes of 8 bits to the values in the page, and captured the window.
  The capture showed 8 lanes between the pins, with only lane 6 lit for the value 2.5, and the
  label `whole[7:0] / 8` above the ribbon. The fake lanes were then removed.
- The CLI test adds a kind to a copy of the hello example. The sum 6.5 has the lanes `00000110`
  with the label `whole`.
- `cargo test --workspace --locked` passed 53 tests. The tests check the graph edits with undo
  and redo, the layout files with a hex project ID, the recovery files, the stop flag, the skipped nodes, the run time reports, the log lines with the limit of a run, the detection of the `log` crate, the pin types, the lanes of a kind, the member globs of the project workspace, the route points, the crate names of **Add crate**, the text of the live values with a `Viewer`, the cut of the badge
  text, `SetCode` with undo, the compile errors with the project paths, the changes that
  need a run or a build, the project names, the recent and trusted lists with a removal, the output lines and
  the refusal of page paths outside the page folder.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` passed with Rust 1.94.0 and
  1.100.0. `npm run build` passed the type check and the build of the page. `npm test` passed
  25 tests: the format of a node run time, the style of a log line, the search of the **Add node** palette, the pin match of a drag from a pin, the lane instances of a bus, the miter normals, the rounded corners of a route, the place of a new route point,
  the SVG path of a route, the orthogonal paths, the sharp corners, the curve through route
  points, the segment lists, the place of the badge, and the move, close, open, resize, reset
  and read of a dock layout.
- The editor binary holds the page files (13.1). `build.rs` embeds `frontend/dist` at compile
  time. A copy of `patina.exe` in a folder outside the checkout passed the self-check.
- `cargo run -p package` makes the portable zip in `target/package` (13.1). The zip has
  `patina`, `patina-cli`, the `portable` file and `licenses/`. The license folder has the two
  Patina license files, the notice files of 233 crates and 31 npm packages, and a list with
  the license of each package.
- 4 crates in the build have no license file: `selectors` (MPL-2.0) and three `webview2-com`
  crates (MIT). The package tool copies their texts from `tools/package/notices`: the MPL-2.0
  text, and the MIT text of the `webview2-rs` repository.
- The **Package** workflow of GitHub Actions makes the Windows and the Linux zip when a person
  starts it or pushes a tag `v*`.
- The editor and the CLI use the toolchain folder (13.3) when it has `cargo`. Its tools get the
  clean environment of 13.2. Else Patina uses the Rust of the system, and **Settings** tells so.
  A unit test checks the clean environment.
- The toolchain prototype installed Rust 1.100.0 (`x86_64-pc-windows-gnu`) and Zig 0.17.1 into
  a toolchain folder on Windows. The install took 52 s, and the folder has 1.3 GB. The folder
  path had a space, an accent and a CJK character. With this folder:
  - `patina-cli run` built a copy of the hello example (`rust = "1.100.0"`) in 1.33 s and
    wrote `6.5`. The cluster imports `msvcrt.dll`, so the gnu toolchain built it.
  - The editor self-check (`--portable` on that folder) passed its 53 steps. The new project
    got `rust = "1.100.0"`, and its cluster imports `msvcrt.dll`.
  - The Zig wrappers were copies of the prototype program, not of `patina.exe`.
- **Install toolchain** in **Settings** installs Rust and Zig with the steps of 13.2. Unit tests
  check the read of the channel manifest and of the Zig index, the C arguments and the hash
  check. On Windows, a test ran the last steps on the installed folder: the copies of the debug
  `patina.exe` as Zig wrappers, the C check and the rename. Then the C fixture of the prototype
  (`libsqlite3-sys`) built with these wrappers. No full install with the button ran, so its
  downloads are not proven. The prototype proved the same download steps.
- The self-check sets **Orthogonal** in the **Inspector**, finds 3 corners or more on the wire,
  clicks **Reset route**, and undoes both edits.
- The self-check opens the **Add node** palette with Shift+A and closes it with Esc. Then it
  opens the palette with a double-click, types `SHO`, pushes Enter, finds the `show` node at the
  point of the double-click, and undoes the edit.
- The self-check drags from the input of `show` to an empty area. The palette lists only
  `number`, because `show` has no output. Enter adds `number` with a wire to the input. One undo
  removes the node and the wire, redo gives them again, and a last undo removes them.
- The self-check clicks **+** of the **Nodes** group and enters `Number`. The field shows the
  problem of the used name. Then it enters `made-node`, finds `made-node` in the **Project**
  panel and the **Code** tab of the new node in the **Inspector**.
- The self-check clicks **Add crate**, enters `-x`, finds the problem of the name and closes the
  field with Esc. This step uses no network.
- `cargo test -p patina-engine --locked -- --include-ignored crate` ran the ignored network test
  on Windows. It adds `itoa@1` to a node of a new project in 1.93 s, and the file on disk keeps
  its old text. A crate that crates.io does not have gives an error with its name.
- A project that was never built has no `build/clusters` folder. The project workspace now
  lists `build/clusters/*` only when the folder exists, because `cargo add` failed without it.
- The page reports each error of the window to the check, but not the browser message
  `ResizeObserver loop`. Monaco causes this message when the **Inspector** shows a new editor.
- The engine test makes `my-node` and reads its input pin. It refuses `My_Node`, `number`,
  `type`, `Con`, `1a`, `a/b`, `..` and an empty name.
- The capture `settings.png` shows the three parts of the settings screen in the charcoal
  palette, with the long paths wrapped in the card.
- CI run 43727365197 of **Add crate** passed on `windows-latest`, `ubuntu-latest`
  and `macos-latest`.
- CI run 43731377929 of the log lines passed on `windows-latest`, `ubuntu-latest` and
  `macos-latest`.
- CI run 43731942012 of the page in the binary passed on `windows-latest`, `ubuntu-latest` and
  `macos-latest`.
- CI run 43732552697 of the toolchain folder passed on `windows-latest`, `ubuntu-latest` and
  `macos-latest`.
- CI run 43734261126 of the **Install toolchain** button passed on `windows-latest`,
  `ubuntu-latest` and `macos-latest`.
- CI run 43735014784 of the package tool passed on `windows-latest`, `ubuntu-latest` and
  `macos-latest`.
- CI runs 43735647411 (the message boxes) and 43736267355 (the **Package** workflow) passed on
  `windows-latest`, `ubuntu-latest` and `macos-latest`.
- **Package** run 43736276055 made both zips: Windows 6467080 bytes and Linux 6982897 bytes.
- The start screen has a **Browse** button for each folder field. The self-check finds the two
  buttons, but does not click them, because the folder dialog waits for a person.
- An error that stops the editor shows in a message box of the platform (`rfd`), except in the
  self-check. The WebView2 check runs before the window opens. On this computer the Runtime is
  installed, so the self-check passed the check.
- `cargo run -p package` made the Windows zip in 41 s (7.2 MB). Unzipped in a scratch folder:
  - The release `patina.exe` passed the 53 steps of the self-check in 13.0 s.
  - Copies of the release `patina.exe` worked as the Zig wrappers. A good C file compiled, a
    bad C file gave the Zig error and exit code 1, and `zig-ar` made a library.
- A copy of the hello example with `log = "0.4"` in the `add` node ran in `patina-cli` on
  Windows. It wrote `INFO adding 2.5 and 4` and `WARN a warning` for the node, and no debug
  line. The build took 1.57 s.

### Known limits

- **Add crate** has no search of crates.io and no list of versions. The self-check does not run
  `cargo add`, because CI must not need crates.io for the UI check.
- The self-check does not click **Install toolchain**, because CI must not download the
  toolchain. The self-check does not click **Remove**. The data test covers the removal.
- No check opened the folder dialog, the fatal message box or the WebView2 message. A person
  must click them. The message of a missing WebView2 Runtime did not run.
- The check drags one node with mouse events of the page, not with the real mouse. A person
  did not use the editor yet.
  The owner check: make the first workflow of 15.10 with the mouse.
- The content security policy was not tested with a page that breaks it.
- The check did not test the SVG wires or a lost WebGL context.
- The capture works only on Windows. The check did not run on Linux or macOS.
- The route points of the wires in a frame stay in place during a drag of the use. The frame
  is read-only. Open the subgraph to edit its nodes.
- These parts of spec 15 are not in the editor yet: view tabs in the center
  (15.2), rust-analyzer in Monaco (15.1) and the **Debug** tab.
- The palette of a drag from a pin compares the type text with no module paths (15.2). A type
  alias does not match its type, and two types of one name in two modules match.
- `stop_requested()` is false on a thread that a node starts. The limits of milestone 12 tell how
  the waits of async nodes (4.6) stop.
- The self-check does not run a node with the `log` crate, because that needs crates.io. The
  type check covers the log lines of the page, but no check showed them in the editor yet.
- The log lines show only the levels Info, Warn and Error. The logger drops a record from a
  thread that a node starts. A run shows 1000 log lines at most.
- A `log` crate with another name or in a target table now turns on the logger (fixed
  2026-11-16). The plan test reads four manifests. No build ran with such a node.
- The `Viewer` gives text only. The table, the image, the plot, the waveform and the thumbnail
  come later (5.3). The **Inspector** reads the full text one time after each run.
- The editor has no bench with 500 nodes, so the far zoom form was not measured for frame
  drops. Two badges can cover each other. After each run, the engine copies the text of each
  value into a new `String`, and the badge text into a second `String`.
- The segments at the two pins of an orthogonal wire cannot move, and a wire with the graph
  default style has no segment bars (15.9). A double-click on an orthogonal or curved
  wire puts the new point on the nearest straight line through the route points. The graph has
  no setting for the default style. **Bus**, **Lock bends** and the crossing bridges are not
  made. A wire with a fixed style can go under a node.
- The automatic route and **Arrange** have no undo of a route change, because a route is not
  saved. The route changes when a node moves, and the wires of a large graph can jump.
  **Arrange** puts all nodes of a layer in one column, so a wide layer makes a tall column, for
  example the 112 switches of `rom-16x7`.
- The recovery file does not have the typing of the last 300 ms before a crash. The check did
  not crash Patina, and it did not click **Discard**. A project that is not trusted keeps its
  old recovery file, and its new changes do not go into the file until the user answers.
- A connection kind matches a type by the type text that the node code writes, not by the type
  crates of 5.1. A panic in a kind gives no lanes. Its message shows only in the **Output**
  panel, from the standard error (4.7). A compile error in a kind
  stops the build of all of the graph.
- The lanes have no groups. The lane hover works only on the WebGL wires, at a zoom of 0.5 or
  more. The badge is always the viewer text.
- The self-check makes a connection kind with **New connection**, but no value of the self-check
  uses it. The lanes ran in the CLI test, and the look
  had one manual check with fake lanes in the page. No check zoomed a bus out below 0.5.
- The check did not drag a tab or a splitter with the mouse.
- Vite shows a warning for the `"use client"` lines of React Flow, and a warning for the size
  of the page script. The build is correct.
- `package.json` sets `dompurify` to 3.4.17 with an override. Monaco 0.57.0 asks for 3.4.15,
  which has the critical advisory GHSA-p98j-92pf-mc4p. `npm audit` now finds 0 problems.
- These shortcuts are open in the code:
  - Monaco has no rust-analyzer (15.1, `frontend/src/code.ts`).
  - Monaco is in the main page script, which is 3.7 MB. Load it when the **Code** tab opens if
    the start time gets slow (`frontend/src/code.ts`).
  - The capture works only on Windows (`src/session/check.rs`).
  - One texture row holds the points of each wire. Above `MAX_TEXTURE_SIZE` wires (2048 or
    more, by GPU), SVG draws the wires (`frontend/src/gl.ts`).
  - A route of more than 62 points draws only its first 62 points. Only a graph file that a
    person edited has such a route (`frontend/src/wires.ts`).
  - Each run formats and sends each held value, also when a graph runs faster than the display
    (15.6) (`crates/patina-sdk/src/glue/probe.rs`).
  - A new lane state builds the instances of all wires again (`frontend/src/wires.ts`).
  - The SVG wires draw a bus as one line, and have no gap at a crossing (`frontend/src/WireLayer.tsx`).
  - A wire with more than 62 points after its gaps draws without its last gaps (`frontend/src/wires.ts`).
  - A graph with a route grid of more than 2 million cells gets the simple orthogonal paths
    (`frontend/src/route.ts`).

## Milestone 1: engine and CLI

Item 1 of section 19 works. `patina-cli run <project folder>` compiles the node folders of a
project, loads the cluster and runs the main graph one time. It ran on 2026-11-15 on Windows 11
Pro, with Rust 1.94.0 and 1.100.0. The CLI tests also passed on the CI runners `windows-latest`,
`ubuntu-latest` and `macos-latest`, with Rust 1.100.0.

### What works

- `patina-engine` reads `patina.toml` and `graphs/main.toml` (6.4). A key that Patina does not
  know, or a newer format, stops the read with a message.
- It reads the pins of each node from the `#[node]` function with `syn` (4.1). Inputs by value,
  by `&`, by `&mut` and as `&str` work. A tuple output and a `Result` output work.
- It checks the graph and gives all the problems at one time: the IDs, the folders, the
  parameter keys, the wire ends, two wires to one input and the cycles.
- It sorts the nodes in run order, with the order wires (9.2). Ties keep the file order.
- It writes the project workspace, the SDK source and one cluster crate, and builds them with
  cargo. It writes a file only when the content changes, so a build with no change is fast.
- It loads the cluster, checks the ABI version, sets the parameters from their text form (4.8)
  and runs the graph. On Windows, only the folder of the library and the system folders give
  other libraries.
- A node that returns `Err` or panics gives a report with its ID and folder. The nodes after it
  do not run (4.7, 15.3). A bad parameter text gives a report, and the graph does not run.
- [examples/hello](examples/hello) adds two numbers. It uses one node folder two times.

### What was checked

- `cargo test --workspace --locked` passed 29 tests. Two CLI tests build and run projects in
  the temporary folder of cargo:
  - `hello_adds_two_numbers`: the exit code is 0, and the output is `6.5`.
  - `failed_nodes_are_reported_and_stop_the_nodes_after_them`: the exit code is 1. The error
    and the panic reports have the node IDs, and the output is empty. Then the parameter text
    changes to `abc`, and the bad parameter report shows.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` passed with Rust 1.94.0 and
  1.100.0.
- A manual run of `examples/hello` with the CLI wrote `6.5`.
- CI run 43704375305 of commit e0ad790 passed on `windows-latest`, `ubuntu-latest` and
  `macos-latest`. The 2 CLI tests ran on each runner.

### Measurements

The projects are very small, so these times do not predict real projects.

| Measurement | Time |
| --- | --- |
| Cold build of `examples/hello`, with the SDK | 0.8 s |
| Rebuild after a change to the SDK source | 0.6 s |
| Build with no change | less than 0.05 s |
| The 2 CLI tests, with the cold build of both projects | 1.2 s |

### Known limits

The later milestones closed the limits of one cluster, a static `std`, function nodes only, the
library in the target folder and the single run of the CLI. These limits are still open:

- The toolchain comes from the toolchain folder (13.3), or else from the `CARGO` variable or
  `PATH`. Its version must be the `rust` version of `patina.toml`.
- **Install toolchain** works only on Windows. It installs rust-analyzer too, but no install
  with rust-analyzer ran yet, because the run downloads about 285 MB. A toolchain from an older
  install has no rust-analyzer. Remove the `toolchain` folder and install again.
- No GitHub release makes the portable zip yet. The **Package** workflow keeps the zips only as
  artifacts of the run.
- A `Result` output is found by the type name `Result` only. A type alias with another name is
  not found, and then an `Err` is a value, not a failure.
- The editor compares the pin types only for the ring of a wire drag (15.9). A drop on a socket
  of another type still adds the wire, and the next build gives the compile error. The type
  label is the text of the code, so `String` and `std::string::String` show differently.
- The CLI finds only the toolchain folder next to it (`crates/patina-cli/src/main.rs`).

## Milestone 0: risk prototypes

| Prototype | Result | Platforms that ran |
| --- | --- | --- |
| Rust types across clusters (5.1, 5.2) | Passed | Windows 11 and CI `windows-latest`, `x86_64-pc-windows-msvc` |
| Hot reload (8.5) | Passed | Windows 11 and CI `windows-latest`, `x86_64-pc-windows-msvc` |
| Toolchain (13.2) | Passed, with the clean computer run open | Windows 11, `x86_64-pc-windows-gnu` |
| Viewport (15.8) | Passed, with one owner check open | Windows 11 |
| Connections (15.9) | Passed, with one owner check open | Windows 11 |

## Rust types across clusters

The code is in [prototypes/types-across-clusters](prototypes/types-across-clusters/README.md).
It ran on 2026-11-15 with Rust 1.94.0, on Windows 11 Pro, in the `dev` and the `release`
profile. It did not run on Linux or macOS.

[CI](.github/workflows/ci.yml) runs the test for each push to `main`. It passed in both
profiles on 2026-11-15, on the GitHub Actions `windows-latest` runner, with Rust 1.100.0. On
the Linux and macOS runners, clippy checks the prototype, but the test has no code to run.

### What was checked

The test `cargo test -p types-across-clusters` ran and passed these checks in both profiles:

- A `String`, a `Vec<Point>` and a `Probe` with `Drop` went from cluster A to cluster B. Cluster
  B read the `String` and took the other two values.
- Cluster B dropped the `Probe` that cluster A made. After the drop, both clusters showed the
  same counts: 0 live, 1 dropped. Thus the two clusters use one copy of `geometry`.
- The engine dropped the values that it kept with the drop function of cluster A.
- `distance` has the type name `units::Meters` on both sides, from `units` 1.0.0 and 2.0.0. The
  type keys are different, and the connection is refused.
- Build 2 changed the fields of `Point` from `u32` to `f32`. The name, the crate version, the
  size and the alignment stayed the same. The types library got a new file name.
- The `points` and `probe` pins of build 1 do not connect to build 2, and the engine refused
  the old values. The `String` of build 1 still went to build 2, because both builds use one
  `std`.
- The engine dropped the old values with the code of build 1. The counts of build 2 did not
  change. Then build 2 ran from cluster A to cluster B.
- Each cluster imports `std-*.dll` and `patina_types_*.dll`, and does not import the other
  cluster. Each import is in `build/` or in Windows.

### Findings

- **One copy of each type works.** A generated `dylib` types crate, `cdylib` clusters and
  `-C prefer-dynamic` give one copy of `std` and of the type crates. The `release` profile
  also passed, with code inlined across crates.
- **One allocator.** The clusters and the types library import `__rust_alloc` from the `std`
  DLL.
- **`TypeId` misses a changed type.** The `TypeId` of `Point` was the same in build 1 and build
  2. The type key found the change.
- **Cargo does not give new names.** The file names of the type crates stayed the same after
  the change. The name of the types library is a hash of its content, so the harness got a new
  name. Spec 8.5 needs this.
- **Every library needs `VCRUNTIME140.dll`** with the MSVC target. A clean Windows does not have
  it. This is input for the toolchain prototype (13.2). The gnu target of that prototype does
  not need it.
- **Export counts are low.** The `std` DLL has 1,401 exports. The types library has 64 in
  `dev` and 18 in `release`. The PE limit is 65,535. A large type crate was not measured.
- **Files of each build.** Each `cdylib` and `dylib` gives a `.dll`, `.dll.lib`, `.dll.exp` and
  `.pdb`. The type and node crates give a `.rlib` and a `.rmeta`. The `std` DLL comes from
  `<sysroot>/bin`.
- **Build times.** A clean build took 0.7 to 0.8 s. The rebuild after the change took 0.5 to
  0.6 s. The project is very small, so these times do not predict real projects.

### Known limits

- The loader supports only Windows. On Linux and macOS, the test does not run.
  [Issue 1](https://github.com/bhowiebkr/Patina/issues/1) tracks this gap.
- The test did not run on a clean Windows computer.
- The only generic type is `Vec<T>`. The type crates have no dependencies, so the build ID
  does not include resolved dependencies yet.
- The harness is not the engine. It shows the method, and the engine will use the same method.

## Hot reload

The code is in [prototypes/hot-reload](prototypes/hot-reload/README.md). It ran on 2026-11-15
with Rust 1.94.0, on Windows 11 Pro, in the `dev` and the `release` profile. It did not run on
Linux or macOS.

CI runs the test for each push to `main`. It passed in both profiles on 2026-11-15, on the
`windows-latest` runner, with Rust 1.100.0, in 12.1 s. On the Linux and macOS runners, clippy
checks the prototype, but the test has no code to run.

Since 2026-11-15, the default run is short: the `dev` profile with 2 node reloads and 1 types
reload. It ran in 3.0 s on Windows, before 13 s. `PATINA_FULL=1` gives the measured run of
both profiles and the LTO check.

### What was checked

The test `cargo test -p hot-reload` ran and passed these checks in both profiles. A long run
did 100 node reloads and 20 types reloads in each profile, and passed the same checks.

- A build with no change added no file to `build/`. The loaded cluster A stayed in use.
- Each node change gave one new file: cluster A. The types library and cluster B kept their
  files.
- After each node reload, the new cluster A ran with the loaded cluster B. The values of the
  old build went to cluster B, and they dropped with the code of their own build.
- A `Probe` of build 0 stayed alive through all reloads. At the end, it dropped with the code
  of build 0, and the counts of the newest build did not change.
- A failed build did not change `build/`, and the loaded build still ran. After the fix, the
  same source gave the same file, so nothing new loaded.
- Each types change gave a new types library and two new cluster files. The engine refused the
  old `points` and `probe` values, and accepted the old `String`.
- The old values of each types reload dropped with the code of their build. The counts of the
  new build did not change.

### Measurements

The project has one node in each cluster, so these numbers do not predict real projects. The
numbers come from the long run. The memory is the growth of the test process.

| Measurement | `dev` | `release` |
| --- | --- | --- |
| Cold build | 0.72 s | 0.93 s |
| Build with no change | 0.03 s | 0.03 s |
| Rebuild after a node change, mean of 100 | 0.33 s (0.26 to 0.54) | 0.30 s (0.27 to 0.37) |
| Rebuild after a types change, mean of 20 | 0.51 s (0.46 to 0.57) | 0.57 s (0.55 to 0.63) |
| `build/` growth for each node reload | 50 KB | 15 KB |
| `build/` growth for each types reload | 181 KB | 111 KB |
| `build/` after 120 reloads | 9.4 MB, 164 files | 4.6 MB, 164 files |
| Private bytes after 120 reloads | +2.6 MB | +2.4 MB |
| Working set after 120 reloads | +9.4 MB | +4.2 MB |
| Cargo target folder at the end | 133 MB, from 12.6 MB | 23.5 MB, from 2.9 MB |

### Findings

- **Old builds stay loaded at a low cost.** 121 copies of cluster A, 21 of cluster B and 21 of
  the types library added less than 10 MB of working set. The memory growth is small next to the
  disk growth.
- **The builds were not reproducible.** Two builds of the same source differed in the PE time
  stamps and the PDB ID. The code was the same. With `-C link-arg=/Brepro`, the two files were
  identical. Spec 8.5 now requires reproducible builds.
- **Content-hash names work.** With `/Brepro`, a build of a restored source gave the name of
  the loaded file. Thus an undo needs no new copy. The cache key must still hash the inputs,
  because the engine needs the key before the compile (8.6).
- **The drop function is in the cluster.** The drop function of `Probe` was in the cluster A
  file, not in the types library, in both profiles. Thus a live value needs its cluster, not
  only the types library.
- **Windows locks a loaded library.** The delete failed with "Access is denied" (os error 5).
  A rename of the loaded library worked.
- **LTO fails with `-C prefer-dynamic`.** With `lto = "fat"` or `"thin"`, rustc refused: "linker
  plugin based LTO is not supported together with `-C prefer-dynamic`". With `-C lto` in
  `RUSTFLAGS`, rustc also refused. Spec 8.3 now has bake mode without LTO, as an agent decision.
- **The cargo target folder grows fast.** Each types reload added about 5.8 MB in `dev` and
  1 MB in `release`, mostly incremental data. The engine needs a cleanup of the target folder.

### Known limits

- The test ran only on Windows, because the loader supports only Windows.
- The cleanup of old builds when a project opens (8.5) is not tested.
- The memory of one row is noisy, because the test and the Windows loader also allocate. Use
  the totals of a long run.
- `/Brepro` is a flag of the MSVC linker. With `rust-lld` on `x86_64-pc-windows-gnu`, the flag
  is `--no-insert-timestamp` (see Toolchain). The hot reload test did not run on that target.
- The cleanup of the cargo target folder is not designed yet.
- The harness is not the engine. It shows the method, and the engine will use the same method.

## Toolchain

The code is in [prototypes/toolchain](prototypes/toolchain/README.md). It ran on 2026-11-15 on
Windows 11 Pro, from an empty folder. It installed Rust 1.100.0 for `x86_64-pc-windows-gnu`
and Zig 0.17.1. It did not run on a clean computer, on Linux or on macOS.

### What was checked

The run passed all 12 steps:

- `curl.exe` downloaded `rustup-init.exe`, the channel manifest and the 4 packages of the
  minimal profile. The SHA-256 hash of each file was correct. Rustup installed from this local
  mirror, with `--no-modify-path`.
- `curl.exe` downloaded Zig, the hash was correct, and `tar.exe` unpacked it.
- The build ran offline, with a clean environment. `PATH` had only the Rust tools and
  System32. The other variables were removed, except 10 system variables.
- Zig compiled the C code of the local crate `cmix` and the bundled SQLite of
  `libsqlite3-sys`: 10 `zig cc` calls and 4 `zig ar` calls.
- `rust-lld` linked `node.dll`. It imports only Windows system DLLs and `msvcrt.dll`. It has
  no Rich header, so the MSVC linker did not make it.
- A new process with the clean environment loaded the library. The C function, the SQLite
  query and 4 Rust threads gave the expected results.
- The folder moved to a name with spaces, an e with an acute accent and a CJK letter. The
  build and the call passed again.
- After the run, the user `PATH` and the Rustup key in the registry had not changed.

Other checks on the same toolchain:

- Two clean `release` builds of `node.dll` gave different files. With
  `-C link-arg=--no-insert-timestamp`, the two files were identical.
- A build of the prototype with `-C target-feature=+crt-static` does not refer to
  `VCRUNTIME140.dll` or to the UCRT DLLs. This static build ran all 12 steps with the files
  that it holds.
- A Zig object with `snprintf` linked with the `msvcrt` of `rust-mingw`, and gave the correct
  text.

### Measurements

| Step | Time |
| --- | --- |
| Install Rust: downloads and rustup | 185.0 s, with a stalled download |
| Install Zig: download and unpack | 24.4 s |
| Download the crates | 1.9 s |
| Cold `dev` build of the node | 5.6 s, and 2.7 s with the Zig cache |
| Build after the move | 4.8 s |

- The downloads are about 265 MB: rustc 104 MB, Zig 100 MB, rust-std 28 MB, `rustup-init`
  15 MB, cargo 12 MB and rust-mingw 5 MB. The crates of the fixture are not included.
- After the run, the folder held 1.4 GB: `rustup` 845 MB, `zig` 416 MB with its cache, `cargo`
  45 MB, and the project with its target folder 82 MB.

### Findings

- **GNU `ld` cannot open a CJK path.** After the move, the GNU `ld` of `rust-mingw` failed:
  "cannot open output file ... Invalid argument". It reads its arguments in the ANSI code
  page, so the CJK letter became "?". The e with an acute accent worked, because code page 1252
  has it. `rust-lld` passed. Spec 13.2 now uses `rust-lld`, as an agent decision.
- **`rust-lld` needs no extra files.** rustc finds it in its own folder and uses the start
  files and import libraries of `rust-mingw`. Cargo also uses it for build scripts, because
  the target is the host.
- **An allowlist firewall blocks new programs.** This computer allows outgoing connections
  only for known programs. The new `rustup-init.exe` and `cargo.exe` failed with os error
  10051. `curl.exe` of System32 is allowed, so the prototype downloads all toolchain files with
  it. Spec 13.2 now says that Patina makes these downloads itself, as an agent decision.
- **A download can stall.** In the clean run, curl stopped at 372 KB of rust-mingw for more
  than 60 s. Then the step passed. Hypothesis: the speed limit stopped the transfer, and the
  retry of curl downloaded the file.
- **The C code links with `msvcrt.dll`, not the UCRT.** Windows has `msvcrt.dll` in System32.
- **Zig adds a sanitizer in `dev` builds.** A Zig debug object had a `__ubsan` symbol. With
  `-fno-sanitize=all`, it had none. The wrappers add this flag.
- **A folder rename can fail one time.** In one run of the static build, the move of the
  folder failed with "Access is denied" (os error 5). No process ran from the folder. The next
  run passed. Hypothesis: a virus scan or the search index held a file. The engine needs a
  retry when it renames a folder.
- **The normal prototype build needs `VCRUNTIME140.dll`.** A clean computer does not have it.
  The static build does not need it, and the Zig wrappers are copies of that file.

### Known limits

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

## Viewport

The code is in [prototypes/viewport](prototypes/viewport/README.md). It ran on 2026-11-15 on
Windows 11 Pro, with Rust 1.94.0, Tauri 2.12.5, `wgpu` 30.0.2 and WebView2 157, in the
`release` profile. It did not run on Linux or macOS. The computer is the computer of the
connections prototype.

### What was checked

The bench ran in an off-screen window that did not take the focus. It measured three frame
paths:

- **native**: the `wgpu` surface is on the window, under a transparent WebView2.
- **shared**: a GPU readback into WebView2 shared memory. The page draws it with WebGL2.
- **channel**: the same readback, sent to the page through a Tauri channel.

For each path, the bench dragged the camera of viewport 1 for 2 s in three scenarios: one
viewport, two viewports, and one viewport at 1920 x 1080. Then 11 checks captured the composed
window with `PrintWindow` and counted the scene pixels:

- The **Inspector** panel and an open menu cover the scene. The scene comes back after the
  menu closes.
- A splitter drag and a window resize change the viewport size. On the native path, the resize
  configured the surface again.
- Viewport 1 hides, restores, closes, reopens, floats into its own window and docks again.
- Keys go to a text field or to the focused viewport, never to both.
- The viewport follows a display scale of 200% and 100%, emulated with the zoom of the web view.
- Two viewports show the same GPU data.
- The bench destroyed the device while the surface textures were held. The render thread made
  a new device, and the scene came back.

The final runs passed 11 of 11 checks in each path: on the RTX 4090 and the Intel UHD 770 with
DX12, and on the RTX 4090 with Vulkan (native only). Each probe passed at its first capture.

### Measurements

The latency on the native path is the time from the input event of the page to the present
call. On the copy paths, it is the time to the end of the page draw. Neither includes the
compositor or the display. Work is the render-thread time of a frame. With DX12, it includes
the vsync wait. CPU is the CPU use of the process tree, in cores. Bytes is the mean copy for
each viewport frame.

RTX 4090, DX12:

| Path | Scenario | Viewport FPS | Page draw FPS | Latency p50 | Work p95 | Bytes | CPU | GPU 3D |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| native | one | 60 | - | 33.0 ms | 17.0 ms | 0 | 0.41 | 0.8% |
| native | two | 120 | - | 33.1 ms | 17.0 ms | 0 | 0.18 | 3.0% |
| native | full | 60 | - | 33.2 ms | 17.0 ms | 0 | 0.22 | 2.5% |
| shared | one | 60 | 60 | 49.5 ms | 1.2 ms | 6.3 MB | 0.34 | 23.9% |
| shared | two | 120 | 120 | 49.3 ms | 1.4 ms | 3.9 MB | 0.43 | 29.2% |
| shared | full | 60 | 59.5 | 49.6 ms | 1.3 ms | 8.3 MB | 0.27 | 29.1% |
| channel | one | 60 | 19.8 | 112.4 ms | 1.4 ms | 6.3 MB | 1.11 | 15.8% |
| channel | two | 120 | 39.7 | 109.5 ms | 1.8 ms | 3.9 MB | 1.28 | 19.3% |
| channel | full | 60 | 15.7 | 132.7 ms | 1.6 ms | 8.3 MB | 0.98 | 19.0% |

Intel UHD 770, DX12 (`--low-power-gpu`):

| Path | Scenario | Viewport FPS | Page draw FPS | Latency p50 | Work p95 | Bytes | CPU | GPU 3D |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| native | one | 60 | - | 33.0 ms | 16.9 ms | 0 | 0.35 | 20.9% |
| native | two | 120 | - | 32.8 ms | 17.1 ms | 0 | 0.18 | 32.5% |
| native | full | 60 | - | 33.0 ms | 17.0 ms | 0 | 0.13 | 31.7% |
| shared | one | 60 | 60 | 49.5 ms | 1.0 ms | 6.3 MB | 0.32 | 31.9% |
| shared | two | 120 | 120 | 49.3 ms | 1.3 ms | 3.9 MB | 0.32 | 44.1% |
| shared | full | 60 | 60 | 49.4 ms | 1.1 ms | 8.3 MB | 0.29 | 43.2% |
| channel | one | 60 | 20 | 99.2 ms | 1.4 ms | 6.3 MB | 0.93 | 16.3% |
| channel | two | 120 | 39.7 | 148.8 ms | 1.9 ms | 3.9 MB | 1.50 | 24.2% |
| channel | full | 60 | 15.2 | 132.7 ms | 1.9 ms | 8.3 MB | 1.04 | 24.2% |

Vulkan, native path:

| GPU | Scenario | Viewport FPS | Latency p50 | Work p95 | CPU | GPU 3D |
| --- | --- | --- | --- | --- | --- | --- |
| RTX 4090 | one | 60 | 33.1 ms | 1.0 ms | 0.35 | 0.5% |
| RTX 4090 | two | 119 | 31.9 ms | 1.4 ms | 0.15 | 2.0% |
| RTX 4090 | full | 59.6 | 17.0 ms | 1.4 ms | 0.17 | 2.5% |
| Intel UHD 770 | one | 60 | 25.5 ms | 0.8 ms | 0.31 | 24.1% |
| Intel UHD 770 | two | 120 | 25.3 ms | 0.9 ms | 0.27 | 28.1% |
| Intel UHD 770 | full | 60 | 25.5 ms | 0.9 ms | 0.28 | 24.5% |

- The Intel UHD 770 rows with Vulkan are from a run before the DX12 default. That run passed
  10 of 11 checks.
- A device loss took 152 to 337 ms from the loss to the first new frame, on both GPUs and both
  backends.
- The upload of a copied frame into a WebGL texture took 0.6 to 0.8 ms at p95.
- An earlier Vulkan run on the RTX 4090, before the last fixes, gave a shared-path page draw of
  30 to 39 FPS with one viewport, and 27.5 ms native latency.

### Findings

- **The native path is the best path.** It held 60 FPS for each viewport on both GPUs, with no
  copied bytes and 0.13 to 0.41 CPU cores. Spec 15.8 now has the native path, as an agent
  decision.
- **Vulkan on the Intel UHD 770 did not show reliably under the web view.** In the last 4
  native runs with Vulkan, the viewport areas of many captures stayed black for 1 s or more.
  1 to 7 checks failed in 3 of the runs. The page areas of the same captures were correct. With
  DX12, 4 runs on the Intel UHD 770 passed 11 of 11, each probe at its first capture. Spec 15.8
  now uses DX12 on Windows, as an agent decision.
- **The copy paths add latency.** The shared path had 49 to 50 ms at p50 on both GPUs, against
  17 to 33 ms on the native path. It copied 378 to 498 MB/s. On DX12, its page drew 60 FPS.
- **The channel is too slow.** The page drew 15 to 40 FPS, with 99 to 149 ms of latency and
  about one CPU core. Each frame also allocates.
- **The latency to the present call does not compare between the backends.** With DX12, it was
  33 to 34 ms in each run, and the frame work included the vsync wait. With Vulkan, it was 17
  to 33 ms, and a timer paced the frames, so the image can wait after the present call.
  `NOT_ENOUGH_INFO` for the display latency: the bench does not measure the compositor.
- **`wgpu` 30.0.2 keeps a held surface texture after a device loss.** It discards the texture
  only on a valid device. Then the surface drop panicked on Vulkan, and a configure failed on
  DX12. The prototype releases the texture with a drop in a caught unwind, the method of
  gfx-rs/wgpu issue 8243.
- **DX12 needs new surfaces after a device loss.** An old surface did not configure on the new
  device. The render thread now makes a new surface for each window.
- **A surface texture must drop before its surface.** Else `wgpu` panicked on DX12 when a
  window closed during the vsync wait.

### Known limits

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

## Connections

The code is in [prototypes/connections](prototypes/connections/README.md). It ran on 2026-11-15
on Windows 11 Pro, with Rust 1.94.0, Tauri 2.12.5, WebView2 157, React 19.3.1 and React Flow
12.12.1. It did not run on Linux or macOS. The computer has one 1920 x 1080 monitor at 60 Hz, a
display scale of 125%, an RTX 4090 and an Intel UHD 770.

### What was checked

The bench ran these checks in an off-screen window that did not take the focus:

- The graph has 500 nodes and 1,000 connections. 200 connections are buses of 8 to 32 lanes.
  The graph has 4,376 lanes.
- A Rust thread sent the lane states at 60 FPS as packed bits in a Tauri channel (15.6). The page
  received all frames, with no gaps.
- The view panned, zoomed, selected a bus each 6 frames and dragged a route point, while the
  states changed.
- The SVG renderer and the WebGL renderer ran all scenarios on the RTX 4090 and on the Intel
  UHD 770.
- Other runs stopped the feed, sent JSON events, and emulated a DPR of 1 and 2.
- Screenshots of the two renderers showed the same wires, lanes, badges, selection and route
  handles. At a far zoom, a bus showed as one line.

### Measurements

The tables come from runs of 10 s with a warmup of 1 s. The defaults are now 3 s and 0.5 s, and
a default run gave the same result in 46 s. Work is the main-thread time of a frame. CPU is the
CPU use of the process tree, in cores. The rows use a DPR of 1.25 and the live feed, unless the
row gives a different value.

RTX 4090:

| Scenario | SVG FPS | SVG dropped | SVG work p95 | SVG CPU | WebGL FPS | WebGL dropped | WebGL work p95 | WebGL CPU |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| idle-far | 60 | 0 | 7.6 ms | 1.31 | 60 | 0 | 0.9 ms | 0.19 |
| idle-near | 60 | 0 | 3.1 ms | 1.31 | 60 | 0 | 0.6 ms | 0.12 |
| pan | 42.63 | 174 | 8.9 ms | 1.51 | 60 | 0 | 3.4 ms | 0.53 |
| zoom | 53.21 | 68 | 15.5 ms | 1.51 | 57.31 | 27 | 18.3 ms | 0.92 |
| select | 60 | 0 | 5 ms | 1.32 | 60 | 0 | 0.8 ms | 0.22 |
| drag | 60 | 0 | 5.8 ms | 1.55 | 60 | 0 | 1.4 ms | 0.24 |

Intel UHD 770 (`--low-power-gpu`):

| Scenario | SVG FPS | SVG dropped | SVG work p95 | SVG CPU | WebGL FPS | WebGL dropped | WebGL work p95 | WebGL CPU |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| idle-far | 60 | 0 | 8.3 ms | 1.43 | 59.9 | 1 | 1.7 ms | 0.39 |
| idle-near | 23.82 | 363 | 4.9 ms | 1.25 | 60 | 0 | 1.2 ms | 0.26 |
| pan | 11.49 | 490 | 22.3 ms | 1.08 | 60 | 0 | 6 ms | 0.94 |
| zoom | 19.2 | 408 | 32.9 ms | 1.34 | 53.89 | 61 | 24.2 ms | 1.23 |
| select | 17.87 | 422 | 8.6 ms | 1.24 | 60 | 0 | 1.1 ms | 0.22 |
| drag | 18.1 | 419 | 10.1 ms | 1.36 | 60 | 0 | 1.4 ms | 0.25 |

Other runs on the RTX 4090. The zoom of the web view emulates the DPR, so the window keeps the
same physical size:

| Run | Scenario | FPS | Dropped | Work p95 | CPU | GPU 3D |
| --- | --- | --- | --- | --- | --- | --- |
| SVG, feed off | idle-near | 60 | 0 | 0.1 ms | 0 | 0% |
| SVG, feed off | pan | 42.5 | 175 | 6.4 ms | 1.35 | 19% |
| WebGL, feed off | idle-near | 60 | 0 | 0.1 ms | 0.03 | 0% |
| WebGL, feed off | pan | 60 | 0 | 3.3 ms | 0.42 | 12% |
| WebGL, JSON events | idle-near | 60 | 0 | 0.7 ms | 0.16 | 6% |
| SVG, DPR 1 | idle-near | 60 | 0 | 4.1 ms | 1.44 | 21% |
| SVG, DPR 1 | pan | 60 | 0 | 10 ms | 1.48 | 26% |
| SVG, DPR 2 | idle-near | 60 | 0 | 2 ms | 1.02 | 22% |
| SVG, DPR 2 | pan | 49.32 | 107 | 5.8 ms | 1.44 | 18% |
| WebGL, DPR 2 | idle-near | 60 | 0 | 0.6 ms | 0.11 | 4% |
| WebGL, DPR 2 | pan | 60 | 0 | 3.5 ms | 0.4 | 11% |

- A frame of 4,376 lanes has 559 bytes. On the RTX 4090, the Rust send took 23 to 35 us at p50
  and 104 to 164 us at p99.
- On the RTX 4090, the latency from the Rust send to the page was 0.23 to 0.26 ms at p50 with
  WebGL. It was 0.5 to 6.3 ms with SVG.
- On the RTX 4090, the GPU 3D use was 3 to 18% with WebGL and 17 to 25% with SVG. On the Intel
  UHD 770, it was 29 to 58% with each renderer.

### Findings

- **The WebGL layer holds 60 FPS.** It held 60 FPS on both GPUs in all scenarios, except the
  zoom. It used 2 to 11 times less CPU than SVG. Spec 15.1 now has the WebGL layer and React,
  as agent decisions.
- **SVG drops frames when the view moves.** On the RTX 4090, the pan ran at 43 FPS with and
  without the feed. Thus the SVG edges cause the cost, not the feed. On the Intel UHD 770, SVG
  ran at 11 to 24 FPS in each scenario with a near view.
- **The DPR changes the SVG cost.** The SVG pan held 60 FPS at a DPR of 1. It dropped frames at
  1.25 and 2, with the same physical window size. `NOT_ENOUGH_INFO` for the cause: the bench
  does not split the paint and the raster time.
- **The nodes drop frames at a far zoom.** In the zoom, WebGL also dropped frames: 27 on the
  RTX 4090 and 61 on the Intel UHD 770. At a 20% zoom, React Flow shows all 500 nodes, and the
  page has 2,331 elements. The graph view needs a simpler node form at a far zoom (15.1).
- **Packed bits are small and fast.** The JSON events also held 60 FPS at this size. In the
  Tauri source, a channel sends a payload under 1 KB by `eval`, not by a fetch. Each send
  allocates one time, because the channel takes the frame by value.
- **A navigation from a Tauri command runs the command twice.** The navigation stops the IPC
  request, and the IPC script of Tauri then sends the command again. Thus the page must
  navigate itself after the command returns.
- **An off-screen window can get the focus.** Two full runs each had one focus event, with
  `focused(false)`. With `focusable(false)`, tao sets `WS_EX_NOACTIVATE`, and two runs had no
  focus event. Hypothesis: Windows activated the next window when another window closed.
- **SVG covers the badges.** In the SVG renderer, a later edge covers the badges of the edges
  before it. The WebGL renderer draws all badges on an overlay above the wires.

### Known limits

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
