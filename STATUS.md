# Status

The current milestone is milestone 5, stateful nodes, node GUIs and generic nodes (spec section
19). Stateful nodes work. Milestone 4 ended on 2026-11-15. Milestones 2, 3 and 4 work, with the open items in their
known limits. The installers of
milestone 3 are not made yet: the WiX tools need a download that the owner did not approve yet.
Milestone 1, the engine and the CLI, works. Milestone 0 ended on 2026-11-15, with three owner
checks open.

## Milestone 5: stateful nodes, node GUIs and generic nodes

Three slices work: stateful nodes (4.2), generic nodes (4.5) and the delay node (9.2). They
ran on 2026-11-15 on Windows 11 Pro, with Rust 1.94.0. Node GUIs (4.3) work in the engine
and in the editor.

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
  widgets: a label, a button, a toggle, a slider, a number field, a text field and a dropdown. A panic in `ui` drops the state, as in `process`.
- A graph can have a feedback loop in the frame domain through a delay node
  (`builtin = "delay"`). The delay gives the value of its input in the previous frame, and
  `Default::default()` in the first frame.

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
- The parser test finds `ui`, and refuses `ui(&self, ...)`, `ui: &Ui` and a private `ui`.
- The `Ui` test reads the events of a button, a toggle and a slider, and clamps the slider.
- The plan test orders a loop through a delay and gets its type from the other wire. It refuses
  a delay with no input wire, an unknown built-in node, and a loop of only generic nodes.

### Known limits

- The editor draws the node GUIs in the node body, and the large size of the selected node in
  the **Inspector**. It draws them again after each run and each frame. A change in a live
  graph runs the graph again (4.3). The frontend test checks the widget lines. A Rust check and
  the frontend build passed. The editor selfcheck does not use a node GUI, and nobody clicked a
  node GUI in the editor.
- A node with `#[node(full_gui)]` has an expand button in its header and in the **Inspector**
  header. The button opens the full size over the graph area. **Back** or Esc closes it. The
  parse test and the macro test check `full_gui`. Nobody opened a full size in the editor.
- Each build makes a new cluster, so a build resets the state of all nodes. Milestone 6 keeps
  the state of each node whose code did not change (7.2).
- A state that is not `Send` makes all the tasks of the cluster run on the thread of the run,
  not only the task of the node (9.2).
- No test makes a real cluster panic in `process`. The glue test checks the rule.
- The editor was not run with a stateful or a generic node. It uses the same engine code as
  the CLI.
- The editor shows a generic pin with the name of its type parameter, not in gray. The Add node
  palette joins a wire to a generic pin of any type. Its list has the built-in delay node, after
  the nodes of the project. The editor selfcheck sees the delay in the list, but does not add it.
- Only stateful nodes can have a GUI. The value widgets, the audio `ui` with `#[settings]` and
  widget folders are not done.
- A node GUI can have a canvas with lines, rectangles, circles and text. The canvas sends the
  pointer down, the moves of a drag (one for each frame at most) and the up. A custom widget
  implements the `Widget` trait. No test runs a canvas in the editor.
- shortcut: the engine draws all node GUIs after each run and frame, also the nodes out of view
  (4.3). Send the visible nodes from the page when graphs get large.
- The delay runs only in the frame domain.
- A loop of only generic nodes and delays has no type, so the engine refuses it.
- Two Linux CI runs counted 1 allocation in 100 frames of the hello test. A slow frame can start
  the worker pool for the first time inside the counted frames. A parallel cluster now prepares
  the pool when it loads. One CI run with this fix passed on all three platforms.
- The types go only from an output to an input. A type parameter that only an output uses
  stays unresolved, also when a wire goes from that output to a pin of a known type.

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
- In the editor, **Run** runs the batch domain and then the frames. **Pause**, **Stop**, a
  failed frame or a change that runs the graph again ends the frames. The engine sends an event
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

- No delay node yet, so a graph cannot have a feedback loop. Milestone 5 added it.
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
- A latest-value wire uses a shared slot with a lock, not a buffer that does not lock (9.1).
  The two domains do not run at the same time yet, so the lock never waits.
- A batch value that a frame node reads stays after the frames end, until the next batch run.
- Ctrl+C stops the CLI at once. The values that the frame domain keeps do not drop.
- These shortcuts are open in the code:
  - The editor copies each frame into a JSON event (`crates/patina-editor/src/engine.rs`).
  - A latest-value wire uses a lock that never waits, because the domains do not run at the
    same time (`crates/patina-engine/src/generate.rs`).

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
  drag. A route has rounded corners.
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
  selected wire.
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
  file has a dot on its tab. The tab shows the build state of the saved code (15.10).
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
- The **Views** menu shows and hides each panel, and **Reset layout** gives the default layout.
  The layout and the pan and zoom of the graph go to `data/layouts/<project id>.json`, 300 ms
  after a change and before a close or a quit.
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
- The check does not drag nodes. It pushes only Ctrl+S and Delete. A person did not use the
  editor yet.
  The owner check: make the first workflow of 15.10 with the mouse.
- The content security policy was not tested with a page that breaks it.
- The check did not test the SVG wires or a lost WebGL context.
- The capture works only on Windows. The check did not run on Linux or macOS.
- These parts of spec 15 are not in the editor yet: float windows, view tabs in the center
  and the **Layers** tab (15.2), rust-analyzer in Monaco (15.1) and the **Debug** tab.
- The palette of a drag from a pin compares the type text (15.2). Two paths of one type, for
  example `String` and `std::string::String`, do not match.
- The **Views** and **Widgets** groups have no **+** button yet. Undo does not remove a new
  node folder (15.2).
- The check does not test the **does not stop** message of **Stop** (15.2), because that needs
  a node that ignores the flag for more than 2 s. `stop_requested()` is false on a thread that a node
  starts. The engine has no async nodes (4.6) yet, so no node drops at a wait.
- The self-check does not run a node with the `log` crate, because that needs crates.io. The
  type check covers the log lines of the page, but no check showed them in the editor yet.
- The log lines show only the levels Info, Warn and Error. The logger drops a record from a
  thread that a node starts. A `log` crate with another name, or in a target table, does not
  turn on the logger. A run shows 1000 log lines at most.
- The `Viewer` gives text only. The table, the image, the plot, the waveform and the thumbnail
  come later (5.3). The **Inspector** reads the full text one time after each run.
- The editor has no bench with 500 nodes, so the far zoom form was not measured for frame
  drops. Two badges can cover each other. After each run, the engine copies the text of each
  value into a new `String`, and the badge text into a second `String`.
- An orthogonal segment cannot move sideways (15.9). A double-click on an orthogonal or curved
  wire puts the new point on the nearest straight line through the route points. The graph has
  no setting for the default style. **Bus**, **Lock bends** and the crossings wait for the
  automatic router. The keyboard cannot add a route point. The editor does not warn when a wire
  goes under a node.
- The check did not test Esc during a drag, the arrow keys, or the route handles with the SVG
  wires.
- The recovery file does not have the typing of the last 300 ms before a crash. The check did
  not crash Patina, and it did not click **Discard**. A project that is not trusted keeps its
  old recovery file, and its new changes do not go into the file until the user answers.
- A connection kind matches a type by the type text that the node code writes, not by the type
  crates of 5.1. A panic in a kind gives no lanes and no message. A compile error in a kind
  stops the build of all of the graph.
- The lanes have no names and no groups, and a bus socket has no contact for each lane (15.9).
  The badge is always the viewer text. The **Code** tab does not open the files of
  `connections/`.
- The self-check project has no connection kind. The lanes ran in the CLI test, and the look
  had one manual check with fake lanes in the page. No check zoomed a bus out below 0.5.
- The check did not drag a tab or a splitter with the mouse. A layout change in the last
  300 ms before the window closes with no unsaved changes is not saved.
- Vite shows a warning for the `"use client"` lines of React Flow, and a warning for the size
  of the page script. The build is correct.
- `package.json` sets `dompurify` to 3.4.17 with an override. Monaco 0.57.0 asks for 3.4.15,
  which has the critical advisory GHSA-p98j-92pf-mc4p. `npm audit` now finds 0 problems.
- These shortcuts are open in the code:
  - Monaco has no rust-analyzer. Milestone 3 connects it (`frontend/src/code.ts`).
  - Monaco is in the main page script, which is 3.7 MB. Load it when the **Code** tab opens if
    the start time gets slow (`frontend/src/code.ts`).
  - The **Code** tab forgets the cursor and the scroll position when it closes
    (`frontend/src/CodeTab.tsx`).
  - The capture works only on Windows (`src/session.rs`).
  - One texture row holds the points of each wire. Above `MAX_TEXTURE_SIZE` wires (2048 or
    more, by GPU), SVG draws the wires (`frontend/src/gl.ts`).
  - A route of more than 62 points draws only its first 62 points. Only a graph file that a
    person edited has such a route (`frontend/src/wires.ts`).
  - Each run formats and sends each held value, also when a graph runs faster than the display
    (15.6) (`crates/patina-sdk/src/glue.rs`).
  - A new lane state builds the instances of all wires again (`frontend/src/wires.ts`).
  - The SVG wires draw a bus as one line (`frontend/src/WireLayer.tsx`).

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

- One cluster, with a static `std`, in the `dev` profile. Clusters that share types, hot reload
  and **Bake** come at milestone 6.
- Only function nodes. Stateful nodes, node settings and generic nodes come at milestone 5.
  Subgraphs come at milestone 7, and async nodes at milestone 12.
- The toolchain comes from the toolchain folder (13.3), or else from the `CARGO` variable or
  `PATH`. Its version must be the `rust` version of `patina.toml`.
- **Install toolchain** works only on Windows. It installs rust-analyzer too, but no install
  with rust-analyzer ran yet, because the run downloads about 285 MB. A toolchain from an older
  install has no rust-analyzer. Remove the `toolchain` folder and install again.
- No GitHub release makes the portable zip yet. The **Package** workflow keeps the zips only as
  artifacts of the run.
- The engine loads the library from the cargo target folder, and never unloads it. Windows
  locks the file, so one process cannot build the same project again after the load.
- A node panic shows two messages: the default panic message of the cluster, and the report.
- A `Result` output is found by the type name `Result` only. A type alias with another name is
  not found, and then an `Err` is a value, not a failure.
- The editor does not compare the pin types when the user connects two pins (5.1). A wire
  between two types that do not agree gives a compile error at the next build. The type key
  check needs the type crates of 5.1. The type label is the text of the code, so `String` and
  `std::string::String` show differently.
- The CLI runs the graph one time. The clock domains (9.1) come later.
- These shortcuts are open in the code:
  - The CLI finds only the toolchain folder next to it (`crates/patina-cli/src/main.rs`).
  - One cluster with a static `std` in the `dev` profile (`crates/patina-engine/src/cargo.rs`).
  - The library loads from the target folder (`crates/patina-engine/src/lib.rs`).
  - Only function nodes (`crates/patina-engine/src/node.rs`,
    `crates/patina-macros/src/lib.rs`).

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
