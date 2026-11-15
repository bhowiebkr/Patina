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
- At a 20% zoom, 500 HTML nodes dropped frames in the same prototype. Thus the graph view needs a simpler node form at a far zoom. Milestone 2 designs it.

### 15.2 Layout

The editor window is a dock system. The built-in panels and the views (15.8) dock the same way.

| Area | Default contents |
|---|---|
| Top bar | Project and graph path, Live indicator, Run, Pause, Bake, undo, redo, zoom, fit, grid, Views menu |
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

**Project panel:**

- The **Project** tab lists the folders of the project (6.2) in groups: **Graphs**, **Nodes**, **Views**, **Widgets**, and **Types**.
- The user drags a node folder into the graph to add a node.
- A click on a view, widget, or type folder opens its code in the inspector.
- **Import** copies a folder from another project (6.2).
- **Update** compares an imported folder with its origin, and copies the new version when the user confirms (6.2).

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

The owner asked for the standard controls. The table, the zoom range and the wheel steps are an agent decision of 2026-11-15.

**Run, Pause, and Bake:**

- Run starts the graph. Batch nodes run one time. Frame and audio domains start their clocks. Triggers start to listen.
- Pause stops the frame and audio clocks and stops new event runs. Active event runs finish.
- Bake fuses the node in the slot back into the base now (7.2).

### 15.3 Visual rules

The [GUI visual guide](../gui-visual-guide.md) defines the palette, dimensions,
shared controls and image-specific corrections. Use neutral charcoal surfaces,
an amber application accent, and the blue faceted Patina logo. Mockup 9 is the
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

**Times:** build time and run time always have separate labels, for example "Built in 1.8 s" and "Run 0.42 ms".

### 15.4 Inspector

- The header shows the node name, the node kind, and the full path, for example "Color Grade / Exposure".
- The header also shows the layer of the node: base or slot (7).
- The **Code** tab shows the files of the node folder (6.2).
- The user can open the code of a view (15.8), a widget or a type crate. Then the header shows its name, its kind and its build state.

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

- Value viewers on wires (see 5.3).
- Run time for each node, shown on the node.
- Log output for each node, through the `log` crate.
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
        let clock = graph.node("Clock");
        if let Some(frame) = graph.output::<u64>(clock, "frame") {
            ui.label(&format!("Frame {frame}"));
        }
        if ui.button("Play").clicked() {
            graph.set_param(clock, "running", true);
        }
    }
}
```

The names of the `Graph` functions in this example are not final.

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
- A milestone 0 prototype decides how the frames reach the editor window (19).

**Build**

- Each view compiles as its own library, outside the base and the slot (8.2).
- While the user edits a view, it builds in edit mode and hot reloads (8.5). It rebuilds in bake mode at the same events as a fuse (7.2).
- When the code of a view changes, the state of each instance resets.

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
- A lane that is off stays visible. Brightness alone never carries a meaning (15.3): the badge or a viewer shows the value as text too.
- Lanes show in a fixed order, the most significant bit first. A route bend never changes the order.
- At a far zoom, a bus shows as one line with a width badge. At a near zoom, it shows each lane.
- A stale value (15.3) shows dim, on the wire as on the node.
- Live values follow the viewer rules (5.3, 9.2). The editor reads them at the display rate at most (15.6), and a read never changes the run.
- A type with no lane description (6.3) shows one line and its type label. The editor does not make lanes from the size of a value or the length of a list.
- How the engine reads a wire inside a fused chain (9.2) is open (18).

**Appearance**

- Provide curved, orthogonal and manually routed polyline paths. Keep route geometry, corner treatment and compact or ribbon display as separate choices.
- Allow a graph default and a connection override. Changing a style does not change execution.
- Use neutral wires by default. A project can use named color families for signal groups. Keep type labels, direction and lane identity available.
- Selection uses the application accent without erasing the signal colors. Keep errors and stale values distinguishable from signal state.
- Cross-domain connections remain dashed (15.3). An order wire remains identifiable as carrying no data (9.2).

**Routing and interaction**

- Let the user add, drag and remove route points. Orthogonal segments can move sideways while keeping their endpoints connected.
- Keep routes clear of unrelated node bodies, expanded GUIs, pins and controls. Allow room for the whole ribbon and its bends.
- Moving a route point changes geometry only. Reconnecting an endpoint is a separate action with the normal graph checks.
- Preserve manual route points when nodes move or resize. Recalculate only the affected unfixed sections where possible.
- Show a routing conflict if fixed points prevent a clear path. Do not silently remove points, disconnect the graph or treat a visual obstruction as a run error.
- Route edits support undo and redo. The storage format and ownership of saved layout remain open questions (18).
- The user can select a connection and trace it to its endpoints at each zoom level. Its hit area can be wider than the visible stroke.

**Optional refinements**

Automatic routing around nodes, crossing minimization, fixed-bend locks and
raised crossing bridges are later refinements. They are not gates for the first
editor prototype. A simple gap can distinguish crossing wires. A bridge changes
only their visual over/under order; it is not a graph node or an execution step.

Future connection styles must preserve the same type, endpoint, selection,
error and routing rules. The guide lists extension cases without requiring all
of them for version 1.

