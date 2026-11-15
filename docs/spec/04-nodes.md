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
- The return type is the output pin. A tuple gives one output pin for each element.
- The source of a node can also have helper functions and `use` statements.

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
- When the code of a node changes, the state of that node resets. Other nodes keep their state (7.2). State migration is a later feature.

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
- The GUI state is the node state, except in audio nodes (below). A GUI change can change the node output, so downstream nodes get the new value.

**Audio nodes**

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
| Basic | Label, button, toggle, slider, number field, text field, dropdown |
| Values | Color picker, curve, plot, table, image, waveform |
| Canvas | Lines, rectangles, circles, text, images. Mouse and key events |
| Viewport | A GPU surface that the code draws into with `wgpu` (15.8). Not in the node body |
| Custom | Widgets from widget folders (6.2) |

The canvas lets a node author make custom controls, for example a gradient editor, a gizmo, an envelope editor, or a timeline. To reuse a custom control, put it in a widget folder (6.2). A custom widget implements the `Widget` trait from `patina-sdk`. Node GUIs and views use it like a built-in widget.

**Rules**

The width and height of a node body fit its GUI. Nodes do not have to share a size. Give plots and image controls room, and keep small utility nodes compact.

1. The engine never calls `ui` while `process` runs. It calls `ui` between runs, so the two functions never use the node state at the same time.
2. A GUI change marks the node as changed. The change applies on the next run of the clock domain of the node.
3. The engine calls `ui` at the display rate at most, and only for visible nodes.
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

- The full size is optional. Only a node with `full_gui` has it.
- The header of the node and the header of the inspector show an expand button.
- When the user clicks it, the GUI fills the area of the graph view. The top bar path shows the node, for example "Main Graph / Curve Editor".
- **Back** in the top bar, or Esc, returns to the graph view.
- The graph keeps running. The other panels stay, and the inspector shows the node.
- A full-size GUI can use a viewport (15.8).

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
- A generic node has only type parameters: no lifetime or const parameters, and no `impl Trait` input. A generic stateful node puts the type parameters on its struct, and its `impl` blocks use the same names, for example `impl<T: Copy> Delay<T>`. (Agent decision, 2026-11-15. The owner can change it.)

### 4.6 Async nodes

```rust
#[node]
pub async fn fetch(url: &str) -> Result<String, FetchError> { ... }
```

- An async node runs in the event domain (see 9.4).
- A wait does not block a worker thread.

### 4.7 Errors

- A node can return `Result<T, E>`. An `Err` stops the downstream nodes and shows on the node.
- Builds use `panic = "unwind"`. The generated glue code wraps each call into node code in `catch_unwind`: `init`, `process`, `ui`, viewers, and drop. The catch is inside the library, so a panic never crosses the C ABI. A panic that leaves an `extern "C"` function stops the process.
- In a fused cluster, each node call has its own catch. A panic shows as an error on the node that panicked. It does not stop the engine.
- A panic can break the state of the node. The engine drops it, and calls `init` before the next run. If drop panics too, the engine leaks the state.
- Limit: an abort, a stack overflow, or a native crash, for example an access violation in C code, stops the engine process. Only process isolation (16) contains these.

### 4.8 Parameters

- A parameter is an input pin with no wire. Each input pin with a value type is a parameter until the user connects a wire to it. The wire then gives the value.
- The Parameters panel shows a control for each parameter. Numbers get a slider, `bool` gets a check box, and strings get a text field. An enum with `#[derive(Param)]` gets a dropdown.
- A type crate gives parameter support to its own types with the `Param` trait from `patina-sdk`. The trait gives a default value, a control (a built-in or a custom widget), and a text form for save and undo. The engine stays free of types (2).
- An input whose type has no parameter support needs a wire. Until it has one, the node shows an error and does not run.
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
- In a package (14), the compiler can make parameters constants. The base (7) keeps them as run-time data, so a parameter change never needs a compile.
