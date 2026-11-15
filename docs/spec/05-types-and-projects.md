# Patina Specification: Data Types, Graphs and Projects

Part of the [Patina Specification](README.md).

## 5. Data types

### 5.1 Ownership of types

- The engine does not define data types. Crates define them.
- A node uses a type crate to declare its pins.
- Two pins can connect only if they have the same **type key**. The engine checks this when the user connects them.
- Type key = hash of: crate name, crate version, type path, size, alignment, project toolchain version, and the build ID of the type crate.
- The build ID is a hash of the source of the type crate, its resolved dependencies, its features, the target, and the compiler settings. A change to a local type crate gives a new key, even when its name and version stay the same. Size and alignment alone do not prove that two types agree.
- The editor shows the type name. The key is internal.

### 5.2 The library boundary

Two rules make Rust types safe across compiled libraries:

1. **Between the engine and a cluster: C ABI.** The engine is prebuilt and can use a different Rust version. It stores each value as an erased value: a pointer, a type key, and a drop function. The engine never reads the value.
2. **Between two nodes: Rust types.** All clusters in a project use the same toolchain. Patina generates one shared library crate (crate type `dylib`) that depends on the type crates of the project. The clusters, the slot, the views, and the connection kinds link to it as a dynamic library (`-C prefer-dynamic`). They link to the `std` library of the toolchain in the same way. So they all use one copy of each type. Milestone 0 proves this recipe and records the files that it makes (19).

More rules for values:

- The library that makes a value gives its drop function. The value drops with the code of that build, even after a reload (8.5).
- All libraries use the one allocator of the shared `std` library. Node and type crates do not set a `#[global_allocator]`.

When the engine loads a cluster, it compares the type keys. If they do not agree, it rebuilds the cluster.

### 5.3 Value viewers

The UI cannot show an unknown type by itself.

- A type crate can implement the `Viewer` trait from `patina-sdk`. A viewer gives text, a table, an image, a plot, or a waveform.
- A viewer can also give a small thumbnail. The node body shows the thumbnail of its output, for example an image preview.
- The viewer does not set how a wire shows the value. The connection kind of the type does that, for example the 8 lanes of a byte (6.3).
- If a type has no viewer but implements `Debug`, the UI shows the `Debug` text.
- Otherwise the UI shows the type name and size.
- The first `Viewer` gives text only: `fn text(&self, out: &mut Text) -> fmt::Result`. `Text` does not allocate, and it stops a text at 4096 bytes. The table, the image, the plot, the waveform and the thumbnail come later. (Agent decision, 2026-11-15. The owner can change it.)

## 6. Graphs and projects

### 6.1 Graphs and subgraphs

- A graph is a set of nodes and connections.
- Each node has a stable random ID. The ID does not change when the node moves to another subgraph or file.
- A subgraph is a group of nodes that shows as one node. The user can collapse and expand it, and reuse it in other graphs.
- A subgraph is a candidate for one cluster (see 8.2).

### 6.2 Project folders

A project is a folder with a stable random ID. Each node, view, custom widget, connection kind, and local type crate has its own folder in it, so that other projects can reuse it.

```
<project>/
  patina.toml        project settings, for example the Rust version (6.4)
  Cargo.toml         the workspace, made by Patina
  Cargo.lock
  graphs/            the main graph and the subgraphs, with their connections (6.4)
  nodes/
    curve-editor/    one node
      Cargo.toml
      src/
        lib.rs       the node code
        ui.rs        the node GUI (optional)
      tests/         tests of the node (optional)
      assets/        files that the node reads, for example a WESL shader (optional)
  views/
    timeline/        one view (15.8)
  widgets/
    gradient-bar/    one custom widget (4.3)
  connections/
    bus/             one connection kind (6.3)
  types/
    image/           one type crate (5.1)
  build/             compiled clusters and cache. Not in Git
```

**Rules**

- A node folder holds all of one node: the `#[node]` code, its GUI if it has one, its tests, and its files.
- Each node, view, widget, connection, and type folder is one crate of the workspace (8.1). The folder name is the crate name.
- A graph uses a node folder as a node. One graph can use the same node folder more than once, and other graphs can use it too. Each use has its own ID (6.1), parameter values, and state. A change to the code applies to each use.
- A node folder does not depend on another node folder. It can depend on crates, and on type and widget folders. This keeps each node folder reusable.

**Import**

- **Import** copies a node, view, widget, connection, or type folder from another project into this project. It also copies the type and widget folders that the folder depends on. Before it copies, it shows each folder that it will copy, each name conflict, and each node of this project that a replaced folder changes.
- This project can already have a folder with the same name but other contents. Then Patina asks the user to replace the old folder, keep it, or import the new folder with another name.
- With another name, Patina changes the crate name in the `Cargo.toml` of the new folder. The copied folders that depend on it point to the new name with the `package` key of Cargo, so their code does not change.
- The copy belongs to this project. A later change in the other project does not change it. Patina never links folders between projects, because a project holds all its source (2, 14), and a change in another project must not break it.
- Patina records the source of the copy. The record holds the stable random ID of the other project, the folder name, and a hash of the contents at import. The record holds no path, so it shows no private folder names when the project is shared. Section 6.4 gives the format of the record.
- **Update** in the **Project** panel compares the copy with the folder in the other project. Patina finds the other project among the projects that the user opened before, or asks for its folder. It shows the changes on both sides, including the changes that the user made to the copy. It copies the new version only when the user confirms. If both sides changed a file, the user picks the version of that file.
- The crate versions of the copy resolve with the `Cargo.lock` of this project (8.1).
- Patina builds the changed folders before it applies an **Import** or an **Update**. If the build fails or the user cancels, the project stays as it was.
- A folder from a project that the user does not trust stays read-only, and out of the build, until the user trusts it (11).

### 6.3 Connection kinds

A connection kind defines how the connections of some types look and how they show their values. Each domain needs other connections. The project defines its connection kinds, as it defines its nodes and views.

| Domain | Example of a connection kind |
|---|---|
| Electronics | A bus: one connection with 8 to 32 lanes in groups, for example address, data and control. It replaces many wires that overlap. A lane lights when its bit is 1. |
| Shader | A `vec4` wire with one lane for each of x, y, z and w. |
| Workflow | A data wire with a badge that shows the number of items. |

**Model**

- A connection kind is Rust source in its own folder in `connections/` (6.2). Thus it can have logic and a look.
- A connection kind names the types that it applies to. Each connection of such a type uses that kind.
- A type with no connection kind uses the default connection: one line with its type label (15.9). Most connections of single values and parameters use it.
- A connection kind changes only the display. It does not change the values, the execution, or the type key check (5.1). Splits, joins and conversions still need explicit nodes (15.9).
- Its code runs in the engine process, as node GUIs and views do (4.3, 15.8). It sends a description to the editor, and the editor draws it with the theme.
- It reads the live value of a connection with the viewer rules (5.3, 9.2). It reads at the display rate at most, and a read never changes the run.
- A graph default and a connection override change only the look (15.9).

**Contents**

A connection kind can give:

- The lanes: the number, the names, the order and the groups of the signals in one value, and the state of each lane.
- The socket, for example a bus socket with a contact for each lane.
- The label and the value badge, for example "data[7:0] / 8" and `0x66`.
- The default look: the path style, a color family for each lane group, and the form at a far zoom.

The SDK form of a connection kind is not fixed. Start with the lanes, the label and the badge. Expect changes as the editor grows.

**The first SDK form** (Agent decision, 2026-11-15. The owner can change it.)

- A connection kind is a crate in `connections/<name>/`, with `patina-sdk = { workspace = true }`. The project workspace has `connections/*` as a member only when that folder has a kind.
- The crate has one `#[connection]` function. It takes `&T`, where `T` is the type of the kind. It returns `Lanes::bits(count, bits)`, with an optional `.label(name)`.
- The kind applies to each output whose type text is the same as `T`, as the node code writes it.
- `Lanes::bits` holds 1 to 64 lanes. Lane 0 is the most significant bit. The label has 64 bytes at most.
- The cluster calls the kind each time it sends the live value of such a wire, and it sends the lanes with the value. The badge stays the viewer text (5.3).
- A panic in a kind gives no lanes for that value, and the wire shows one line. A compile error in a kind stops the build, as an error in a node does.
- Lane names, lane groups, socket contacts and a custom badge are not in the first form.

### 6.4 Project file format

This is an agent decision of 2026-11-15 for question 1 (18). The owner can change it.

The project files are TOML. Patina writes them, and Git can merge them. A person can edit them by hand, but Patina does not keep comments.

**`patina.toml`**

```toml
format = 1
id = "3f9c2a7d41e0b85c"     # the stable random ID of the project (6.2)
rust = "1.94.0"             # the Rust version of the project (8.1)

[imports."nodes/curve-editor"]   # the folder in this project (6.2)
project = "9a1b4c0d2e7f6385"     # the ID of the other project
folder = "curve-editor"          # the folder name in the other project
hash = "sha256:<64 hex characters>"
```

- The import hash is a SHA-256 hash of the sorted relative paths and the bytes of each file in the folder. The paths use `/` on each platform.
- An import record holds no path of the other project (6.2).

**Graph files**

- Each graph and each subgraph is one file in `graphs/`. The file name is the graph name. `graphs/main.toml` is the main graph.
- A subgraph file can have public pins. Each use of the subgraph is a node with these pins (6.1).

```toml
format = 1

[[node]]
id = "5c0e9b2a7f13d846"         # the stable random ID of this use (6.1)
folder = "nodes/curve-editor"   # or subgraph = "full-adder" for a use of graphs/full-adder.toml
pos = [120.0, -40.0]
domain = "audio"                # only when the user selected the domain (9.1)
settings = "<text form>"        # only for a node with #[settings] (4.3)

[node.params]
radius = "2.5"                  # the text form of the Param trait (4.8)

[[wire]]
from = "5c0e9b2a7f13d846.out"
to = "a01d77c3e5b9f210.image"
style = "orthogonal"             # curved, orthogonal or polyline; only to override the graph default (15.9)
corners = "sharp"                # rounded or sharp; only for sharp corners (15.9)
points = [[200.0, -40.0], [200.0, 80.0]]   # the manual route points (15.9)

[[order]]                        # an order wire (9.2)
before = "5c0e9b2a7f13d846"
after = "a01d77c3e5b9f210"

[[collar]]                       # a branch collar of a fan-out (15.9)
pos = [260.0, 80.0]
wires = ["a01d77c3e5b9f210.image", "c7e2f0918b3d4a65.image"]

[[cable]]                        # a visual cable group (15.9)
name = "control"
wires = ["a01d77c3e5b9f210.gain", "c7e2f0918b3d4a65.gain"]

[[input]]                        # a public input pin of a subgraph
key = "4f2a9c1e"
name = "carry_in"
pos = [0.0, 40.0]
to = ["e1d4c2b7a9f03856.b"]

[[output]]                       # a public output pin of a subgraph
key = "b83d0e57"
name = "sum"
pos = [400.0, 40.0]
from = "e1d4c2b7a9f03856.out"
```

**Rules**

- A node ID is 16 lowercase hexadecimal characters, from a random 64-bit number. Two nodes of one graph never have the same ID.
- A wire end is `<node ID>.<pin key>`. The pin key of an input is the name of its function parameter (4.1). The output pin key is `out`. A tuple gives `out0`, `out1` and so on. A `&mut` input gives an output with the same key (4.4). On a use of a subgraph, the pin key is the key of the public pin.
- An input has one wire at most. Thus the `to` end names the wire, and collars and cables name wires by it.
- A public pin of a subgraph has a key of 8 random hexadecimal characters. The key does not change when the pin gets a new name or a new order, so the connections of each use stay. The order of the `[[input]]` and `[[output]]` tables is the order of the pins.
- A parameter value and a settings value are the text form that their type gives (4.8). The engine stores the text and does not read it, so it stays free of types (2).
- A wire has 12 route points at most. A route edit with more points fails with a message. The editor draws each wire in one row of 64 points on the GPU, and 12 rounded corners fit in this row. A path with more corners, for example an orthogonal path, gets fewer points for each corner. (Agent decision, 2026-11-15. The owner can change it.)
- The route points, collars and cables are in the graph file, so the project owns them. The dock layout, the camera of the graph and the selection are not in the graph file. Each user keeps them (15.2).
- A wire to a pin that no longer exists stays in the file. The node shows an error until the user removes the wire or the pin comes back (15.5). Patina never deletes a wire without a user action.

**Writing**

- Patina writes UTF-8 with LF line ends. The nodes sort by ID, and the wires by their `to` end. The order, collar and cable tables sort by their first value. Each key has a fixed place.
- A number has the shortest form that reads back as the same value. The files hold no time stamps and no paths outside the project.
- The same project gives the same bytes at each save. Thus a change shows in Git as a small diff.
- A file with a newer `format`, or with a key that Patina does not know, does not open. Patina shows the file and the key. It never drops data that it does not understand.
- Patina opens an older `format`, and the next save writes the current format.
