<div align="center">
<h1>Patina</h1>

<p><b>A visual node graph editor and engine. Each node is Rust code.</b></p>

<img src="docs/mockups/mockup-1-main-editor-colored-headers.png" alt="Mockup of the Patina main editor: a layer list on the left, an image-processing graph in the center, the inspector with the Rust code and parameters of the selected node on the right, and logs at the bottom" width="900">

<sub>A design mockup, not a screenshot. <a href="docs/spec/README.md#0-notes-for-the-agent">Section 0 of the spec</a> lists where it differs from the design.</sub>

</div>

> **Patina is at an early stage.** The engine runs a simple graph from the command line. There
> is no editor yet. This README tells what Patina will be. The
> [specification](docs/spec/README.md) is the full design. [STATUS.md](STATUS.md) tells what
> works now.

- [What Patina is](#what-patina-is)
- [Nodes](#nodes)
- [Connections](#connections)
- [Views](#views)
- [Layers](#layers)
- [Clock domains](#clock-domains)
- [GPU](#gpu)
- [Install](#install)
- [Packages](#packages)
- [Run a project from the command line](#run-a-project-from-the-command-line)
- [How it is built](#how-it-is-built)
- [Status and roadmap](#status-and-roadmap)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [License](#license)

## What Patina is

You make a graph of nodes, and the graph runs.

- Each node is Rust source code. You edit it in the editor.
- Patina compiles the whole graph to native machine code, optimized as one build.
- Patina finds the nodes that can run at the same time, and runs them in parallel. You give no
  hints.
- A change to one node recompiles only that node. The target is a few seconds. The rest of the
  graph stays optimized.
- A node can have its own interactive GUI. Views add custom panels to the editor, for example a
  timeline or a 3D viewport. With both, a project can become a tool with its own editors.

Patina does not limit what a graph does. Some examples:

- Sound effect editors
- Image and shader editors
- Automation pipelines
- Simulation
- Scientific data processing

Patina is the Rust successor to [PyFlowGraph](https://github.com/bhowiebkr/PyFlowGraph).

## Nodes

A node is a Rust function with the `#[node]` attribute. Patina reads the signature and makes the
pins.

```rust
#[node]
pub fn exposure(
    image: &Image,
    #[param(default = 0.0, min = -4.0, max = 4.0)] amount: f32,
) -> Image { ... }
```

- Each argument is an input pin. The return type is the output pin.
- An input pin with no wire is a parameter. The **Parameters** panel shows a slider, a check box
  or a text field for it. A parameter change needs no compile.
- A node can use any crate from crates.io.
- A node can return a `Result`. An error shows on the node, and the nodes after it do not run.
  They show **Skipped**. A panic also shows on the node. It does not stop the engine.
- After a run, each node that ran shows its run time at the bottom, for example **Run 0.42 ms**.

A node that keeps data between runs is a struct:

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

A `ui` function in the node code draws the GUI of the node: buttons, sliders, curves, plots,
images, and a canvas for custom controls. The GUI shows small in the node and large in the
inspector. A node can also open its GUI in the whole graph area, for example a large curve
editor.

Nodes can also be generic or async. Patina does not define data types. Crates define them, and
two pins connect only when they have the same type.

Each node has its own folder, with its code, its GUI and its tests. Views, custom widgets and
data types have their own folders too. Another project can import a folder and reuse it. The
import is a copy. **Update** in the **Project** panel shows the changes in the original folder
and copies them when you confirm.

```
image-pipeline/
  graphs/          the main graph and the subgraphs, with their connections
  nodes/
    curve-editor/  one node: its code, its GUI and its tests
  views/           custom panels
  widgets/         custom widgets for node GUIs and views
  connections/     connection kinds: how the wires of each type look
  types/           data types
```

Node code runs with your user rights. A project from another person opens read-only until you
trust it.

## Graph view

The graph view moves at once, with no animation.

- Turn the mouse wheel to zoom at the mouse pointer. A touchpad pinch also zooms.
- Drag with the middle mouse button to pan. You can also hold Space and drag.
- Drag on an empty area to select the nodes and the wires in a box.
- Press F to fit the selection. With no selection, F fits the full graph.

## Connections

A wire shows the value that it carries while the graph runs, as a node shows its state.

- A bus carries one value with many signals, for example the 8 bits of a byte. The wire shows
  each signal as a lane, and a lane lights when its bit is 1. A badge shows the value as text.
- The project defines how the wires of each data type look, with a connection kind. For
  example, a bus kind sets the lanes of a byte. A type with no connection kind shows one wire
  and its type name.
- Hold the pointer on a pin name to see its Rust type. The **Inspector** shows the type of the
  selected wire.
- A wire can be curved, or straight with rounded or sharp corners. You can add bends and route
  a wire around a node. The look of a wire never changes what it connects.
- Double-click a wire to add a bend. Select the wire to set its look in the **Look** section of
  the **Inspector**: **Route**, **Corners** and **Reset route**.
- One output can feed many inputs. Splitting or joining a value takes a node.

To add a connection kind:

1. Make the folder `connections/<name>/` in the project, with a `Cargo.toml` that has
   `patina-sdk = { workspace = true }`.
2. In `src/lib.rs`, write a `#[connection]` function. It takes a reference to the value and
   gives its lanes:

   ```rust
   #[patina_sdk::connection]
   pub fn byte(value: &u8) -> patina_sdk::Lanes {
       patina_sdk::Lanes::bits(8, *value as u64).label("data")
   }
   ```

3. Run the graph. Each wire of that type shows its lanes and the label `data[7:0] / 8`.

## Views

A view is a custom panel in the editor. A project can add its own views, for example a timeline,
a 3D viewport for procedural generation, a scientific plot, or a spectrum display for a sound
tool.

- A view uses the same GUI API as node GUIs, and it can draw with the GPU.
- A view is not part of the graph, but it can read and change the graph: parameters, outputs,
  nodes and wires.
- Open a view from the **Views** menu. It docks like the other panels, and it can float in its
  own window.

## Layers

Patina compiles a graph in two layers, so that the graph runs fast and your changes build fast.

- **The base** is the whole graph, compiled and optimized as one build. Small nodes are inlined
  into each other.
- **The slot** holds the node that you edit. Patina compiles that node alone and swaps it in
  while the graph runs. The target is a few seconds for each change.

When you open the code of a node, Patina makes a slot for it in the background. Each change to
the code then compiles only that node. If the code does not compile, the graph keeps running
with the last build that did.

When you open another node, stop editing for a while, or click **Bake**, Patina fuses the node
back into the optimized base.

Patina never runs the graph as separately compiled nodes. Only the node that you edit runs on
its own.

## Clock domains

Each node runs in a clock domain. The domain decides when the node runs.

| Domain | When it runs | Example uses |
|---|---|---|
| Batch | One time, when you click **Run** or an input changes | Data processing, export |
| Frame | Each display or simulation tick, at 60 FPS | Interactive tools, previews |
| Audio | Each block of samples from the audio device | Sound effects, synthesis |
| Event | On a schedule, a webhook, a file change or a GUI event | Automation pipelines |

One graph can use more than one domain. A slow domain never stops a fast one.

## GPU

GPU support comes from crates and from `patina-sdk`, not from the engine.

- CUDA through `cudarc` and NVRTC, on NVIDIA. The CUDA Toolkit is not necessary.
- WESL shaders through `wgpu`, on NVIDIA, AMD, Intel and Apple. WESL is WGSL with imports,
  conditions and shader packages.
- GPU data stays on the GPU between GPU nodes.
- A shader change needs no Rust compile.

## Install

Patina will come in two editions, on GitHub Releases.

- **Portable:** a zip file, for Windows and Linux. Patina runs from its folder and keeps
  everything in it. It needs no admin rights and makes no changes to the system.
- **Installed:** the installer of your system. An `.msi` file on Windows, a `.dmg` file on
  macOS, and a `.deb` package or an AppImage on Linux.

Windows is the main platform. The Linux and macOS builds are basic, and contributors who use
those systems improve them.

To start the portable edition:

1. Unzip the file.
2. Start `patina`.

To install on Windows:

1. Run the `.msi` file.
2. Select **Just me** or **All users**. **Just me** needs no admin rights.

To install on macOS:

1. Open the `.dmg` file.
2. Drag Patina to **Applications**.
3. Start Patina. The first time, macOS blocks it, because the build is not notarized.
4. Open **System Settings > Privacy & Security**, and click **Open Anyway**.

In both editions, click **Install toolchain** the first time. Patina downloads Rust, Zig as the
C compiler and linker, and rust-analyzer into its toolchain folder. NVRTC for CUDA nodes is
optional.

Neither edition changes your `PATH` or installs Rust for the whole system. The installed
edition keeps the toolchain and your projects in your user folders, and they stay when you
uninstall Patina. To uninstall the portable edition, delete its folder.

Some systems need one more thing. macOS can need the Xcode Command Line Tools. Linux needs
WebKitGTK, which most desktop distributions include.

## Packages

A finished graph can become a package that runs without the editor:

- An executable. The graph inputs become command-line arguments.
- A library with a C API, to call from other software.
- A Python module.

A package holds the full source of the project. The editor can open a package and show the
original project. A package also holds the license notices of Patina and of the crates in it.

## Run a project from the command line

`patina-cli` builds the main graph of a project and runs it one time. It needs Rust, with the
version that `patina.toml` gives. To run the example project from a checkout of this
repository:

```
cargo run -p patina-cli -- run examples/hello
```

The example adds two numbers and writes `6.5`. A node that fails or panics gives a message with
its ID. The nodes after it do not run, and each one gives a `skipped` message. The exit code is
then 1.

## Open the editor

The editor is at an early stage. It makes or opens a project, edits the main graph, runs it and
saves it. To start it from a checkout of this repository, you need Rust and Node.js:

1. In `crates/patina-editor/frontend`, run `npm ci` and then `npm run build`.
2. In the root folder, run `cargo run -p patina-editor`.

To make your first graph:

1. On the start screen, click **New project**. Give a name and click **Create**.
2. Drag `number` and `show` from the **Project** panel into the graph. You can also
   double-click an empty area of the graph, type a part of the name and push Enter.
3. Drag from the output of `number` to the input of `show`. You can also drag from a pin into
   an empty area. The palette then shows the nodes that fit the pin, and connects the node that
   you add.
4. Select `number`, and type a value in the **Inspector**.
5. Click **Run**. The **Output** tab shows the value, and a badge on the wire shows it too.
6. Click the wire. The **Inspector** shows the full value.
7. Push Ctrl+S to save the project.

To make a new node, click **+** in the title of the **Nodes** group of the **Project** panel.
Type a name and push Enter. The **Inspector** shows the code of the new node. Edit the code
and push Ctrl+S.

To use a crate of crates.io in a node, click **Add crate** in the **Code** tab of the node. Type
the name, for example `rand` or `rand@0.9`, and push Enter. The `Cargo.toml` tab shows the new
dependency, and the next build downloads the crate. Undo removes it.

While the graph runs, a change of a parameter runs it again. Click **Pause** to stop the runs.
Click **Stop** to also end the active run: Patina starts no new node call. The nodes that did
not start show **Stale**. A node with a long loop can read `patina_sdk::stop_requested()` and
return early.

Click **Settings** on the start screen to see the Rust toolchain, the data folders and the
trusted projects. Click **Remove** to take a project off the trusted list. Patina then asks
again before it builds the project.

If Patina stops before you save, it keeps your changes. When you open the project again, click
**Restore** to get them back, or **Discard** to delete them.

The panels dock at the left, the right and the bottom of the graph:

- Drag a tab to another area, or right-click the tab to move or close it.
- Drag the line between an area and the graph to change the size of the area.
- Use the **Views** menu to show or hide a panel. **Reset layout** gives the default layout.

Patina keeps the layout and the pan and zoom of each project for each user.

## How it is built

| Part | What it does |
|---|---|
| `patina-engine` | Compiles the graph in layers, schedules it and runs it. No UI code. |
| `patina-editor` | The desktop app: Tauri 2 with a web frontend. |
| `patina-cli` | Runs a project or a package from the command line. |
| `patina-sdk` | The crate that node and view code uses: the `#[node]` and `#[view]` macros, the `Ui` API, the `Graph` handle, viewers, buffer pools and the GPU context. |

All code is Rust, except the web frontend of the editor.

## Status and roadmap

Milestone 2 is in progress. [STATUS.md](STATUS.md) gives the results. Each milestone gives a
tool that works.

0. **Risk prototypes**, proven on Windows first:
   - Rust types across two compiled clusters, with a shared Rust library, and type keys that
     catch a changed type.
   - Hot reload with new file names, with the memory and disk growth measured.
   - The toolchain on a clean Windows computer with no Visual Studio, with Zig as the C compiler
     and linker for crates with C code.
   - A GPU viewport in a panel of the editor window that docks, floats and resizes.
   - A graph view that draws many wires and buses with live values at 60 FPS.
1. Engine: compile one node, load it, and run a simple graph from the command line.
2. Editor: the dock layout, the **Project** panel, the node editor, the code editor, node
   states, errors, value viewers, and wires with bends and live values.
3. The **Install toolchain** button, the portable zip and the installers.
4. Automatic parallelism and the frame domain.
5. Stateful nodes, node GUIs with the full size, custom widgets and generic nodes.
6. Layers: clusters, the base, the slot, fuse back and **Bake**.
7. Subgraphs, and the import and update of folders from other projects.
8. Packaging.
9. GPU.
10. Views.
11. Audio domain.
12. Event domain.

Some design questions are still open, for example the side effects of a change while the
graph runs.
[Section 18 of the spec](docs/spec/16-roadmap.md#18-open-questions) lists them.

## Documentation

- [Specification](docs/spec/README.md): the full design, in one file for each group of
  sections.
- [Mockups](docs/mockups/): the visual direction of the editor.
- [GUI visual guide](docs/gui-visual-guide.md): palette, node sizing, custom controls,
  views and implementation notes for each mockup.
- [Mockup plan](docs/mockup-plan.md): the mockups still to make, and the rules for them.
- [Circuit graph design](docs/circuit-graph-design.md): buses, ribbon wires and logic circuits.
- [Laser measurement design](docs/laser-measurement-design.md): nodes and views for a laser
  measurement tool.
- [Before building](docs/before-building.md): the setup, prototypes and decisions that are still
  open, by milestone.
- [AGENTS.md](AGENTS.md): working rules for AI agents in this repository.

## Contributing

Issues and pull requests are welcome, from people and from their AI agents. At this stage, a
report about a gap or a contradiction in the spec helps as much as a report about code. Agents
follow the rules in [AGENTS.md](AGENTS.md).

A source file can have 1000 lines at most. When a file gets longer, refactor it.
`cargo test --workspace` checks this limit.

Windows is the main platform. If you use Linux or macOS, your tests and fixes for that system
are welcome. They are the way those builds get better.

## License

Patina is under two licenses. You can use it under the license that you choose:

- The [MIT license](LICENSE-MIT)
- The [Apache License 2.0](LICENSE-APACHE)

A contribution that you send for inclusion in Patina is under the same two licenses, with no
other terms. This applies unless you clearly write otherwise. The word "contribution" has the
meaning that the Apache License 2.0 gives it.
