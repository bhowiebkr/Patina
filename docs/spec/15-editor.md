# Patina Specification: Editor

Part of the [Patina Specification](README.md).

## 15. Editor

### 15.1 Frontend

| Part | Choice |
|---|---|
| Desktop shell | Tauri 2 |
| Framework | React |
| Node editor | React Flow, with a WebGL layer under the nodes for the connections |
| Code editor | Monaco, connected to rust-analyzer |

- Dark theme.
- Sharp text and graphics on high-DPI screens at all zoom levels.
- If HTML/SVG becomes slow with thousands of nodes, change the graph view to a WebGL canvas.
- The connections (15.9) draw on one WebGL2 canvas under the nodes, not as SVG edges. In the connections prototype (19), SVG dropped frames in a pan, and the WebGL layer held 60 FPS with 2 to 11 times less CPU. React Flow with this layer met the targets, so the framework is React. Svelte Flow was not measured. (Agent decision, 2026-11-15. The owner can change it. The measurements are in STATUS.md.)
- Each connection keeps an invisible React Flow edge path on the same curve. This path gets the click, the box selection and the keyboard focus. The WebGL layer draws the visible wires and the selection band. Without WebGL2, or after a lost context, CSS shows the edge paths as SVG wires. (Agent decision, 2026-11-15. The owner can change it.)
- At a 20% zoom, 500 HTML nodes dropped frames in the same prototype. Thus the graph view needs a simpler node form at a far zoom.
- Below a zoom of 0.5, each node keeps its box, its header with the title and the state icon, its border, its pins and its message. Its rows have no pin names and no parameter text. The wires hide their value badges and type labels, except on a selected wire. A bus keeps its label "/ n" (15.9). (Agent decision, 2026-11-15. The owner can change it.)

### 15.2 Layout

The editor window is a dock system. The built-in panels and the views (15.8) dock the same way.

| Area | Default contents |
|---|---|
| Top bar | Project and graph path, Live indicator, Run, Pause, Stop, Bake, undo, redo, zoom, fit, grid, Views menu |
| Left panel | Layers tab: the base and the slot, with build state and build time (7.3). Project tab |
| Center | Graph view, or the full-size GUI of a node (4.3) |
| Right panel (inspector) | Selected node: Parameters, Code, Logs tabs, and the large node GUI |
| Bottom panel | Logs, Output, Debug tabs |
| Status bar | State, error count, project, layer state |

**Docking:**

- The left, right, bottom, and center areas are dock areas. The panels in one area show as tabs.
- The user drags a tab to another area, or out of the window. A tab out of the window floats in its own window, for example on a second screen.
- The **Views** menu opens a view (15.8) as a new tab, in the center by default.
- The top bar and the status bar do not move.
- The table above is the default layout. **Reset layout** in the **Views** menu returns to it.

**Version 1 of the dock:**

This is an agent decision of 2026-11-15. The owner can change it.

- The left, right and bottom areas take panels. The center holds only the graph view. In version 1 of views, the **View** panel opens in the right area (15.8). Tabs in the center come later.
- The panels are **Layers**, **Project**, **Inspector**, **Logs** and **Output**. The **Debug** tab comes later. The left area has the **Layers** tab and the **Project** tab, and **Project** is active at the start.
- Float windows come later. In version 1, a tab cannot go out of the window.
- A tab moves with a drag and drop, or with its right-click menu: **Move to the left area**, **Move to the right area**, **Move to the bottom area** and **Close**. The menu is the method for the keyboard and for screen readers.
- An area with no tabs takes no space. During a drag, it shows as a drop zone.
- A splitter between an area and the center changes the size of the area. An arrow key moves a focused splitter by 16 px, and Shift and an arrow key move it by 64 px. A size is 120 px to 1600 px.
- The **Views** menu has a check mark for each panel. A click shows or hides the panel. A hidden panel opens again in its default area.
- **Reset layout** keeps the pan and zoom of each graph.
- Patina saves the layout 300 ms after a change, and before the project closes or Patina quits. A layout file is JSON of 64 KiB or less.
- Patina saves a layout only for a project ID of 1 to 64 hex digits. For another ID, it shows an error and uses the default layout. Reason: the ID is a file name, so it must not be a reserved name of Windows.
- A graph with no saved pan and zoom fits in the view when the project opens.
- Reason: these parts give the workflow of 15.10 with the panels where the user wants them. Float windows need a second Tauri window for each panel, and the views do not exist yet.

**Saved layout:**

This is an agent decision of 2026-11-15 for question 8 (18). The owner can change it.

- Each user keeps their own layout for each project. Patina saves it in `data/layouts/<project id>.json` of the data folder (13.3), not in the project folder.
- The layout file holds the dock areas and their tabs, the size of each area, the float windows with their screen positions, the open views, and the pan and zoom of each graph.
- A project with no layout file opens with the default layout. A project that moves to another folder keeps its layout, because the file name is the project ID (6.2).
- The state of a view instance (15.8), for example the camera of a 3D viewport, is not saved in version 1. Each view instance starts with `init`.
- Reason: the project folder goes into Git. A layout in it changes each time a user moves a panel, and it gives merge conflicts. Float windows also depend on the screens of each user.
- App mode (question 6 of 18) can add a layout that the project owns. That layout is separate from the layout of each user.

**Project panel:**

- The **Project** tab lists the folders of the project (6.2) in groups: **Graphs**, **Nodes**, **Views**, **Widgets**, and **Types**.
- The user drags a node folder into the graph to add a node.
- A click on a view, widget, or type folder opens its code in the inspector.
- **Import** copies a folder from another project (6.2).
- **Update** compares an imported folder with its origin, and copies the new version when the user confirms (6.2).

The **+** button in the title of the **Nodes** group makes a new node folder. (Agent decision, 2026-11-15. The owner can change it.)

- The button opens a name field under the title. Enter makes the folder `nodes/<name>`. Esc or a click out of the field cancels.
- A name has 1 to 64 letters, digits, `_` and `-`, and starts with a letter. The field shows the problem of a name that does not pass, and keeps the text.
- These names do not pass: the keywords of Rust, the crate names of Rust and Patina, the folder names of Cargo (`build`, `deps`, `examples`, `incremental`) and the device names of Windows (`con`, `prn`, `aux`, `nul`, `com1` to `com9`, `lpt1` to `lpt9`).
- A name also does not pass when a folder in `nodes`, `views`, `widgets`, `connections` or `types` has the same crate name. The compare ignores case and reads `-` as `_`, as Cargo and Windows do.
- The new folder has a `Cargo.toml` and a `src/lib.rs` with a start node: a `#[node]` function with the crate name, an `f64` input `value` and an `f64` output. The **Inspector** shows the new `src/lib.rs`.
- Undo does not remove the folder. The user deletes it in the file manager.
- The **Widgets** group has the same **+** button. A new widget folder has a start widget: a struct that implements `Widget` and draws a bar on a canvas (4.3). The **Inspector** shows its code with the **Code** tab only.
- An untrusted project has no **+** button. The **Views** group gets the same button when views (15.8) come.

The **Import** button at the end of the **Project** panel copies a folder from another project (6.2). (Agent decision, 2026-11-15. The owner can change it.)

- The button opens the folder dialog for the other project. Then the **Import** dialog lists the node, view, widget, connection and type folders of that project.
- When the user selects a folder, the dialog shows each folder that the import copies, with **New**, **Same files, not copied**, or the conflict. For each conflict, the user picks **Replace**, **Keep** or **Rename**, and a rename has a name field. The conflict also shows the folders of this project that a replace changes.
- **Import** stays off until each conflict has a choice. During the build check the dialog shows **Building**. An error stays in the dialog, and the project stays as it was.
- An untrusted project has no **Import** button.
- Above the **Import** button, the **Imports** list shows each imported folder. A click on a folder opens the **Update** dialog for it. Patina finds the other project, or opens the folder dialog.
- The **Package** button after **Import** opens the folder dialog. Patina saves the project and makes an executable package in `<chosen folder>/<project name>` (14.1). The **Logs** panel shows **Making the package**, the warnings and the package folder. An error shows in the status bar. (Agent decision, 2026-11-15. The owner can change it.)
- The **Update** dialog lists the files that are different, with **Only in the other project**, **Only in this project** or **Different**. When the two sides changed, each file has a choice of **Other project** or **This project**. **Update** stays off until each file has a choice, or when the other project did not change. During the build check the dialog shows **Building**.

**Graph view navigation:**

The graph view is snappy. This is an owner decision of 2026-11-15.

- Each input changes the view in the next frame. Pan, zoom, **Fit** and selection have no animation, no easing and no inertia.
- A dragged node or route point stays under the mouse pointer.
- The same rule applies to pan and zoom in the other canvases of the editor, for example value viewers and views.
- The mouse wheel zooms at the mouse pointer. Wheel up zooms in. A touchpad pinch also zooms.
- One wheel step changes the zoom by a fixed factor. A smaller touchpad step changes it by a smaller amount.
- The zoom goes from 10% to 400%. The top bar shows the zoom as a percent.
- The standard controls:

| Input | Action |
|---|---|
| Middle mouse button drag | Pan |
| Space and left drag | Pan |
| Left drag on an empty area | Select the nodes and the connections in a box |
| Shift and click, Ctrl and click | Add to the selection, toggle in the selection |
| F key | Fit the selection, or the full graph when nothing is selected |
| Delete | Delete the selection |
| Esc | Cancel the drag, or clear the selection |
| Double-click on an empty area, Shift+A | Open the **Add node** palette |
| Ctrl+G | Collapse the selected nodes into a new subgraph |
| Ctrl+Shift+G | Expand the selected uses of subgraphs |
| Drag from a pin into an empty area | Open the **Add node** palette for that pin |

The owner asked for the standard controls. The table, the zoom range and the wheel steps are an agent decision of 2026-11-15.

The **Add node** palette is the second way to add a node, after a drag from the **Project** panel. (Agent decision, 2026-11-15. The owner can change it.)

- A double-click opens it at the pointer. Shift+A opens it at the pointer, or at the middle of the canvas when the pointer is not on it.
- It has a search field and a list of the nodes of the project. The list shows each name that contains the text, with no regard to case. The names that start with the text come first.
- The arrow keys move in the list. Enter or a click adds the node, with its top left corner at the point where the palette opened. Esc or a click out of the palette closes it.
- A drag from a pin that ends on an empty area opens the palette at that point. The list has only the nodes with a pin that can join the pin of the drag: an input for an output, and an output for an input, of the same type. The add connects the first such pin of the new node. The new node and its wire are one undo step. (Agent decision, 2026-11-15. The owner can change it.)
- In this match, a `&T` or `&mut T` input takes a `T`, and a `&str` input takes a `String` (5.1). The editor compares the type text, and the build does the full type check (5.2).
- Tab does not open the palette, because Tab moves the focus between the controls.
- An untrusted project has no palette, as it has no drag of a node.

**Run, Pause, Stop, and Bake:**

- Run starts the graph. Batch nodes run one time. Frame and audio domains start their clocks. Triggers start to listen.
- Pause stops the frame and audio clocks and stops new event runs. Active event runs finish.
- Stop does all that Pause does, and it also asks the active runs to stop. This is an agent decision of 2026-11-15 for question 10 (18). The owner can change it:
  - The engine starts no new node call. The nodes that did not run show **Stale** (15.3).
  - An async node (4.6) drops at its next wait.
  - Stop sets a stop flag. A node with a long loop can read it with `patina_sdk::stop_requested()`, and return early.
  - The generated code reads the flag before each node call. The engine shares the flag and the index of the active node with the cluster (C ABI version 3), so the editor can name a node that does not stop.
  - Stop never kills a thread. Native code cannot stop safely in the middle of a call. A call that does not read the flag runs to its end.
  - When a call runs for 2 s after Stop, the status bar shows "Node X does not stop" and the time. Only process isolation (16) can end it. Without process isolation, the user can close Patina.
  - Run after Stop starts the graph again. Node states stay.
- Bake fuses the node in the slot back into the base now (7.2).

### 15.3 Visual rules

The [GUI visual guide](../gui-visual-guide.md) defines the palette, dimensions,
shared controls and image-specific corrections. Use neutral charcoal surfaces,
an amber application accent, and the Aged Dovetail branding artwork of
[assets/branding](../../assets/branding/README.md). Mockup 9 is the
main theme reference. Do not copy the blue-tinted surfaces of earlier mockups.
Node bodies fit their contents; their widths and heights need not match.

**Colors:**

| Group | Colors | Use |
|---|---|---|
| Status | Green, yellow, red | Node state and messages: the state dot, the error border, and the Logs |
| Header | Any color, from the node code | The header of the node. The selected node, and the focused control in its GUI, use a lighter shade of it |
| Accent | One accent color of the theme | Selection and focus everywhere else, for example a selected wire or a field in a panel |

- The node code sets the header color, for example `#[node(color = "#175c59")]`. Nodes for one kind of work usually share a color, for example teal for image filters. The guide proposes a header color for each common kind of work. The **Project** panel shows the color next to each node folder.
- A node with no header color has a neutral gray header. The accent color shows its selection.
- A header color never means a state. The state shows only with the state dot, its icon, and the red error border (the table below).
- [mockup-1-main-editor-colored-headers.png](../mockups/mockup-1-main-editor-colored-headers.png) shows the direction for header colors. Its selected node has the accent outline. Patina uses a lighter shade of the header color instead.
- Subgraph borders and other structure use neutral gray.
- The node in the slot shows a slot icon (7.3).
- Each state also has an icon. Color alone never carries a meaning.
- Node icons show the node kind: function, stateful, generic, async, trigger, or subgraph. Patina uses its own icons.
- A small icon on each node shows its clock domain.
- Cross-domain wires use a dashed line.

**Node states:**

| State | Display |
|---|---|
| Ready | Green dot |
| Compiling | Animated dot |
| Compile error | Red border, error icon |
| Run error | Red border, error icon, error text on the node |
| Stale | Dim. Inputs changed or the last run stopped. Old values and times stay visible but dim |
| Skipped | Dim, with a "skipped" label. An upstream node failed |
| Unresolved | Gray pins. A generic node with unknown types |

The engine reports a node as skipped when a wired input has no value, because an upstream node failed or was skipped. After **Stop**, each node that did not start is stale. A skipped or stale node keeps the old values of its outputs, and their badges show dim. The status bar counts the skipped and the stale nodes. (Agent decision, 2026-11-15. The owner can change it.)

**Times:** build time and run time always have separate labels, for example "Built in 1.8 s" and "Run 0.42 ms".

### 15.4 Inspector

- The header shows the node name, the node kind, and the full path, for example "Color Grade / Exposure".
- The header also shows the layer of the node: base or slot (7).
- The **Code** tab shows the files of the node folder (6.2).
- The user can open the code of a view (15.8), a widget or a type crate. Then the header shows its name, its kind and its build state.

The large node GUI (4.3) goes at the top of the **Inspector**, under the header and above the tabs. (Agent decision, 2026-11-15. The owner can change it.)

- Reason: the GUI is the main control of the node. It stays in view while the user changes tabs.
- The GUI area has the height that the node asks for, up to half of the height of the **Inspector**. A higher GUI scrolls in its area.
- A node with no `ui` function has no GUI area, and the tabs start under the header.

A crate of crates.io is a dependency of one node or widget folder, in its `Cargo.toml` (8.1). (Agent decision, 2026-11-15. The owner can change it.)

- The user edits `[dependencies]` in the `Cargo.toml` tab of the **Code** tab, or clicks **Add crate** above the code.
- **Add crate** opens a field. The user types `name` or `name@version`, for example `rand` or `rand@0.9`, and pushes Enter. Esc cancels.
- The name of a widget folder of the project adds that widget by its path, so a node can use it with `ui.add` (4.3).
- Patina runs `cargo add` of the project toolchain for the folder. The new `Cargo.toml` is one `SetCode` edit, so undo removes the crate. The next build downloads the crate.
- A name has 1 to 64 letters, digits, `_` and `-`, and starts with a letter. Cargo gets the text as one argument, so it cannot be an option, a path or a second crate.
- The field shows the problem of a bad name, of a crate that crates.io does not have, and of a crate that the node has already.
- Patina refuses **Add crate** while the `Cargo.toml` has unsaved changes. An untrusted project has no **Add crate** button.
### 15.5 Errors

- A compile error shows in four places:
  - A red underline and a gutter mark on the line in the code.
  - A red border on the node.
  - A red line in Logs.
  - The error count in the status bar.
- A click on an error in Logs or the status bar selects the node and shows the line.

### 15.6 Engine-to-UI data

- Small messages: Tauri commands and events.
- Large data, for example images: Tauri 2 binary channels, or shared memory.
- Viewers, node GUIs, and views update at the display rate at most.

### 15.7 Debugging

- Value viewers on wires (see 5.3). Select a wire to see the full text of its value in the **Inspector**. The engine keeps the full texts of the last run, and the run event carries only the badge texts. (Agent decision, 2026-11-15. The owner can change it.)
- Run time for each node, shown on the node. A footer under the pins shows the time of the node call in the last run, for example **Run 0.42 ms**. A time under 1 ms shows in microseconds, for example **Run 3.20 us**. A time of 1 s or more shows in seconds. A node that did not run has no footer. (Agent decision, 2026-11-15. The owner can change it.)
  - The cluster measures each call with `std::time::Instant`, and sends the nanoseconds to the engine as a report (C ABI version 5). The time includes the copies of the inputs and the drop of the outputs that no node reads.
- Log output for each node, through the `log` crate. A node folder adds the `log` crate, for example with **Add crate**, and calls `log::info!`, `log::warn!` or `log::error!`. The **Logs** panel shows each line with the node folder and ID. The **Logs** tab of the **Inspector** shows the lines of the selected node. `patina-cli` writes the lines to stderr. (Agent decision, 2026-11-15. The owner can change it.)
  - When a node or kind folder of the plan has `log` in `[dependencies]`, the cluster also depends on `log`. Then it installs a logger for the levels Info, Warn and Error. A project with no `log` crate needs no download of `log`.
  - The logger sends each record to the engine as a report for the node that runs (C ABI version 6). It drops a record from another thread or from outside a node call.
  - A run sends 1000 log lines at most. The 1001st line is a WARN line about the limit, and the cluster drops the lines after it.
  - The logger is for the full cluster library. When one node has `log`, the log lines of all crates in the cluster show, for the node that runs.
- Debug tab: a timeline of the threads of each domain. It shows which nodes ran in parallel, the frame budget (16.7 ms at 60 FPS) and the audio block deadline.
- Later: breakpoints.

### 15.8 Views

A view is a custom panel of the editor. Examples are a timeline, a 3D viewport for procedural generation, and a 3D object to paint textures on. Other examples are a scientific plot and a spectrum display for a sound tool. Views are separate from node GUIs. With nodes and views, a project can build the editors of a tool.

```rust
#[view]
pub struct Timeline { zoom: f32 }

impl Timeline {
    pub fn init() -> Self { Self { zoom: 1.0 } }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        let clock = graph.nodes().find(|id| graph.folder(id) == Some("nodes/clock")).map(str::to_owned);
        let Some(clock) = clock else { return };
        if let Some(frame) = graph.output::<u64>(&clock, "frame") {
            ui.label(&format!("Frame {frame}"));
        }
        if ui.button("Play").clicked() {
            graph.set_param(&clock, "running", true);
        }
    }
}
```

The example uses the `Graph` functions of version 1. See **Version 1** below.

**Model**

- A view is Rust source in its own folder in `views/` (6.2).
- A view uses the same `Ui` API as node GUIs (4.3), and it can use a viewport (see below).
- A view is not part of the graph. It has no pins and no clock domain.
- The `ui` function runs in the engine process, as node GUIs do. It sends a widget tree to the editor.
- The engine calls `ui` at the display rate at most, and only while the view is visible.
- The user opens a view from the **Views** menu (15.2). It docks like a built-in panel, and it can float in its own window.
- The user can open more than one instance of a view. Each instance has its own state.
- The theme applies to the widgets of a view. The content of a canvas or a viewport is data and can use any color (15.3).

**Graph access**

A view gets a `Graph` handle. The handle gives the same access as the editor. Patina leaves it open on purpose, because a view can be any kind of tool. For example, a view can:

- Read the nodes, pins, wires, subgraphs, node states, and errors.
- Read and write parameters.
- Read the outputs of nodes: the latest value, or a queue for streams such as audio blocks (9.1).
- Send the same edits as the editor (7.4), for example add a node or connect a wire.
- Start trigger nodes (9.4).
- Select nodes.

**Rules**

1. A view reads values between runs, or gets a copy, as viewers do. A view never stops a clock domain.
2. Writes and edits from a view take the same path as the edits of the user. They apply on the next run of the clock domain, and undo removes them.
3. To read a typed output, a view uses the type crate of the output. The type key check applies (5.1).
4. The CLI and packages do not run views. A graph with views still runs headless. See also 18.
5. Views are native code. Section 11 applies to them.

**Viewport**

- A viewport is a GPU surface. The code draws into it with `wgpu` and the shared GPU context (10.2), so a view can draw GPU data from nodes. Its shaders are WESL (10.1).
- Views, full-size node GUIs, and large node GUIs can use a viewport. The node body cannot.
- Mouse and key events on the viewport go to the code, for example to turn a 3D camera.
- The frames reach the editor window on the native path. The `wgpu` surface is on the editor window, under a transparent web view. The viewport panels of the page are transparent, and the other panels are opaque, so a menu or a panel covers the scene. One render thread draws all viewports of a window in one render pass, with a scissor rectangle for each viewport. Each float window has its own surface. (Agent decision, 2026-11-15. The owner can change it. The measurements are in STATUS.md.)
- In the viewport prototype (19), the native path held 60 FPS on the RTX 4090 and the Intel UHD 770, and copied no bytes. A copy into the web view through WebView2 shared memory added about 16 to 33 ms of input latency, and copied 6.3 to 8.3 MB for each frame. With Vulkan, its page drew only 30 to 39 FPS. A copy through a Tauri channel drew 15 to 40 FPS.
- On Windows, the native path uses the DX12 backend of `wgpu`. On the Intel UHD 770, the Vulkan swapchain often did not show under the web view for 1 s or more. DX12 showed in each check on both GPUs. (Agent decision, 2026-11-15. The owner can change it.)
- The shared-memory copy is the fallback, for example on a platform where a web view cannot be transparent over a GPU surface. The native path ran only on Windows (13.5).
- The native path needs the GPU device and the editor window in one process. Process isolation (16) moves native code into a different process. Then a viewport needs a shared GPU texture or the copy path.
- After a device loss, the render thread makes a new device, new surfaces and new GPU objects for each viewport. In the prototype, this took 150 to 340 ms.

**Build**

- Each view compiles as its own library, outside the base and the slot (8.2).
- While the user edits a view, it builds in edit mode and hot reloads (8.5). It rebuilds in bake mode at the same events as a fuse (7.2).
- When the code of a view changes, the state of each instance resets.

**Version 1**

Version 1 is the first part of views that works. (Agent decision, 2026-11-15. The owner can
change it.)

- A view folder is a crate with `crate-type = ["cdylib"]`. **New view** in the **Project** panel
  makes the folder with a start view.
- `#[view]` writes five exports: `patina_abi_version`, `patina_view_host`, `patina_view_new`,
  `patina_view_ui` and `patina_view_free`. `patina_view_host` takes the shared GPU context and
  the project folder before the first call. The writes of the view use the report kind `EDIT`. The ABI version stays 14.
- The engine builds a view in edit mode on its first `ui` call. It copies the library to
  `build/loaded/view.<name>.<hash>`, with the Rust runtime, as for the clusters (8.5).
- The `Graph` handle is a copy of the open graph as text lines: the nodes, the parameters, the
  wires, the live values and the node errors of the last run, and the selected nodes. The read
  functions are `nodes`, `folder`, `param`, `wires`, `output_text`, `output`, `error` and
  `selected`.
- The write functions are `set_param`, `add_node`, `remove_node`, `connect` and `disconnect`.
  They become the edits `SetParam`, `AddNode`, `RemoveNode`, `Connect` and `Disconnect` (7.4).
- The editor applies the writes of one `ui` call as one `Group` edit, so one undo removes them.
  When one write is not valid, no write applies, and the panel shows the error.
- `add_node` makes a random ID of 16 hex digits (6.4) and returns it. Thus the same call can
  connect the new node.
- `selected` reads the selected nodes of the graph view. `select` selects nodes in the graph
  view. A selection is not an edit, so undo does not remove it.
- The editor has one **View** panel. The **Views** menu lists the views of the project and opens
  one in the panel. The panel asks for the widget lines at the display rate while it shows.
- A save of the code of a view drops the loaded view and does not build the clusters. The next
  `ui` call builds the view again. After a panic, the next call makes a new state.
- `Ui::viewport` shows a viewport and returns its place and input: the window handle, the
  window size, the area, the pointer and the wheel, in physical pixels. The page sends the place
  at each display frame.
- The view keeps a `patina_gpu::ViewportSurface` in its state and calls `draw` in its `ui` call,
  on the engine thread. Thus there is no render thread and no scissor in version 1. A window
  has one surface, so it shows one viewport.
- `draw` renders into a texture of the area size and copies it into the window surface at the
  area. The rest of the window clears to the page color #141414. The surface uses Mailbox when
  it can, so `draw` does not wait for the display. There is no depth buffer yet.
- On Windows, the editor window is transparent. While a viewport shows, the page root and each
  parent of the viewport are transparent. Thus the **View** panel shows the page color around
  the viewport.
- The shared GPU context loads the view library before its first call, so a view uses the same
  device as the nodes. On Windows, the context uses only DX12.
- A click gives the focus to the viewport. While it has the focus, each key down and key up goes
  to the view as the physical key name, for example `KeyW`. Tab moves the focus out, and then
  each held key comes up. A held key does not repeat.
- A node GUI shows a note in place of a viewport. On Linux and macOS, a view shows the note too.

The limits of version 1:

- One instance of each view, in one panel. No float window.
- No typed outputs through type crates (rule 3). A view reads the text of an output.
- No trigger (9.4), because the event domain does not exist yet. No subgraph edits and no reads
  of node states.
- One viewport in a window, drawn on the engine thread, only on Windows. No viewport in node
  GUIs.
- No bake build of a view.

### 15.9 Connections and routing

Connections are part of the node interface. Their sockets, labels, paths and
interaction need the same care as node bodies. A spline is one available style,
not the definition of a connection. The
[connection guide](../gui-visual-guide/connections.md) gives the visual
rules and examples.

**Meaning and appearance**

- A connection keeps its source, destination, type and domain information when its route or appearance changes.
- A typed bus can show its ordered signals as lanes. Its connection kind (6.3) describes the lanes. A visual cable groups separate connections without joining their values.
- Fan-out connects one source to several readers, subject to the ownership rules (4.4). Splitting, joining, merging or converting values requires explicit nodes with declared semantics.
- A crossing is not a junction. Only an explicit connection edit changes which pins connect.
- Types remain in type crates (5.1). The editor must not infer type compatibility from a color, lane count or cable shape.

**Live values**

A connection shows the value that it carries while the graph runs. Connections are as important as nodes for reading a running graph.

- A wire of a type with lanes (6.3) shows each lane and its state. For example, a bus of 8 bits shows 8 lanes, and a lane lights when its bit is 1.
- A wire with one signal shows its state too, for example a lit 1-bit wire with a value badge.
- A bus socket shows a contact for each lane. Each contact shows the state of its lane.
- A value badge on the wire shows the value as text, for example `0x66`. A label shows the name and the width, for example "data[7:0] / 8".
- The lanes, the contacts, the badge, and the viewers of both nodes show the same value from the same run.
- The cluster sends the text of each value that a wire reads after its node runs, also between two nodes of one task (9.2). Milestone 4 measured 71 to 76 ns for each live value, from the text in the cluster to the outcome of the engine, with 1000 values in a `release` build on Windows (`measure_live_values` in `load.rs`). Milestone 6 measures the cost in a fused chain again. This is the agent decision of 2026-11-15 for question 12 (18). The owner can change it.
- A lane that is off stays visible. Brightness alone never carries a meaning (15.3): the badge or a viewer shows the value as text too.
- Lanes show in a fixed order, the most significant bit first. A route bend never changes the order.
- At a far zoom, a bus shows as one line with a width badge. At a near zoom, it shows each lane.
- A bus draws each lane as a line of 2 px at a pitch of 3 px, so a dark gap separates the lanes. Lane 0 is on the left of the wire direction. A lit lane is light, an off lane is dark gray, and a stale lit lane is between the two. The label "name[n-1:0] / n" is above the ribbon, and its tooltip shows the type. Below a zoom of 0.5, the bus is one line of 4 px and the label is "/ n". A selected bus has one accent ribbon under its lanes. The lanes come with the live values, so a bus shows one line and its type label before a run and after a failed build. (Agent decision, 2026-11-15. The owner can change it.)
- A stale value (15.3) shows dim, on the wire as on the node.
- The badge of a wire with one value is at the middle of the default curve, or at the middle of the longest segment of a route. A route point never covers it. (Agent decision, 2026-11-15. The owner can change it.)
- The badge shows the text of the value by the viewer rules (5.3). The engine cuts the badge text at 64 bytes, and the **Inspector** shows the full text. The badge cuts its text at 160 px with "...", and a selected wire shows all of the badge text. A stale badge has dim text and a dashed border. A failed build removes the badges. (Agent decision, 2026-11-15. The owner can change it.)
- Live values follow the viewer rules (5.3, 9.2). The editor reads them at the display rate at most (15.6), and a read never changes the run.
- A type with no lane description (6.3) shows one line and its type label. The editor does not make lanes from the size of a value or the length of a list.
- The type label is the Rust type of the output as the node code writes it, for example `f64` or `Vec<u8>`. It shows in small muted text above the middle of the wire, above the badge. A pin shows its type in a tooltip, and the **Inspector** shows the type of the selected wire. (Agent decision, 2026-11-15. The owner can change it.)
- How the engine reads a wire inside a fused chain (9.2) is open (18).

**Appearance**

- Provide curved, orthogonal and manually routed polyline paths. Keep route geometry, corner treatment and compact or ribbon display as separate choices.
- Allow a graph default and a connection override. Changing a style does not change execution.
- The **Inspector** shows the look of the selected wire in its **Look** section. There is no toolbar on the canvas. **Route** is **Graph default**, **Curved**, **Orthogonal** or **Polyline**. **Corners** is **Rounded** or **Sharp**, and a curve has no corners. **Reset route** removes the route points and keeps the style. Each change is one edit with undo. (Agent decision, 2026-11-15. The owner can change it.)
- The graph default is fixed for now: a wire with no route points is curved, and a wire with route points is a polyline with rounded corners. A graph setting for the default comes later. (Agent decision, 2026-11-15. The owner can change it.)
- An orthogonal wire is horizontal at its pins. It turns at a right angle before each route point that is not in line, and its last leg turns at the middle between the last point and the target. A leg that goes back leaves the pin by 20 px first. A curved wire with route points is a smooth curve through the points. (Agent decision, 2026-11-15. The owner can change it.)
- **Bus** (compact or ribbon display), **Lock bends** and the crossing controls come with the automatic router (see the optional refinements below). (Agent decision, 2026-11-15. The owner can change it.)
- Use neutral wires by default. A project can use named color families for signal groups. Keep type labels, direction and lane identity available.
- Selection uses the application accent without erasing the signal colors. Keep errors and stale values distinguishable from signal state.
- Cross-domain connections remain dashed (15.3). An order wire remains identifiable as carrying no data (9.2).

**Routing and interaction**

- Let the user add, drag and remove route points. Orthogonal segments can move sideways while keeping their endpoints connected.
- A double-click on a connection adds a route point on the nearest segment. A selected connection shows square handles on its points. A double-click or the Delete key on a focused handle removes that point, and the arrow keys move it by 10 px, or by 1 px with Shift. Esc cancels a drag. With no style, a manual route is a polyline with rounded corners. (Agent decision, 2026-11-15. The owner can change it.)
- Keep routes clear of unrelated node bodies, expanded GUIs, pins and controls. Allow room for the whole ribbon and its bends.
- Moving a route point changes geometry only. Reconnecting an endpoint is a separate action with the normal graph checks.
- Preserve manual route points when nodes move or resize. Recalculate only the affected unfixed sections where possible.
- Show a routing conflict if fixed points prevent a clear path. Do not silently remove points, disconnect the graph or treat a visual obstruction as a run error.
- Route edits support undo and redo. The graph file saves the route points, the collars and the cables (6.4). Each user keeps their own dock layout (15.2).
- The user can select a connection and trace it to its endpoints at each zoom level. Its hit area can be wider than the visible stroke.

**Optional refinements**

Automatic routing around nodes, crossing minimization, fixed-bend locks and
raised crossing bridges are later refinements. They are not gates for the first
editor prototype. A simple gap can distinguish crossing wires. A bridge changes
only their visual over/under order; it is not a graph node or an execution step.

Future connection styles must preserve the same type, endpoint, selection,
error and routing rules. The guide lists extension cases without requiring all
of them for version 1.


### 15.10 Save and the first workflow

This is an agent decision of 2026-11-15 for question 9 (18). The owner can change it.

**Save**

- A graph edit goes to the engine at once, with undo (7.4). A wiring change rebuilds its cluster (7.2). A code edit goes to the engine when the user saves it.
- Ctrl+S saves all changes of the project: the graph files and the code files (6.4). A saved code file starts the build of each unit that uses it (7.2, 8.2).
- A save sends one `SetCode` edit for each changed code file (7.4). A save does not clear the undo history.
- Reason: Cargo builds from the files on disk. Thus the build that runs always matches a saved file. Patina does not write half-typed code into the folder of the user.
- After each change, Patina writes the unsaved changes to a recovery file in `data/` (11). A save deletes the recovery file. When a project opens with a recovery file, Patina asks the user to restore it or to discard it. The next part gives the details.
- When the user closes a project with unsaved changes, Patina asks: **Save**, **Discard** or **Cancel**.

**Open a subgraph**

This is an agent decision of 2026-11-15. The owner can change it.

- A double-click on a use of a subgraph opens the subgraph file in the graph view (6.1). A path bar at the top left shows `main / <name>`. Its **main** button opens the main graph again.
- The graph view shows each public pin of the subgraph as a small node with the pin name, and a wire to each inner end (6.4).
- The user edits the public pins as nodes. A drag moves a pin. A wire from a public input or to a public output connects the pin, and the editor refuses a wire between two pins. Delete removes the selected pins and pin wires. A double-click on a pin opens a field for its name.
- The **Add node** palette of an open subgraph has the entries **Public input** and **Public output**. A new pin gets a random key. When the palette opens from a wire drag, the pin connects to that pin and takes its key as the name.
- A removed pin also removes its wires in the subgraph. A wire of a use to a removed pin stays in the outer graph, and the check shows a problem for it.
- Only the open graph can have unsaved changes. Thus a change of graph saves all first, as Ctrl+S does. Each graph keeps its undo history while the project is open.
- A run uses the unsaved changes of the open subgraph, as it does for the main graph.
- Reason: one unsaved graph keeps the save, the recovery file and the close dialog simple. A user who opens a subgraph usually wants the change of the main graph to stay.

**Collapse into a subgraph**

This is an agent decision of 2026-11-15. The owner can change it.

- Ctrl+G asks for a name and moves the selected nodes into the new file `graphs/<name>.toml` (6.1). A use of the subgraph takes their place, at the top left corner of the selection.
- Each outer output that feeds the selection gets one public input, with a wire to each inner input that it fed. Each inner output that feeds the outer graph gets one public output. A pin takes the key of its first inner pin as its name, with `_2`, `_3` for a repeated name.
- The wires and the order wires between the selected nodes go into the subgraph. An order wire that crosses goes to the use. The collars and the cables keep only their outer wires.
- The editor refuses a name that has a graph, and a selection that makes a cycle through the use.
- The change of the open graph is one undo step. An undo keeps the subgraph file, because a file write is not part of the history.
- Ctrl+Shift+G expands each selected use: the nodes of the subgraph take its place, as one undo step. Their top left corner goes to the position of the use. An inner node keeps its ID, unless the graph has the ID. A parameter of the use goes to the inner inputs of its pin. The subgraph file stays.

**Recovery file**

This is an agent decision of 2026-11-15. The owner can change it.

- The file is `data/recovery/<hash>.json`. The hash is the 64-bit FNV-1a hash of the path of the project folder, as 16 hex digits. On Windows, the path has no case.
- The file holds the format version 1, the project folder, the open graph and its name when it has unsaved changes, and the unsaved text of each code file by path. A restore opens that graph first. A file of more than 16 MiB is an error.
- Patina writes the file after each graph edit, undo, redo and save. It writes the file 300 ms after the user stops typing in the **Code** tab. The first unsaved code change, and the last one that goes, write the file at once.
- Patina writes a temporary file and then renames it, so a stop during a write does not break the old file.
- When the project has no unsaved changes, Patina deletes the file. A close or a quit after the user saved or discarded the changes deletes it too.
- When a project opens with a recovery file, a dialog asks **Restore** or **Discard**. Until the user answers, Patina does not change the file. A restored graph is one step of undo. The restored code goes into the **Code** tab as unsaved text, and Ctrl+S saves it.
- Patina shows the dialog only for a trusted project (11), because a build runs the restored code.
- Patina does not restore a file that holds another project folder or that it cannot read. A bad file shows as a problem of the project, and the next write replaces it.
- Reason: the hash keeps each copy of a project apart, and the folder in the file catches a collision of the hash. The delay of 300 ms keeps the writes few while the user types. A crash loses only the last 300 ms of typing.

**What the editor shows**

- An unsaved code file has a dot on its tab in the **Code** tab. The top bar shows a dot after the project name when the project has unsaved changes.
- The **Code** tab shows the build state of the saved code: **Building**, **Built in 1.8 s** or **Error** (15.3).
- If the saved code does not compile, the graph runs the last build that compiled (7.2). The **Code** tab then shows **Running the last build that compiled**.

**First workflow**

These are the fewest steps from a new project to a reopen:

1. On the start screen, click **New project**. Select a folder and a name.
2. Drag the `number` node folder from the **Project** panel into the graph.
3. Drag the `show` node folder into the graph.
4. Connect the output of `number` to the input of `show`.
5. Click **Run**. The **Output** tab shows the value.
6. Push Ctrl+S.
7. Close the project. On the start screen, select it in the recent projects list.

- A new project has the `number` and `show` node folders and an empty main graph. `number` has a parameter and gives its value. `show` writes its input to the **Output** tab.
- A new project goes on the list of trusted projects (11).
- A project opens with its graph paused. The user clicks **Run**.
- The start screen has **New project**, **Open project** and a list of recent projects.
- The user can type each folder, or click **Browse** to select it in the folder dialog of the platform. This is an agent decision of 2026-11-15. The owner can change it.

**Branding**

This is an agent decision of 2026-11-15. The owner can change it.

- The start screen shows the splash artwork, with the name **Patina** and the version on its fade. It has an **About** button and a **Settings** button.
- The top bar shows the title accent artwork, with the name **Patina** above its fading bar. The **Help** menu at its right end has **About Patina**.
- The **About** dialog shows the banner artwork with the name on its plate, then the version and the license. **Close** or Esc closes it.
- The artwork has no text. The page draws all names, versions and labels, so they stay sharp and can change.
- The window, the taskbar and the executable use `icons/icon.ico`. The installer uses the banner and dialog bitmaps of `icons/` (13.1).

**Settings screen**

This is an agent decision of 2026-11-15. The owner can change it.

- The start screen has a **Settings** button. The settings screen replaces the start screen. **Back** or Esc returns to the start screen.
- The **Toolchain** part shows the Rust version and the path of `cargo`, or the problem that stops a build. It is the toolchain panel of 13.2. When the toolchain folder has no `cargo`, the part shows **Install toolchain**, and then each install step.
- The **Folders** part shows the data folder, with **(portable)** for the portable edition (13.3), and the folder of new projects.
- The **Trusted projects** part lists the trusted folders (11). **Remove** takes a folder off the list. Patina asks again when that project opens.
- Patina refuses to remove the open project from the list. The start screen has no open project, so this case does not occur from the settings screen.
- Reason: no project is open on the start screen, so a change of trust has no effect on a build or a run. The settings of a project, for example the wire look, stay in the **Inspector**. Later settings of Patina, for example the keys, go on this screen.
