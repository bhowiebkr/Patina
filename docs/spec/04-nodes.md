# Patina Specification: Nodes

Part of the [Patina Specification](README.md).

## 4. Nodes

### 4.1 Function nodes

A function node has no state. Patina parses the signature with `syn` and makes the pins.

```rust
#[node]
pub fn blur(image: &Image, radius: f32) -> Image { ... }

#[node]
pub fn split(text: &str) -> (String, String) { ... }
```

- Each parameter is an input pin.
- A node function has at most 16 parameters, with the `&mut self` of `process` (4.2). The fused cluster calls each node through a tuple of its arguments, with no closure, so a large graph builds fast. (Agent decision, 2026-11-15. The owner can change it.)
- The return type is the output pin. A tuple gives one output pin for each element.
- The source of a node can also have helper functions and `use` statements.
- `syn` parses with recursion, so Patina parses node code on a thread with a 64 MB stack. A debug build then parses types with about 1000 levels of nesting. Code that nests deeper can still stop the process. (Agent decision, 2026-11-16. The owner can change it.)

A node with only `bool`, `char` and number wires is a light node. Its parameters can have each type, for example a text. Light nodes share one task (9.2). A light node that does much work, for example a long loop, can set `heavy`:

```rust
#[node(heavy)]
pub fn mandelbrot(x: f64, y: f64, steps: u32) -> u32 { ... }
```

- A `heavy` node keeps a task of its own, so it can run in parallel with other nodes.
- `heavy` changes only the tasks. The node gets the same values and gives the same results. (Agent decision, 2026-11-17. The owner can change it.)

### 4.2 Stateful nodes

A stateful node keeps data between runs, for example a counter, a file handle, or a GPU context.

```rust
#[node]
pub struct Counter { count: u64 }

impl Counter {
    pub fn init() -> Self { Self { count: 0 } }
    pub fn process(&mut self, step: u64) -> u64 {
        self.count += step;
        self.count
    }
}
```

- Pins come from the `process` signature.
- Each use of the node in a graph has its own state. The engine calls `init` before the first call of `process`, not when it loads the build. A panic in `init` shows on the node, and the next run calls `init` again. (Agent decision, 2026-11-15. The owner can change it.)
- `Drop` releases resources.
- When the code of a node changes, the state of that node resets. Other nodes keep their state (7.2). A change to a shared crate, to the features or to the toolchain resets all states. State migration is a later feature.

### 4.3 Node GUI

A node can have its own interactive GUI. The GUI code is Rust, in a `ui` function in the node folder (6.2). The `ui` function can be in the same file as the node code or in another file of the folder.

```rust
#[node]
pub struct CurveEditor { points: Vec<[f32; 2]> }

impl CurveEditor {
    pub fn init() -> Self { Self { points: vec![] } }

    pub fn process(&mut self) -> Curve { Curve::from(&self.points) }

    pub fn ui(&mut self, ui: &mut Ui) {
        if ui.button("Reset").clicked() { self.points.clear(); }
        let canvas = ui.canvas(240.0, 140.0);
        for p in &self.points { canvas.circle(*p, 3.0); }
        if let Some(pos) = canvas.clicked() { self.points.push(pos); }
    }
}
```

**Model**

- The `Ui` API comes from `patina-sdk`. It is immediate mode: the `ui` function describes the full GUI each time it runs.
- The `ui` function runs in the node code, on the engine side. It sends a widget tree to the editor. The editor draws the widgets with the Patina theme.
- User input (clicks, drags, keys) goes back to the node as events. The next call of `ui` handles them.
- The editor finds a widget by its order in the call of `ui`. A widget that shows only sometimes changes the order of the widgets after it, so put it last. This is an agent decision of 2026-11-15 that the owner can change.
- The editor draws the GUI of each node after each run and each frame: the node body size in the node, and the large size of the selected node in the **Inspector**. A change in a live graph runs it again. A change of a frame node applies at the next frame. A change of a batch node runs the batch domain again, without a build. A slider sends its value at the end of a drag, as in 9.1. This is an agent decision of 2026-11-15 that the owner can change.
- A widget writes its value before it applies an event. So after the events, `ui` runs one more time without events, and the editor shows the new state at once. This is an agent decision of 2026-11-15 that the owner can change.
- The value widgets have limits, so a large value cannot slow down the editor. A plot sends at most 1024 values: a longer plot sends the minimum and the maximum of 512 parts. A waveform sends at most 256 columns, with the minimum and the maximum of each column, and an optional playhead from 0 to 1. A curve has at most 256 points from 0 to 1, in the order of x. A drag of a curve point sends one change at the end of the drag. An image has at most 256 x 256 RGBA pixels. A table sends the row that the user selects. This is an agent decision of 2026-11-15 that the owner can change.
- A number field can have a unit and a step. The unit shows after the value. Two step buttons and the up and down arrows change the value by the step. (Agent decision, 2026-11-16. The owner can change it.)
- Tabs are a row of names, and the node shows the widgets of the selected tab after the row. A section has a head that opens and closes it, and the node keeps the open state. A closed section still runs its widgets and sends an empty line for each, so the widgets after it keep their order. A section more than 8 levels deep shows its widgets in its parent. (Agent decision, 2026-11-16. The owner can change it.)
- A waveform can have a trim range from 0 to 1 with two handles. A drag moves the nearest handle and sends the range at the end of the drag. The arrow keys move the focused handle. A handle stops at the other handle, and the editor dims the parts outside the range. (Agent decision, 2026-11-16. The owner can change it.)
- A plot can have axes: the x range and a unit for each axis. The editor labels the ends of each axis with the value and the unit. A scatter plot has the same axes and sends at most 1024 points: a longer list sends points at equal steps. A curve can have a histogram behind it, with at most 256 bars: a longer histogram sends the sum of equal parts. The editor scales the bars to the highest bar. (Agent decision, 2026-11-16. The owner can change it.)
- An icon button has an icon name and a text. The editor shows the icon and uses the text as the accessible name and the tooltip. An unknown icon name shows the text. Icon buttons on adjacent lines share one row. A progress bar shows a value from 0 to 1 as a percent, and a value that is not finite shows 0. (Agent decision, 2026-11-16. The owner can change it.)
- A color ramp has stops with a position from 0 to 1 and a color, in the order of position, and a blend: linear, ease or constant. The SDK keeps the selected stop, so the node body and the **Inspector** show the same stop. A click on the bar adds a stop with the color of the ramp at that point. A drag or an arrow key moves a stop, and Delete or a double-click removes it. A ramp keeps 1 stop and has 32 stops at most. A row below the bar shows the color, the blend and the position of the selected stop. (Agent decision, 2026-11-16. The owner can change it.)
- A layer stack shows one row for each layer, with the first layer at the top. A row has a handle, a visibility toggle, a swatch, the name and an amount from 0 to 1. A layer without an amount, for example a base, has no amount field. A drag of the handle or the up and down arrows move a layer, and a move selects it. The SDK keeps the selected layer. The node shows the properties of the selected layer below the stack, for example in a section. The node adds and removes layers, so the stack has no add or remove control. (Agent decision, 2026-11-16. The owner can change it.)
- A 2D transform has a move in pixels, a rotation from -180 to 180 degrees, a scale for each axis from 0.01 to 100, a pivot from 0 to 1 and an aspect lock. The editor draws the frame, an optional image of at most 256 x 256 pixels and a gizmo on them. A drag of the box moves it, and a drag of an axis arrow moves it on the x or the y axis of the frame. A drag of the ring rotates it around the pivot, and Shift snaps the angle to 15 degrees. A drag of a corner scales it around the pivot. With the aspect lock, the two scales keep their ratio. The arrow keys move the focused box by 1 pixel, or 10 pixels with Shift. Number fields under the gizmo show the move, the rotation and the scale. Nine buttons set the pivot and keep the box in place. A drag sends one change at its end. `Transform2d::apply` and `Transform2d::invert` map the points of the frame, for example to draw a transformed image. (Agent decision, 2026-11-17. The owner can change it.)
- A timeline has a transport, a time ruler and lanes, as the Timeline view of mockup 5. The transport has **Play**, **Pause**, **Rewind**, the time as `00:00:01.240`, **Loop** and **Snap**. The ruler shows the loop range with a handle at each end and the playhead. A click on the ruler moves the playhead. A clip lane holds clips with a color, a name and the waveform between their trims. A drag of a clip moves it, and a drag of an end trims it. A trim cannot go past the source. An automation lane draws a smooth curve through its points between a low and a high value, with a unit, for example `500 Hz` to `5.0 kHz`. A drag moves a point between its neighbors, a double click adds a point and a double click on a point removes it. Each lane has **M** (mute) and **S** (solo) buttons. Drags snap to the snap step. The arrow keys move the focused playhead, loop end, clip, trim or point, and Shift gives a step of 0.01 s. A drag sends one change at its end. The node calls `Timeline::advance` to move the playhead while the timeline plays, and `Timeline::audible` and `Automation::value` to play it. A lane reorder is not in this version. (Agent decision, 2026-11-17. The owner can change it.)
- A file path field has a **Browse** button and a kind: open a file, save a file or select a folder. The button opens the dialog of the platform at the folder of the current path. A path in the project comes back relative to the project root, with `/`. A path outside the project stays absolute. The node gives the path to `patina_sdk::project_path` to get the full path. A path pick needs no trust, because no project code runs. (Agent decision, 2026-11-16. The owner can change it.)
- An asset picker is a dropdown of the project files with one of its extensions, for example `glb`. The editor lists the files again when the dropdown gets the focus. The list skips `build/`, the `target` folders and hidden files, and has 1000 files at most. The value is a path relative to the project root, as for a file path field, and empty for **None**. A path that is not in the list stays as an option. (Agent decision, 2026-11-16. The owner can change it.)
- The widget lines of one GUI or view have 1 MB at most. When the lines are longer, the node or the view shows an error and no widgets, so a draw loop with no end cannot freeze the editor. (Agent decision, 2026-11-16. The owner can change it.)
- The GUI state is the node state, except in audio nodes (below). A GUI change can change the node output, so downstream nodes get the new value.

**Audio nodes**

The owner deferred the GUI of audio nodes on 2026-11-15: the settings, the messages and the display values below. The design stays for a later milestone. Until then, an audio node has no `ui` function.

The audio thread has a hard deadline (9.3). So the `ui` function of an audio node never runs on the audio thread, and it never gets the node state:

```rust
#[derive(Clone, Param)]
pub struct EnvSettings { a: f32, d: f32, s: f32, r: f32 }

#[node(domain = audio)]
pub struct AdsrEnvelope { #[settings] env: EnvSettings, stage: Stage }

impl AdsrEnvelope {
    pub fn process(&mut self, gate: &Gate, #[messages] msgs: &[EnvMsg]) -> Envelope { ... }

    pub fn ui(env: &mut EnvSettings, shown: &EnvDisplay, ui: &mut Ui) {
        ui.slider("Attack", &mut env.a, 0.0..=2.0);
        if ui.button("Audition").clicked() { ui.send(EnvMsg::Audition); }
    }
}
```

- The `ui` function runs on a GUI thread of the engine. It gets the settings of the node, the field with `#[settings]`, and it can change them.
- After a change, the engine copies the settings to the audio side. `process` sees the new settings at the start of the next block. The audio thread never waits for the GUI, and the copy does not allocate or drop on the audio thread.
- The settings save with the project and take part in undo, as parameters do (4.8).
- One-shot actions, for example **Audition** or **Reset**, are messages. `ui.send` puts a message into a queue with a fixed size (9.1). `process` gets the messages of the block in its `#[messages]` argument. If the queue is full, the node shows an error.
- For display, the node can have a `display(&self)` function that returns a small `Copy` value, for example the position of the playhead. The engine calls it after a block, at the display rate at most. `ui` gets the latest value.
- The nodes of the other domains use the model above: `ui` gets `&mut self` between two runs.

**Widgets**

| Group | Widgets |
|---|---|
| Basic | Label, button, icon button, toggle, slider, number field (with a unit and step buttons), text field, code field, file path field (with a **Browse** button), dropdown, tabs, section |
| Values | Color picker, color ramp, layer stack, 2D transform gizmo, timeline, asset picker, curve with an optional histogram, plot with optional axes, scatter plot, progress bar, table, image, waveform |
| Canvas | Lines, rectangles, circles, SVG paths, text, images. Mouse and key events |
| Viewport | A GPU surface that the code draws into with `wgpu` (15.8). Not in the node body |
| Custom | Widgets from widget folders (6.2) |

The canvas lets a node author make custom controls, for example an envelope editor, a step sequencer or a piano roll. To reuse a custom control, put it in a widget folder (6.2). A custom widget implements the `Widget` trait from `patina-sdk`. Node GUIs and views use it like a built-in widget.

A canvas gets key events only when its `ui` calls `keys()`, for example a joystick pad. A click on such a canvas gives it the focus, and Tab moves the focus away. Each key goes to the node one time when it goes down and one time when it comes up, by its physical name, for example `KeyW` or `ArrowLeft`. A key without Ctrl, Alt or Meta stops at the canvas, so a shortcut of the graph does not act. When the canvas loses the focus, each held key comes up. (Agent decision, 2026-11-16. The owner can change it.)

A drag on a canvas of a graph node moves the node. Thus the user can move a node that draws its whole body. A press that moves less than 4 px is a click. The canvas gets the down at the release, or after a hold of 250 ms. A canvas that calls `grab()` gets each press at once, with the moves. An example is a slider. A canvas in a view always gets each press at once. (Agent decision of 2026-11-18, for the owner request. The owner can change it.) A canvas that calls `grab_rect(at, size)` gets each press in that rectangle at once. A drag that starts outside of each rectangle moves the node. Examples are the cap of a button that acts while it is held and the buttons of a joystick. (Agent decision of 2026-11-19. Reason: a button that grabs all of its body left no place to drag it. The owner can change it.)

The code field edits a text of many lines in a monospace font, for example a program. It sends the text when it loses the focus and on Ctrl+Enter. Esc puts back the last text. In the GUI tree, a line break is `\n`, a tab is `\t` and a backslash is `\\`. (Agent decision, 2026-11-16. The owner can change it.)

A code editor is a code field with line numbers, for a full size. The node gives a current line, for example the line that runs, and errors with a line each. The editor shows a band on the current line, colors each error line, and shows the error text as the tooltip of its line number. (Agent decision, 2026-11-16. The owner can change it.)

A node GUI can set a parameter of its own node with `ui.set_param(key, value)`, for example the file that an asset picker selected. The editor applies the write as a parameter edit, so undo removes it and the project saves it. Only a call of `ui` with user input writes. The editor skips a write that changes nothing. The writes of one input are one undo step, and a canvas drag from the down to the up is one input. (Agent decision of 2026-11-18, for the owner request. The owner can change it.) A node in a subgraph use is read-only (15.3), so it can not write. The editor then shows an error that tells the user to open the subgraph and use the **Inspector**. A write changes all uses of the subgraph, so a write from one use does not apply. (Agent decision, 2026-11-16, and 2026-11-17 for the error. The owner can change it.)

**Rules**

The width and height of a node body fit its GUI. Nodes do not have to share a size. Give plots and image controls room, and keep small utility nodes compact.

The size of a node comes from the sizes that its GUI declares, never from the values that it shows. A node is 200 px wide. A canvas makes the node wider, to the canvas width plus the padding, up to 480 px. A wider canvas scales down to fit. A label stays on one line and clips with an ellipsis, and its full text shows as a tooltip. Nothing draws outside the node. A node that shows values gives its canvas the size of the largest content, for example all 16 x 2 cells of an LCD. Then the node does not grow when the content changes. (Agent decision, 2026-11-15. The owner can change it.)

1. The engine never calls `ui` while `process` runs. It calls `ui` between runs, so the two functions never use the node state at the same time.
2. A GUI change marks the node as changed. The change applies on the next run of the clock domain of the node.
3. The engine calls `ui` at the display rate at most, and only for visible nodes.
   - Agent decision (2026-11-16): after a run, the editor engine draws each node GUI one time, so
     each node gets its size. The frames then draw only the nodes in the view of the graph, and
     the nodes of the **Inspector** and the full size. Below a zoom of 0.25 (15.1), no node body
     GUI draws. The owner can change it.
4. A node can show its GUI in up to three sizes. `ui.mode()` tells the node which size to draw:
   - **Compact:** in the node body.
   - **Large:** in the inspector.
   - **Full:** in the whole graph area. This size is optional (see below).
5. The CLI and packages do not call `ui`. A graph with node GUIs still runs headless.
6. The GUI code compiles with the node code. Hot reload updates the GUI and the node together.

**Full size**

A node can open its GUI in the whole graph area, for a GUI that needs more room than the inspector. Examples: a large curve editor, a gradient editor with many stops, a sequencer grid.

```rust
#[node(full_gui)]
pub struct CurveEditor { points: Vec<[f32; 2]> }
```

- The full size is optional. Only a node with `full_gui` has it. A node with `full_gui` must have a `ui` function, else the node shows an error.
- The header of the node and the header of the inspector show an expand button. A node with a free layout shows it after its name, above the shape. (Agent decision of 2026-11-18. The owner can change it.)
- When the user clicks it, the GUI fills the area of the graph view. The top bar path shows the node, for example "Main Graph / Curve Editor".
- **Back** in the top bar, or Esc, returns to the graph view.
- The graph keeps running. The other panels stay, and the inspector shows the node.
- A full-size GUI can use a viewport (15.8).

**Free layout**

A node can draw its whole body, for example a round push button or a knob. Then the node has its own shape and its own pin places.

```rust
#[node(free_layout)]
pub struct Button { held: bool }
```

- A node with `free_layout` must have a `ui` function, else the node shows an error.
- The compact GUI is the whole node. The node has no header, no pin rows, no box and no padding. A canvas has no background and no border.
- A canvas shows no label row. Its label is the accessible name. Thus a drawing starts at the origin of the pin and widget places. (Agent decision of 2026-11-18, for the owner request. The owner can change it.)
- The node width is the largest canvas width, up to 480 px. A node with no canvas is 200 px wide.
- `ui.pin(key, [x, y])` puts a pin at a point, in pixels from the top left corner of the node body. A pin with no place goes to the left edge (inputs) or the right edge (outputs), in equal steps over the canvas height.
- A wire still leaves an output to the right and goes into an input from the left.
- A node with a free layout and a bus pin (15.9) shows a socket column on each side of the drawing, as a node with rows does. The column is 96 px wide. An input with no lanes shows as a pin row in the left column, and an output in the right column. The drawing is between the columns, at the vertical center. `ui.pin` places do not apply, and the name shows above the shape. Thus each lane keeps its contact and its LED. (Owner decision of 2026-11-18.)
- `ui.place([x, y])` puts the next widget at a point, on top of the other widgets. A widget in a section keeps its place in the section.
- A placed image or field fills the width from `x` to the same margin at the right edge. The height of an image follows its aspect ratio. Thus an image fits in a drawn frame, for example the glass of a monitor. A field is a `number`, `text`, `path` or `dropdown` widget. (Agent decision of 2026-11-18, for the owner request. The owner can change it.)
- The **Inspector** and the full size GUI also show a GUI with placed widgets with no padding and no canvas label. Thus each widget stays at its place on the drawing. (Agent decision of 2026-11-18. The owner can change it.)
- `canvas.path(data, color, fill)` draws SVG path data, for example `M 0 0 L 40 0 L 20 30 Z`. The editor skips a path of more than 4096 characters or with other letters than the path commands. The shot check fails when a canvas has a skipped shape. (Agent decision of 2026-11-18. The owner can change it.)
- The name and the state icon of the node show above the shape. The selection is an outline. An error message shows below the shape.
- The name starts after the left order pin and ends before the right order pin, so no pin covers the name. On a narrow node, the icons and buttons wrap to rows above the name. (Agent decision of 2026-11-18. The owner can change it.)
- The name, the state icon and the order pins show only when the pointer is on the node, or when the node is selected, has the focus or has an error. The order pins also show during a wire drag. (Agent decision of 2026-11-18, for the owner request. The owner can change it.)
- The user drags the node by its name or by a drag on a canvas (see the canvas text above).
- Until the node draws its first GUI, it shows the normal layout.
- (Agent decision of 2026-11-18. The owner can change it.)

**Symbols**

A node or a subgraph can show a circuit symbol in place of the box, for example a NAND gate or a CPU chip. Thus a logic circuit looks like a schematic. (Agent decision of 2026-11-18, for the owner request. The owner can change it.)

```rust
#[node(symbol = "nand")]
pub fn nand(a: &bool, b: &bool) -> bool { !(*a && *b) }
```

- A subgraph sets the symbol of each use in the `[symbol]` table of its file (6.4). The **Shape** list, the **Label** field, the **Color** and the look editor in the **Look** section of the **Inspector** edit it, with undo. The **Inspector** of a use has an **Edit look** button that opens the subgraph.
- The shapes:
  - `and`, `nand`, `or`, `nor`, `xor`, `xnor`, `not` and `buffer`: the logic gates, with the distinctive shapes of IEEE 91.
  - `mux`: a trapezoid. `alu`: a chevron with a notch. `adder`: a block with a sigma sign.
  - `chip`: a DIP package with a notch, a pin 1 mark, metal legs and the pin names.
  - `bus`: a bus bar, for the nodes that split a bus into lines or join lines into a bus.
  - `custom`: only for a subgraph. The `outline` is a polygon of 3 to 32 corners, in symbol pixels from 0 to 480. The user draws it in the look editor.
- A subgraph can also set a body `color` as `#rrggbb`. Without it, the symbol has the theme color. The label text is dark on a light color.
- The look editor shows the symbol with a handle on each corner and on the middle of each edge. A drag moves a corner on a grid of 2 pixels. A drag on a middle handle adds a corner. A double-click or Delete removes a corner, but 3 stay. The arrow keys move the selected corner by 2 pixels, or by 10 with Shift. Each release or key is one edit, with undo.
- A custom symbol has its inputs on the left and its outputs on the right, as a chip. Each lead goes straight to the outline. A bad outline shows a chip, and a bad color shows the theme color.
- (Agent decision of 2026-11-19, for the owner request to draw the look of a subgraph in the GUI. The owner can change it.)
- Another shape is an error on the node. A node cannot have `custom`, because a node has no outline. A subgraph file with another shape does not open.
- A chip, a block and a bus bar show a label: the `label` text, or the node name in upper case. A gate shows no label.
- The inputs are on the left edge and the outputs are on the right edge, in their order. A gate with one input has it at the middle.
- On a `mux` and an `alu`, a control input goes to the bottom edge. A control input has the key or the name `sel`, `s`, `op`, or `s` or `f` and one digit. A wire still goes into an input from the left.
- On a `chip`, a pin with the name `clk` or `clock` has the clock triangle.
- Each pin has a short lead line. The lead is lit when the live value of the pin is `true`, so the user sees the logic levels on the schematic. The wire badge still shows the value as text (15.3).
- The name and the state icon show above the symbol, with the rules of the free layout. The selection is an outline. An error message shows below the symbol.
- A node with a symbol shows no compact GUI and no parameter rows on the canvas. The **Inspector** shows them.
- An open subgraph use shows its frame, as a use with no symbol does.
- The symbols use the colors of the node GUIs (15.3), so all node kinds have one style.

A node can set the color of its header (15.3):

```rust
#[node(color = "#175c59")]
pub fn blur(image: &Image, radius: f32) -> Image { ... }
```

- The color is "#" and 6 hex digits. Other text is an error on the node.
- The arguments go into one list, for example `#[node(domain = frame, color = "#655219")]`.

### 4.4 Input forms

| Form | Meaning | Engine rule |
|---|---|---|
| `&T` | Read | Many nodes can read the same value at the same time. |
| `T` | Take | Move if this node is the only consumer. Else clone if `T: Clone`. Else show an error on the wire. |
| `&mut T` | Change in place, then pass on | Same rule as `T`. Prevents a copy of large data. |

- A `&mut T` input also makes an output pin with the same name. After the call, the changed value leaves on that output.
- If that output has no wire, the value drops after the call.
- For example, `fn sharpen(image: &mut Image, amount: f32)` has the input `image`, the parameter `amount`, and the output `image`.

### 4.5 Generic nodes

```rust
#[node]
pub fn add<T: std::ops::Add<Output = T>>(a: T, b: T) -> T { a + b }
```

- A generic pin shows as "unresolved" until the user connects it.
- Patina compiles the node when it knows all type parameters. Each set of types is a separate compiled copy.
- The engine gets the type parameters from the types of the input wires, in run order. If no input wire gives a type parameter, the engine refuses the graph and names the node. (Agent decision, 2026-11-15. The owner can change it.)
- A type parameter that no input wire gives comes from the wires of the outputs: a wire from an output to an input of a known type gives the type. A trigger input does not run in the witness (8.2), so it gives no type. When two wires give two types, the engine refuses the graph and names the node. The build of each cluster also calls those readers, so rustc sees the type. (Agent decision, 2026-11-16. The owner can change it.)
- A generic node has only type parameters: no lifetime or const parameters, and no `impl Trait` input. A generic stateful node puts the type parameters on its struct, and its `impl` blocks use the same names, for example `impl<T: Copy> Delay<T>`. (Agent decision, 2026-11-15. The owner can change it.)

### 4.6 Async nodes

```rust
#[node]
pub async fn fetch(url: &str) -> Result<String, FetchError> { ... }
```

- An async node runs in the event domain (see 9.4).
- A wait does not block a worker thread.
- In version 1, an async node is a plain function. An async stateful node is an error. An async node needs a trigger (9.4). Agent decision of 2026-11-16, the owner can change it.

### 4.7 Errors

- A node can return `Result<T, E>`. An `Err` stops the downstream nodes and shows on the node.
- Builds use `panic = "unwind"`. The generated glue code wraps each call into node code in `catch_unwind`: `init`, `process`, `ui`, viewers, and drop. The catch is inside the library, so a panic never crosses the C ABI. A panic that leaves an `extern "C"` function stops the process.
- In a fused cluster, each node call has its own catch. A panic shows as an error on the node that panicked. It does not stop the engine.
- The error of a panic has the panic message and the place in the code, for example `panic: no input (src/lib.rs:12:5)`. The standard error does not get the standard panic message of Rust too, so each panic shows one time. When `RUST_BACKTRACE` is set, the standard error also gets the standard message with the backtrace. A panic on a thread that a node starts is not caught, so it keeps the standard message. A panic that gives no report, for example in a viewer text, a connection kind or the defaults of a cluster, also keeps it. A panic that node code catches itself gives no standard message. (Agent decision, 2026-11-16. The owner can change it.)
- The engine keeps the first 16 KB of the text of an error or a panic, and adds `...` to a longer text. A view edit stays whole. (Agent decision, 2026-11-16. The owner can change it.)
- A panic can break the state of the node. The engine drops it, and calls `init` before the next run. If drop panics too, the engine leaks the state.
- Limit: an abort, a stack overflow, or a native crash, for example an access violation in C code, stops the process that runs the node. In the editor, that is the node process (16.1), so the editor shows the crash and does not stop. In `patina-cli` and a package, the run stops.

### 4.8 Parameters

- A parameter is an input pin with no wire. Each input pin with a value type is a parameter until the user connects a wire to it. The wire then gives the value.
- The Parameters panel shows a control for each parameter. Numbers get a slider, `bool` gets a check box, and strings get a text field. An enum with `#[derive(Param)]` gets a dropdown.
- A `bool` parameter also shows its check box in the row of its pin in the node body. A click sets it, as in a logic simulator, so a switch node needs no Inspector. (Agent decision, 2026-11-15. The owner can change it.)
- A type crate gives parameter support to its own types with the `Param` trait from `patina-sdk`. The trait gives a default value, a control (a built-in or a custom widget), and a text form for save and undo. The engine stays free of types (2).
- An input whose type has no parameter support needs a wire. Until it has one, the node shows an error and does not run.
- The check finds parameter support for a scalar, a `String` and a `&str`. For a type parameter, it checks the type that the wires give (4.5). It also finds a type with an `impl Param for` it in the node crate or in a type crate of the project. (Agent decision of 2026-11-18, for the owner request. The owner can change it.)
- The project file saves the text form of each value (6.4).
- Node code can set ranges and defaults with an attribute:

```rust
#[node]
pub fn exposure(
    image: &Image,
    #[param(default = 0.0, min = -4.0, max = 4.0)] amount: f32,
) -> Image { ... }
```

- During editing, parameters are data at run time. A parameter change does not need a compile.
- When a parameter loses its text, for example after an undo, it gets the default value of its type again. The states of the nodes stay. The engine gives a null text to `patina_cluster_set` (C ABI version 18). (Agent decision, 2026-11-16. The owner can change it.)
- In a package (14), the compiler can make parameters constants. The base (7) keeps them as run-time data, so a parameter change never needs a compile.
