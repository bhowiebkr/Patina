# Write a custom node

This guide tells a person or an AI agent how to write a Patina node: its logic, its state, its
GUI and its look. The [specification](spec/README.md) is the full design. Section 4 of the spec
is the source of this guide.

## The node folder

Each node is one crate in the `nodes/` folder of a project:

```
nodes/
  readout/
    Cargo.toml
    src/lib.rs
```

`Cargo.toml` names the crate and uses the SDK of the project:

```toml
[package]
name = "readout"
version = "0.1.0"
edition = "2024"

[dependencies]
patina-sdk = { workspace = true }
```

- `src/lib.rs` has one item with `#[node]`: a function or a struct.
- The folder name is the node name. The editor makes a new node folder with the **+** button of
  **Nodes** in the **Project** panel.
- The file can also have helper functions, constants and `use` statements.

## The logic

### A function node

A function node has no state. Each argument is an input pin. The return type is the output pin.
A tuple gives one output pin for each element.

```rust
use patina_sdk::node;

/// A constant bit that the user sets.
#[node(color = "#655219")]
pub fn switch(on: bool) -> bool {
    on
}
```

- An input pin with no wire is a parameter. The **Inspector** shows a control for it.
- A `bool` parameter also shows a check box in the node row.
- A single output is the pin `out`. The outputs of a tuple are `out0`, `out1` and so on.
- `&T` reads a value. `T` takes it. `&mut T` changes it, and the changed value leaves on an
  output pin with the same name (spec 4.4).
- Set ranges and defaults with `#[param(default = 0.0, min = -4.0, max = 4.0)]` on an argument
  (spec 4.8).
- Return `Result<T, E>` to show an error on the node (spec 4.7).

### A stateful node

A struct node keeps data between runs. It has `init` and `process`. The pins come from the
arguments of `process`.

```rust
#[node]
pub struct Counter {
    count: u64,
}

impl Counter {
    pub fn init() -> Self {
        Counter { count: 0 }
    }

    pub fn process(&mut self, step: u64) -> u64 {
        self.count += step;
        self.count
    }
}
```

- Each use of the node has its own state.
- The engine calls `init` before the first `process`. A code change resets the state of the node.
- A node with no wired inputs must name its clock domain, for example
  `#[node(domain = frame)]`. The domains are `batch`, `frame`, `audio` and `event`.

## The description

The doc comment above `#[node]` is the description of the node. The node header, the
**Add node** palette and the **Project** panel show it as a tooltip. The **Inspector** shows it
under its title.

```rust
/// Shows a byte as 8 lamps and as a number. A change writes it to the standard output.
#[node(color = "#59366f")]
pub struct Readout { value: u8 }
```

Write one or two short sentences. Tell what the node does and what each input changes.

## The attribute

Put the arguments of `#[node]` in one list, for example
`#[node(domain = frame, color = "#655219", free_layout)]`.

| Argument | Effect |
|---|---|
| `color = "#rrggbb"` | The header color of the node. Other text is an error on the node. |
| `domain = <name>` | The clock domain: `batch`, `frame`, `audio` or `event`. |
| `heavy` | A light node with much work gets a task of its own (spec 4.1). |
| `full_gui` | The node can open its GUI in the whole graph area. It needs `ui`. |
| `free_layout` | The GUI is the whole node, with its own shape and pin places. It needs `ui`. |

## The GUI

A struct node can have a `ui` function. The engine calls it between runs, never at the same time
as `process`. A GUI change applies on the next run.

```rust
use patina_sdk::{Ui, node};

pub fn ui(&mut self, ui: &mut Ui) {
    ui.label("Speed");
    if ui.slider("Rate", &mut self.rate, 0.0..=10.0).changed() {
        // The new value is in self.rate.
    }
}
```

- Each call adds one widget in a column. The node body grows to fit the column.
- `ui.mode()` tells the size: `Mode::Compact` (the node body), `Mode::Large` (the
  **Inspector**) or `Mode::Full` (the whole graph area, only with `full_gui`).
- A widget that the user changes returns a `Response`. `clicked()` and `changed()` tell what
  happened.
- `ui.set_param(key, value)` sets a parameter of the node, as an edit that undo removes.
- The node size comes from the sizes that the GUI declares, never from the values that it shows.
  Give a canvas the size of its largest content.

The widgets are in `crates/patina-sdk/src/ui.rs` and in `crates/patina-sdk/src/ui/`:

| Group | Calls |
|---|---|
| Basic | `label`, `button`, `icon_button`, `toggle`, `slider`, `number`, `number_unit`, `text_field`, `dropdown`, `tabs`, `section`, `progress` |
| Values | `color`, `plot`, `plot_axes`, `scatter`, `waveform`, `waveform_trim`, `table`, `curve`, `curve_histogram`, `image`, the code field, the path field, the asset picker, the color ramp, the layer stack, the 2D transform and the timeline |
| Drawing | `canvas`, `viewport` (only in the **Inspector** and the full size) |
| Custom | `add` with a type that implements `Widget` |

### The canvas

`ui.canvas(text, [width, height])` gives a drawing area. The draw calls take pixels from the top
left corner. A color is `[red, green, blue]`.

| Call | Draws |
|---|---|
| `line(from, to, color)` | A line. |
| `rect(at, size, color, fill)` | A rectangle. |
| `circle(center, radius, color, fill)` | A circle. |
| `path(data, color, fill)` | SVG path data, for example `M 0 0 L 40 0 L 20 30 Z`. |
| `text(at, color, text)` | One line of text. `at` is the left end of the base line. |

- `canvas.pointer()` gives the pointer inputs since the last `ui` call: `Pointer::Down`,
  `Pointer::Move` and `Pointer::Up`, each with a point.
- `canvas.keys()` lets the canvas take the focus on a click. It then gives `Key::Down` and
  `Key::Up` with the physical key name, for example `KeyW`.
- Read the pointer before the draw calls when the input changes what the canvas draws.
- An empty `text` shows no label above the canvas.

The readout node of the computer example draws 8 lamps and a number:

```rust
const ON: [u8; 3] = [223, 166, 75];
const OFF: [u8; 3] = [102, 102, 97];
const TEXT: [u8; 3] = [238, 238, 235];

pub fn ui(&mut self, ui: &mut Ui) {
    let (text, number) = (self.text(), self.value);
    let mut canvas = ui.canvas(&format!("{text} = {number}"), [214.0, 36.0]);
    for i in 0..8 {
        // A gap splits the two hex digits.
        let x = 12.0 + 20.0 * i as f32 + if i > 3 { 6.0 } else { 0.0 };
        let on = number >> (7 - i) & 1 == 1;
        canvas.circle([x, 18.0], 7.0, if on { ON } else { OFF }, on);
    }
    canvas.text([178.0, 23.0], TEXT, &number.to_string());
}
```

## A free layout

With `free_layout`, the compact GUI is the whole node. The node has no header, no pin rows, no
box and no padding. A canvas has no background and no border, so the shapes are the look of the
node.

- `ui.pin(key, [x, y])` puts a pin at a point, in pixels from the top left corner of the node.
- A pin with no place goes to the left edge (inputs) or the right edge (outputs).
- A wire leaves an output to the right and goes into an input from the left. Put outputs on the
  right side of the shape and inputs on the left side.
- `ui.place([x, y])` puts the next widget at a point, on top of the other widgets. For example,
  put a slider on a drawn panel.
- The node width is the largest canvas width. The name of the node shows above the shape. The
  user drags the node by its name.

The button node of the computer example is a round push button:

```rust
use patina_sdk::{Pointer, Ui, node};

const RIM: [u8; 3] = [102, 102, 97];
const CAP: [u8; 3] = [101, 82, 25];
const LIT: [u8; 3] = [223, 166, 75];

/// A round push button. The output is on while the user holds the button down.
#[node(domain = frame, color = "#655219", free_layout)]
pub struct Button {
    held: bool,
}

impl Button {
    pub fn init() -> Self {
        Button { held: false }
    }

    pub fn process(&mut self) -> bool {
        self.held
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let mut canvas = ui.canvas("", [56.0, 56.0]);
        for p in canvas.pointer() {
            match p {
                Pointer::Down(_) => self.held = true,
                Pointer::Up(_) => self.held = false,
                Pointer::Move(_) => {}
            }
        }
        canvas.circle([28.0, 28.0], 26.0, RIM, true);
        canvas.circle([28.0, 28.0], 20.0, if self.held { LIT } else { CAP }, true);
        ui.pin("out", [56.0, 28.0]);
    }
}
```

## The look

- Use the colors of the [GUI visual guide](gui-visual-guide.md). The text color is
  `[238, 238, 235]`, a dim part is `[102, 102, 97]` and a lit part is `[223, 166, 75]`.
- Give each node of a group the same header color, for example `#655219` for inputs.
- Keep a compact GUI small. Put large controls in the **Inspector** (`Mode::Large`) or in the
  full size.
- A node is 200 px wide. A canvas makes it wider, up to 480 px.

## Check a node

- Run the graph with `patina-cli` (see the README). A node GUI does not change a headless run.
- Put tests in the node crate, as in any Rust crate.
- An error in the code shows on the node in the editor, with the line of the error.
