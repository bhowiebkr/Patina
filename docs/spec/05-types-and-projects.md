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
- The first `Viewer` gives text: `fn text(&self, out: &mut Text) -> fmt::Result`. `Text` does not allocate, and it stops a text at 4096 bytes. (Agent decision, 2026-11-15. The owner can change it.)
- A `Viewer` can also give a thumbnail: `fn thumbnail(&self, out: &mut Thumbnail)`. `out.pixels(width, height)` gives the RGBA bytes to fill. A side is at most `Thumbnail::MAX`, 64 pixels. The default gives no thumbnail. The cluster sends it after the text as a `THUMB` report in base64 (ABI 20), and the engine refuses a bad size. The node body shows the thumbnail of its first output that has one, under the pins. The **Inspector** shows it for the selected wire. A thumbnail from an old run is dim. `GpuImage` gives a thumbnail. (Agent decision, 2026-11-17. The owner can change it.) A node with a free layout or a circuit symbol (4.3) shows no thumbnail, because its GUI or its shape is the whole node. The **Inspector** shows the thumbnail of its wires. (Agent decision of 2026-11-18. The owner can change it.) The **Inspector** shows a thumbnail at most 128 pixels high, so the widgets of the viewer stay the large view. (Agent decision of 2026-11-18. The owner can change it.)
- A `Viewer` can also show the value with the widgets of a node GUI (4.3): `fn show(&self, ui: &mut Ui)`, for example a plot, a waveform or a table. The `Ui` has the **Inspector** size, and its widgets take no input. The default shows none. The cluster sends the widget lines after the thumbnail as a `VIEW` report (ABI 21). The run event leaves them out. The **Inspector** asks for them with the full text of the selected wire. More than 1 MB of lines gives no widgets. (Agent decision, 2026-11-17. The owner can change it.)
- A `Viewer` can also give a wave: `fn wave(&self) -> &[f32]`, samples from -1 to 1. The default gives none. The cluster splits the samples into 64 parts at most, and sends the largest sample of each part as an integer from -100 to 100. It sends them after the widget lines as a `WAVE` report (ABI 24), and the engine refuses a bad text. The wire draws the wave (15.9). `AudioBlock` gives its samples. (Agent decision, 2026-11-17, after the owner named Bespoke Synth as the reference for audio wires. The owner can change it.)

## 6. Graphs and projects

### 6.1 Graphs and subgraphs

- A graph is a set of nodes and connections.
- Each node has a stable random ID. The ID does not change when the node moves to another subgraph or file.
- A subgraph is a group of nodes that shows as one node. The user can collapse and expand it, and reuse it in other graphs.
- A subgraph is a candidate for one cluster (see 8.2).
- A project has one or more top graphs. `main` is the first top graph. Each top graph is a
  workbench: an empty canvas or a different build that uses the same folders and subgraphs.
- Only a top graph runs, and only one at a time. When the user opens a top graph, it runs in
  place of the last top graph. When a subgraph is open, the last top graph runs on.
- A subgraph use cannot name a top graph, and a clip cannot bring one (15.11).
- A project opens with `main` as the top graph that runs. `patina-cli run --graph <name>` runs
  another top graph (9.1). A package runs the top graph that it was made from (14.1).
- (Agent decision of 2026-11-18, for the owner request of several graphs in one example. The
  owner can change it.)

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

**Libraries**

The owner asked for a way to reuse nodes and subgraphs from other projects (2026-11-17). This is an agent decision of 2026-11-17. The owner can change it.

- A library is a normal Patina project folder (18, question 14). The user adds it in **Settings** > **Libraries** (15.10), and Patina trusts it, because a pick builds its code (11).
- The **Add node** palette shows the node folders and the subgraphs of each library, with the name of the library. The open project does not show as a library. The palette of a pin drag (15.2) shows the library nodes that fit the pin. The paste and its wire are one undo step. (Agent decision of 2026-11-17. The owner can change it.)
- A pick adds the node or the subgraph as a paste of one node (15.11). The paste imports the node folders and the subgraphs that the pick needs.
- The **Library** tab of the editor (15.2) lists the nodes and the subgraphs of each library. Its preview shows the pins, the doc and what a pick brings, before the project changes. (Agent decision of 2026-11-18, for the owner request. The owner can change it.)
- An import of a type folder also copies the connection folders of the other project that use that type (6.3). This project keeps its own connection kind for the type when it has one. Reason: without its kind, a wire of the type loses its look and its lanes. (Agent decision of 2026-11-18. The owner can change it.)
- **Copy** in one project and **Paste** in another project also reuse nodes and networks (15.11).
- A library can also come from a Git repository. The user gives the HTTPS URL of the repository and a tag. An empty tag selects the newest tag. Agent decision of 2026-11-17. The owner can change it.
  - Patina reads the tags with a plain HTTPS request to `<url>/info/refs?service=git-upload-pack`, so it needs no `git` program. Any Git host gives this list.
  - Patina downloads the `.tar.gz` archive of the tag with `curl`, as the toolchain install does (13.2). The archive URL is `<url>/archive/<tag>.tar.gz`, or `<url>/-/archive/<tag>/<name>-<tag>.tar.gz` for a host with `gitlab` in its name.
  - The archive must hold the commit of the tag in its `comment` header, as `git archive` writes it. Patina refuses an archive with another commit or with no commit.
  - Patina unpacks the archive into the `libraries/<name>-<tag>` folder of the data folder (13.3). It refuses a path with `..`, an absolute path, a link, more than 20000 files and more than 512 MB. The root of the archive must hold `patina.toml`.
  - Patina records the URL, the tag, the commit and the SHA-256 hash of the archive in `library-pins.json` of the data folder. The record pins the library to the tag.
  - **Check for updates** reads the tags again. A newer tag shows **Update to** with the tag. A pinned tag that points to another commit shows a warning, and Patina keeps the old copy.
  - **Update to** downloads the new tag, and replaces the copy in the list. **Remove** deletes the downloaded copy. A local library folder stays on the disk.
  - Reason: a tag and a commit pin the code that Patina builds, and a plain HTTPS request works on each computer without an install.
- The library index is the file `libraries.toml` in the root of the Patina repository. Each `[[library]]` table has a `name`, a `url`, a `description` and a pinned `tag`. A pull request to the Patina repository adds a library. Agent decision of 2026-11-17. The owner can change it.
  - **Browse libraries** in **Settings** downloads the index from the `main` branch on GitHub and shows its libraries. **Add** downloads the tag of a library as **Add from Git** does, so the same checks apply.
  - Patina leaves out an entry with a bad URL or tag, so one bad entry does not hide the others. The index is not trusted: the user trusts each library when it is added.
  - Reason: the index needs no second repository, and the review of a pull request checks each new entry.
- The copy belongs to this project. A later change in the other project does not change it. Patina never links folders between projects, because a project holds all its source (2, 14), and a change in another project must not break it.
- Patina records the source of the copy. The record holds the stable random ID of the other project, the folder name, and a hash of the contents at import. The record holds no path, so it shows no private folder names when the project is shared. Section 6.4 gives the format of the record.
- **Update** in the **Project** panel compares the copy with the folder in the other project. Patina finds the other project among the projects that the user opened before, or asks for its folder. It shows the changes on both sides, including the changes that the user made to the copy. It copies the new version only when the user confirms. If both sides changed a file, the user picks the version of that file.
- The crate versions of the copy resolve with the `Cargo.lock` of this project (8.1).
- Patina builds the changed folders before it applies an **Import** or an **Update**. If the build fails or the user cancels, the project stays as it was.
- An **Import**, an **Update** or a paste from a project of a newer Rust version raises the Rust version of this project to that version (8.1). An older toolchain stops it with the error of 8.1, and the project stays as it was. Agent decision of 2026-11-17. The owner can change it.
- A folder from a project that the user does not trust stays read-only, and out of the build, until the user trusts it (11).

The import rules of the first version. (Agent decision, 2026-11-15. The owner can change it.)

- Patina imports only from a trusted project into a trusted project. The build check compiles the copy, so the code of an untrusted project does not get into the build. The user opens the other project one time and trusts it first.
- The dependencies of a folder are the `path` entries of `[dependencies]`, `[dev-dependencies]` and `[build-dependencies]` in its `Cargo.toml`. A path to a folder that is not in `types/` or `widgets/` of the other project stops the import.
- A folder with the same path and the same files is not copied. A name conflict with a folder in another group, for example `nodes/image` for `types/image`, allows only a rename.
- Patina copies the folders to `build/import/`, moves each replaced folder there, and moves the copies into the project. Then it writes the records and builds the copied node, widget and connection crates and the folders that use a replaced folder. A failed build moves the old folders back and writes the old `patina.toml`.
- A renamed folder gets a new `Cargo.toml` from the parsed file, without its comments.
- The record hash is the hash of the folder in the other project, before a rename. Undo does not remove an import.
- Reason: a trust rule for one folder needs a second trust list and a filter of the workspace members. The trusted source keeps the first version small and safe.

The update rules of the first version. (Agent decision, 2026-11-15. The owner can change it.)

- **Update** works on one imported folder. Patina finds the other project in the list of trusted projects by its ID, or asks for its folder. The other project must be trusted.
- Patina compares the copy with the folder in the other project. The `Cargo.toml` of the other folder gets the crate names of this project first, so a rename of the import is not a change.
- The record hash shows if the other project changed the folder. When the other project did not change the folder, **Update** has nothing to do.
- The `files` table of the record has the hash of each file that the import or the last update copied from the other project. A file that only one side changed after that gets the version of that side. When the two sides changed a file, the user picks its version. (Agent decision, 2026-11-16. The owner can change it.)
- A record with no `files` table is from before the file hashes. Then each different file comes from the other project when the `copy` hash shows no change, else the user picks the version of each different file.
- A new dependency of the folder that is not in this project stops the update. The user imports that folder first.
- Patina writes the new version to `build/import/` and moves it into the project. Then it builds the folder and the folders that use it. A failed build moves the old folder back and writes the old `patina.toml`.

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
- `#[connection]` takes optional arguments. The engine reads them from the code before a build, so the editor shows the lanes and the sockets before a run (15.9). (Agent decision, 2026-11-16, after the owner asked that each lane show what it is. The owner can change it.)
  - `lanes = 8` gives the number of lanes. `lanes = ["HLT", "MI", "RI"]` gives a name for each lane, lane 0 first. With a number, each lane has its bit number as its name, for example 7 to 0.
  - `label = "data"` gives the bus name when the wire and its pins give none.
  - `color = "green"` gives the color family of the lanes: `green`, `teal`, `violet`, `pink` or `copper`. With no color, the lanes are neutral.
  - A lane name has 16 bytes at most and the label has 64 bytes at most. A bad argument is a problem of the kind folder, as a code that does not parse is.
  - When a live value has a different number of lanes, the editor shows the live lanes with their bit numbers.
- Lane groups, a color for each lane and a custom badge are not in the first form.

### 6.4 Project file format

This is an agent decision of 2026-11-15 for question 1 (18). The owner can change it.

The project files are TOML. Patina writes them, and Git can merge them. A person can edit them by hand, but Patina does not keep comments.

**`patina.toml`**

```toml
format = 1
id = "3f9c2a7d41e0b85c"     # the stable random ID of the project (6.2)
rust = "1.100.0"            # the oldest Rust version that builds the project (8.1)
frame_rate = 600            # optional: the frames each second, 1 to 100000 (9.1)

[imports."nodes/curve-editor"]   # the folder in this project (6.2)
project = "9a1b4c0d2e7f6385"     # the ID of the other project
folder = "curve-editor"          # the folder name in the other project
hash = "sha256:<64 hex characters>"
copy = "sha256:<64 hex characters>"   # the copy in this project after the import or the last update

[imports."nodes/curve-editor".files]     # each file that the import or the last update copied
"Cargo.toml" = "sha256:<64 hex characters>"
"src/lib.rs" = "sha256:<64 hex characters>"
```

- The import hash is a SHA-256 hash of the sorted relative paths and the bytes of each file in the folder. The paths use `/` on each platform. For each file, the hash reads the path, a zero byte, the length as 8 little-endian bytes, and the bytes. Hidden files, links and `target` folders are not part of a folder. (Agent decision, 2026-11-15. The owner can change it.)
- An import record holds no path of the other project (6.2).
- `frame_rate` is optional. Without it, the frame domain runs at 60 FPS. Patina does not write the default value. The **FPS** field in the toolbar of the editor sets it, and Save writes it (15.10). (Agent decision, 2026-11-16. The owner can change it.)
- The `copy` hash uses the same method. An old record with no `copy` key counts as a copy that changed. (Agent decision, 2026-11-15. The owner can change it.)
- A hash of the `files` table uses the same method with only that file. It is the hash of the file from the other project, with the crate names of this project in `Cargo.toml`. (Agent decision, 2026-11-16. The owner can change it.)

**Graph files**

- Each graph and each subgraph is one file in `graphs/`. The file name is the graph name. `graphs/main.toml` is the main graph.
- A graph file with `top = true` is a top graph (6.1). Patina writes `top` only when it is true. `main` is a top graph without it.
- A subgraph file can have public pins. Each use of the subgraph is a node with these pins (6.1).
- The type of a public pin is the value type of the inner pin that it ends at, without `&` or `&mut`. A pin that ends at a generic pin keeps the type parameter, and the use is generic in it (4.5). (Agent decision, 2026-11-16. The owner can change it.)

```toml
format = 1
top = true                       # only in a top graph other than main (6.1)
style = "curved"                 # the path of the wires with no style; only to override automatic (15.9)

[symbol]                         # only in a subgraph file: the symbol of each use (4.3)
shape = "chip"                   # and, nand, or, nor, xor, xnor, not, buffer, mux, alu, adder, chip or bus
label = "CPU"                    # optional: the text on a chip, a block or a bus bar; 32 bytes at most

[[node]]
id = "5c0e9b2a7f13d846"         # the stable random ID of this use (6.1)
folder = "nodes/curve-editor"   # or subgraph = "full-adder" for a use of graphs/full-adder.toml,
                                # or builtin = "delay" for the delay node (9.2)
pos = [120.0, -40.0]
domain = "audio"                # only when the user selected the domain (9.1)
settings = "<text form>"        # only for a node with #[settings] (4.3)

[node.params]
radius = "2.5"                  # the text form of the Param trait (4.8)

[[wire]]
from = "5c0e9b2a7f13d846.out"
to = "a01d77c3e5b9f210.image"
style = "orthogonal"             # automatic, curved, orthogonal or polyline; only to override the graph default (15.9)
corners = "sharp"                # rounded or sharp; only for sharp corners (15.9)
points = [[200.0, -40.0], [200.0, 80.0]]   # the manual route points (15.9)
name = "addr"                    # the name of a bus, only to override the default name (15.9)

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
- A public pin of a subgraph has a key of 8 random hexadecimal characters. The key does not change when the pin gets a new name or a new order, so the connections of each use stay. The order of the `[[input]]` and `[[output]]` tables is the order of the pins. The edit `OrderPin` moves a pin to a new place in its list (7.4).
- A parameter value and a settings value are the text form that their type gives (4.8). The engine stores the text and does not read it, so it stays free of types (2).
- A wire has 12 route points at most. A route edit with more points fails with a message. The editor draws each wire in one row of 64 points on the GPU, and 12 rounded corners fit in this row. A path with more corners, for example an orthogonal path, gets fewer points for each corner. (Agent decision, 2026-11-15. The owner can change it.)
- The `[symbol]` table changes only the look, so the build does not read it. (Agent decision of 2026-11-18. The owner can change it.)
- A wire name has 32 bytes at most. It changes only the look, so the build does not read it. (Agent decision, 2026-11-16. The owner can change it.)
- The route points, collars and cables are in the graph file, so the project owns them. The dock layout, the camera of the graph and the selection are not in the graph file. Each user keeps them (15.2).
- A wire to a pin that no longer exists stays in the file. The node shows an error until the user removes the wire or the pin comes back (15.5). Patina never deletes a wire without a user action.

**Subgraph uses in a build**

These rules are an agent decision of 2026-11-15. The owner can change them.

- Before a check, the engine replaces each use of a subgraph with the nodes and wires of its file. A use in a subgraph file expands too.
- A node of a use gets the ID path `<use ID>/<node ID>`, for example `5c0e9b2a7f13d846/e1d4c2b7a9f03856`. Thus two uses of one subgraph do not share IDs, and the live values and the errors find the use.
- A use has no folder, built-in node, domain or settings. A subgraph that uses itself, directly or through another subgraph, does not pass the check.
- A wire to a public input of a use goes to each `to` end of the pin. A wire from a public output goes from its `from` end.
- A public output with no `from` is not connected. A wire from it gives no value, so its input gets its default value.
- A parameter of a use, with the key of a public input, goes to each `to` end of the pin. It replaces the parameter of the inner node.
- An order wire to a use orders all the nodes of the use.
- The route points, collars and cables of a subgraph file are visual only, so the build does not read them.

**Writing**

- Patina writes UTF-8 with LF line ends. The nodes sort by ID, and the wires by their `to` end. The order, collar and cable tables sort by their first value. Each key has a fixed place.
- A number has the shortest form that reads back as the same value. The files hold no time stamps and no paths outside the project.
- The same project gives the same bytes at each save. Thus a change shows in Git as a small diff.
- A file with a newer `format`, or with a key that Patina does not know, does not open. Patina shows the file and the key. It never drops data that it does not understand.
- Patina opens an older `format`, and the next save writes the current format.
