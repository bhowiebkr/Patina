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
  patina.toml        project settings, for example the Rust version (format TBD, see 18)
  Cargo.toml         the workspace, made by Patina
  Cargo.lock
  graphs/            the main graph and the subgraphs, with their connections (format TBD, see 18)
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
- Patina records the source of the copy. The record holds the stable random ID of the other project, the folder name, and a hash of the contents at import. The record holds no path, so it shows no private folder names when the project is shared. The format of the record is part of question 1 (18).
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
