# Patina Specification: Layers and Compilation

Part of the [Patina Specification](README.md).

## 7. Layers

Layers give fast execution and fast iteration at the same time. The graph runs as one optimized build. A change to one node builds fast. The target is a few seconds.

### 7.1 Model

Patina compiles a graph in two layers:

| Layer | Contents | Build mode (8.3) | Purpose |
|---|---|---|---|
| Base | The whole graph, except the node in the slot. Small nodes inline into each other | Bake | Fast execution |
| Slot | The node that the user edits, compiled alone | Edit | Fast iteration |

- The base is the normal state. With no node in the slot, the whole graph is one optimized build.
- Patina never runs the graph as separately compiled nodes. Only the node in the slot runs as its own library.
- The base calls the slot through a function pointer. This costs one indirect call each time the node runs, and only while the node is in the slot.
- If the graph uses the node folder more than once (6.2), each use calls the slot. For a generic node, the slot holds a copy of the call for each use, with the types of that use (4.5).
- A change to a shared type crate (5.2) rebuilds the base. The slot does not help there.
- The slot is a library of its own, in `build/clusters/slot`. It holds the node in the slot and the state of each use. The base gives the slot library a pointer to each input and to the `Option` of the result. The engine gives each cluster the slot library after the load (`patina_cluster_slot` and `patina_cluster_call`, ABI version 12).
- The base takes the type of the result from dead code with the call of the node, as a link does (8.2). Thus the base depends on the node crate, but runs no code of it.
- The engine builds the slot alone only while the code of the base, the signature of N and the outline of the folder of N stay the same. Else it builds the base and the slot again. The outline is the Rust code of the folder with no function bodies, and the other files of the folder. A `const fn`, a function that gives an `impl` type and `build.rs` keep their bodies, because these bodies can change a type. The future of an `async fn` stays in the slot library, so its body does not count. Thus a change of a type alias or of a field builds all, and a change of a function body builds the slot alone. (Agent decision of 2026-11-17. The owner can change it.)
- The slot library names no type. A witness of each use of a generic node gives the types by inference, as in the base (4.5). Thus the slot library also depends on the node crates that give the types. (Agent decision of 2026-11-17. The owner can change it.)
- An event node also goes into the slot (9.4). The slot call of an async node waits for its future on the thread of the run. After **Stop** the result stays empty. (Agent decision of 2026-11-17. The owner can change it.)
- A new slot library gives new states to the uses of N, because the code of N changed. When N fuses back, the base takes the state of each use from the slot library, if the key did not change (7.2).

### 7.2 Edit cycle

1. The user opens the code of node N. The engine starts a background build of the base with an empty slot for N. Only the clusters that hold a use of N rebuild (8.2). The cache keeps the other clusters (8.6).
2. Until that build is ready, the old base keeps running with the old code of N.
3. At each save of the code of N (15.10), the engine compiles N alone in edit mode, with a target of a few seconds. It swaps the new build into the slot between two runs of the graph.
4. If N does not compile, the slot keeps the last build that compiled, and the graph keeps running. The node shows the error (15.5), and its outputs keep their normal look. If N never compiled, it has no build: it shows the error, and its downstream nodes are Skipped (15.3).
5. The engine fuses N back into the base when one of these occurs:
   - The user opens the code of another node.
   - The user makes no change to N for 30 s (Q7 of section 18).
   - The user clicks **Bake**.
6. To fuse, the engine rebuilds the cluster of N in bake mode, with the new code of N inline. It swaps in the new cluster between two runs. If the user opened node M, the same build makes the slot for M.

Rules:

- A swap keeps the state of each stateful node whose code did not change. Only the edited node resets (4.2).
- Each build gives a state key to each use of a stateful node. The new build takes the state of a node from the old build only when the node ID and the key did not change. The key is a hash of these items:
  - The toolchain: the `rustc` path, the Rust version, the host target, the library folder and the rustflags.
  - Each unit that cargo built, except the cluster crate: the package ID, the target, the debug assertions of the profile, the features, and the cfg values and environment variables of its build script. A unit of a node folder goes only into the key of that node. A unit of two cargo runs, for example of the clusters and of the slot, counts one time.
  - The files of each path package, because its version does not change when its files change.
  - The node folder name, the types of the outputs of the use and the types of its input wires. The wire types set the type parameters of a generic node (4.5).
- The state move depends on a hypothesis: rustc gives the same layout to the same type with the same compiler and cfg values. The optimization level does not change the layout, so a state moves from an edit build to a bake build. A change to a shared crate, to the features or to the toolchain resets all states. This is an agent decision of 2026-11-15. The owner can change it.
- The state that moves keeps its links to the old build, for example a vtable. The old build stays loaded (8.5).
- The slot takes a change only when the pins of N stay the same. If a change adds, removes, or changes a pin, the engine rebuilds each cluster that holds a use of N, as for a wiring change.
- Each build has the revision of the code that it built. The engine never swaps in a build older than the active one, and it shows only the errors of the newest revision.
- When a swap touches more than one cluster, the engine loads and checks all the new builds before the first switch. If one fails, all the active builds stay. Each clock domain switches at its next run boundary.
- A change to a widget folder rebuilds each cluster and each view that uses it, as a wiring change does.
- A wiring change, for example a new node or a new connection, rebuilds the affected cluster in edit mode. That cluster fuses back after the same set time with no change, or when the user clicks **Bake**.
- The editor of version 1 starts the slot at the first save of a Rust file of N, not when the user opens the code. That save builds the base with the slot in edit mode, and the next saves build the slot alone. A save of `Cargo.toml` of N builds all clusters, because a new crate can change the base. Agent decision of 2026-11-15. The owner can change it.
- The fuse builds in a background thread, so the graph runs during the fuse. The engine swaps in the fused clusters only when no change, pause or step came after the start of the fuse. Builds run one at a time. A change, an unload or a pause during a fuse stops the fuse: the engine kills its cargo run, so the next build does not wait. A view build also stops the fuse, and the fuse starts again after the idle time. A **Package** waits for the fuse, because it builds the same bake clusters and uses their crates. (Agent decision of 2026-11-17. The owner can change it.)
- A paused graph does not build, so it does not fuse. **Bake** fuses only a live graph. A paused graph fuses after the next **Run** and the set time. Agent decision of 2026-11-15. The owner can change it.
- Patina does not build when nothing changed, also after the user closes and opens the editor. The nodes and the layers show **Compiling** and **Building** only while cargo compiles a crate. Owner decision of 2026-11-16.
- The first build after a project opens has the mode of the newest cluster library in `build/target`. A project with no build bakes. So cargo finds all crates fresh and compiles nothing when nothing changed. (Agent decision, 2026-11-16, for the owner decision above. The owner can change it.)
- A build with the same wiring as the loaded clusters, and no node in the slot, keeps the mode of the loaded clusters. So **Run** after an open compiles nothing. An edit build fuses only after a run. (Agent decision, 2026-11-16. The owner can change it.)
- That build keeps the bake mode only when no file of a crate folder of the last build changed. A change from outside the editor, for example of a node or of a shared crate, builds in edit mode and fuses later. (Agent decision of 2026-11-17. The owner can change it.)

### 7.3 Visual display

- The Layers panel (15.2) shows the base and the slot. Each row shows its build state and its build time, for example "Built in 1.8 s".
- The node in the slot shows a slot icon. The slot uses no new color (15.3).
- The status bar shows the layer state, for example "Fused" or "Slot: Exposure".

### 7.4 Undo and redo

- The UI sends each user action to the engine as one edit:

```rust
enum Edit {
    AddNode(NodeId, NodeDef),
    RemoveNode(NodeId),      // also removes the wires of the node
    MoveNode(NodeId, Point),
    Connect(Port, Port),     // replaces the wire that goes to the same input
    Disconnect(Port, Port),
    SetCode(PathBuf, String), // a file in a node, view, widget, or connection folder (6.2)
    SetParam(NodeId, String, Value),
    Order(NodeId, NodeId),   // order wire, see 9.2
    Group(Vec<Edit>),        // one step of undo, for example a move of many nodes
}
```

- Undo removes the last edit. Redo applies it again.
- A move is an edit, so undo can put a node back. A move does not build or run the graph.
- Undo and redo of a code change go through the slot, the same as other code changes.

## 8. Compilation

### 8.1 Toolchain and dependencies

- Each project records a Rust version in `patina.toml` (6.4). A new project gets the version of the toolchain.
- A toolchain of this version or a newer version builds the project, because Rust keeps the code of older stable versions working. An older toolchain gives an error that names the two versions. Agent decision of 2026-11-17. The owner can change it.
- The reason: **Install toolchain** installs the newest stable Rust (13.2). The check before needed the same version, so each project of an older version failed.
- All example projects and the SDK crates use one Rust version: the current stable Rust, 1.100.0 on 2026-11-17. Raise them together. Owner decision of 2026-11-17.
- An import or a paste of code from a project of a newer Rust version raises the version of the project (6.2).
- All projects share one toolchain. **Update toolchain** (13.2) brings it to the newest stable Rust. Open option, not done: a toolchain for each exact Rust version, side by side. Each one needs about 300 MB, and a newer Rust builds the code of an older stable Rust.
- Each project is one Cargo workspace with one `Cargo.lock`. The lock file gives one resolved set of crate versions for all nodes.
- Cargo can still put two incompatible versions of one crate in the build, for example `image` 0.24 and 0.25. Their types are different types. The type key check rejects a wire between them (5.1).
- Each node, view, widget, connection, and type folder is one crate of the workspace (6.2).
- Nodes can use any crate from crates.io, as source.
- One shared crate cache serves all projects.

### 8.2 Units

| Unit | Rust form | Output |
|---|---|---|
| Node | One library crate, from its node folder (6.2) | `.rlib` |
| Widget | One library crate, from its widget folder (6.2) | `.rlib`, linked into the nodes and views that use it |
| Cluster | Generated crate that depends on its node crates | One `.dll` / `.so` / `.dylib` |
| Shared types | `std` + type crates of the project | One shared Rust library |
| Slot | One node, compiled alone (7) | One `.dll` / `.so` / `.dylib` |
| View | One view, compiled alone (15.8) | One `.dll` / `.so` / `.dylib` |
| Connection kind | One connection kind, compiled alone (6.3) | One `.dll` / `.so` / `.dylib` |

- A node crate is a unit of the build cache. A node never runs as its own library, except in the slot (7).
- A cluster is a part of the graph that compiles as one unit.
- Put cluster boundaries at large nodes. Keep small nodes inside one cluster, so the compiler can inline and fuse them.
- A cluster never crosses a clock domain boundary.
- Now the engine makes one cluster for each clock domain with a step. A domain with no step gets no cluster. The split at large nodes comes when milestone 6 measures the need. This is an agent decision of 2026-11-15. The owner can change it.
- A value that a node of another domain reads is a link: a latest-value wire (9.1). The cluster that makes the value keeps it in a shared `Arc`. After the load, the engine gives the `Arc` of each link to the cluster that reads it (`patina_cluster_export` and `patina_cluster_import`, ABI version 11 and later).
- The reader cluster names no type. Its crate depends on the crates of the producer and of the steps before it, and dead code with their calls gives Rust the type of each link. Thus a change of the producer also builds the reader again.
- The same dead code gives each shared value of a cluster its type where the cluster makes the value. Without it, rustc carries thousands of open types through the cluster function, and its type check time grows quadratically. This is an agent decision of 2026-11-15. The owner can change it.
- The link depends on the same layout hypothesis as the state move (7.2).
- The engine loads and links all clusters of a build before it uses one. If one fails, no new cluster stays loaded (7.2).

### 8.3 Build modes

| Mode | Optimization | Use |
|---|---|---|
| Edit | Low, no LTO | The slot (7): fast compile of the node that the user edits |
| Bake | Full, no LTO | The base (7) and packages: fast execution |

- Bake mode uses `opt-level = 3` and `codegen-units = 1`. It is the cargo profile `bake` of the project workspace, which inherits `release`.
- Edit mode is the cargo profile `edit`, which inherits `dev`.
- Edit mode keeps `opt-level = 0`. On 2026-11-17, `opt-level = 1` made the frames of `examples/computer` 6 times faster, but its wiring changes built in 10.5 s, not 5.7 s. The fuse to a bake build gives the fast frames after an idle time. Agent decision of 2026-11-17. The owner can change it.
- The cluster crates build with no debug info in edit mode. The DWARF debug info of a large cluster function overflows the stack of LLVM on Linux and macOS. Node crates keep their debug info. Agent decision of 2026-11-15. The owner can change it.
- The two modes have no debug assertions. Thus `cfg(debug_assertions)` is the same in the two modes, and a state moves from one mode to the other (7.2). A `debug_assert!` in a node does not run. Agent decision of 2026-11-15. The owner can change it.
- `patina-cli run` builds in bake mode. `--edit` makes it build in edit mode, for a fast build. The tests use `--edit`.
- Bake mode does not use LTO. The clusters share `std` and the types library through `-C prefer-dynamic` (5.2), and rustc refuses LTO with `-C prefer-dynamic`. The milestone 0 prototype "hot reload" found this on Windows, in each form of LTO that it tried (19).
- The compiler inlines a function across crates without LTO when the function is generic or has `#[inline]`. Thus `#[node]` marks the run function of each node `#[inline]`, so small nodes inline into each other inside one cluster. Milestone 6 measures this.
- This is an agent decision of 2026-11-15. The owner can change it.

### 8.4 Base and slot

Section 7 describes the base and the slot. The base builds in bake mode. The slot builds in edit mode.

- `Checked::build` builds the clusters in the given mode, and the slot library always in edit mode, in a second cargo run.
- A bake build with no slot is a fuse. `Cluster::adopt` then moves the state of each use from the old clusters and the slot library (7.2).

### 8.5 Hot reload

- Each build gets a new file name: clusters, the slot, views, connection kinds, and the shared types library. Windows locks loaded `.dll` files. Windows also gives an already loaded library to each later request with the same file name. Thus a new build with an old name would run the old code.
- The file name of a library holds a hash of the file content. A build that gives the same bytes gets the same name, so the loaded library stays in use. Thus an undo to an earlier source does not load a new copy.
- The builds must be reproducible. Without a flag, the linker writes a new time stamp into each library. The engine links with `--no-insert-timestamp` on the `x86_64-pc-windows-gnu` target with `rust-lld` (13.2), and with `/Brepro` on an MSVC target. The toolchain prototype and the hot reload prototype proved each flag (STATUS.md). This is an agent decision of 2026-11-15.
- The engine loads each library by its full path. It finds the libraries that a library needs only in the `build/` folder of the project. On Windows, `LoadLibraryExW` with the `LOAD_LIBRARY_SEARCH` flags limits the search path. The `build/` folder holds each library that a cluster needs, including the `std` library of the toolchain. On the gnu target of Windows, `std` needs `libgcc_s_seh-1.dll` and `libwinpthread-1.dll` of the toolchain, so `build/` holds them too. On Linux and macOS, a library finds the others in its own folder through an rpath.
- Values, node states, futures, and callbacks keep a link to the build that made them. Old builds stay loaded, so these drop with their own code.
- A failed build never deletes or replaces a loaded file.
- When a project opens, Patina deletes the old builds in `build/`, except the loaded builds and the builds in the cache.
- Windows refuses to delete a loaded library, but it renames one. Thus a delete during a session skips the loaded builds.
- The engine does not unload old builds. Rust libraries do not unload safely, and macOS often does not unload them. Memory and disk use grow with each reload until a restart. Milestone 0 measures the growth (19).
- In the editor, a crash or a second **Stop** starts a new node process (16.1), which frees all old builds.

### 8.6 Cache

- The engine stores each compiled cluster with a hash of its inputs as the key: the node sources, the wiring, the build settings, the toolchain, and the build ID of the types library (5.1). The engine knows the key before it compiles, so a cache hit needs no compile.

| Change | Compile needed? |
|---|---|
| Change a parameter | No |
| Undo to a graph that is in the cache | No |
| Change node code | Only that node, in the slot (7). Later, a fuse of its cluster |
| Change wiring | Only the affected cluster |
| Change shader source in a shader node (see 10.3) | No Rust compile |
