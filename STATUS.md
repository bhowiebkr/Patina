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

- Hello and gpu examples: hello is a pocket calculator with keypads, tapes and an **AVG** subgraph. gpu has color bars, a mix, a knob and a waveform monitor.
- Node symbols and styles: Gate and block nodes show circuit symbols, and a subgraph sets a **Shape** for its uses. Each example has its own style crate in `widgets/`.
- Subgraph tools: **New subgraph** in the **Graphs** group makes an empty subgraph. Two or more selected nodes show **Collapse into subgraph...** in the **Inspector**.
- Editor menus: The **Project** panel has right-click menus, and the WebView menu does not show. A build keeps the node GUIs, and only the slot node shows **Compiling**.
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
  does not save, so the terminal, the frame and the key buffer are empty after an open. A
  hidden `--shot` run types a text into the keyboard and sees it in the key buffer (2026-11-18).
  The shot then picks `echo.asm` in **Program** and sees the keys on the terminal (2026-11-18).

### Start screen

- The **x** button of a recent project removes it from the list or moves its folder to the Recycle
  Bin (2026-11-17). The self-check clicks **Close**, the **x** of the project and **Remove from
  list**, and runs the refusal of **Delete project** through the API. No check moved a project to
  the Recycle Bin.
- A new project has no node folders (2026-11-17). The engine test and the self-check open an empty
  new project, and the self-check then writes the hello nodes into it.
- The self-check uses the start screen as a person does (2026-11-18). It fills the **New project**
  form and clicks **Create**. It removes the trust of the project in **Settings**, opens the
  project with the **Open project** form, and clicks **Trust project** in the banner. It also
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
  it on its canvas. The example is `examples/computer/nodes/button`.
- The parameter writes of one node GUI input are one undo step (2026-11-18). A canvas drag from
  the down to the up is one input. The writes go out in order. An engine test and the free knob
  step of the self-check read it.
- Known limits: the self-check drop is a page event, not a drag of the operating system. A wire
  leaves a free pin only to the left or the right.
- A bus wire fans its lanes in to the dot pin of a free node or a symbol node over its last 16 px
  (2026-11-18). A frontend unit test reads the geometry and the lane under the pointer in the
  taper. A hidden shot showed the 8 lanes of **op** fan in to the **SPLIT** dot of the cpu.
- `#[node(symbol = "<shape>")]` shows a gate or block symbol in place of the box (2026-11-18).
  A subgraph sets **Shape** and **Label** in the **Look** section, with undo. The engine, CLI
  and frontend tests read it. The self-check gives a subgraph the chip shape and undoes it. It
  also adds a node with the AND symbol.
- Each example has a style crate (2026-11-18): `widgets/parts` (a circuit board) in computer,
  `widgets/calc` (a pocket calculator) in hello and `widgets/scope` (a lab instrument) in gpu.
  A placed image or field fills the width to the same right margin. A free node shows no canvas
  label row, so its pins and placed widgets line up with the drawing. The tests of each crate pass.
- The **Inspector** put a placed widget at the wrong place, for example the **Type** field of
  the keyboard on the canvas title (fixed 2026-11-18). A GUI with placed widgets now has no
  padding and no canvas label there too. The shot checks that each placed widget is on its canvas.
- A node covered the full badge of a selected wire, for example `GpuImage 64x16` in gpu (fixed
  2026-11-18). The badge and the label of a selected wire are now above the nodes. The shot
  checks that no node covers the badge.
- The shot checks the layout of each graph that it opens: no widget is out of its node or the
  **Inspector**, no label cuts its text, no two nodes overlap, no wire goes through a node body
  and no wire has a route conflict mark (added 2026-11-18). It also selects each node of the
  main graph, and the first node of each new kind in a subgraph. Its **Inspector** must not
  scroll to the side, cut a label, show an error or show a GUI that cannot draw (added
  2026-11-18).
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
- The hello example has a pocket calculator (2026-11-18): two **keypad** nodes, an
  **operation** with four keys, two paper **tape** nodes and an `average` subgraph with the
  **AVG** chip. The unit tests of the nodes and the CLI tests pass. A hidden shot showed 36
  and 7.5 on the tapes.
- Known limits: a click on a key gives a new run, so the tape prints each edit, also an edit
  that the user undoes. Nobody clicked the keys by hand.
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
- While the audio thread of the editor runs, a view reads no output of an audio node, and the
  GUI of an audio node does not draw. The badges of the audio outputs still show.
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
- Only stateful nodes can have a GUI. The audio `ui` with `#[settings]` comes with the audio
  domain (milestone 11).
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
  A script sent the messages of the client to rust-analyzer 1.100 on a copy of `examples/hello`.
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
- `crates/patina-editor/frontend/src/canvas/NodeView.tsx:332`: A wire leaves a free pin only to the left or the right. Add a side to `Ui::pin` when a node needs it.
- `crates/patina-editor/frontend/src/canvas/palette.ts:37`: The palette drops module paths, so two types of one name match. Rustc checks the types at build time.
- `crates/patina-editor/frontend/src/wires/route.ts:36`: A graph with a larger route grid gets simple orthogonal paths. Use a sparser grid when such graphs matter.
- `crates/patina-editor/frontend/src/wires/WireEdge.tsx:164`: The SVG fallback draws a bus as one line. Draw its lanes when a GPU without WebGL2 matters.
- `crates/patina-editor/frontend/src/wires/WireEdge.tsx:165`: The SVG fallback has no crossing gaps. Add a mask when the fallback is more than a lost context.
- `crates/patina-editor/frontend/src/wires/wires.ts:360`: A path with more than `MAX_POINTS` - 2 bends draws only its first bends. Split the row when this matters.
- `crates/patina-editor/frontend/src/wires/wires.ts:601`: A new lane state builds the instances of all wires again. Put the lane states in a texture for larger graphs.
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
